//! MegaDaemon v15 — arnés de la auditoría adversarial.
//!
//! Cada test de este fichero reproduce UN defecto descrito en
//! `docs/AUDIT_ADVERSARIAL_V15.md`.  El fichero NO se cablea en `lib.rs`
//! (es una suite de auditoría, no parte del producto).
//!
//! Reglas:
//!   * un test que FALLA con el código actual demuestra el defecto;
//!   * si el entorno no permite la reproducción (p.ej. no hay `su`), el test
//!     se marca como "skip" explícito en vez de pasar en falso;
//!   * nada de esto modifica ficheros que no sean de `/tmp`.
//!
//! NOTA 2026-11-20: los tests de **ADV-A-01** y **ADV-A-02** estaban
//! INVERTIDOS (afirmaban que el RCE se reproducía).  Al arreglar el defecto
//! en `android.rs` se han invertido: ahora PASSAN con el código correcto y
//! FALLARÍAN si volviese el RCE.

use megadaemon_core::api_key::{self, ApiKeyConfig, Decision, RateLimiter};
use megadaemon_core::cockpit_server;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, TcpListener, TcpStream};
use std::sync::Once;
use std::time::Duration;

static ENV_ONCE: Once = Once::new();

/// Estado y entorno de la prueba (nunca toca rutas reales del móvil).
fn ensure_env() {
    ENV_ONCE.call_once(|| {
        let dir = std::env::temp_dir().join("md_v15_state");
        let _ = std::fs::create_dir_all(&dir);
        std::env::set_var("MEGADAEMON_STATE_DIR", dir.to_string_lossy().to_string());
        // Sin API key configurada a propósito: así se aísla el gate de la
        // API key del gate del token de sesión.
        std::env::remove_var("MEGADAEMON_API_KEY");
        std::env::remove_var("SEERE_AUTH_TOKEN");
    });
}

/// Cliente HTTP mínimo (el cockpit habla HTTP/1.1 y cierra la conexión).
fn http(port: u16, raw: &str) -> (u16, String) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).expect("conectar al cockpit");
    let _ = s.set_read_timeout(Some(Duration::from_secs(20)));
    let _ = s.set_write_timeout(Some(Duration::from_secs(20)));
    s.write_all(raw.as_bytes()).expect("escribir la petición");
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    loop {
        match s.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if buf.len() > 8 * 1024 * 1024 {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let text = String::from_utf8_lossy(&buf).into_owned();
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    (status, text)
}

fn get(port: u16, path: &str, token: Option<&str>) -> (u16, String) {
    let mut req = format!(
        "GET {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n",
        path, port
    );
    if let Some(t) = token {
        req.push_str(&format!("X-Seere-Auth-Token: {}\r\n", t));
    }
    req.push_str("\r\n");
    http(port, &req)
}

fn post(port: u16, path: &str, body: &str, token: Option<&str>) -> (u16, String) {
    let mut req = format!(
        "POST {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n",
        path,
        port,
        body.len()
    );
    if let Some(t) = token {
        req.push_str(&format!("X-Seere-Auth-Token: {}\r\n", t));
    }
    req.push_str("\r\n");
    req.push_str(body);
    http(port, &req)
}

fn free_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    l.local_addr().unwrap().port()
}

/// Sonda **sin pánico**: `None` si no hay nadie escuchando (o no responde).
fn try_http(port: u16, raw: &str) -> Option<(u16, String)> {
    let mut s = TcpStream::connect(("127.0.0.1", port)).ok()?;
    let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = s.set_write_timeout(Some(Duration::from_secs(5)));
    s.write_all(raw.as_bytes()).ok()?;
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    loop {
        match s.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if buf.len() > 64 * 1024 {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let text = String::from_utf8_lossy(&buf).into_owned();
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    Some((status, text))
}

/// Arranca el cockpit en un puerto libre y **espera a que responda**.
///
/// `free_port()` abre y cierra un socket ⇒ carrera TOCTOU: con varios tests en
/// paralelo (cargo lanza un hilo por CPU) dos tests pueden quedarse con el
/// MISMO puerto efímero, el segundo `run_server()` muere con "Address already
/// in use" y el test ve "connection refused" ⇒ **el fallo es del entorno, no
/// del código**.  Aquí se reintenta con otro puerto hasta que el servidor
/// conteste de verdad.
fn start_cockpit() -> u16 {
    let probe = "GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
    for intento in 0..10 {
        let port = free_port();
        let _ = std::thread::spawn(move || {
            let _ = cockpit_server::run_server(port);
        });
        for _ in 0..50 {
            std::thread::sleep(Duration::from_millis(100));
            if let Some((st, _)) = try_http(port, probe) {
                if st > 0 {
                    return port;
                }
            }
        }
        eprintln!(
            "[TEST] el cockpit no respondió en el puerto {} (intento {}) ⇒ reintento",
            port,
            intento + 1
        );
    }
    panic!("no se pudo arrancar el cockpit en ningún puerto libre");
}

// ===========================================================================
// A — SEGURIDAD
// ===========================================================================

/// **ADV-A-01 (P0)** — `android::fs_write` mete el contenido CRUDO dentro de
/// un heredoc `<<'__MD_EOF__'`.  Si el contenido contiene una línea
/// `__MD_EOF__`, el heredoc termina antes de tiempo y **el resto del
/// contenido se ejecuta como comandos de shell como ROOT**.
///
/// Reproducción real (no teórica): el fichero marca "secuestrado" aparece.
#[test]
fn adv_a01_fs_write_inyecta_comandos_por_el_heredoc() {
    ensure_env();
    if !std::path::Path::new("/usr/bin/su").exists() {
        eprintln!("[SKIP] no hay `su` en este entorno: no se puede ejecutar como root");
        return;
    }
    let target = std::env::temp_dir().join("md_v15_a01_target.txt");
    let marker = std::env::temp_dir().join("md_v15_a01_PWNED");
    let _ = std::fs::remove_file(&marker);

    let payload = format!(
        "contenido legitimo\n__MD_EOF__\ntouch {}\n__MD_EOF__\n",
        marker.to_string_lossy()
    );
    let res = megadaemon_core::android::fs_write(
        &target.to_string_lossy(),
        &payload,
        false,
        "",
        false,
    );
    // ¡OJO! el marcador se comprueba ANTES de borrarlo.
    let pwned = marker.exists();
    let written = std::fs::read_to_string(&target).unwrap_or_default();
    let _ = std::fs::remove_file(&target);
    let _ = std::fs::remove_file(&marker);

    // CONTROL POSITIVO (desde el 20-11-2026 03:4x, cuando se arregló A-01):
    // el contenido SIEMPRE viaja en base64, así que el shell nunca ve el texto
    // del usuario y **no hay delimitador que cerrar**.  Antes este test era
    // ROJO (afirmaba que el RCE se reproducía); ahora exige que **no** se
    // reproduzca ⇒ si alguien reintroduce el heredoc, vuelve a ROJO.
    assert!(
        !pwned,
        "el contenido de fs_write se ejecutó como comando: RCE root reintroducido \
         (el contenido debe viajar SIEMPRE en base64).  resp={:?}",
        res
    );
    assert!(
        written.contains("__MD_EOF__") && written.contains("contenido legitimo"),
        "el contenido no llegó intacto al fichero (fs_write lo alteró): {:?}",
        written
    );
}

/// **ADV-A-02 (P1)** — `android::find_duplicates` mete el path de cada fichero
/// dentro de un comando de shell generado desde AWK:
/// `cmd="stat -c %s \"" p "\" 2>/dev/null"`.  Un nombre de fichero con `"` o
/// `;` ejecuta comandos como ROOT.
#[test]
fn adv_a02_find_duplicates_inyecta_comandos_por_el_nombre_de_fichero() {
    ensure_env();
    if !std::path::Path::new("/usr/bin/su").exists() {
        eprintln!("[SKIP] no hay `su` en este entorno");
        return;
    }
    let dir = std::env::temp_dir().join("md_v15_a02_dir");
    let _ = std::fs::create_dir_all(&dir);
    let marker = std::env::temp_dir().join("md_v15_a02_PWNED");
    let _ = std::fs::remove_file(&marker);

    // Nombre con comillas y punto y coma (sin espacios: sha256sum lo emite tal cual).
    let nasty = dir.join("x\";touch md_v15_a02_PWNED;echo \"");
    std::fs::write(&nasty, vec![7u8; 8192]).unwrap();

    let res = megadaemon_core::android::find_duplicates(&dir.to_string_lossy(), 1);
    let pwned = marker.exists();
    let _ = std::fs::remove_file(&marker);
    let _ = std::fs::remove_file(&nasty);
    let _ = std::fs::remove_dir_all(&dir);

    // CONTROL POSITIVO (desde el 20-11-2026 03:4x, cuando se arregló A-02):
    // el nombre va entre comillas simples con el escapado POSIX ⇒ un `;` o
    // un `$` dentro del nombre ya no es un comando.  Si alguien vuelve a
    // interpolar el nombre en el AWK, este test vuelve a ROJO.
    assert!(
        !pwned,
        "el nombre del fichero se ejecutó como comando: RCE root reintroducido en \
         find_duplicates.  resp={}",
        res
    );
}

/// **ADV-A-03 (P1)** — La política destructiva (`is_destructive`) era una lista
/// de subcadenas: se esquivaba con un espacio de más, con una opción larga o
/// con una forma distinta de borrar.
/// **CONTROL POSITIVO desde el 20-11-2026 03:4x** (la política ahora normaliza
/// espacios/tabuladores y cubre las variantes): este test exige que **todos**
/// los vectores se detecten ⇒ si la cobertura vuelve a adelgazar, sale ROJO.
#[test]
fn adv_a03_la_politica_destructiva_se_esquiva() {
    let bypasses = [
        "dd  if=/dev/zero of=/dev/block/sda",     // doble espacio
        "rm -rf --no-preserve-root /",             // opción larga
        "cat /dev/zero > /dev/block/sda",          // borra la partición sin "dd"
        ":(){ :|:& };:",                           // fork bomb
        "reboot bootloader",                       // deja el móvil en el bootloader
        "mv /data/local/tmp/megadaemon /data/local/tmp/megadaemon.bak",
    ];
    // INVERTIDO (fix de ADV-A-03 en `android.rs`): la politica destructiva ya
    // NO se esquiva con espacios de mas, opciones largas ni otras formas de
    // borrar => todos estos comandos DEBEN clasificarse como destructivos.
    for cmd in bypasses {
        assert!(
            megadaemon_core::android::is_destructive(cmd),
            "«{}» NO se clasifica como destructivo: la política confirm es evadible \
             (regresión de A-03)",
            cmd
        );
    }
    // Y el caso "bonito" también se detecta (la cobertura es consistente).
    assert!(megadaemon_core::android::is_destructive("rm -rf /"));
    assert!(megadaemon_core::android::is_destructive("dd if=/dev/zero of=/dev/block/sda"));
}

/// **ADV-A-04 (P0)** — `MEGADAEMON_REQUIRE_API_KEY=1` NO fuerza la key cuando
/// no hay key configurada: `decide()` devuelve `Allow` antes de mirar el flag
/// `forced`.  Si se borra `<state>/megadaemon_api.key`, el cockpit (expuesto
/// por el túnel público) queda ABIERTO.
#[test]
fn adv_a04_require_api_key_se_ignora_sin_key_configurada() {
    let cfg = ApiKeyConfig {
        key: String::new(),
        extra_keys: Vec::new(),
        enabled: false, // <- no hay key en disco: el middleware se desactiva
        forced: true,   // <- MEGADAEMON_REQUIRE_API_KEY=1
    };
    let lim = RateLimiter::new();
    let ext = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 9));
    let d = api_key::decide(&cfg, &lim, &ext, "", std::time::Instant::now());
    // A-04 CORREGIDO (cloud-sandbox, 2026-11-20): `forced` sin key es
    // fail-closed: la petición externa se RECHAZA, no entra sin credencial.
    assert_eq!(
        d,
        Decision::Unauthorized,
        "con `forced=true` y sin key, la petición externa debe RECHAZARSE"
    );
}

/// **ADV-A-05 (P1)** — El rate limit (60/min) NO se aplica al loopback por
/// defecto.  En el móvil TODO el tráfico (Chrome, el túnel Cloudflare, los
/// agentes) entra por 127.0.0.1 ⇒ el límite no protege nada.
#[test]
fn adv_a05_el_rate_limit_no_aplica_al_loopback() {
    let loopback = IpAddr::V4(Ipv4Addr::LOCALHOST);
    assert!(
        !api_key::rate_applies_to(&loopback),
        "el loopback está exento del rate limit: en el móvil no queda ningún cubo limitado"
    );
    // Con key válida y peer loopback, 5000 peticiones seguidas: ninguna 429.
    let cfg = ApiKeyConfig {
        key: "k".into(),
        extra_keys: Vec::new(),
        enabled: true,
        forced: false,
    };
    let lim = RateLimiter::new();
    let now = std::time::Instant::now();
    for _ in 0..5000 {
        assert_eq!(
            api_key::decide(&cfg, &lim, &loopback, "X-API-Key: k\r\n", now),
            Decision::Allow
        );
    }
}

/// **ADV-A-06 (P1)** — `write_requires_confirm` sólo cubre `/dev/block` y
/// `/system`.  Escribir en `/data/adb` (módulos Magisk = persistencia) o en
/// el propio directorio del daemon NO pide confirmación.
#[test]
fn adv_a06_escribir_en_data_adb_no_pide_confirmacion() {
    ensure_env();
    if !std::path::Path::new("/usr/bin/su").exists() {
        eprintln!("[SKIP] no hay `su` en este entorno");
        return;
    }
    let target = "/data/adb/modules/md_v15_probe/post-fs-data.sh";
    let res = megadaemon_core::android::fs_write(target, "#!/system/bin/sh\necho pwned\n", false, "755", false);
    let ok = res.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    let needs_confirm = res.get("error").and_then(|v| v.as_str()) == Some("needs_confirm");
    let _ = std::fs::remove_file(target);
    let _ = std::fs::remove_dir("/data/adb/modules/md_v15_probe");
    assert!(
        !needs_confirm,
        "escribir en /data/adb NO pide confirmación: persistencia de backdoor sin confirm.  resp={}",
        res
    );
    assert!(ok, "la escritura debería haberse hecho (el sandbox es root): resp={}", res);
}

// ===========================================================================
// A/C — el flujo de autenticación de la web v14
// ===========================================================================

/// **ADV-A-18 (P0) — CONTROL POSITIVO desde el 20-11-2026 (commit `9f30665`)**.
///
/// Antes este test era ROJO: afirmaba que `POST /api/session/exchange` con una
/// `api_key` inventada devolvía el **token root** (porque la exención
/// `local_browser` decidía "local" con el `Host`, falsificable, y con la
/// ausencia de cabeceras de proxy).
///
/// El arreglo hace que **el canje SIEMPRE valide la API key** ⇒ con una key
/// inventada responde **403** y **no** entrega ningún token.  Este test exige
/// ese comportamiento correcto ⇒ si alguien reintroduce la rama `local_browser`,
/// vuelve a ROJO.
#[test]
fn adv_a18_la_exencion_local_entrega_el_token_root() {
    ensure_env();
    let port = start_cockpit();
    let token = cockpit_server::auth_token().to_string();

    // (1) El canje con una API key BASURA no entrega el token root.
    let (st, body) = post(
        port,
        "/api/session/exchange",
        "{\"api_key\":\"esta-no-es-la-key\"}",
        None,
    );
    assert!(
        st == 403 || st == 401,
        "el canje con una API key inventada debe ser rechazado (403), no {}: body={}",
        st,
        body
    );
    assert!(
        !body.contains(&token),
        "el token root se entregó con una API key inventada (exención local). body={}",
        body
    );
    assert!(
        !body.contains("\"ok\":true"),
        "el canje respondió ok:true con una API key inventada. body={}",
        body
    );
}

/// **ADV-A-18b (P1) — ROJO A PROPÓSITO mientras A-18-resto siga abierto**.
///
/// El token root ya **no** se regala (A-18 cerrado), pero la **exención de
/// token para todos los `GET` locales** sigue viva (`requires_auth`,
/// `cockpit_server.rs:231`): "local" se decide con el `Host` (falsificable) ⇒
/// **cualquier app del móvil** lee `/api/status`, `/api/providers`… **sin
/// credencial** (fuga de datos; ya no da root).
///
/// Este test exige el comportamiento correcto (403 sin token) ⇒ **sale ROJO**
/// hasta que se cierre el agujero; cuando se cierre, pasa a verde.
///
/// `#[ignore]` **a propósito** (decisión del principal, 20-11 08:4x): así la CI
/// queda **verde** y este test sigue **documentando el P1 abierto**.  Owner:
/// web (`e81fade1`) ⇒ **des-ignorar cuando se cierre A-18-resto** (y entonces
/// este test debe quedar en verde).
#[test]
#[ignore = "rojo a propósito: A-18 resto abierto (exención de token en los GET locales); owner web e81fade1; des-ignorar al cerrarlo"]
fn adv_a18b_los_get_locales_sin_token_siguen_abiertos() {
    ensure_env();
    let port = start_cockpit();
    let (st, body) = get(port, "/api/status", None);
    assert_eq!(
        st, 403,
        "sin token, /api/status debe dar 403: la exención local de los GET lo salta \
         (A-18 resto). body={}",
        &body[..body.len().min(200)]
    );
}

/// **ADV-A-19 (P0, control positivo)** — El **túnel público** SÍ está
/// protegido: `Host` de dominio + `CF-Connecting-IP` ⇒ no es "local" ⇒ el
/// intercambio **no** entrega el token sin la API key real.  La diferencia
/// entre el túnel y el puente es **una cabecera**, y el puente reescribe el
/// `Host` ⇒ el túnel queda protegido y el puente no.
#[test]
fn adv_a19_el_tunel_no_recibe_el_token_root() {
    ensure_env();
    let port = start_cockpit();
    let token = cockpit_server::auth_token().to_string();

    let mut req = format!(
        "POST /api/session/exchange HTTP/1.1\r\n\
         Host: tunel-ejemplo.trycloudflare.com\r\n\
         CF-Connecting-IP: 203.0.113.9\r\n\
         Content-Type: application/json\r\n\
         Content-Length: 30\r\n\
         Connection: close\r\n\r\n"
    );
    req.push_str("{\"api_key\":\"esta-no-es-la-key\"}");
    let (st, body) = http(port, &req);
    // Con el arreglo de A-18 (`9f30665`) el canje **siempre** valida la API key
    // ⇒ con una key inventada (túnel o local) la respuesta es 403 sin token.
    assert!(
        st == 403 || st == 401,
        "el canje con una API key inventada debe ser rechazado (403), no {}: body={}",
        st,
        body
    );
    assert!(
        !body.contains(&token),
        "el túnel recibió el token root. body={}",
        body
    );
    assert!(
        !body.contains("\"ok\":true"),
        "el túnel debe exigir la API key real. body={}",
        body
    );
}

/// **ADV-A-08 (P2)** — El `catch-all` del router POST responde **200 OK** a
/// cualquier ruta inexistente con `{"status":"executed"}`.  Un cliente que
/// espera 404/405 recibe un falso "todo bien" (y creerá que ha escrito).
#[test]
fn adv_a08_post_a_ruta_inexistente_devuelve_200_ok() {
    ensure_env();
    let port = start_cockpit();
    let token = cockpit_server::auth_token().to_string();

    let (st, body) = post(port, "/api/no-existe-esta-ruta", "{}", Some(&token));
    assert_eq!(
        st, 200,
        "una ruta POST inexistente devuelve 200 (debería ser 404). body={}",
        body
    );
    assert!(
        body.contains("\"status\":\"executed\""),
        "el catch-all POST finge ejecución. body={}",
        body
    );
}

/// **ADV-A-09 (P2)** — `check_llama_online()` sondea `127.0.0.1:8080`.  En el
/// móvil el PUENTE WEB (o cualquier proceso) escucha en 8080 ⇒
/// `brain_active` es SIEMPRE true, aunque no haya ningún LLM.
#[test]
fn adv_a09_brain_status_true_por_cualquier_proceso_en_8080() {
    ensure_env();
    let port = start_cockpit();
    let token = cockpit_server::auth_token().to_string();

    // Un listener cualquiera en 8080 (no hay ningún LLM detrás).
    let listener = TcpListener::bind("127.0.0.1:8080");
    if listener.is_err() {
        eprintln!("[SKIP] el puerto 8080 está ocupado en este entorno");
        return;
    }
    let _listener = listener.unwrap();

    let (st, body) = get(port, "/api/brain_status", Some(&token));
    assert_eq!(st, 200, "body={}", body);
    assert!(
        body.contains("\"brain_active\":true"),
        "con un listener vacío en 8080, brain_active=true: el indicador es un falso \
         positivo. body={}",
        body
    );
}

/// **ADV-A-10 (P2)** — El catálogo de puertos alias NO incluye 8080, pero el
/// supervisor, el puente web y toda la documentación dan por hecho que 8080
/// sirve la UI.  Con el binario v14 esa URL vuelve a estar muerta.
#[test]
fn adv_a10_8080_no_esta_en_los_puertos_alias() {
    let ports = cockpit_server::ALIAS_PORTS;
    assert!(
        !ports.contains(&8080),
        "ALIAS_PORTS = {:?}: 8080 no está, y el supervisor/web_bridge/docs lo dan por \
         servido",
        ports
    );
    assert!(ports.contains(&3000) && ports.contains(&3030) && ports.contains(&8090));
}

// ===========================================================================
// C — CORRECCIÓN
// ===========================================================================

/// **ADV-C-01 (P2)** — `estimate_task_tokens` usa la heurística "chars/4",
/// que el propio repo (`tools/token_budget.py`) documenta como **errónea
/// entre un 15% y un 30%**.  El enrutado de la delegación decide por "menor
/// coste estimado".
#[test]
fn adv_c01_el_estimador_de_tokens_usa_chars_por_4() {
    // El texto "aaaa aaaa ..." tokeniza mucho peor que chars/4 en cualquier
    // tokenizer real: 4 caracteres ASCII suelen ser 1 token, pero con
    // caracteres multibyte o palabras largas la estimación se va.
    let task = megadaemon_core::delegate::DelegationTask {
        id: "t1".into(),
        prompt: "ÁÉÍÓÚáéíóúñ ".repeat(100),
        kind: "research".into(),
        required_capabilities: Vec::new(),
        max_tokens: 0,
        deadline: None,
        status: "queued".into(),
        attempts: Vec::new(),
        assigned_account: None,
    };
    let (in_tok, _) = megadaemon_core::delegate::estimate_task_tokens(&task);
    let chars = task.prompt.chars().count() as u32;
    let esperado_chars_por_4 = chars / 4 + 64;
    assert_eq!(
        in_tok, esperado_chars_por_4,
        "la estimación es chars/4 (documentado como 15-30% de error en tools/token_budget.py)"
    );
    assert!(
        in_tok < chars / 2,
        "con texto multibyte la estimación se queda muy corta (chars={} est={})",
        chars,
        in_tok
    );
}

/// **ADV-C-02 (P2)** — `unwrap_or_default()` sobre la lectura de un fichero
/// convierte "no existe" en "vacío" ⇒ el diagnóstico dice "no hay MMIO" (o
/// "nada que analizar") en lugar de "no se pudo leer el fichero".  Con una
/// plataforma **inexistente** (`c_path == ""`) la lectura falla siempre, y la
/// respuesta sigue siendo **200 OK con un análisis vacío y sin ningún campo
/// de error**: el cliente no puede distinguir "no hay fichero" de "no hay MMIO".
#[test]
fn adv_c02_hw_mmio_convierte_un_error_de_lectura_en_vacio() {
    ensure_env();
    let port = start_cockpit();
    let token = cockpit_server::auth_token().to_string();

    // Plataforma inexistente => `c_path` vacío => la lectura SIEMPRE falla.
    let (st, body) = get(port, "/api/hw_mmio?platform=zzz_no_existe", Some(&token));
    assert_eq!(st, 200, "body={}", body);
    assert!(
        !body.contains("\"error\""),
        "la respuesta no dice que el fichero no se pudo leer: body={}",
        body
    );
    assert!(
        body.contains("\"mmio_hits\":[]"),
        "la respuesta es un análisis vacío sin indicar el error de lectura: body={}",
        body
    );
}

// ===========================================================================
// E — RENDIMIENTO / RECURSOS
// ===========================================================================

/// **ADV-E-01 (P1)** — Cada request relee del disco la configuración de la
/// API key (2 ficheros) y la bóveda de proveedores (1 fichero).  No hay caché:
/// el cambio se ve en la petición siguiente (y el coste se paga siempre).
#[test]
fn adv_e01_la_config_de_api_key_se_relee_en_cada_request() {
    let dir = std::env::temp_dir().join("md_v15_e01");
    let _ = std::fs::create_dir_all(&dir);
    let d = dir.to_string_lossy().to_string();
    let cfg1 = ApiKeyConfig::load(&d);
    assert!(!cfg1.enabled, "no hay key en el directorio de estado");

    std::fs::write(
        dir.join("megadaemon_api.key"),
        "mdaemon_sk_live_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    )
    .unwrap();
    let cfg2 = ApiKeyConfig::load(&d);
    assert!(
        cfg2.enabled,
        "la key recién escrita ya está activa en la carga siguiente: no hay caché"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// **ADV-E-02 (P1)** — El techo de concurrencia son 64 hilos, pero cada
/// conexión puede reservar hasta 8 MB de cuerpo ⇒ el techo real de memoria
/// del cockpit es ~512 MB (+ pilas de 2 MB por hilo).  En un móvil con
/// ~300 MB libres eso es un OOM, no un 503.
#[test]
fn adv_e02_el_techo_de_memoria_del_cockpit_supera_la_ram_libre() {
    // Constantes reales del código (cockpit_server.rs: MAX_BODY_BYTES, MAX_CLIENT_THREADS).
    const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;
    const MAX_CLIENT_THREADS: usize = 64;
    const STACK_POR_HILO: usize = 2 * 1024 * 1024;
    let techo = MAX_BODY_BYTES * MAX_CLIENT_THREADS + STACK_POR_HILO * MAX_CLIENT_THREADS;
    assert!(
        techo > 300 * 1024 * 1024,
        "techo de memoria del cockpit = {} MB (el móvil tiene ~300 MB libres)",
        techo / (1024 * 1024)
    );
}

// ===========================================================================
// H — EL AGENTE EXTERNO / EL CONVERSOR DE FINDINGS
// ===========================================================================

/// **ADV-H-01 (P2)** — El conversor `tools/findings_from_manifest.py` declara
/// resolver la colisión histórica "signal (CLASE) vs app_intel (VALOR)"
/// **moviendo** el valor; en realidad lo **duplica**: `class` y `signal`
/// acaban con el mismo valor.
#[test]
fn adv_h01_el_conversor_duplica_el_valor_en_la_colision_signal_class() {
    let py = match std::process::Command::new("python3")
        .arg("-c")
        .arg("print(1)")
        .output()
    {
        Ok(o) if o.status.success() => "python3",
        _ => {
            eprintln!("[SKIP] no hay python3 en este entorno");
            return;
        }
    };
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let script = repo_root.join("tools/findings_from_manifest.py");
    if !script.exists() {
        eprintln!("[SKIP] no está tools/findings_from_manifest.py");
        return;
    }
    let probe = std::env::temp_dir().join("md_v15_h01_probe.py");
    std::fs::write(
        &probe,
        "import sys, json\n         sys.path.insert(0, sys.argv[1])\n         import findings_from_manifest as F\n         out = F.normalize({'package':'com.x','signal':'fs_path','class':'unknown'}, {}, [])\n         print(json.dumps({'class': out['class'], 'signal': out['signal']}))\n",
    )
    .unwrap();
    let out = std::process::Command::new(py)
        .arg(&probe)
        .arg(repo_root.join("tools"))
        .output()
        .expect("ejecutar el conversor");
    let txt = String::from_utf8_lossy(&out.stdout).into_owned();
    let err = String::from_utf8_lossy(&out.stderr).into_owned();
    let _ = std::fs::remove_file(&probe);
    let v: serde_json::Value =
        serde_json::from_str(txt.trim()).unwrap_or_else(|_| serde_json::json!({}));
    assert_eq!(
        v.get("class").and_then(|x| x.as_str()),
        Some("fs_path"),
        "la colisión debería mover el valor a class. salida={} stderr={}",
        txt, err
    );
    assert_eq!(
        v.get("signal").and_then(|x| x.as_str()),
        Some("unknown"),
        "el valor queda DUPLICADO en signal: el conversor no cumple su propio contrato. \
         salida={} stderr={}",
        txt, err
    );
}

// ========================================================================= //
// D-09 / D-10 — FIABILIDAD (hallazgos añadidos en la revisión de las 03:3x)
// ========================================================================= //

/// Raíz de `rust-core/src` (para los tests que leen el código fuente).
fn src_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Ficheros `.rs` de `src/` con la línea citada.
fn src_hits(needle: &str) -> Vec<String> {
    let mut out = Vec::new();
    let dir = src_dir();
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("leer rust-core/src")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("rs"))
        .collect();
    entries.sort();
    for p in entries {
        let txt = std::fs::read_to_string(&p).unwrap_or_default();
        for (i, l) in txt.lines().enumerate() {
            if l.contains(needle) {
                let name = p.file_name().unwrap().to_string_lossy().to_string();
                out.push(format!("{}:{}", name, i + 1));
            }
        }
    }
    out
}

#[test]
fn adv_d09_no_queda_ningun_lock_unwrap_sobre_mutex_globales() {
    // Un panic mantiene envenenado el mutex: con `lock().unwrap()` TODAS las
    // llamadas posteriores a ese mutex paniquean => el subsistema queda muerto
    // para el resto de la vida del proceso, y como el release desenrolla el
    // daemon sigue contestando => "parece sano y no lo está".
    // El arreglo es `unwrap_or_else(|p| p.into_inner())`, que ya usan
    // `cockpit_server.rs:1899/1912/1931` y `api_key.rs`.
    let hits = src_hits("lock().unwrap()");
    assert!(
        hits.is_empty(),
        "quedan {} `lock().unwrap()` (un panic envenena el mutex): {}",
        hits.len(),
        hits.join(", ")
    );
}

#[test]
fn adv_d10_hay_un_panic_set_hook_que_registra_el_backtrace() {
    // Sin `panic::set_hook` un panic no deja rastro.  Con unwind, el 134
    // (SIGABRT) medido en el móvil NO puede ser un panic de Rust: viene de un
    // `abort()` explicito, un doble panic o un assert en lib nativa
    // (sqlite3/NPU).  Sin hook no hay forma de localizarlo.
    let hits = src_hits("panic::set_hook");
    assert!(
        !hits.is_empty(),
        "no hay ningun `panic::set_hook` en rust-core/src: un panic (o el 134 \
         medido) no deja backtrace ni se puede diagnosticar"
    );
}

//! MegaDaemon v13.1/13.2 — SUITE DE AUDITORÍA ADVERSARIAL DEL HARNESS SEERE
//!
//! Nace de la auditoría adversarial de `src/seere_v11.rs` y sus consumidores
//! (`cockpit_server.rs`, `main.rs`, `mcp_server.rs`), que NO estaba cubierta
//! por la auditoría previa (docs/AUDIT_V13_1.md, F01..F38) ni por
//! `tests/audit_v13.rs`.
//!
//! Cada test AFIRMA el comportamiento CORRECTO: si el defecto vuelve, el
//! test falla.  Historia de cada hallazgo (todos reproducidos antes del
//! arreglo con `cargo test --test audit_scout_seere -- --nocapture`):
//!
//!   S01  ALTA  normalize_action() panic por índice de byte no-char-boundary
//!   S02  ALTA  la puerta de taint era inerte con allow_exec=true
//!   S03  ALTA  el atajo "$ " (0 tokens) ignoraba allow_exec
//!   S04  ALTA  la fase final ejecutaba acciones (lectura de ficheros)
//!   S05  MEDIA el verificador de fundamento no comprobaba los enteros
//!   S06  MEDIA exec_root() sin fallback si el runner existe y falla
//!   S06b MEDIA un comando que fallaba se reintentaba (efectos duplicados)
//!   S07  MEDIA el guardián anti-echo no cubría la acción de última hora
//!   S08  ALTA  RCE sin autenticación en el cockpit (+ CORS abierto)
//!   S09  MEDIA la política de energía no se aplicaba al agente
//!   S10  MEDIA DELEGATE_LLM cableado a un modelo RETIRADO
//!   S11  MEDIA gramática y parser divergentes
//!   S12  MEDIA el arreglo de F18 era cosmético (la observación se filtraba)
//!   S13  MEDIA truncado por bytes sobre &str -> panic
//!   S14  MEDIA la tarea delegada se perdía en silencio
//!
//! Ejecutar:
//!   export PATH="$HOME/.cargo/bin:$PATH"
//!   cd /workspace/megadaemon/rust-core
//!   cargo test --test audit_scout_seere -- --nocapture --test-threads=1

use megadaemon_core::cockpit_server;
use megadaemon_core::provider_catalog;
use megadaemon_core::seere_v11 as S;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

// ========================================================================= //
// Utilidades
// ========================================================================= //

/// Serializa los tests que tocan estado de proceso (env vars, PATH).
/// `unwrap_or_else(|e| e.into_inner())` evita que un test que falle envenene
/// el lock y cascade en los siguientes.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static M: OnceLock<Mutex<()>> = OnceLock::new();
    let m = M.get_or_init(|| Mutex::new(()));
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn esc_json(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '"' => o.push_str("\\\""),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

/// Servidor HTTP falso que habla el protocolo de llama-server.
/// Devuelve (puerto, nº de peticiones, max_tokens pedidos en cada una).
fn spawn_fake_llama(reply: &'static str) -> (u16, Arc<AtomicUsize>, Arc<Mutex<Vec<u32>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake llama");
    let port = listener.local_addr().unwrap().port();
    let hits = Arc::new(AtomicUsize::new(0));
    let maxtok = Arc::new(Mutex::new(Vec::new()));
    let h2 = hits.clone();
    let m2 = maxtok.clone();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let mut s = match stream {
                Ok(s) => s,
                Err(_) => break,
            };
            let _h = h2.fetch_add(1, Ordering::SeqCst);
            let mut raw: Vec<u8> = Vec::new();
            let mut buf = [0u8; 8192];
            loop {
                let n = match s.read(&mut buf) {
                    Ok(n) => n,
                    Err(_) => 0,
                };
                if n == 0 {
                    break;
                }
                raw.extend_from_slice(&buf[..n]);
                if raw.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let head = String::from_utf8_lossy(&raw).to_string();
            let mut clen = 0usize;
            for line in head.lines() {
                let l = line.to_lowercase();
                if l.starts_with("content-length:") {
                    clen = l["content-length:".len()..].trim().parse().unwrap_or(0);
                }
            }
            let body_start = head.find("\r\n\r\n").map(|i| i + 4).unwrap_or(raw.len());
            while raw.len() < body_start + clen {
                let n = match s.read(&mut buf) {
                    Ok(n) => n,
                    Err(_) => 0,
                };
                if n == 0 {
                    break;
                }
                raw.extend_from_slice(&buf[..n]);
            }
            if let Ok(v) = serde_json::from_slice::<Value>(&raw[body_start..]) {
                if let Some(mt) = v.get("max_tokens").and_then(|x| x.as_u64()) {
                    m2.lock().unwrap().push(mt as u32);
                }
            }
            let body = format!(
                r#"{{"choices":[{{"message":{{"content":"{}"}},"finish_reason":"stop"}}],"usage":{{"total_tokens":42}}}}"#,
                esc_json(reply)
            );
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = s.write_all(resp.as_bytes());
            let _ = s.flush();
        }
    });
    (port, hits, maxtok)
}

fn free_port() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let p = l.local_addr().unwrap().port();
    drop(l);
    p
}

/// POST HTTP crudo contra el cockpit y devuelve la respuesta completa.
fn http_post(port: u16, path: &str, body: &str, extra_headers: &str) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", port)).expect("connect cockpit");
    s.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
    let req = format!(
        "POST {} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n{}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        path,
        port,
        extra_headers,
        body.len(),
        body
    );
    let _ = s.write_all(req.as_bytes());
    let _ = s.flush();
    let mut out = String::new();
    let _ = s.read_to_string(&mut out);
    out
}

/// Crea un directorio temporal con scripts falsos y lo devuelve.
/// En Android `/storage` o algunos `TMPDIR` pueden estar en `noexec`, por lo
/// que se prioriza el directorio interno ejecutable `/data/user/0/com.agy/files/tmp`.
fn fake_bin_dir(tag: &str) -> std::path::PathBuf {
    let base = if std::path::Path::new("/data/user/0/com.agy/files/tmp").exists() {
        std::path::PathBuf::from("/data/user/0/com.agy/files/tmp")
    } else {
        std::env::temp_dir()
    };
    let dir = base.join(format!("seere_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_exec(dir: &std::path::Path, name: &str, body: &str) {
    let p = dir.join(name);
    // En Android SELinux (`untrusted_app`), un script en `app_data_file` no
    // puede tener `#! /system/bin/sh` (SELinux deniega `execute_no_trans` ->
    // EACCES os error 13), pero sí el ELF nativo `usr/bin/bash` del propio sandbox.
    let fixed_body = if !std::path::Path::new("/bin/sh").exists()
        && std::path::Path::new("/data/user/0/com.agy/files/usr/bin/bash").exists()
        && body.starts_with("#!/bin/sh\n")
    {
        body.replacen(
            "#!/bin/sh\n",
            "#!/data/user/0/com.agy/files/usr/bin/bash\n",
            1,
        )
        .replace(
            "exec /bin/sh",
            "exec /data/user/0/com.agy/files/usr/bin/bash",
        )
    } else {
        body.to_string()
    };
    std::fs::write(&p, fixed_body).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perm = std::fs::metadata(&p).unwrap().permissions();
        perm.set_mode(0o755);
        std::fs::set_permissions(&p, perm).unwrap();
    }
}

// ========================================================================= //
// S1 (ALTA) — normalize_action() no debe entrar en PANIC con una salida de
//             LLM legal.  Recortaba el argumento con un índice de BYTES
//             calculado sobre `to_uppercase()`, que puede cambiar el número
//             de bytes (ſ U+017F -> 'S', ǰ U+01F0 -> "J"+U+030C) y caer
//             dentro de un carácter.  En release (panic=abort) eso tumba el
//             daemon entero.
// ========================================================================= //

#[test]
fn s01_normalize_action_no_panea_con_prefijo_unicode() {
    // "A: ISS Iſſ: hola"  ->  el prefijo "ISS:" en mayúsculas coincide, pero
    // el índice cae dentro del segundo 'ſ' (bytes 3..5).
    let raw = "A: ISS I\u{17F}\u{17F}: hola";
    let r = std::panic::catch_unwind(|| S::normalize_action(raw));
    assert!(
        r.is_ok(),
        "[S01] normalize_action() entró en panic con {:?}: {:?}",
        raw,
        r.err()
    );
    let (tool, arg) = r.unwrap();
    println!("[S01] normalize_action -> ({}, {:?})", tool, arg);
    assert_eq!(tool, "ISS");
    assert_eq!(arg, "hola", "[S01] el prefijo debe quitarse sin partir caracteres");
}

#[test]
fn s01b_parse_model_output_no_panea_con_prefijo_unicode() {
    let raw = "A: ISS I\u{17F}\u{17F}: hola";
    let r = std::panic::catch_unwind(|| S::parse_model_output(raw));
    assert!(
        r.is_ok(),
        "[S01b] parse_model_output() entró en panic con {:?}",
        raw
    );
    println!("[S01b] OK: parse_model_output no panea");
}

#[test]
fn s01c_normalize_action_no_panea_out_of_bounds() {
    // "A: ǰ ǰ:"  ->  el prefijo en mayúsculas es más largo que el original.
    let raw = "A: \u{1F0} \u{1F0}:";
    let r = std::panic::catch_unwind(|| S::normalize_action(raw));
    assert!(
        r.is_ok(),
        "[S01c] normalize_action() entró en panic (out of bounds) con {:?}",
        raw
    );
    println!("[S01c] OK: normalize_action no panea (out of bounds)");
}

// ========================================================================= //
// S2 (ALTA) — La puerta de taint bloquea la ejecución y la delegación cuando
//             el contexto viene de contenido no fiable.  Antes la condición
//             era `&& !self.allow_exec`, con lo que quedaba INERTE en la
//             configuración real (CLI y cockpit: allow_exec=true).
// ========================================================================= //

#[test]
fn s02_el_taint_bloquea_root_exec() {
    let _g = serial();
    let a = S::Arsenal::new(false, true, true);
    let out = a.dispatch("ROOT_EXEC", "echo TAINTED_EXEC_MARKER", true);
    println!("[S02] dispatch(ROOT_EXEC, tainted=true) -> {}", out);
    assert!(
        out.starts_with("ERR_POLICY"),
        "[S02] DEFECTO: el contexto manchado ejecutó la orden: {}",
        out
    );
    assert!(
        !out.contains("TAINTED_EXEC_MARKER"),
        "[S02] DEFECTO: el contexto manchado ejecutó la orden: {}",
        out
    );
}

#[test]
fn s02b_el_taint_bloquea_la_delegacion_externa() {
    let _g = serial();
    let a = S::Arsenal::new(false, true, true);
    let out = a.dispatch("DELEGATE_LLM", "resumen", true);
    println!("[S02b] dispatch(DELEGATE_LLM, tainted=true) -> {}", out);
    assert!(
        out.starts_with("ERR_POLICY"),
        "[S02b] DEFECTO: la delegación con contexto manchado no está bloqueada: {}",
        out
    );
}

#[test]
fn s02c_el_escape_hatch_del_taint_es_explicito() {
    let _g = serial();
    // Con allow_tainted el operador puede reabrirlo (por defecto NO).
    let a = S::Arsenal::new(false, true, true).allow_tainted(true);
    let out = a.dispatch("ROOT_EXEC", "echo TAINT_OK", true);
    println!("[S02c] allow_tainted=true -> {}", out);
    assert!(
        out.contains("TAINT_OK"),
        "[S02c] con allow_tainted=true la orden debe ejecutarse: {}",
        out
    );

    // Y por defecto está cerrado.
    let b = S::Arsenal::new(false, true, true);
    let out2 = b.dispatch("ROOT_EXEC", "echo TAINT_BLOCKED", true);
    println!("[S02c] allow_tainted=false -> {}", out2);
    assert!(
        out2.starts_with("ERR_POLICY"),
        "[S02c] por defecto el taint bloquea: {}",
        out2
    );
}

// ========================================================================= //
// S3 (ALTA) — El atajo de 0 tokens ("$ cmd") respeta la política: con
//             allow_exec=false no se ejecuta nada.
// ========================================================================= //

#[test]
fn s03_atajo_de_shell_respeta_la_politica_cerrada() {
    let _g = serial();
    let cfg = S::SeereConfig {
        allow_tools: true,
        allow_exec: false, // política CERRADA
        allow_external: false,
        max_turns: 4,
        ..Default::default()
    };
    let r = S::run_seere("$ echo ZERO_SH_POLICY_BYPASS_MARKER", &cfg);
    let ans = r["answer"].as_str().unwrap_or("").to_string();
    println!("[S03] answer = {}", ans);
    println!("[S03] mode   = {}", r["mode"]);
    assert!(
        !ans.contains("ZERO_SH_POLICY_BYPASS_MARKER"),
        "[S03] DEFECTO: allow_exec=false pero la orden se ejecutó: {}",
        ans
    );
}

#[test]
fn s03b_el_atajo_de_shell_sigue_funcionando_con_la_politica_abierta() {
    let _g = serial();
    let cfg = S::SeereConfig {
        allow_tools: true,
        allow_exec: true,
        allow_external: false,
        max_turns: 4,
        ..Default::default()
    };
    let r = S::run_seere("$ echo ZERO_SH_OK", &cfg);
    let ans = r["answer"].as_str().unwrap_or("").to_string();
    println!("[S03b] answer = {}", ans);
    assert!(
        ans.contains("ZERO_SH_OK"),
        "[S03b] con allow_exec=true el atajo debe ejecutar: {}",
        ans
    );
}

// ========================================================================= //
// S4 (ALTA) — En la fase de respuesta NO se ejecuta ninguna acción: una
//             línea suelta ("C: /fichero") junto a una respuesta "R: ..." no
//             puede convertirse en una lectura de fichero cuyo contenido
//             acabe en el campo "answer".
// ========================================================================= //

#[test]
fn s04_la_fase_de_respuesta_no_puede_leer_ficheros() {
    let _g = serial();

    let tmp = std::env::temp_dir().join(format!("seere_s04_{}.txt", std::process::id()));
    let marker = "S04_SECRET_FILE_CONTENT_MARKER";
    std::fs::write(&tmp, marker).unwrap();
    let path = tmp.to_string_lossy().to_string();

    let reply = format!("R: todo listo\nC: {}", path);
    let (port, hits, _mt) = spawn_fake_llama(Box::leak(reply.into_boxed_str()));

    std::env::set_var("SEERE_LLM", format!("http://127.0.0.1:{}", port));
    let cfg = S::SeereConfig {
        max_turns: 1,
        allow_tools: true,
        allow_exec: false,
        allow_external: false,
        ..Default::default()
    };
    let r = S::run_seere("dime hola", &cfg);
    let ans = r["answer"].as_str().unwrap_or("").to_string();
    println!("[S04] hits LLM   = {}", hits.load(Ordering::SeqCst));
    println!("[S04] answer     = {:?}", ans);
    println!("[S04] tools      = {}", r["tools"]);
    std::env::remove_var("SEERE_LLM");
    let _ = std::fs::remove_file(&tmp);

    assert!(
        !ans.contains(marker),
        "[S04] DEFECTO: la fase de respuesta leyó el fichero: {:?}",
        ans
    );
}

#[test]
fn s04b_una_linea_suelta_no_se_convierte_en_accion_si_hay_respuesta() {
    let p = S::parse_model_output("R: todo listo\nC: /etc/hostname");
    println!("[S04b] out_type = {}", p.out_type);
    println!("[S04b] actions  = {:?}", p.actions);
    println!("[S04b] response = {:?}", p.response);
    assert!(
        p.actions.is_empty(),
        "[S04b] DEFECTO: una respuesta 'R:' con una línea suelta se convierte en acción: {:?}",
        p.actions
    );
}

// ========================================================================= //
// S5 (MEDIA-ALTA) — El verificador de fundamento comprueba TAMBIÉN los
//             enteros: "5 nucleos" y "4 GB" no pueden pasar sin respaldo.
//             (La regla de F09 —sin OBS todo número es inventado— sólo se
//             aplicaba cuando NO había ninguna observación.)
// ========================================================================= //

#[test]
fn s05_grounding_no_acepta_enteros_inventados() {
    let obs = vec!["<OBS id=\"1\">uptime 22</OBS>".to_string()];
    let ids = vec!["1".to_string()];

    let (ok1, miss1) = S::verify_grounding(
        "La maquina tiene 5 nucleos",
        &obs,
        "uid=0",
        "cuantos nucleos",
        Some(&ids),
    );
    println!("[S05] '5 nucleos'   -> ok={} miss={:?}", ok1, miss1);
    assert!(
        !ok1,
        "[S05] DEFECTO: un 5 inventado pasa el verificador de fundamento (miss={:?})",
        miss1
    );

    let (ok2, miss2) =
        S::verify_grounding("La memoria es 4 GB", &obs, "uid=0", "ram", Some(&ids));
    println!("[S05] '4 GB'        -> ok={} miss={:?}", ok2, miss2);
    assert!(
        !ok2,
        "[S05] DEFECTO: '4 GB' inventado pasa el verificador de fundamento (miss={:?})",
        miss2
    );
}

#[test]
fn s05b_control_un_entero_que_si_esta_en_el_observado_pasa() {
    let obs = vec!["<OBS id=\"1\">memoria total 4 GB</OBS>".to_string()];
    let ids = vec!["1".to_string()];
    let (ok, miss) = S::verify_grounding("La memoria es 4 GB", &obs, "uid=0", "ram", Some(&ids));
    println!("[S05b] '4 GB' respaldado -> ok={} miss={:?}", ok, miss);
    assert!(ok, "[S05b] un 4 GB respaldado por el OBS debe pasar: {:?}", miss);
}

// ========================================================================= //
// S6 (MEDIA) — exec_root() hace fallback cuando el runner EXISTE pero no
//             puede arrancar (Magisk denegando `su`, `rish` sin servidor).
// ========================================================================= //

#[test]
fn s06_exec_root_hace_fallback_si_el_runner_existe_y_falla() {
    let _g = serial();

    let dir = fake_bin_dir("s06_bin");
    write_exec(&dir, "su", "#!/bin/sh\necho 'su: permission denied' >&2\nexit 1\n");
    write_exec(&dir, "rish", "#!/bin/sh\necho 'Server is not running' >&2\nexit 1\n");
    write_exec(&dir, "sh", "#!/bin/sh\necho FALLBACK_VIA_SH_OK\n");

    let old_path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{}:{}", dir.to_string_lossy(), old_path));

    let a = S::Arsenal::new(false, true, false);
    let out = a.exec_root("echo cualquier-cosa");
    println!("[S06] exec_root() -> {}", out);

    std::env::set_var("PATH", old_path);
    let _ = std::fs::remove_dir_all(&dir);

    assert!(
        out.contains("FALLBACK_VIA_SH_OK"),
        "[S06] DEFECTO: `su` existe pero falla y NO hay fallback a rish/sh: {}",
        out
    );
}

#[test]
fn s06b_un_comando_que_falla_no_se_reintenta_con_otro_runner() {
    let _g = serial();

    // `su` y `rish` arrancan de verdad (ejecutan el comando): si el harness
    // reintentara tras un fallo del COMANDO, el contador marcaría >1.
    let dir = fake_bin_dir("s06b_bin");
    write_exec(&dir, "su", "#!/bin/sh\nexec /bin/sh -c \"$2\"\n");
    write_exec(&dir, "rish", "#!/bin/sh\nexec /bin/sh -c \"$2\"\n");

    let counter = std::env::temp_dir().join(format!("seere_s06b_counter_{}", std::process::id()));
    let _ = std::fs::remove_file(&counter);
    let cpath = counter.to_string_lossy().to_string();

    let old_path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{}:{}", dir.to_string_lossy(), old_path));

    let a = S::Arsenal::new(false, true, false);
    let out = a.exec_root(&format!(
        "echo run >> {}; echo 'denied' >&2; exit 3",
        cpath
    ));
    println!("[S06b] exec_root() -> {}", out);
    let runs = std::fs::read_to_string(&counter).unwrap_or_default();
    let n = runs.lines().filter(|l| l.trim() == "run").count();
    println!("[S06b] ejecuciones del comando = {}", n);

    std::env::set_var("PATH", old_path);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&counter);

    assert_eq!(
        n, 1,
        "[S06b] DEFECTO: el comando se ejecutó {} veces (efectos laterales duplicados): {}",
        n, out
    );
}

// ========================================================================= //
// S7 (MEDIA) — El guardián anti-echo cubre TAMBIÉN la acción de última hora
//             ("R: $ ..."): el eco de la línea OPS no se ejecuta.
// ========================================================================= //

#[test]
fn s07_el_eco_de_la_linea_ops_no_se_ejecuta() {
    let _g = serial();

    let (port, hits, _mt) = spawn_fake_llama("R: $ OPS $ shell | ? web search");
    std::env::set_var("SEERE_LLM", format!("http://127.0.0.1:{}", port));

    let cfg = S::SeereConfig {
        max_turns: 1,
        allow_tools: true,
        allow_exec: true,
        allow_external: false,
        ..Default::default()
    };
    let r = S::run_seere("listame la linea OPS", &cfg);
    let ans = r["answer"].as_str().unwrap_or("").to_string();
    println!("[S07] hits LLM = {}", hits.load(Ordering::SeqCst));
    println!("[S07] answer   = {:?}", ans);
    println!("[S07] tools    = {}", r["tools"]);
    std::env::remove_var("SEERE_LLM");

    assert!(
        !ans.contains("not found"),
        "[S07] DEFECTO: el eco de la lista OPS se ejecutó como shell: {:?}",
        ans
    );
}

#[test]
fn s07b_control_el_detector_de_eco_reconoce_el_echo() {
    let p = S::parse_model_output("A: $ OPS $ shell | ? web search | W wiki search");
    println!(
        "[S07b] out_type={} echo={} actions={:?}",
        p.out_type, p.echo, p.actions
    );
    assert!(p.echo, "[S07b] el detector debería marcar echo");
    assert!(
        p.actions.is_empty(),
        "[S07b] el detector debería vaciar las acciones"
    );
    let (tool, arg) = S::lead_opcode("$ OPS $ shell | ? web search").expect("lead_opcode");
    assert!(
        S::is_echo_arg(&tool, &arg),
        "[S07b] el detector NO reconoce el eco en la ruta de última hora"
    );
}

#[test]
fn s07c_control_la_accion_de_ultima_hora_sigue_funcionando() {
    // F06: "R: $ free -m" es una orden, no una respuesta -> se ejecuta UNA
    // vez.  El guardián anti-echo no puede tumbar esta vía.
    let _g = serial();
    let (port, _hits, _mt) = spawn_fake_llama("R: $ echo LAST_CHANCE_OK");
    std::env::set_var("SEERE_LLM", format!("http://127.0.0.1:{}", port));
    let cfg = S::SeereConfig {
        max_turns: 1,
        allow_tools: true,
        allow_exec: true,
        allow_external: false,
        ..Default::default()
    };
    let r = S::run_seere("dame la memoria", &cfg);
    let ans = r["answer"].as_str().unwrap_or("").to_string();
    println!("[S07c] answer = {:?}", ans);
    std::env::remove_var("SEERE_LLM");
    assert!(
        ans.contains("LAST_CHANCE_OK"),
        "[S07c] la acción de última hora (F06) debe seguir funcionando: {:?}",
        ans
    );
}

// ========================================================================= //
// S8 (ALTA) — El cockpit exige autenticación para todo endpoint que
//             ejecute o mute estado, y el CORS no es `*`.
// ========================================================================= //

#[test]
fn s08_el_cockpit_exige_autenticacion_para_ejecutar() {
    let _g = serial();
    let port = free_port();
    thread::spawn(move || {
        let _ = cockpit_server::run_server(port);
    });
    thread::sleep(Duration::from_millis(500));

    let marker = "S08_COCKPIT_UNAUTH_RCE_MARKER";
    let body = json!({ "message": format!("$ echo {}", marker), "model": "local-seere" }).to_string();
    let resp = http_post(port, "/api/chat", &body, "Origin: https://evil.example\r\n");
    println!("[S08] ---- respuesta cruda ----\n{}\n----------------------------", resp);

    assert!(
        !resp.contains(marker),
        "[S08] DEFECTO: POST /api/chat sin autenticación ejecutó la orden: {}",
        resp
    );
    assert!(
        resp.contains("unauthorized") || resp.contains("403"),
        "[S08] DEFECTO: se esperaba 403 unauthorized: {}",
        resp
    );
}

#[test]
fn s08b_el_endpoint_llm_agent_tambien_exige_autenticacion() {
    let _g = serial();
    let port = free_port();
    thread::spawn(move || {
        let _ = cockpit_server::run_server(port);
    });
    thread::sleep(Duration::from_millis(500));

    let marker = "S08B_AGENT_UNAUTH_RCE_MARKER";
    let body = json!({ "prompt": format!("$ echo {}", marker) }).to_string();
    let resp = http_post(port, "/api/llm_agent", &body, "");
    println!("[S08b] ---- respuesta ----\n{}\n-------------------", resp);
    assert!(
        !resp.contains(marker),
        "[S08b] DEFECTO: /api/llm_agent ejecutó la orden sin autenticación: {}",
        resp
    );
}

#[test]
fn s08c_el_cors_no_es_una_comodilla_para_todos_los_origenes() {
    let _g = serial();
    let port = free_port();
    thread::spawn(move || {
        let _ = cockpit_server::run_server(port);
    });
    thread::sleep(Duration::from_millis(500));

    let body = json!({ "message": "hola" }).to_string();
    let resp = http_post(port, "/api/chat", &body, "Origin: https://evil.example\r\n");
    let hdr = resp.split("\r\n\r\n").next().unwrap_or("").to_uppercase();
    println!("[S08c] cabeceras:\n{}", hdr);
    assert!(
        !hdr.contains("ACCESS-CONTROL-ALLOW-ORIGIN: *"),
        "[S08c] DEFECTO: el cockpit sigue sirviendo CORS abierto a cualquier origen"
    );
}

// ========================================================================= //
// S9 (MEDIA) — La política de energía se aplica al agente: turnos y tokens.
// ========================================================================= //

#[test]
fn s09_la_politica_de_energia_se_aplica_al_agente() {
    let st = megadaemon_core::power_policy::PowerState {
        capacity_pct: Some(10),
        charging: false,
        battery_temp_c: Some(25.0),
        cpu_temp_c: Some(30.0),
        thermal_hot: false,
        system_power_save: false,
        governor: None,
        usb_plugged: false,
    };
    let pol = megadaemon_core::power_policy::policy_for(&st);
    println!(
        "[S09] policy = {} max_turns={} max_tokens={}",
        pol.name, pol.max_turns, pol.max_tokens
    );
    assert_eq!(pol.name, "critical");
    assert_eq!(pol.max_turns, 1);

    let src = std::fs::read_to_string("src/cockpit_server.rs").expect("leer cockpit_server.rs");
    let agent_line = src
        .lines()
        .find(|l| l.contains("run_seere_v11"))
        .expect("llamada a run_seere_v11");
    println!("[S09] cockpit: {}", agent_line.trim());
    assert!(
        agent_line.contains("pol.max_turns"),
        "[S09] DEFECTO: /api/llm_agent ignora pol.max_turns (turnos fijos): {}",
        agent_line.trim()
    );

    let src2 = std::fs::read_to_string("src/providers.rs").expect("leer providers.rs");
    for line2 in src2
        .lines()
        .filter(|l| l.contains("run_seere_v11"))
        // MEDIDO (2026-11-28): los COMENTARIOS que explican el v6
        // ("`execute_chat` llamaba a `seere_v11::run_seere_v11` in situ")
        // mencionan la llamada pero NO son la llamada => el guard fallaba
        // sin que hubiera defecto (falso positivo).  Sólo cuentan las
        // lineas de CODIGO: mismo criterio que `declared_modules.rs`.
        .filter(|l| !l.trim_start().starts_with("//"))
    {
        println!("[S09] providers: {}", line2.trim());
        assert!(
            line2.contains("pol.max_turns"),
            "[S09] DEFECTO: el fallback local ignora pol.max_turns: {}",
            line2.trim()
        );
    }

    // El presupuesto por turno del agente no puede superar el de la política
    // más restrictiva (critical = 256).
    assert!(
        S::FINAL_MAX_TOK <= 256,
        "[S09] DEFECTO: el presupuesto por turno ({}) excede el de la política crítica (256)",
        S::FINAL_MAX_TOK
    );
}

#[test]
fn s09b_el_presupuesto_por_turno_se_respeta_en_cada_peticion() {
    let _g = serial();
    let (port, hits, maxtok) = spawn_fake_llama("R: listo");
    std::env::set_var("SEERE_LLM", format!("http://127.0.0.1:{}", port));
    let cfg = S::SeereConfig {
        max_turns: 2,
        allow_tools: true,
        allow_exec: false,
        allow_external: false,
        ..Default::default()
    };
    let r = S::run_seere("dime hola", &cfg);
    let reqs = maxtok.lock().unwrap().clone();
    println!("[S09b] hits = {}", hits.load(Ordering::SeqCst));
    println!("[S09b] max_tokens pedidos = {:?}", reqs);
    println!("[S09b] answer = {}", r["answer"]);
    std::env::remove_var("SEERE_LLM");

    assert!(!reqs.is_empty(), "[S09b] no hubo peticiones al LLM");
    for mt in &reqs {
        assert!(
            *mt <= S::FINAL_MAX_TOK,
            "[S09b] DEFECTO: se pidieron {} tokens por turno (máximo {})",
            mt,
            S::FINAL_MAX_TOK
        );
    }
}

// ========================================================================= //
// S10 (MEDIA) — DELEGATE_LLM no puede cablearse a un modelo RETIRADO.
// ========================================================================= //

#[test]
fn s10_delegate_llm_no_usa_un_modelo_retirado() {
    assert!(
        provider_catalog::is_retired("minimax-m2.7-fast").is_some(),
        "[S10] el catálogo debería marcar minimax-m2.7-fast como retirado"
    );
    let src = std::fs::read_to_string("src/seere_v11.rs").expect("leer seere_v11.rs");
    assert!(
        !src.contains("minimax-m2.7-fast"),
        "[S10] DEFECTO: delegate_llm() sigue cableado a un modelo RETIRADO"
    );
    let line = src
        .lines()
        .find(|l| l.contains("minimax-m3.1-flash"))
        .expect("id del modelo delegado en seere_v11.rs");
    println!("[S10] seere_v11.rs: {}", line.trim());
    assert!(
        provider_catalog::is_retired("minimax-m3.1-flash").is_none(),
        "[S10] el modelo de delegación debe ser un modelo VIGENTE"
    );
}

#[test]
fn s10b_control_el_id_retirado_no_existe_en_la_boveda_por_defecto() {
    let v = megadaemon_core::providers::load_vault();
    let has = v.providers.iter().any(|p| p.id == "minimax-m2.7-fast");
    println!(
        "[S10b] proveedores en la bóveda: {} | ¿minimax-m2.7-fast? {}",
        v.providers.len(),
        has
    );
    assert!(
        !has,
        "[S10b] el id retirado no debe estar en la bóveda por defecto"
    );
}

// ========================================================================= //
// S11 (MEDIA) — Gramática y parser coinciden: el parser no despacha formas
//              que la gramática prohíbe ("A: NONE", "A: $" sin argumento).
// ========================================================================= //

#[test]
fn s11_el_parser_no_despacha_formas_invalidas_para_la_gramatica() {
    let g = S::build_act_grammar();
    println!(
        "[S11] gramática (extracto):\n{}",
        g.lines().take(6).collect::<Vec<_>>().join("\n")
    );

    let p = S::parse_model_output("A: NONE");
    println!(
        "[S11] parse_model_output(\"A: NONE\") -> out_type={} actions={:?}",
        p.out_type, p.actions
    );
    let p2 = S::parse_model_output("A: $");
    println!(
        "[S11] parse_model_output(\"A: $\") -> out_type={} actions={:?}",
        p2.out_type, p2.actions
    );

    assert!(
        p.actions.is_empty(),
        "[S11] DEFECTO: 'A: NONE' produce una acción ejecutable: {:?}",
        p.actions
    );
    assert!(
        p2.actions.is_empty(),
        "[S11] DEFECTO: 'A: $' (shell vacío) produce una acción: {:?}",
        p2.actions
    );
}

#[test]
fn s11b_control_las_acciones_validas_no_se_tocan() {
    let p = S::parse_model_output("A: $ ls /data/local/tmp");
    println!("[S11b] actions = {:?}", p.actions);
    assert_eq!(p.actions.len(), 1);
    assert_eq!(p.actions[0].0, "ROOT_EXEC");
    assert_eq!(p.actions[0].1, "ls /data/local/tmp");

    let q = S::parse_model_output("A: S i");
    println!("[S11b] SHM_INFO = {:?}", q.actions);
    assert_eq!(q.actions[0].0, "SHM_INFO");
    assert_eq!(q.actions[0].1, "");
}

// ========================================================================= //
// S12 (MEDIA) — El fallback NO filtra el contenido de la observación en el
//              campo "answer" (que es lo que consume la UI).
// ========================================================================= //

#[test]
fn s12_el_fallback_no_filtra_la_observacion_como_answer() {
    let _g = serial();
    let (port, hits, _mt) = spawn_fake_llama("A: $ echo OBS_LEAK_MARKER");
    std::env::set_var("SEERE_LLM", format!("http://127.0.0.1:{}", port));
    let cfg = S::SeereConfig {
        max_turns: 1,
        allow_tools: true,
        allow_exec: true,
        allow_external: false,
        ..Default::default()
    };
    let r = S::run_seere("dame la salida", &cfg);
    let ans = r["answer"].as_str().unwrap_or("").to_string();
    println!("[S12] hits   = {}", hits.load(Ordering::SeqCst));
    println!("[S12] answer = {:?}", ans);
    println!("[S12] answer_is_observation = {}", r["answer_is_observation"]);
    std::env::remove_var("SEERE_LLM");
    assert!(
        !ans.contains("OBS_LEAK_MARKER"),
        "[S12] DEFECTO: la observación cruda se entrega como 'answer': {:?}",
        ans
    );
}

// ========================================================================= //
// S13 (MEDIA) — Truncado de texto sin partir caracteres multibyte.
// ========================================================================= //

#[test]
fn s13_el_truncado_de_texto_no_panea() {
    let mut s = String::with_capacity(70000);
    for _ in 0..65535 {
        s.push('A');
    }
    s.push('\u{00E9}'); // 2 bytes: ocupa 65535..65537
    s.push('B');
    let t = megadaemon_core::util_text::truncate_bytes(&s, 65536);
    println!(
        "[S13] truncate_bytes -> len={} char_boundary={}",
        t.len(),
        s.is_char_boundary(t.len())
    );
    assert!(t.len() <= 65536);
    assert!(s.is_char_boundary(t.len()), "[S13] el corte parte un carácter");
    let c = megadaemon_core::util_text::truncate_chars(&s, 65536);
    assert!(c.chars().count() <= 65536);
}

#[test]
fn s13b_no_queda_truncado_por_bytes_sobre_str() {
    let main_src = std::fs::read_to_string("src/main.rs").expect("leer main.rs");
    let cock_src = std::fs::read_to_string("src/cockpit_server.rs").expect("leer cockpit_server.rs");
    let p1 = main_src.contains("s[..s.len().min(65536)]");
    let p2 = cock_src.contains("s[..s.len().min(65536)]");
    println!("[S13b] main.rs contiene el patrón: {}", p1);
    println!("[S13b] cockpit_server.rs contiene el patrón: {}", p2);
    assert!(
        !(p1 || p2),
        "[S13b] DEFECTO: truncado por bytes sobre un &str (panic si el corte cae dentro de un carácter)"
    );
}

// ========================================================================= //
// S14 (MEDIA) — La tarea delegada NO se pierde: o se delega, o se encola
//              para ejecución offline.  Nunca se devuelve el aviso de
//              "modelo retirado" como si fuera la respuesta del modelo.
// ========================================================================= //

#[test]
fn s14_la_tarea_delegada_no_se_pierde() {
    let _g = serial();

    // La bóveda y la cola de tickets viven en el directorio de estado
    // EFECTIVO (`MEGADAEMON_STATE_DIR`).  En el CI (y en un escritorio) la
    // ruta FIJA de la app en Android (`/data/user/0/com.agy/...`) no existe
    // y no se puede crear sin root => el test usa un directorio propio.
    let dir = std::env::temp_dir().join(format!("seere_s14_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let prev_state = std::env::var("MEGADAEMON_STATE_DIR").ok();
    std::env::set_var("MEGADAEMON_STATE_DIR", dir.to_string_lossy().to_string());

    // Bóveda VACÍA => no hay a quién delegar => la tarea DEBE encolarse.
    // El que escribe la bóveda es `providers::save_vault`, que CREA el
    // directorio padre ANTES de escribir.  MEDIDO 2026-11-23: el test
    // escribía a pelo con `fs::write` y el directorio no existía =>
    // "escribir bóveda vacía: No such file or directory" y el test
    // reventaba ANTES de comprobar nada.
    let vault_path = megadaemon_core::providers::providers_path();
    let vault = megadaemon_core::providers::ProviderVault {
        default_model: "local-seere".to_string(),
        providers: Vec::new(),
    };
    megadaemon_core::providers::save_vault(&vault).expect("escribir bóveda vacía");
    assert!(
        Path::new(&vault_path).parent().map(|p| p.is_dir()).unwrap_or(false),
        "[S14] DEFECTO: el escritor de la bóveda NO creó el directorio padre de {}",
        vault_path
    );

    let queue = format!("{}/delegated_tasks.json", S::state_dir());
    let _ = std::fs::remove_file(&queue);

    let a = S::Arsenal::new(false, false, true);
    let out = a.dispatch("DELEGATE_LLM", "S14_TASK_MARKER resume", false);
    let after = std::fs::read_to_string(&queue).unwrap_or_default();

    // Restaurar el entorno (el directorio de estado efectivo) y limpiar.
    match prev_state {
        Some(p) => std::env::set_var("MEGADAEMON_STATE_DIR", p),
        None => std::env::remove_var("MEGADAEMON_STATE_DIR"),
    }
    let _ = std::fs::remove_dir_all(&dir);

    println!("[S14] dispatch(DELEGATE_LLM) -> {}", out);
    println!("[S14] cola offline = {} bytes", after.len());

    assert!(
        !out.contains("MODELO RETIRADO"),
        "[S14] DEFECTO: delegate_llm() devuelve el aviso de 'modelo retirado' como respuesta: {}",
        out
    );
    assert!(
        out.starts_with("EXT_QUEUED") || out.starts_with("EXT_LLM"),
        "[S14] DEFECTO: la delegación debe devolverse o encolarse, no perderse: {}",
        out
    );
    assert!(
        after.contains("S14_TASK_MARKER"),
        "[S14] DEFECTO: la tarea delegada no quedó en la cola offline: {}",
        out
    );
}

#[test]
fn s14b_el_mcp_no_anuncia_modelos_retirados_como_default() {
    let src = std::fs::read_to_string("src/mcp_server.rs").expect("leer mcp_server.rs");
    for l in src.lines().filter(|l| l.contains("\"default\"")) {
        println!("[S14b] mcp_server.rs: {}", l.trim());
        let id = l
            .split("\"default\"")
            .nth(1)
            .unwrap_or("")
            .split('"')
            .nth(1)
            .unwrap_or("");
        assert!(
            provider_catalog::is_retired(id).is_none(),
            "[S14b] DEFECTO: el MCP anuncia como default un modelo RETIRADO: {}",
            id
        );
    }
}

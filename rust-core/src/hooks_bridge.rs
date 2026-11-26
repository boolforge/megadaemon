//! Puente **daemon → móvil** para la instrumentación por hooks.
//!
//! Expone la tool MCP `inject_hooks` con cuatro acciones:
//!
//! | acción     | qué hace                                                        |
//! |------------|-----------------------------------------------------------------|
//! | `estado`   | ¿hay payload? ¿hay inyector? ¿hay `frida-server`? ¿SELinux?      |
//! | `instalar` | mete el payload (y el inyector) en el móvil, listos para usar    |
//! | `inyectar` | adjunta al proceso indicado y devuelve el informe del inyector   |
//! | `leer`     | lee las últimas líneas del NDJSON de hooks                       |
//!
//! TODO lo que necesita root se hace con `root_exec` (que ya usa
//! `su -mm -c` ⇒ dominio `magisk`): el inyector **tiene** que correr como
//! root porque `ptrace` a una app de terceros lo exige.
//!
//! REGLAS del repo que se respetan aquí:
//! * nada de `unwrap()`: todo error es un JSON con `ok:false` y el motivo;
//! * los ficheros que se escriben en el móvil van con permisos 0600/0755
//!   explícitos, nunca con los que traiga el `umask`;
//! * **ningún secreto en los logs**: el NDJSON de hooks ya viene redactado
//!   por el payload y aquí sólo se copia.

use crate::android::{audit, fs_write, root_exec, sh_quote};
use serde_json::{json, Value};

/// Directorio de trabajo LEGADO en el móvil (VOLÁTIL: `/data/local/tmp`).
///
/// MEDIDO (2026-11-23): `/data/local/tmp` se borra al reiniciar Android ⇒ el
/// payload de hooks, el inyector y el NDJSON de eventos se perdían en cada
/// reinicio del móvil.  Ahora se usa el directorio **base de la instancia**
/// (en el móvil, `/data/adb/megadaemon`, que es persistente) y el state dir
/// canónico para los datos.
pub const DIR_REMOTO_LEGADO: &str = "/data/local/tmp/megadaemon";
/// Payload (la `.so` de hooks) LEGADO.
pub const PAYLOAD_REMOTO_LEGADO: &str = "/data/local/tmp/megadaemon/hooks-payload-arm64.so";
/// Inyector (`md-inject`) LEGADO.
pub const INYECTOR_REMOTO_LEGADO: &str = "/data/local/tmp/megadaemon/md-inject";
/// NDJSON de hooks LEGADO.
pub const NDJSON_REMOTO_LEGADO: &str = "/data/local/tmp/megadaemon/state/hooks.ndjson";

/// Directorio de trabajo en el móvil (persistente).
pub fn dir_remoto() -> String {
    crate::cockpit_server::instance_dir()
}

/// Payload (la `.so` de hooks) en su ruta de referencia.
pub fn payload_remoto() -> String {
    format!("{}/hooks-payload-arm64.so", dir_remoto())
}

/// Inyector (`md-inject`) en el móvil.
pub fn inyector_remoto() -> String {
    format!("{}/md-inject", dir_remoto())
}

/// NDJSON de hooks (ruta de referencia; el payload puede usar otra).
pub fn ndjson_remoto() -> String {
    format!("{}/hooks.ndjson", crate::state_dir::state_dir())
}
/// Paquete por defecto (el que se busca si no se dice otro).
pub const PAQUETE_POR_DEFECTO: &str = "com.google.antigravity";
/// Permisos del payload (el proceso de la app tiene que poder LEERLO).
const PERM_PAYLOAD: &str = "755";
/// Permisos del inyector (lo ejecuta root).
const PERM_INYECTOR: &str = "755";

/// Definición MCP de la tool.
pub fn tool_definition() -> Value {
    json!({
        "name": "inject_hooks",
        "description": "Instrumentación real de un proceso Android por ptrace: instala el payload de hooks (SQLite, red y ficheros) en el móvil, lo inyecta en el proceso indicado y lee el NDJSON que genera. El inyector se ejecuta como root (su -mm -c): ptrace a una app de terceros lo exige.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["estado", "instalar", "inyectar", "leer"]
                },
                "package": {
                    "type": "string",
                    "description": "Paquete de la app objetivo (por defecto com.google.antigravity)."
                },
                "pid": {
                    "type": "integer",
                    "description": "PID del proceso a instrumentar (si no, se busca por el nombre del paquete)."
                },
                "lib_path": {
                    "type": "string",
                    "description": "Ruta LOCAL del payload (.so) para la acción instalar. Si falta, se busca en el directorio de estado del daemon."
                },
                "bin_path": {
                    "type": "string",
                    "description": "Ruta LOCAL del inyector (md-inject) para la acción instalar."
                },
                "via": {
                    "type": "string",
                    "enum": ["ptrace", "mem"],
                    "description": "Vía de escritura en la memoria del objetivo. ptrace = principal; mem = /proc/<pid>/mem."
                },
                "lines": {
                    "type": "integer",
                    "default": 100,
                    "description": "Cuántas líneas del NDJSON leer (acción leer)."
                }
            },
            "required": ["action"]
        }
    })
}

/// Despacha la tool.
pub fn dispatch(args: &Value) -> Value {
    let s = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    let i = |k: &str| args.get(k).and_then(|v| v.as_i64()).unwrap_or(0);
    match s("action").as_str() {
        "estado" | "status" => estado(&s("package")),
        "instalar" | "install" => instalar(&s("package"), &s("lib_path"), &s("bin_path")),
        "inyectar" | "inject" => inyectar(&s("package"), i("pid"), &s("via")),
        "leer" | "read" => leer(&s("package"), i("lines").max(1) as usize),
        "" => error("falta_action", "hace falta la acción (estado|instalar|inyectar|leer)"),
        otra => error(
            "accion_desconocida",
            &format!("acción no conocida: {}", otra),
        ),
    }
}

/// Error estructurado.
fn error(codigo: &str, motivo: &str) -> Value {
    json!({"ok": false, "error": codigo, "hint": motivo})
}

/// Descubre el paquete de la app objetivo en el móvil.
fn descubrir_paquete(paquete: &str) -> String {
    if !paquete.is_empty() {
        return paquete.to_string();
    }
    // MEDIDO (2026-11-26, móvil): la app de Antigravity se llama `com.agy`
    // (NO `com.google.antigravity`) ⇒ el grep por "antigravity" no
    // encontraba nada y todo iba al paquete por defecto (que no existe en
    // el móvil) ⇒ el payload se instalaba en un directorio inexistente.
    let r = root_exec(
        "pm list packages 2>/dev/null | grep -i -E 'antigravity|^package:com\\.agy$' | head -1 | cut -d: -f2",
        20_000,
        1024,
    );
    let p = r.stdout.trim().to_string();
    if p.is_empty() {
        PAQUETE_POR_DEFECTO.to_string()
    } else {
        p
    }
}

/// `estado`: qué hay puesto en el móvil.
fn estado(paquete: &str) -> Value {
    let script = format!(
        r#"echo '--- PAQUETES ---'
pm list packages 2>/dev/null | grep -i antigravity | head -5
echo '--- SELINUX ---'
getenforce 2>&1
echo '--- PAYLOAD-REFERENCIA ---'
ls -lZ {p} 2>&1 | head -2
echo '--- INYECTOR ---'
ls -lZ {i} 2>&1 | head -2
echo '--- FRIDA-SERVER ---'
ls -l /data/local/tmp/frida-server* 2>&1 | head -2
command -v frida-server 2>&1 | head -1
echo '--- NDJSON ---'
ls -lZ {n} 2>&1 | head -2
echo '--- PROCESOS ---'
ps -A -o PID,NAME 2>/dev/null | grep -i antigravity | head -5
"#,
        p = sh_quote(&payload_remoto()),
        i = sh_quote(&inyector_remoto()),
        n = sh_quote(&ndjson_remoto()),
    );
    let r = root_exec(&script, 40_000, 32 * 1024);
    let salida = r.combined();
    let tiene_payload = salida.contains("hooks-payload-arm64.so")
        && !salida.contains("No such file")
        && !salida.contains("Permission denied");
    let tiene_inyector = salida.contains("md-inject")
        && !salida.contains("No such file")
        && !salida.contains("Permission denied");
    let tiene_frida = salida.contains("frida-server")
        && !salida.contains("No such file")
        && !salida.contains("not found");
    json!({
        "ok": true,
        "paquete": descubrir_paquete(paquete),
        "payload_presente": tiene_payload,
        "inyector_presente": tiene_inyector,
        "frida_server_presente": tiene_frida,
        "payload_esperado": payload_remoto(),
        "inyector_esperado": inyector_remoto(),
        "ndjson_esperado": ndjson_remoto(),
        "informe": salida,
    })
}

/// `instalar`: deja el payload (y el inyector) listos en el móvil.
fn instalar(paquete: &str, lib_path: &str, bin_path: &str) -> Value {
    let pkg = descubrir_paquete(paquete);

    // 1) El payload: tiene que estar en el móvil en un sitio **legible por el
    //    proceso de la app** (el directorio privado de la app: etiqueta
    //    `app_data_file`; en `/data/local/tmp` la etiqueta es
    //    `shell_data_file` y `untrusted_app` no puede leerla).
    let destino_privado = format!("/data/data/{}/files/megadaemon/hooks-payload-arm64.so", pkg);
    let mut pasos: Vec<Value> = Vec::new();

    match lib_local(lib_path) {
        Ok((base64, origen)) => {
            let mut sub = Vec::new();
            for destino in [destino_privado.clone(), payload_remoto()] {
                let r = fs_write(&destino, &base64, true, PERM_PAYLOAD, false);
                let ok = r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
                sub.push(json!({"destino": destino, "ok": ok, "detalle": r}));
            }
            // Etiqueta SELinux y propietario: el proceso de la app tiene que
            // poder LEER el fichero.
            let fix = format!(
                "restorecon {d} 2>&1; chown -R {uid}:{gid} {dir} 2>&1; chmod {perm} {d} 2>&1; ls -lZ {d} 2>&1",
                d = sh_quote(&destino_privado),
                uid = sh_quote(&uid_de(&pkg)),
                gid = sh_quote(&gid_de(&pkg)),
                dir = sh_quote(&format!("/data/data/{}/files/megadaemon", pkg)),
                perm = PERM_PAYLOAD,
            );
            let r = root_exec(&fix, 30_000, 4096);
            pasos.push(json!({
                "que": "payload",
                "origen_local": origen,
                "escrituras": sub,
                "selinux": r.combined(),
            }));
        }
        Err(e) => pasos.push(json!({"que": "payload", "error": e})),
    }

    // 2) El inyector (`md-inject`): lo ejecuta root.
    //    MEDIDO (2026-11-26): si no está local, se DESCARGA del release
    //    (ver [`asegurar_inyector_local`]): sin inyector no hay carga.
    match asegurar_inyector_local(bin_path) {
        Ok((base64, origen)) => {
            let r = fs_write(&inyector_remoto(), &base64, true, PERM_INYECTOR, false);
            let fix = format!(
                "restorecon {i} 2>&1; chmod {perm} {i} 2>&1; ls -lZ {i} 2>&1",
                i = sh_quote(&inyector_remoto()),
                perm = PERM_INYECTOR,
            );
            let r2 = root_exec(&fix, 30_000, 4096);
            pasos.push(json!({
                "que": "inyector",
                "origen_local": origen,
                "escritura": r,
                "permisos": r2.combined(),
            }));
        }
        Err(e) => pasos.push(json!({"que": "inyector", "error": e})),
    }

    audit("inject_hooks.instalar", &json!({"paquete": pkg}));
    json!({
        "ok": true,
        "paquete": pkg,
        "payload_destino": destino_privado,
        "pasos": pasos,
        "siguiente": "llama a la acción 'inyectar' con el pid o el paquete",
    })
}

/// `inyectar`: el inyector hace su trabajo y devuelve su informe.
fn inyectar(paquete: &str, pid: i64, via: &str) -> Value {
    let pkg = descubrir_paquete(paquete);

    // ¿Está el inyector en el móvil?
    let check = root_exec(
        &format!("ls -l {i} 2>&1", i = sh_quote(&inyector_remoto())),
        20_000,
        2048,
    );
    if !check.stdout.contains("md-inject") {
        // MEDIDO (2026-11-26): el inyector no venía en el release; ahora se
        // descarga solo (con sha256) y se deja en su sitio.  Si tampoco se
        // puede, el error dice exactamente qué falta.
        let _ = instalar(&pkg, "", "");
        let check2 = root_exec(
            &format!("ls -l {i} 2>&1", i = sh_quote(&inyector_remoto())),
            20_000,
            2048,
        );
        if !check2.stdout.contains("md-inject") {
            return error(
                "falta_inyector",
                &format!(
                    "no hay inyector en {} (instálalo con la acción 'instalar'): {}",
                    inyector_remoto(),
                    check2.combined().trim()
                ),
            );
        }
    }

    // ¿Está el payload?
    let payload_privado = format!("/data/data/{}/files/megadaemon/hooks-payload-arm64.so", pkg);
    let cual = root_exec(
        &format!(
            "for f in {a} {b}; do [ -f \"$f\" ] && echo $f; done",
            a = sh_quote(&payload_privado),
            b = sh_quote(&payload_remoto()),
        ),
        20_000,
        2048,
    );
    let ruta_payload = match cual.stdout.lines().find(|l| l.trim().ends_with(".so")) {
        Some(l) => l.trim().to_string(),
        None => {
            return error(
                "falta_payload",
                &format!(
                    "no hay payload en {} ni en {} (instálalo con la acción 'instalar')",
                    payload_privado, payload_remoto()
                ),
            )
        }
    };

    // El PID: explícito o buscado por el nombre del proceso.
    let pid_real = if pid > 0 {
        pid
    } else {
        // MEDIDO (2026-11-26, móvil): el LS de Antigravity NO se llama como
        // el paquete: el proceso es `libloader.so` (que carga
        // `liblanguage_server.so`).  Buscar por el nombre del paquete
        // inyectaba en el proceso de la APP, no en el LS ⇒ las sondas
        // (direcciones del pclntab del LS) no enganchaban nada.
        let r_ls = root_exec(
            "ps -A -o PID,ARGS 2>/dev/null | grep -a liblanguage_server | grep -v grep | head -1 | awk '{print $1}'",
            20_000,
            1024,
        );
        let r_app = root_exec(
            &format!(
                "ps -A -o PID,NAME 2>/dev/null | grep -i {p} | head -1 | awk '{{print $1}}'",
                p = sh_quote(&pkg)
            ),
            20_000,
            1024,
        );
        let primero = |s: &str| s.trim().split_whitespace().next().and_then(|x| x.parse::<i64>().ok());
        match primero(&r_ls.stdout).or_else(|| primero(&r_app.stdout)) {
            Some(p) => p,
            None => {
                return error(
                    "proceso_no_encontrado",
                    &format!(
                        "no hay ningún proceso del LS (liblanguage_server) ni de '{}' en el móvil",
                        pkg
                    ),
                )
            }
        }
    };

    let via_arg = match via.trim() {
        "" | "ptrace" => "--mem=ptrace".to_string(),
        "mem" => "--mem=mem".to_string(),
        otra => {
            return error(
                "via_desconocida",
                &format!("vía no conocida: {} (ptrace|mem)", otra),
            )
        }
    };

    let cmd = format!(
        "{i} inject {pid} {lib} {via} 2>&1",
        i = sh_quote(&inyector_remoto()),
        pid = pid_real,
        lib = sh_quote(&ruta_payload),
        via = via_arg,
    );
    let r = root_exec(&cmd, 60_000, 64 * 1024);
    audit(
        "inject_hooks.inyectar",
        &json!({"pid": pid_real, "via": via_arg, "rc": r.code}),
    );
    // El inyector escribe JSON: se intenta parsear (el `2>&1` puede meter ruido).
    let texto = r.stdout.trim();
    let informe: Value = match serde_json::from_str::<Value>(texto) {
        Ok(v) => v,
        Err(_) => json!({"ok": false, "error": "salida_no_json", "salida": texto}),
    };
    json!({
        "ok": informe.get("ok").and_then(|v| v.as_bool()).unwrap_or(false),
        "pid": pid_real,
        "payload": ruta_payload,
        "informe": informe,
        "stderr": r.stderr,
    })
}

/// `leer`: las últimas líneas del NDJSON de hooks.
fn leer(paquete: &str, lineas: usize) -> Value {
    let pkg = descubrir_paquete(paquete);
    // MEDIDO (2026-11-26, móvil): el PAYLOAD escribe el NDJSON en una de sus
    // candidatas (`/data/local/tmp/megadaemon/state/hooks.ndjson`,
    // `…/hooks-{uid}.ndjson`, `/data/user/0/<pkg>/cache/megadaemon-hooks.ndjson`…)
    // y el LECTOR miraba otras tres ⇒ no se solapaban NINGUNA ⇒ aunque la
    // inyección funcionase, `leer` decía "sin_ndjson".  Ahora se buscan las
    // MISMAS candidatas que usa el payload (`hooks_payload::candidatas_ndjson`
    // es la lista de referencia).
    let uid = uid_de(&pkg);
    let candidatos = vec![
        ndjson_remoto(),
        "/data/local/tmp/megadaemon/state/hooks.ndjson".to_string(),
        format!("/data/local/tmp/megadaemon/state/hooks-{uid}.ndjson"),
        format!("/data/local/tmp/hooks-{uid}.ndjson"),
        format!("/data/user/0/{pkg}/cache/megadaemon-hooks.ndjson"),
        format!("/data/data/{pkg}/cache/megadaemon-hooks.ndjson"),
        format!("/data/data/{pkg}/files/megadaemon/hooks.ndjson"),
        format!("/data/user/0/{pkg}/files/megadaemon/hooks.ndjson"),
    ];
    let mut lista = String::new();
    let mut encontrado: Option<String> = None;
    for c in &candidatos {
        let r = root_exec(
            &format!(
                "if [ -f {f} ]; then tail -n {n} {f}; fi",
                f = sh_quote(c),
                n = lineas
            ),
            30_000,
            512 * 1024,
        );
        if !r.stdout.trim().is_empty() {
            encontrado = Some(c.clone());
            lista.push_str(&r.stdout);
            break;
        }
    }
    match encontrado {
        Some(ruta) => json!({
            "ok": true,
            "fichero": ruta,
            "lineas": lineas,
            "ndjson": lista,
        }),
        None => json!({
            "ok": false,
            "error": "sin_ndjson",
            "hint": "no hay NDJSON de hooks: el payload no se ha inyectado o el proceso no ha escrito nada",
            "candidatos": candidatos,
        }),
    }
}

/// Rutas locales donde puede estar un artefacto (payload o inyector).
///
/// Se acepta un camino explícito; si no, se busca en el directorio de estado
/// del daemon con **todos** los nombres posibles: el release los publica con
/// el sufijo del asset (`hooks-payload-arm64`, `md-inject-arm64`) y el
/// operador puede haberlos renombrado.
fn rutas_lib_local(lib_path: &str, nombres: &[&str]) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if !lib_path.trim().is_empty() {
        out.push(std::path::PathBuf::from(lib_path.trim()));
        return out;
    }
    let dir = crate::state_dir::state_dir();
    for n in nombres {
        out.push(std::path::Path::new(&dir).join(n));
        out.push(std::path::Path::new(&dir).join("hooks").join(n));
    }
    out
}

/// Carga un fichero local y lo devuelve en base64.
///
/// `extra` son rutas ADICIONALES donde se busca ese binario concreto
/// (NO se mezclan entre binarios: el payload y el inyector tienen nombres
/// distintos y uno no puede acabar instalado en el sitio del otro).
fn bin_local(
    ruta: &str,
    nombres: &[&str],
    extra: &[std::path::PathBuf],
) -> Result<(String, String), String> {
    use base64ct::Encoding;
    let mut candidatas = rutas_lib_local(ruta, nombres);
    candidatas.extend(extra.iter().cloned());
    for candidata in candidatas {
        if let Ok(bytes) = std::fs::read(&candidata) {
            let b64 = base64ct::Base64::encode_string(&bytes);
            return Ok((b64, candidata.to_string_lossy().to_string()));
        }
    }
    Err(format!(
        "no se encuentra {} localmente (indica 'lib_path' o 'bin_path')",
        nombres.join(" / ")
    ))
}

/// El payload (`.so` de hooks).
///
/// MEDIDO (2026-11-26, móvil): el payload NO estaba en el `state_dir` (el
/// daemon lo instala EMBEBIDO en
/// `/data/local/tmp/megadaemon/hooks-payload.so`, ver
/// `hooks_payload::RUTA_PAYLOAD`) ⇒ la búsqueda fallaba con "no se
/// encuentra hooks-payload-arm64.so localmente" y la instrumentación no se
/// podía instalar.  Ahora se busca también ahí.
fn lib_local(ruta: &str) -> Result<(String, String), String> {
    bin_local(
        ruta,
        &["hooks-payload-arm64.so", "hooks-payload-arm64"],
        &[
            std::path::PathBuf::from(crate::hooks_payload::RUTA_PAYLOAD),
            std::path::PathBuf::from(payload_remoto()),
        ],
    )
}

/// El inyector (`md-inject`).
fn injector_local(ruta: &str) -> Result<(String, String), String> {
    bin_local(
        ruta,
        &["md-inject-arm64", "md-inject"],
        &[
            std::path::PathBuf::from(dir_remoto()).join("md-inject-arm64"),
            std::path::PathBuf::from(inyector_remoto()),
        ],
    )
}

/// Asegura que haya un inyector local, **descargándolo del release** si
/// falta.
///
/// MEDIDO (2026-11-26, móvil): el inyector NO venía en el release (el CI
/// publicaba `channel-publish`, `md-clipboard`… pero no `md-inject`) ⇒ en
/// el móvil no había ningún cargador: ni cliente `frida`, ni `frida-server`
/// arrancado, ni módulo Zygisk ⇒ **el payload de hooks no se podía
/// inyectar** y no había ni una línea de datos en vivo.
///
/// El asset `md-inject-arm64` se verifica con su `.sha256` antes de
/// ejecutarlo (es un binario que se va a meter en un proceso ajeno).
fn asegurar_inyector_local(ruta: &str) -> Result<(String, String), String> {
    let release = crate::self_update::url_release();
    let url = format!("{release}/md-inject-arm64");
    let url_sha = format!("{url}.sha256");
    let texto_sha = crate::self_update::descargar_texto(&url_sha)
        .map_err(|e| format!("descargar {url_sha}: {e}"))?;
    let esperado = crate::self_update::parsear_sha256(&texto_sha)
        .ok_or_else(|| format!("{url_sha} no trae un sha256 legible"))?;
    // MEDIDO (2026-11-26, móvil): el inyector se descargaba UNA vez y ya no
    // se volvía a tocar ⇒ se quedaba con los bugs de aquel release (el de
    // leer `/proc/<pid>/maps` como texto, p. ej.) aunque el daemon ya fuese
    // nuevo.  Ahora se compara su sha256 con el del release ACTUAL y, si no
    // coincide, se vuelve a descargar.
    if let Ok((b64, origen)) = injector_local(ruta) {
        if let Ok(bytes) = std::fs::read(&origen) {
            let sha_local = sha256_hex(&bytes);
            if sha_local.eq_ignore_ascii_case(&esperado) {
                return Ok((b64, origen));
            }
        }
    }
    let bytes = crate::self_update::descargar_bytes(&url)
        .map_err(|e| format!("descargar {url}: {e}"))?;
    let real = sha256_hex(&bytes);
    if !real.eq_ignore_ascii_case(&esperado) {
        return Err(format!(
            "sha256 del inyector NO coincide: esperado {esperado}, real {real}"
        ));
    }
    let destino = std::path::PathBuf::from(dir_remoto()).join("md-inject-arm64");
    if let Some(dir) = destino.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::fs::write(&destino, &bytes).map_err(|e| format!("{}: {e}", destino.display()))?;
    set_modo(&destino, 0o700);
    use base64ct::Encoding;
    Ok((
        base64ct::Base64::encode_string(&bytes),
        destino.to_string_lossy().to_string(),
    ))
}

/// sha256 en hexadecimal de un buffer.
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    let mut h = sha2::Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Fija los permisos de un fichero (best effort).
fn set_modo(ruta: &std::path::Path, modo: u32) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(ruta, std::fs::Permissions::from_mode(modo));
    }
    #[cfg(not(unix))]
    let _ = (ruta, modo);
}

/// UID del paquete (para que el fichero quede legible por la app).
fn uid_de(paquete: &str) -> String {
    let r = root_exec(
        &format!("stat -c %u {p} 2>/dev/null", p = sh_quote(&format!("/data/data/{}", paquete))),
        20_000,
        128,
    );
    let v = r.stdout.trim().to_string();
    if v.is_empty() {
        "0".to_string()
    } else {
        v
    }
}

/// GID del paquete.
fn gid_de(paquete: &str) -> String {
    let r = root_exec(
        &format!("stat -c %g {p} 2>/dev/null", p = sh_quote(&format!("/data/data/{}", paquete))),
        20_000,
        128,
    );
    let v = r.stdout.trim().to_string();
    if v.is_empty() {
        "0".to_string()
    } else {
        v
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_tool_esta_declarada_con_sus_acciones() {
        let d = tool_definition();
        assert_eq!(d["name"], "inject_hooks");
        let acciones = d["inputSchema"]["properties"]["action"]["enum"]
            .as_array()
            .expect("la acción tiene que ser un enum");
        let lista: Vec<&str> = acciones.iter().filter_map(|v| v.as_str()).collect();
        for a in ["estado", "instalar", "inyectar", "leer"] {
            assert!(lista.contains(&a), "falta la acción {}", a);
        }
        assert_eq!(d["inputSchema"]["required"][0], "action");
    }

    #[test]
    fn una_accion_desconocida_da_error_estructurado() {
        let r = dispatch(&json!({"action": "inventada"}));
        assert_eq!(r["ok"], json!(false));
        assert_eq!(r["error"], "accion_desconocida");
        assert!(r["hint"].as_str().unwrap_or("").contains("inventada"));
    }

    #[test]
    fn sin_accion_da_error_estructurado() {
        let r = dispatch(&json!({}));
        assert_eq!(r["ok"], json!(false));
        assert_eq!(r["error"], "falta_action");
    }
}

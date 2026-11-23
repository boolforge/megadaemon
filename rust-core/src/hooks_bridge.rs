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

/// Directorio de trabajo en el móvil.
pub const DIR_REMOTO: &str = "/data/local/tmp/megadaemon";
/// Payload (la `.so` de hooks) en su ruta de referencia.
pub const PAYLOAD_REMOTO: &str = "/data/local/tmp/megadaemon/hooks-payload-arm64.so";
/// Inyector (`md-inject`) en el móvil.
pub const INYECTOR_REMOTO: &str = "/data/local/tmp/megadaemon/md-inject";
/// NDJSON de hooks (ruta de referencia; el payload puede usar otra).
pub const NDJSON_REMOTO: &str = "/data/local/tmp/megadaemon/state/hooks.ndjson";
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
    let r = root_exec(
        "pm list packages 2>/dev/null | grep -i antigravity | head -1 | cut -d: -f2",
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
        p = sh_quote(PAYLOAD_REMOTO),
        i = sh_quote(INYECTOR_REMOTO),
        n = sh_quote(NDJSON_REMOTO),
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
        "payload_esperado": PAYLOAD_REMOTO,
        "inyector_esperado": INYECTOR_REMOTO,
        "ndjson_esperado": NDJSON_REMOTO,
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

    match lib_local(lib_path, "hooks-payload-arm64.so") {
        Ok((base64, origen)) => {
            let mut sub = Vec::new();
            for destino in [destino_privado.clone(), PAYLOAD_REMOTO.to_string()] {
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
    match bin_local(bin_path, "md-inject") {
        Ok((base64, origen)) => {
            let r = fs_write(INYECTOR_REMOTO, &base64, true, PERM_INYECTOR, false);
            let fix = format!(
                "restorecon {i} 2>&1; chmod {perm} {i} 2>&1; ls -lZ {i} 2>&1",
                i = sh_quote(INYECTOR_REMOTO),
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
        &format!("ls -l {i} 2>&1", i = sh_quote(INYECTOR_REMOTO)),
        20_000,
        2048,
    );
    if !check.stdout.contains("md-inject") {
        return error(
            "falta_inyector",
            &format!(
                "no hay inyector en {} (instálalo con la acción 'instalar'): {}",
                INYECTOR_REMOTO,
                check.combined().trim()
            ),
        );
    }

    // ¿Está el payload?
    let payload_privado = format!("/data/data/{}/files/megadaemon/hooks-payload-arm64.so", pkg);
    let cual = root_exec(
        &format!(
            "for f in {a} {b}; do [ -f \"$f\" ] && echo $f; done",
            a = sh_quote(&payload_privado),
            b = sh_quote(PAYLOAD_REMOTO),
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
                    payload_privado, PAYLOAD_REMOTO
                ),
            )
        }
    };

    // El PID: explícito o buscado por el nombre del proceso.
    let pid_real = if pid > 0 {
        pid
    } else {
        let r = root_exec(
            &format!(
                "ps -A -o PID,NAME 2>/dev/null | grep -i {p} | head -1 | awk '{{print $1}}'",
                p = sh_quote(&pkg)
            ),
            20_000,
            1024,
        );
        match r.stdout.trim().split_whitespace().next().and_then(|s| s.parse::<i64>().ok()) {
            Some(p) => p,
            None => {
                return error(
                    "proceso_no_encontrado",
                    &format!("no hay ningún proceso de '{}' en el móvil", pkg),
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
        i = sh_quote(INYECTOR_REMOTO),
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
    let candidatos = [
        NDJSON_REMOTO.to_string(),
        format!("/data/data/{}/files/megadaemon/hooks.ndjson", pkg),
        format!("/data/user/0/{}/files/megadaemon/hooks.ndjson", pkg),
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

/// Rutas locales donde puede estar el payload.
fn rutas_lib_local(lib_path: &str, nombre: &str) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if !lib_path.trim().is_empty() {
        out.push(std::path::PathBuf::from(lib_path.trim()));
        return out;
    }
    let dir = crate::state_dir::state_dir();
    out.push(std::path::Path::new(&dir).join(nombre));
    out.push(std::path::Path::new(&dir).join("hooks").join(nombre));
    out
}

/// Carga un fichero local y lo devuelve en base64.
fn bin_local(ruta: &str, nombre: &str) -> Result<(String, String), String> {
    use base64ct::Encoding;
    for candidata in rutas_lib_local(ruta, nombre) {
        if let Ok(bytes) = std::fs::read(&candidata) {
            let b64 = base64ct::Base64::encode_string(&bytes);
            return Ok((b64, candidata.to_string_lossy().to_string()));
        }
    }
    Err(format!(
        "no se encuentra {} localmente (indica 'lib_path' o 'bin_path')",
        nombre
    ))
}

/// Alias de [`bin_local`] para el payload (mismo formato).
fn lib_local(ruta: &str, nombre: &str) -> Result<(String, String), String> {
    bin_local(ruta, nombre)
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

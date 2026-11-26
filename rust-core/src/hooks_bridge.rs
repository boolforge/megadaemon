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

/// Variante del payload según la **libc del proceso objetivo**.
///
/// MEDIDO (2026-11-26, móvil): el LS de Antigravity NO usa la libc de
/// Bionic: corre con un runtime de **GLIBC** propio
/// (`/data/user/0/com.agy/files/runtime/lib/libc.so.6`, confirmado en
/// `/proc/<pid>/maps`).  Un payload enlazado contra Bionic (NEEDED
/// `libc.so`, `libdl.so`, `liblog.so`) **no** se puede cargar ahí: el
/// enlazador de glibc no encuentra esas librerías.  Por eso hay dos
/// variantes del payload: una para Bionic (Android) y otra para glibc.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VariantePayload {
    /// Enlazada contra Bionic (`libc.so`, `libdl.so`, `liblog.so`).
    Bionic,
    /// Enlazada contra glibc (`libc.so.6`, `libm.so.6`).
    Glibc,
}

impl VariantePayload {
    /// Nombre del asset en el release.
    pub fn asset(&self) -> &'static str {
        match self {
            VariantePayload::Bionic => crate::hooks_payload::ASSET_PAYLOAD,
            VariantePayload::Glibc => crate::hooks_payload::ASSET_PAYLOAD_GLIBC,
        }
    }

    /// Nombre del fichero tal y como queda en el móvil.
    pub fn nombre(&self) -> &'static str {
        match self {
            VariantePayload::Bionic => "hooks-payload-arm64.so",
            VariantePayload::Glibc => "hooks-payload-linux-arm64.so",
        }
    }
}

/// Detecta qué libc usa el proceso `pid` leyendo su `/proc/<pid>/maps`.
///
/// Si el proceso tiene mapeada `libc.so.6` (o el linker de glibc) es un
/// proceso de **glibc** ⇒ payload de glibc.  Si no se puede leer el maps se
/// asume Bionic (el caso normal de una app de Android).
pub fn detectar_variante(pid: i64) -> VariantePayload {
    let mapas = crate::inject::leer_maps_en(std::path::Path::new("/proc"), pid as i32);
    let es_glibc = mapas.iter().any(|m| {
        let r = &m.ruta;
        r.contains("libc.so.6") || r.contains("ld-linux-aarch64.so") || r.contains("ld-linux-x86")
    });
    if es_glibc {
        VariantePayload::Glibc
    } else {
        VariantePayload::Bionic
    }
}

/// Ruta del payload de una variante en el directorio remoto del móvil.
pub fn payload_remoto_para(variante: VariantePayload) -> String {
    format!("{}/{}", dir_remoto(), variante.nombre())
}

/// Ruta del payload de una variante en el directorio privado de la app.
pub fn payload_privado_para(paquete: &str, variante: VariantePayload) -> String {
    format!(
        "/data/data/{}/files/megadaemon/{}",
        paquete,
        variante.nombre()
    )
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
ls -lZ {p2} 2>&1 | head -2
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
        p2 = sh_quote(&payload_remoto_para(VariantePayload::Glibc)),
        i = sh_quote(&inyector_remoto()),
        n = sh_quote(&ndjson_remoto()),
    );
    let r = root_exec(&script, 40_000, 32 * 1024);
    let salida = r.combined();
    let tiene_payload = salida.contains("hooks-payload-arm64.so")
        && !salida.contains("No such file")
        && !salida.contains("Permission denied");
    let tiene_payload_glibc = salida.contains("hooks-payload-linux-arm64.so")
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
        "payload_glibc_presente": tiene_payload_glibc,
        "inyector_presente": tiene_inyector,
        "frida_server_presente": tiene_frida,
        "payload_esperado": payload_remoto(),
        "payload_esperado_glibc": payload_remoto_para(VariantePayload::Glibc),
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
    //
    //    Se instalan LAS DOS VARIANTES (Bionic y glibc): el objetivo puede
    //    ser un proceso de cualquiera de los dos runtimes, y `inyectar`
    //    elige la que corresponda (ver [`detectar_variante`]).
    let mut pasos: Vec<Value> = Vec::new();

    for variante in [VariantePayload::Bionic, VariantePayload::Glibc] {
        let destino_privado = payload_privado_para(&pkg, variante);
        let origen = match variante {
            // La de Bionic va EMBEBIDA en el daemon: siempre está.
            VariantePayload::Bionic => lib_local(lib_path),
            // La de glibc se DOWNLOAD del release (no cabe en el binario
            // principal: son 3 MB de gum por variante).
            VariantePayload::Glibc => asegurar_payload_local(lib_path, variante),
        };
        match origen {
            Ok((base64, origen_local)) => {
                let mut sub = Vec::new();
                for destino in [destino_privado.clone(), payload_remoto_para(variante)] {
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
                    "variante": format!("{:?}", variante),
                    "origen_local": origen_local,
                    "escrituras": sub,
                    "selinux": r.combined(),
                }));
                // MEDIDO (2026-11-26, móvil): el payload de glibc tiene
                // NEEDED `libgcc_s.so.1`, que está en el runtime del LS pero
                // el proceso NO tiene DT_RPATH (su `--library-path` de glibc
                // no se traduce a un RPATH del ELF) ⇒ se copia junto al
                // payload, que la busca con `$ORIGIN`.
                if variante == VariantePayload::Glibc {
                    let runtime = format!("/data/user/0/{}/files/runtime/lib", pkg);
                    let dir = format!("/data/data/{}/files/megadaemon", pkg);
                    let deps = format!(
                        "for l in libgcc_s.so.1; do if [ -f {r}/$l ]; then cp -f {r}/$l {d}/ 2>&1; \
                         chmod 755 {d}/$l 2>&1; fi; done; ls -l {d} 2>&1 | head -8",
                        r = sh_quote(&runtime),
                        d = sh_quote(&dir),
                    );
                    let r = root_exec(&deps, 30_000, 4096);
                    pasos.push(json!({
                        "que": "dependencias del payload (glibc)",
                        "salida": r.combined(),
                    }));
                }
            }
            Err(e) => pasos.push(json!({"que": "payload", "variante": format!("{:?}", variante), "error": e})),
        }
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
        "payload_destino": payload_privado_para(&pkg, VariantePayload::Bionic),
        "payload_destino_glibc": payload_privado_para(&pkg, VariantePayload::Glibc),
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

    // El payload se busca DESPUÉS de saber el PID: la variante depende de la
    // libc del proceso objetivo (ver [`detectar_variante`]).

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

    // ¿Está el payload?  La VARIANTE depende de la libc del objetivo:
    // MEDIDO (2026-11-26, móvil): el LS de Antigravity usa un runtime de
    // **glibc** (`libc.so.6`), no la de Bionic ⇒ hay que cargarle el
    // payload de glibc, no el de Android.
    let variante = detectar_variante(pid_real);
    let ruta_payload = match payload_para_inyectar(&pkg, variante) {
        Ok(p) => p,
        Err(e) => return error("falta_payload", &e),
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
        "variante": format!("{:?}", variante),
        "informe": informe,
        "stderr": r.stderr,
    })
}

/// Deja el payload de una variante en el directorio **privado** de la app
/// (lo único que puede leer el proceso objetivo) y devuelve su ruta.
///
/// Si no está, se **descarga del release** (verificando su sha256) y se
/// escribe troceado (`fs_write` trocea: son 3 MB).
fn payload_para_inyectar(pkg: &str, variante: VariantePayload) -> Result<String, String> {
    let privado = payload_privado_para(pkg, variante);
    let remoto = payload_remoto_para(variante);
    let cual = root_exec(
        &format!(
            "for f in {a} {b}; do [ -f \"$f\" ] && echo $f; done",
            a = sh_quote(&privado),
            b = sh_quote(&remoto),
        ),
        20_000,
        2048,
    );
    if let Some(l) = cual.stdout.lines().map(|l| l.trim()).find(|l| l.ends_with(".so")) {
        if l == privado {
            return Ok(privado);
        }
    }
    // No está (o sólo está en la ruta de referencia, que la app no puede
    // leer): se consigue el binario y se escribe en la privada.
    let (base64, _origen) = asegurar_payload_local("", variante).map_err(|e| {
        format!(
            "el payload de {:?} no está en {} ni en {} y no se pudo descargar: {}",
            variante, privado, remoto, e
        )
    })?;
    let r = fs_write(&privado, &base64, true, PERM_PAYLOAD, false);
    if !r.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        return Err(format!("no se pudo escribir el payload en {}: {}", privado, r));
    }
    let fix = format!(
        "restorecon {d} 2>&1; chown -R {uid}:{gid} {dir} 2>&1; chmod {perm} {d} 2>&1",
        d = sh_quote(&privado),
        uid = sh_quote(&uid_de(pkg)),
        gid = sh_quote(&gid_de(pkg)),
        dir = sh_quote(&format!("/data/data/{}/files/megadaemon", pkg)),
        perm = PERM_PAYLOAD,
    );
    let _ = root_exec(&fix, 30_000, 2048);
    Ok(privado)
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

/// Asegura que haya un payload local de una variante, **descargándolo del
/// release** si falta (o si su sha256 no coincide con el del release actual).
///
/// MEDIDO (2026-11-26, móvil): el payload de **glibc** no cabe en el binario
/// principal (son 3 MB de gum) ⇒ se descarga del release, con su `.sha256`,
/// igual que se hace con el inyector.
fn asegurar_payload_local(
    ruta: &str,
    variante: VariantePayload,
) -> Result<(String, String), String> {
    // 1) Ruta explícita del operador: se respeta tal cual.
    if !ruta.trim().is_empty() {
        let p = std::path::PathBuf::from(ruta.trim());
        if let Ok(bytes) = std::fs::read(&p) {
            use base64ct::Encoding;
            return Ok((
                base64ct::Base64::encode_string(&bytes),
                p.to_string_lossy().to_string(),
            ));
        }
    }
    let release = crate::self_update::url_release();
    let url = format!("{}/{}", release, variante.asset());
    let url_sha = format!("{url}.sha256");
    let nombres = [variante.nombre(), variante.asset()];
    // MEDIDO (2026-11-26, móvil): el asset puede NO estar en el release que
    // ahora es el "latest" (los releases se solapan y el "latest" cambia
    // con cualquier push).  Si no se puede traer el `.sha256`, se usa el
    // payload que ya está en el móvil (validado cuando se descargó).
    let texto_sha = match crate::self_update::descargar_texto(&url_sha) {
        Ok(t) => t,
        Err(e) => {
            if let Ok((b64, origen)) = bin_local(
                "",
                &nombres,
                &[std::path::PathBuf::from(payload_remoto_para(variante))],
            ) {
                return Ok((b64, origen));
            }
            return Err(format!("descargar {url_sha}: {}", e));
        }
    };
    let esperado = crate::self_update::parsear_sha256(&texto_sha)
        .ok_or_else(|| format!("{url_sha} no trae un sha256 legible"))?;
    // 2) Local: sólo vale si su sha256 es el del release ACTUAL.
    if let Ok((b64, origen)) = bin_local(
        "",
        &nombres,
        &[std::path::PathBuf::from(payload_remoto_para(variante))],
    ) {
        if let Ok(bytes) = std::fs::read(&origen) {
            if sha256_hex(&bytes).eq_ignore_ascii_case(&esperado) {
                return Ok((b64, origen));
            }
        }
    }
    // 3) Descargar del release y verificar.
    let bytes = crate::self_update::descargar_bytes(&url)
        .map_err(|e| format!("descargar {url}: {e}"))?;
    let real = sha256_hex(&bytes);
    if !real.eq_ignore_ascii_case(&esperado) {
        return Err(format!(
            "sha256 del payload {} NO coincide: esperado {esperado}, real {real}",
            variante.asset()
        ));
    }
    let destino = std::path::PathBuf::from(dir_remoto()).join(variante.asset());
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

//! **"MegaDaemon en medio"**: intercepción de las llamadas del `.so` de
//! Antigravity (language server) desde el propio daemon.
//!
//! QUÉ ES ESTO (y por qué no es lo de las variables de entorno)
//! -----------------------------------------------------------
//! El language server de Antigravity es un binario **Go 1.28** *stripped*:
//! 558 símbolos dinámicos (sólo del runtime) ⇒ **no hay PLT/GOT que
//! parchear** y sus llamadas internas son `BL` directos.  Además Go hace
//! la E/S de red con syscalls crudas, así que los ganchos de libc
//! (`connect`, `sendto`) del payload **no disparan** contra él.
//!
//! ⇒ Para interceptar el `.so` de verdad hay que hookear **funciones Go**,
//! y para eso está el `pclntab`: el motor RE del repo (`go-retool`, ya
//! dependencia del daemon) recupera **161.682 nombres** con su dirección.
//!
//! MEDIDO (2026-11-23, `/workspace/agy-re/liblanguage_server.so`):
//!
//! * `pclntab` en `0x1449ff0`, 161.682 funciones, **161.682 nombres
//!   resueltos**; el módulo carga con `p_vaddr 0` ⇒ **dirección en runtime
//!   = bias + VA del ELF** (el bias lo da `inject::bias_de_carga` leyendo
//!   `/proc/<pid>/maps`).
//! * Las funciones que importan (extraídas del mapa de símbolos):
//!
//!   | función | VA | para qué |
//!   |---|---|---|
//!   | `codeassistclient.(*CodeAssistClient).GenerateContentFromAICode` | `0x66a2f78` | **la petición del modelo** |
//!   | `codeassistclient.(*CodeAssistClient).StreamGenerateContentFromAICode` | `0x66a27f8` | la petición en streaming |
//!   | `codeassistclient.getAICodeURL` | (ver plan) | **la URL** que se va a usar |
//!   | `gateway.ParseEnvOverrides` | `0x6d92c28` | `AGY_LLM_GATEWAY_*` |
//!   | `gateway.LoadEffectiveSettings` | `0x6d938e8` | `admin_settings.json` |
//!   | `gateway.(*AuthProvider).getLastDiscoveredCatalog` | `0x6d94448` | **el catálogo descubierto** (Tier 3) |
//!
//! CÓMO SE USA
//! ----------
//! 1. MegaDaemon resuelve los objetivos ([`resolver`]) y calcula la
//!    **dirección absoluta** en el proceso vivo ([`plan`]).
//! 2. Escribe el plan en [`ruta_plan`] (JSON).
//! 3. El **payload de frida-gum** (el que MegaDaemon ya instala y activa:
//!    `crates/hooks-payload`, bindings oficiales de Frida) lee ese plan y
//!    hace `Interceptor::attach` en cada dirección ⇒ **loguea masivamente y
//!    deja pasar el `.so` real** (Frida no altera el flujo si no se cambia
//!    el retorno).
//! 4. La **sustitución** se hace en dos sitios:
//!    * `tarea = "sustituir_url"` ⇒ el retorno de `getAICodeURL` (una
//!      `string` de Go: puntero + longitud) se sustituye por la URL del
//!      gateway ⇒ **el LS habla con MegaDaemon**, y MegaDaemon decide qué
//!      modelo real responde (MiniMax, por ejemplo).
//!    * el **cuerpo** de la petición se reescribe en el gateway (nivel
//!      HTTP), que es donde se pueden cambiar modelo,配额 y prompts.
//!
//! LO QUE SIGUE SIN VERIFICAR
//! --------------------------
//! Que el payload de frida-gum pueda **cambiar el valor de retorno** de una
//! función Go (el `string` de `getAICodeURL`): **NO VERIFICADO** (el móvil
//! llevaba días sin daemon).  El resto del mecanismo (resolución de
//! direcciones, plan, carga del payload) sí es medible sin dispositivo.

use serde_json::{json, Value};
use std::path::PathBuf;

/// Tareas que puede hacer un hook.
pub const OBSERVAR: &str = "observar";
/// El hook sustituye la URL que devuelve la función.
pub const SUSTITUIR_URL: &str = "sustituir_url";

/// Los objetivos de intercepción y su tarea.
///
/// Los nombres son los de Go (con el prefijo del paquete interno de Google)
/// ⇒ los que devuelve el `pclntab`.
pub const OBJETIVOS: &[(&str, &str)] = &[
    (
        "google3/third_party/jetski/language_server/code_assist_client/codeassistclient.(*CodeAssistClient).GenerateContentFromAICode",
        OBSERVAR,
    ),
    (
        "google3/third_party/jetski/language_server/code_assist_client/codeassistclient.(*CodeAssistClient).StreamGenerateContentFromAICode",
        OBSERVAR,
    ),
    (
        "google3/third_party/jetski/language_server/code_assist_client/codeassistclient.getAICodeURL",
        SUSTITUIR_URL,
    ),
    (
        "google3/third_party/jetski/gateway/gateway.ParseEnvOverrides",
        OBSERVAR,
    ),
    (
        "google3/third_party/jetski/gateway/gateway.LoadEffectiveSettings",
        OBSERVAR,
    ),
    (
        "google3/third_party/jetski/gateway/gateway.(*AuthProvider).getLastDiscoveredCatalog",
        OBSERVAR,
    ),
];

/// Un hook del plan: una función Go del `.so` + qué se le hace.
#[derive(Debug, Clone)]
pub struct Objetivo {
    /// Nombre Go de la función.
    pub simbolo: String,
    /// Qué hace el hook (`observar` / `sustituir_url`).
    pub tarea: &'static str,
    /// Dirección virtual en el ELF (sin bias).
    pub va_elf: u64,
    /// Tamaño de la función en bytes (0 si el pclntab no lo dice).
    pub bytes: u64,
    /// Dirección **absoluta** en el proceso (bias + `va_elf`).
    pub direccion: u64,
}

impl Objetivo {
    /// Nombre corto del hook (lo que ve el payload en el NDJSON).
    pub fn id(&self) -> &str {
        self.simbolo.rsplit('.').next().unwrap_or(&self.simbolo)
    }
}

/// Parsea una dirección hexadecimal del pclntab (`"0x66a2f78"`).
pub fn va_de_hex(s: &str) -> Option<u64> {
    let t = s.trim();
    let t = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X"))?;
    u64::from_str_radix(t, 16).ok()
}

/// Resuelve los objetivos en el `.so` (bytes del fichero) con el `pclntab`.
///
/// `base` es el **bias de carga** del módulo en el proceso vivo (0 si
/// todavía no se ha calculado: las direcciones salen en VA del ELF).
pub fn resolver(datos: &[u8], base: u64) -> Result<Vec<Objetivo>, String> {
    let (offset, _ver) = crate::re_engine::detectar_go_pclntab(datos)
        .ok_or_else(|| "el binario no tiene pclntab de Go (go1.18+)".to_string())?;
    let opt = go_retool::ParseOptions {
        file_offset: offset,
        ..Default::default()
    };
    let analisis = go_retool::parse_go118_plus(datos, opt).map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for (simbolo, tarea) in OBJETIVOS {
        let mut encontrado = None;
        for m in &analisis.modules {
            for f in &m.functions {
                let Some(nombre) = f.observed_name.as_deref() else {
                    continue;
                };
                if nombre != *simbolo {
                    continue;
                }
                let va = f.start_va.as_deref().and_then(va_de_hex).unwrap_or(0);
                let fin = f.end_va.as_deref().and_then(va_de_hex).unwrap_or(va);
                encontrado = Some(Objetivo {
                    simbolo: nombre.to_string(),
                    tarea,
                    va_elf: va,
                    bytes: fin.saturating_sub(va),
                    direccion: base.wrapping_add(va),
                });
                break;
            }
            if encontrado.is_some() {
                break;
            }
        }
        if let Some(o) = encontrado {
            out.push(o);
        }
    }
    Ok(out)
}

/// Construye el **plan de hooks** (JSON) que consume el payload de frida-gum.
pub fn plan(objetivos: &[Objetivo], base: u64) -> Value {
    json!({
        "version": 1,
        "modulo": "liblanguage_server.so",
        "base": base,
        "hooks": objetivos.iter().map(|o| json!({
            "id": o.simbolo.rsplit('.').next().unwrap_or(&o.simbolo),
            "simbolo": o.simbolo,
            "tarea": o.tarea,
            "va_elf": format!("0x{:x}", o.va_elf),
            "direccion": o.direccion,
            "bytes": o.bytes,
        })).collect::<Vec<_>>(),
    })
}

/// El plan en el **formato que lee el payload** (texto, tabulado).
///
/// Una línea por hook:
///
/// ```text
/// RUTA_DEL_MODULO<TAB>VA_ELF_HEX<TAB>ID<TAB>TAREA
/// ```
///
/// El payload (frida-gum) lo lee, busca el módulo en el proceso vivo y
/// engancha `base + va_elf`.  Texto y no JSON a propósito: el payload no
/// depende de serde.
///
/// ⚠️ `va_elf` es la dirección del **pclntab** (la del ELF), no la del
/// índice de Ghidra: aquel está 1 MiB por encima (medido, 93 % de las
/// funciones) ⇒ un plan hecho con Ghidra apunta 1 MiB equivocado.
pub fn plan_txt(objetivos: &[Objetivo], modulo: &str) -> String {
    let mut out = String::new();
    for o in objetivos {
        out.push_str(&format!(
            "{}\t0x{:x}\t{}\t{}\n",
            modulo,
            o.va_elf,
            o.id(),
            o.tarea
        ));
    }
    out
}

/// Ruta del plan en el móvil (la misma carpeta donde vive el payload).
pub fn ruta_plan() -> PathBuf {
    PathBuf::from("/data/local/tmp/megadaemon/hooks-plan.json")
}

/// Escribe el plan en el móvil (idempotente, sobreescribe).
pub fn escribir_plan(plan: &Value, ruta: &PathBuf) -> Result<(), String> {
    if let Some(dir) = ruta.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{dir:?}: {e}"))?;
    }
    let texto = serde_json::to_string_pretty(plan).map_err(|e| e.to_string())?;
    std::fs::write(ruta, texto.as_bytes()).map_err(|e| format!("{ruta:?}: {e}"))
}

/// Calcula el **bias de carga** del `.so` en un proceso vivo.
///
/// Lee `/proc/<pid>/maps` y usa el primer mapeo del fichero (el `offset`
/// del primer segmento es 0 en este módulo ⇒ `bias = inicio - offset`).
pub fn bias_del_proceso(pid: i32, ruta: &str) -> Option<u64> {
    let maps = std::fs::read_to_string(format!("/proc/{pid}/maps")).ok()?;
    let mapas = crate::inject::parse_maps(&maps);
    crate::inject::bias_de_carga(&mapas, ruta)
}

/// Resuelve **y** escribe el plan: el atajo que usará el MCP.
pub fn preparar(base: u64, ruta_salida: &PathBuf) -> Result<Value, String> {
    let ruta_ls = crate::antigravity_endpoint::ruta_ls();
    let datos = std::fs::read(&ruta_ls).map_err(|e| format!("{}: {e}", ruta_ls.display()))?;
    let objetivos = resolver(&datos, base)?;
    let plan = plan(&objetivos, base);
    escribir_plan(&plan, ruta_salida)?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsea_va_hexadecimal() {
        assert_eq!(va_de_hex("0x66a2f78"), Some(0x66a2f78));
        assert_eq!(va_de_hex("0X10"), Some(0x10));
        assert_eq!(va_de_hex("66a2f78"), None);
        assert_eq!(va_de_hex("0xzz"), None);
    }

    #[test]
    fn el_plan_lleva_direccion_absoluta_y_tarea() {
        let objs = vec![
            Objetivo {
                simbolo: "pkg.Foo".into(),
                tarea: OBSERVAR,
                va_elf: 0x1000,
                bytes: 64,
                direccion: 0x7f00_0000 + 0x1000,
            },
            Objetivo {
                simbolo: "pkg.getURL".into(),
                tarea: SUSTITUIR_URL,
                va_elf: 0x2000,
                bytes: 32,
                direccion: 0x7f00_0000 + 0x2000,
            },
        ];
        let p = plan(&objs, 0x7f00_0000);
        let hooks = p.get("hooks").and_then(|h| h.as_array()).expect("hooks");
        assert_eq!(hooks.len(), 2);
        assert_eq!(hooks[0].get("tarea").and_then(|t| t.as_str()), Some("observar"));
        assert_eq!(hooks[1].get("tarea").and_then(|t| t.as_str()), Some("sustituir_url"));
        assert_eq!(
            hooks[1].get("direccion").and_then(|d| d.as_u64()),
            Some(0x7f00_2000)
        );
        assert_eq!(hooks[0].get("id").and_then(|i| i.as_str()), Some("Foo"));
    }

    #[test]
    fn el_plan_txt_usa_tabuladores_y_va_en_hex() {
        let objs = vec![Objetivo {
            simbolo: "pkg.getURL".into(),
            tarea: SUSTITUIR_URL,
            va_elf: 0x66a2f78,
            bytes: 32,
            direccion: 0x1000,
        }];
        let t = plan_txt(&objs, "liblanguage_server.so");
        assert_eq!(
            t,
            "liblanguage_server.so\t0x66a2f78\tgetURL\tsustituir_url\n"
        );
    }

    #[test]
    fn todos_los_objetivos_son_del_ls_y_tienen_tarea() {
        for (s, t) in OBJETIVOS {
            assert!(s.contains("jetski"), "objetivo fuera de jetski: {s}");
            assert!(*t == OBSERVAR || *t == SUSTITUIR_URL, "tarea rara: {t}");
        }
    }
}

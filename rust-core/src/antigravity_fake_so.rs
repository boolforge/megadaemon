//! El **.so** de Antigravity instrumentado: un **fichero falso** que el
//! daemon genera y pone en lugar del original, quedándose **con el
//! original**.
//!
//! EL PROBLEMA (medido el 2026-11-22)
//! ----------------------------------
//! El language server (`liblanguage_server.so`, 181.358.800 B) es el que
//! habla con el proveedor de modelos: lleva dentro, como constante, la
//! URL `https://daily-cloudcode-pa.googleapis.com`.  Para que el tráfico
//! pase por el MegaDaemon hay que **sustituir esa constante** por una
//! URL local de **la MISMA longitud** (así el layout del ELF no cambia).
//!
//! LO QUE HACE ESTE MÓDULO (y por qué un "fichero falso")
//! -------------------------------------------------------
//! El parche "en el sitio" (`antigravity_endpoint::aplicar_endpoint`)
//! deja el respaldo **dentro del directorio de la app**
//! (`runtime/bin/liblanguage_server.so.bak-megadaemon`): la app lo ve, lo
//! puede tocar y la pristine original no la posee nadie.
//!
//! Aquí el daemon:
//!
//! 1. **posee el original**: copia el `.so` prístino a su **propio**
//!    directorio (`<state>/agy/lib/liblanguage_server.so.original`) y
//!    anota su sha256.  A partir de ahí, el original **lo tiene el
//!    daemon** (fuera del alcance de la app).
//! 2. **fabrica el falso**: los bytes del **original** con la URL
//!    sustituida (misma longitud ⇒ mismo layout) ⇒
//!    `liblanguage_server.so.fake`.  El falso es un artefacto aparte:
//!    se puede inspeccionar, empujar y comparar.
//! 3. **instala el falso EN EL SITIO**: se **escribe encima** del fichero
//!    de la app **sin borrarlo primero** ⇒ se conservan el **inode**, la
//!    etiqueta SELinux, el dueño y los permisos (si se borrara y se
//!    recreara, el contexto SELinux se perdería y la app no podría
//!    cargarlo).
//! 4. **restaura** cuando se le pida, comprobando el sha256 del
//!    original.
//!
//! DIFERENCIA CON EL "EN EL SITIO"
//! -------------------------------
//! | | en el sitio (`aplicar_endpoint`) | **falso** (este módulo) |
//! |---|---|---|
//! | Copia prístina | dentro del dir de la app | **en el dir del daemon** |
//! | Quién la posee | la app | **el daemon** |
//! | Artefacto "falso" separado | no | **sí** (`*.so.fake`) |
//! | Restaurar | copia el `.bak` | copia el **original** y verifica el sha256 |
//!
//! **NO VERIFICADO en dispositivo** (el S21 Ultra está caído desde el
//! 2026-11-23): el parche en sí ya estaba medido por
//! `antigravity_endpoint`; lo nuevo (la propiedad del original, el falso
//! como artefacto y la instalación en el sitio) no se ha podido probar en
//! el móvil.  Los comandos exactos están en `docs/MITM-SQLITE-Y-PROXY.md`.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// El `.so` del language server, tal como lo carga la app.
pub fn ruta_ls() -> PathBuf {
    crate::antigravity_endpoint::ruta_ls()
}

/// Directorio del daemon donde vive el original y el falso.
pub fn dir_daemon() -> PathBuf {
    PathBuf::from(crate::cockpit_server::state_dir()).join("agy/lib")
}

/// El **original prístino**, en poder del daemon.
pub fn ruta_original() -> PathBuf {
    dir_daemon().join("liblanguage_server.so.original")
}

/// El **falso** (con la URL sustituida): artefacto aparte e inspeccionable.
pub fn ruta_falsa() -> PathBuf {
    dir_daemon().join("liblanguage_server.so.fake")
}

/// Estado del "falso" (JSON, 0600).
pub fn ruta_estado() -> PathBuf {
    dir_daemon().join("estado.json")
}

fn sha256_hex(datos: &[u8]) -> String {
    crate::self_update::sha256_hex(datos)
}

/// Construye el **falso**: los bytes del original con la URL de Cloud Code
/// sustituida por la del gateway.
///
/// La sustitución es **de la MISMA longitud** ⇒ el ELF conserva su
/// layout (no cambia ni el tamaño ni los offsets).
///
/// Devuelve `(bytes, numero_de_cambios)`.
pub fn construir_falso(datos: &[u8], url_destino: &str) -> Result<(Vec<u8>, usize), String> {
    let antes = crate::antigravity_endpoint::CLOUCODE_URL.as_bytes();
    let despues = url_destino.as_bytes();
    if antes.len() != despues.len() {
        return Err(format!(
            "url_destino_invalida: {} bytes (se necesitan {})",
            despues.len(),
            antes.len()
        ));
    }
    let mut out = datos.to_vec();
    let mut pos = 0usize;
    let mut cambios = 0usize;
    while pos + antes.len() <= out.len() {
        if out[pos..pos + antes.len()] == *antes {
            out[pos..pos + antes.len()].copy_from_slice(despues);
            cambios += 1;
            pos += antes.len();
        } else {
            pos += 1;
        }
    }
    Ok((out, cambios))
}

/// Plan (no escribe nada).
pub fn plan(puerto: u16) -> Value {
    let ruta = ruta_ls();
    let datos = match std::fs::read(&ruta) {
        Ok(d) => d,
        Err(e) => {
            return json!({
                "ok": false,
                "error": e.to_string(),
                "ruta": ruta.to_string_lossy(),
                "nota": "si el LS esta en otro sitio, se puede mover a runtime/bin/liblanguage_server.so",
            })
        }
    };
    let url = crate::antigravity_endpoint::url_destino(puerto);
    let (_, cambios) = match construir_falso(&datos, &url) {
        Ok(v) => v,
        Err(e) => return json!({ "ok": false, "error": e }),
    };
    json!({
        "ok": true,
        "ruta_app": ruta.to_string_lossy(),
        "bytes": datos.len(),
        "sha256_actual": sha256_hex(&datos),
        "original_del_daemon": ruta_original().to_string_lossy(),
        "falso": ruta_falsa().to_string_lossy(),
        "cambios": cambios,
        "de": crate::antigravity_endpoint::CLOUCODE_URL,
        "a": url,
        "conserva_longitud": url.len() == crate::antigravity_endpoint::CLOUCODE_URL.len(),
        "nota": "el daemon se queda con el ORIGINAL (fuera del dir de la app) y pone el FALSO en su sitio (mismo inode: conserva etiqueta SELinux, dueño y permisos)",
    })
}

/// Activa el falso: el daemon posee el original y la app carga el falso.
///
/// Pasos:
/// 1. El daemon **copia el original** a su directorio (si no lo tenía).
/// 2. Fabrica el **falso** desde el original (no desde el fichero actual,
///    que podría estar ya parcheado).
/// 3. Lo **escribe encima** del fichero de la app (mismo inode).
/// 4. Actualiza el sha256 del manifiesto del runtime.
pub fn activar(puerto: u16, confirmar: bool) -> Result<Value, String> {
    if !confirmar {
        return Err("confirm_required".to_string());
    }
    let ruta = ruta_ls();
    let datos = std::fs::read(&ruta).map_err(|e| format!("leer {}: {e}", ruta.display()))?;

    std::fs::create_dir_all(dir_daemon()).map_err(|e| format!("crear {}: {e}", dir_daemon().display()))?;

    // 1) El daemon posee el original.
    let original = ruta_original();
    let (sha_original, original_nuevo) = if original.exists() {
        let d = std::fs::read(&original).map_err(|e| format!("leer {}: {e}", original.display()))?;
        (sha256_hex(&d), false)
    } else {
        std::fs::write(&original, &datos).map_err(|e| format!("guardar {}: {e}", original.display()))?;
        (sha256_hex(&datos), true)
    };

    // 2) El falso, SIEMPRE desde el original (fuente de verdad).
    let bytes_original = std::fs::read(&original).map_err(|e| format!("leer {}: {e}", original.display()))?;
    let url = crate::antigravity_endpoint::url_destino(puerto);
    if url.len() != crate::antigravity_endpoint::CLOUCODE_URL.len() {
        return Err(format!(
            "url_destino_invalida: {} bytes (se necesitan {})",
            url.len(),
            crate::antigravity_endpoint::CLOUCODE_URL.len()
        ));
    }
    let (falso, cambios) = construir_falso(&bytes_original, &url)?;
    if cambios == 0 {
        return Err("sin_cambios: la URL original no aparece en el original (¿ya parcheado antes?)".to_string());
    }
    let sha_falsa = sha256_hex(&falso);
    std::fs::write(&ruta_falsa(), &falso).map_err(|e| format!("guardar {}: {e}", ruta_falsa().display()))?;

    // 3) Instalación EN EL SITIO (mismo inode: se conservan etiqueta
    //    SELinux, dueño y permisos; borrar+recrear los perdería).
    std::fs::write(&ruta, &falso).map_err(|e| format!("instalar {}: {e}", ruta.display()))?;

    // 4) El manifiesto del runtime lleva el sha256 del fichero.
    let manifiesto = crate::antigravity_endpoint::actualizar_manifesto_publico(&sha_falsa);

    let estado = json!({
        "accion": "activar",
        "ts": ahora(),
        "puerto": puerto,
        "url_destino": url,
        "ruta_app": ruta.to_string_lossy(),
        "original_del_daemon": original.to_string_lossy(),
        "falso": ruta_falsa().to_string_lossy(),
        "sha256_original": sha_original,
        "sha256_falso": sha_falsa,
        "original_nuevo": original_nuevo,
        "cambios": cambios,
        "manifiesto_actualizado": manifiesto,
        // Metadatos del fichero instalado: permiten comprobar en un ciclo
        // automatico si la app ha restaurado el original SIN releer los
        // 181 MB (medido: hashear 3 veces el .so cuesta varios segundos y
        // el movil va con 4 GB de RAM).
        "app_bytes": std::fs::metadata(&ruta).map(|m| m.len()).unwrap_or(0),
        "app_mtime": mtime(&ruta),
    });
    escribir_estado(&estado)?;

    Ok(json!({
        "ok": true,
        "accion": "activar",
        "original_del_daemon": original.to_string_lossy(),
        "falso": ruta_falsa().to_string_lossy(),
        "sha256_original": sha_original,
        "sha256_falso": sha_falsa,
        "cambios": cambios,
        "manifiesto_actualizado": manifiesto,
        "efecto": "la app carga el FALSO (su trafico va al gateway local); el ORIGINAL lo tiene el daemon, fuera del alcance de la app.",
    }))
}

/// Restaura el original (byte a byte, verificado por sha256).
pub fn desactivar(confirmar: bool) -> Result<Value, String> {
    if !confirmar {
        return Err("confirm_required".to_string());
    }
    let original = ruta_original();
    if !original.exists() {
        return Err("no tengo el original: no se puede restaurar".to_string());
    }
    let bytes = std::fs::read(&original).map_err(|e| format!("leer {}: {e}", original.display()))?;
    let sha = sha256_hex(&bytes);
    let ruta = ruta_ls();
    std::fs::write(&ruta, &bytes).map_err(|e| format!("restaurar {}: {e}", ruta.display()))?;
    let manifiesto = crate::antigravity_endpoint::actualizar_manifesto_publico(&sha);
    let _ = std::fs::remove_file(ruta_falsa());
    let estado = json!({
        "accion": "desactivar",
        "ts": ahora(),
        "ruta_app": ruta.to_string_lossy(),
        "sha256_original": sha,
        "manifiesto_actualizado": manifiesto,
    });
    escribir_estado(&estado)?;
    Ok(json!({
        "ok": true,
        "accion": "desactivar",
        "sha256_restaurado": sha,
        "manifiesto_actualizado": manifiesto,
        "efecto": "la app vuelve a cargar el ORIGINAL (el falso ya no esta).",
    }))
}

/// mtime del fichero en segundos Unix (0 si no existe).
pub fn mtime(ruta: &std::path::Path) -> u64 {
    std::fs::metadata(ruta)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// **Comprobación BARATA**: ¿sigue instalado nuestro `.so` falso?
///
/// No lee el fichero: compara (tamaño, mtime) con lo que se guardó al
/// instalar.  Si la app restaura su `.so` original, el mtime cambia ⇒
/// hay que re-instrumentar.  Si no cambió, es el nuestro.
pub fn instalado_cheap() -> bool {
    let ruta = ruta_ls();
    if !ruta.exists() {
        return false;
    }
    let guardado = leer_estado().unwrap_or(Value::Null);
    let bytes = std::fs::metadata(&ruta).map(|m| m.len()).unwrap_or(0);
    let mt = mtime(&ruta);
    let gb = guardado.get("app_bytes").and_then(|v| v.as_u64()).unwrap_or(0);
    let gm = guardado.get("app_mtime").and_then(|v| v.as_u64()).unwrap_or(0);
    if gb == 0 || gm == 0 {
        // Sin metadatos guardados: no se puede saber sin hashear.
        return false;
    }
    bytes == gb && mt == gm
}

/// Estado: qué hay instalado, quién tiene el original y sus sha256.
///
/// MEDIDO (2026-11-23): hashear las tres copias del `.so` (181 MB cada una)
/// cuesta varios segundos y jeringa RAM en un móvil de 4 GB.  Ahora el sha256
/// del `.so` de la app **sólo** se calcula cuando los metadatos indican que
/// algo ha cambiado; en el caso normal no se lee el fichero.
pub fn estado() -> Value {
    let ruta = ruta_ls();
    let mut guardado = leer_estado().unwrap_or(Value::Null);
    let barato_ok = instalado_cheap();
    // En el caso normal (metadatos intactos) NO se lee el .so.
    let (sha_app, instalado) = if barato_ok {
        (
            guardado
                .get("sha256_falso")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            true,
        )
    } else {
        let s = std::fs::read(&ruta).ok().map(|d| sha256_hex(&d)).unwrap_or_default();
        let sf = std::fs::read(ruta_falsa())
            .ok()
            .map(|d| sha256_hex(&d))
            .unwrap_or_default();
        let inst = !s.is_empty() && !sf.is_empty() && s == sf;
        (s, inst)
    };
    let sha_original = guardado
        .get("sha256_original")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let sha_falsa = guardado
        .get("sha256_falso")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let sha_original = if sha_original.is_empty() && !barato_ok {
        std::fs::read(ruta_original())
            .ok()
            .map(|d| sha256_hex(&d))
            .unwrap_or_default()
    } else {
        sha_original
    };
    let sha_falsa = if sha_falsa.is_empty() && !barato_ok {
        std::fs::read(ruta_falsa())
            .ok()
            .map(|d| sha256_hex(&d))
            .unwrap_or_default()
    } else {
        sha_falsa
    };
    // MEDIDO (2026-11-23): el parche del endpoint reinstala el binario de la
    // app de forma atómica (fichero nuevo) ⇒ cambia su mtime ⇒ la
    // comprobación barata (bytes + mtime) fallaba y la UI pintaba
    // «.so sin aplicar» aunque el sha256 del binario de la app era IDÉNTICO
    // al del falso (medido: a25b406f… en ambos).  Cuando el hash demuestra
    // que el falso está instalado, se refrescan los metadatos guardados
    // para que la comprobación barata vuelva a ser válida (autorreparación
    // del estado, sin tocar el binario de la app).
    if instalado && !barato_ok {
        let bytes_actual = std::fs::metadata(&ruta).map(|m| m.len()).unwrap_or(0);
        let mt_actual = mtime(&ruta);
        if let Some(obj) = guardado.as_object_mut() {
            if obj.get("app_bytes").and_then(|v| v.as_u64()) != Some(bytes_actual)
                || obj.get("app_mtime").and_then(|v| v.as_u64()) != Some(mt_actual)
            {
                obj.insert("app_bytes".to_string(), json!(bytes_actual));
                obj.insert("app_mtime".to_string(), json!(mt_actual));
                let _ = escribir_estado(&guardado);
            }
        }
    }
    json!({
        "ok": true,
        "ruta_app": ruta.to_string_lossy(),
        "app_existe": ruta.exists(),
        "sha256_app": sha_app,
        "original_del_daemon": {
            "ruta": ruta_original().to_string_lossy(),
            "existe": ruta_original().exists(),
            "sha256": sha_original,
        },
        "falso": {
            "ruta": ruta_falsa().to_string_lossy(),
            "existe": ruta_falsa().exists(),
            "sha256": sha_falsa,
        },
        "falso_instalado": instalado,
        "estado_guardado": guardado,
        "comprobacion_barata": barato_ok,
        "nota": "el daemon posee el ORIGINAL (fuera del dir de la app) y el FALSO es lo que la app carga.",
    })
}

/// Guarda el estado (0600).
fn escribir_estado(v: &Value) -> Result<(), String> {
    let p = ruta_estado();
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("crear {}: {e}", dir.display()))?;
    }
    let txt = serde_json::to_string_pretty(v).map_err(|e| format!("serializar: {e}"))?;
    std::fs::write(&p, txt).map_err(|e| format!("escribir {}: {e}", p.display()))?;
    set_0600(&p)?;
    Ok(())
}

/// Lee el estado.
pub fn leer_estado() -> Option<Value> {
    let txt = std::fs::read_to_string(ruta_estado()).ok()?;
    serde_json::from_str(&txt).ok()
}

fn set_0600(p: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("chmod 0600 {}: {e}", p.display()))
}

fn ahora() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Tool MCP: `antigravity_fake_so`.
pub fn tool_definitions() -> Value {
    json!([
        {
            "name": "antigravity_fake_so",
            "description": "INSTRUMENTA el binario del language server de Antigravity (liblanguage_server.so) con un FICHERO FALSO: el daemon se queda con el ORIGINAL (fuera del dir de la app, en su propio state dir) y pone en su lugar un falso con la URL del proveedor sustituida por la del gateway local (misma longitud => mismo layout del ELF). Se escribe EN EL SITIO (mismo inode) para no perder la etiqueta SELinux, el dueno ni los permisos. Acciones: plan (no escribe), activar (exige confirm), desactivar (exige confirm; restaura el original y verifica su sha256), estado.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["estado", "plan", "activar", "desactivar"], "default": "estado" },
                    "puerto": { "type": "integer", "description": "Puerto del gateway local (defecto 9090)." },
                    "confirm": { "type": "boolean", "description": "Obligatorio para activar/desactivar: escriben en el binario de la app." }
                }
            }
        }
    ])
}

/// CLI + MCP.
pub fn dispatch(name: &str, args: &Value) -> Option<Value> {
    if name != "antigravity_fake_so" {
        return None;
    }
    let s = |k: &str| args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let accion = {
        let a = s("action");
        if a.trim().is_empty() {
            "estado".to_string()
        } else {
            a
        }
    };
    let confirm = args.get("confirm").and_then(|v| v.as_bool()).unwrap_or(false);
    let puerto: u16 = args
        .get("puerto")
        .and_then(|v| v.as_u64())
        .unwrap_or(crate::antigravity_endpoint::PUERTO_GATEWAY as u64) as u16;
    let v = match accion.as_str() {
        "estado" => estado(),
        "plan" => plan(puerto),
        "activar" => match activar(puerto, confirm) {
            Ok(v) => v,
            Err(e) => json!({ "ok": false, "error": e }),
        },
        "desactivar" => match desactivar(confirm) {
            Ok(v) => v,
            Err(e) => json!({ "ok": false, "error": e }),
        },
        _ => json!({
            "ok": false,
            "error": "accion_desconocida",
            "acciones": ["estado", "plan", "activar", "desactivar"],
        }),
    };
    Some(json!({ "tool": "antigravity_fake_so", "accion": accion, "resultado": v }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_falso_sustituye_la_url_y_conserva_la_longitud() {
        let original = crate::antigravity_endpoint::CLOUCODE_URL.as_bytes();
        let datos = [b"AAA".as_slice(), original, b"BBB"].concat();
        let url = crate::antigravity_endpoint::url_destino(9090);
        let (falso, cambios) = construir_falso(&datos, &url).expect("falso");
        assert_eq!(cambios, 1);
        assert_eq!(falso.len(), datos.len(), "la longitud no puede cambiar");
        assert!(!falso
            .windows(original.len())
            .any(|w| w == original));
        assert!(falso.windows(url.len()).any(|w| w == url.as_bytes()));
    }

    #[test]
    fn una_url_de_otra_longitud_se_rechaza() {
        let datos = crate::antigravity_endpoint::CLOUCODE_URL.as_bytes().to_vec();
        let r = construir_falso(&datos, "http://corto");
        assert!(r.is_err(), "una URL mas corta no vale: cambia el layout");
        let e = r.err().unwrap_or_default();
        assert!(e.contains("url_destino_invalida"));
    }

    #[test]
    fn sin_la_url_no_hay_cambios() {
        let datos = b"nada que ver aqui".to_vec();
        let url = crate::antigravity_endpoint::url_destino(9090);
        let (falso, cambios) = construir_falso(&datos, &url).expect("falso");
        assert_eq!(cambios, 0);
        assert_eq!(falso, datos);
    }

    #[test]
    fn el_plan_no_escribe_nada() {
        // Sin el movil no hay .so: el plan lo dice, pero NO falla.
        let v = plan(9090);
        assert!(v.get("ok").is_some());
    }
}

//! Puente de endpoint: hacer que el language server (LS) de Antigravity
//! hable **siempre** con el MegaDaemon canónico, y que el selector de la
//! GUI ofrezca el modelo `megadaemon`.
//!
//! # Qué está medido (móvil S21, `com.agy` 2.11.0, 2026-11-22)
//!
//! 1. `runtime/bin/liblanguage_server.so` (ELF aarch64, 181.358.800 B) contiene
//!    **una** vez la cadena `https://daily-cloudcode-pa.googleapis.com`
//!    (42 B) → es el endpoint de Cloud Code. El LS llama
//!    `<base>/v1internal:generateContent` (MEDIDO: strings
//!    `/v1internal:generateContent` y `/v1internal:streamGenerateContent`).
//! 2. `web/generated/main.js` contiene la condición del selector de modelos:
//!    `osName==="linux"&&k.isGoogleInternal` ⇒ en Android los modelos
//!    personalizados se marcan `disabled=true` y **la GUI no los ofrece**
//!    (aunque el BYOM esté registrado en `antigravity_state.pbtxt`).
//!
//! # Referencia externa
//!
//! Proyecto `vahapogut/antigravity-add-model` (Apache-2.0), encontrado en el
//! móvil en `/data/user/0/com.agy/files/antigravity-add-model`. Hace lo mismo
//! que aquí pero con **su** proxy:
//!   - `scripts/language-server-binary-patch.mjs`: sustituye la URL de Cloud
//!     Code por `http://localhost:50999/v1internal/xxxxxxx` (42 B ⇒ el
//!     reemplazo **conserva la longitud**, por eso vale para cualquier SO).
//!   - `scripts/ide.mjs`: el IDE lee el ajuste oculto
//!     `jetski.cloudCodeUrl` y lo inyecta como `CLOUD_CODE_URL` (o
//!     `AGY_LLM_GATEWAY_URL`, que **sustituye la lista de modelos** nativa).
//!
//! Aquí no se reimplementa ese proxy: se usa el **gateway canónico** del
//! MegaDaemon (dialecto Gemini ya implementado en `llm_gateway.rs`), de modo
//! que no hay un tercer motor de inferencia.
//!
//! # Reglas
//!
//! * Todo parche es **reversible**: copia `.bak-megadaemon` antes de tocar.
//! * El parche del binario **conserva la longitud** ⇒ no cambia el layout
//!   del ELF y el sha256 del manifiesto se actualiza después.
//! * Nada se escribe sin `confirm=true` (MCP) / `--confirm` (CLI).

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Endpoint de Cloud Code presente en el LS de Antigravity (42 bytes).
pub const CLOUCODE_URL: &str = "https://daily-cloudcode-pa.googleapis.com";

/// Condición que la GUI usa para habilitar los modelos personalizados.
pub const GATE_CONDICION: &str = "osName===\"linux\"&&k.isGoogleInternal";

/// Puerto por defecto del gateway (cockpit) del MegaDaemon en el móvil.
pub const PUERTO_GATEWAY: u16 = 9090;

/// Raíz de los datos de la app en el móvil.
pub const RAIZ_APP: &str = "/data/user/0/com.agy/files";

fn sha256_hex(datos: &[u8]) -> String {
    crate::self_update::sha256_hex(datos)
}

fn sha256_de(p: &Path) -> Option<String> {
    let datos = std::fs::read(p).ok()?;
    Some(sha256_hex(&datos))
}

/// Ruta del binario del language server.
pub fn ruta_ls() -> PathBuf {
    PathBuf::from(RAIZ_APP).join("runtime/bin/liblanguage_server.so")
}

/// Ruta del manifiesto del runtime (sha256 por fichero).
pub fn ruta_manifest() -> PathBuf {
    PathBuf::from(RAIZ_APP).join("runtime/manifest.json")
}

/// Ruta del bundle web que decide si los modelos personalizados se ofrecen.
pub fn ruta_gate() -> PathBuf {
    PathBuf::from(RAIZ_APP).join("web/generated/main.js")
}

/// URL destino para el LS, **de la misma longitud** que [`CLOUCODE_URL`].
///
/// `http://127.0.0.1:<puerto>/v1internal/xxxx…` → 42 bytes con cualquier
/// puerto de 4 o 5 dígitos (los `x` se ajustan: 12 - nº de dígitos).
pub fn url_destino(puerto: u16) -> String {
    let base = format!("http://127.0.0.1:{puerto}/v1internal/");
    let cible = CLOUCODE_URL.len() - base.len();
    let x = if cible == 0 {
        String::new()
    } else {
        "x".repeat(cible)
    };
    let mut s = format!("{base}{x}");
    s.truncate(CLOUCODE_URL.len());
    s
}

/// ¿La URL destino conserva la longitud exacta de la original?
pub fn url_destino_valido(puerto: u16) -> bool {
    url_destino(puerto).len() == CLOUCODE_URL.len()
}

/// Catálogo de modelos en formato Gemini (`GET /v1beta/models`).
///
/// Lo consume el LS de Antigravity cuando su endpoint apunta al gateway
/// (modo "LLM gateway"): la lista **sustituye** a la nativa de Google, así
/// que aquí sale el modelo `megadaemon` y los proveedores del vault.
pub fn catalogo_gemini() -> Value {
    let vault = crate::providers::load_vault();
    let mut models: Vec<Value> = Vec::new();

    models.push(json!({
        "name": "models/megadaemon",
        "version": "1.0",
        "displayName": "MegaDaemon",
        "description": "Gateway canónico del MegaDaemon: enruta a los proveedores del vault con fallback real.",
        "inputTokenLimit": 1_000_000,
        "outputTokenLimit": 65_536,
        "supportedGenerationMethods": ["generateContent", "countTokens", "streamGenerateContent"],
        "temperature": 0.7,
        "topP": 0.9,
        "topK": 40,
    }));

    for p in vault.providers.iter().filter(|p| p.enabled) {
        models.push(json!({
            "name": format!("models/{}", p.id),
            "version": "1.0",
            "displayName": if p.name.trim().is_empty() { p.id.clone() } else { p.name.clone() },
            "description": format!("Proveedor del MegaDaemon: {} ({}).", p.vendor, p.model),
            "inputTokenLimit": 200_000,
            "outputTokenLimit": 32_768,
            "supportedGenerationMethods": ["generateContent", "countTokens", "streamGenerateContent"],
            "temperature": 0.7,
            "topP": 0.9,
            "topK": 40,
        }));
    }

    json!({ "models": models })
}

/// Estado (sólo lectura) de las dos palancas.
pub fn estado() -> Value {
    let ls = ruta_ls();
    let gate = ruta_gate();
    let manifest = ruta_manifest();

    let (occ, parcheado, sha) = match std::fs::read(&ls) {
        Ok(d) => {
            let original = CLOUCODE_URL.as_bytes();
            let occ = d.windows(original.len()).filter(|w| *w == original).count();
            let destino = url_destino(PUERTO_GATEWAY);
            let parcheado = d.windows(destino.len()).any(|w| w == destino.as_bytes());
            (occ, parcheado, Some(sha256_hex(&d)))
        }
        Err(_) => (0, false, None),
    };

    let gate_info = match std::fs::read_to_string(&gate) {
        Ok(t) => {
            let occ = t.matches(GATE_CONDICION).count();
            json!({
                "ruta": gate.to_string_lossy(),
                "ocurrencias": occ,
                "parcheado": occ == 0 && t.contains("isGoogleInternal") == false,
                "nota": "parcheado = la condición `osName===\"linux\"&&k.isGoogleInternal` ya no aparece",
            })
        }
        Err(e) => json!({ "ruta": gate.to_string_lossy(), "error": e.to_string() }),
    };

    let manifest_sha = std::fs::read_to_string(&manifest)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| {
            v.get("files")
                .and_then(|f| f.get("bin/liblanguage_server.so"))
                .and_then(|s| s.as_str().map(|s| s.to_string()))
        });

    json!({
        "endpoint": {
            "ruta": ls.to_string_lossy(),
            "ocurrencias_url_original": occ,
            "parcheado": parcheado,
            "url_original": CLOUCODE_URL,
            "url_destino": url_destino(PUERTO_GATEWAY),
            "longitud_conservada": url_destino_valido(PUERTO_GATEWAY),
            "sha256": sha,
            "sha256_manifesto": manifest_sha,
            "sha256_coincide_manifesto": match (&sha, &manifest_sha) {
                (Some(a), Some(b)) => a == b,
                _ => false,
            },
        },
        "gate": gate_info,
        "ok": true,
    })
}

/// Plan del parche del binario (no escribe nada).
pub fn plan_endpoint(puerto: u16) -> Value {
    let url = url_destino(puerto);
    let datos = match std::fs::read(ruta_ls()) {
        Ok(d) => d,
        Err(e) => {
            return json!({ "ok": false, "error": e.to_string(), "ruta": ruta_ls().to_string_lossy() });
        }
    };
    let occ = datos.windows(CLOUCODE_URL.len()).filter(|w| *w == CLOUCODE_URL.as_bytes()).count();
    json!({
        "ok": true,
        "ruta": ruta_ls().to_string_lossy(),
        "ocurrencias": occ,
        "de": CLOUCODE_URL,
        "a": url,
        "longitud_original": CLOUCODE_URL.len(),
        "longitud_destino": url.len(),
        "conserva_longitud": url.len() == CLOUCODE_URL.len(),
        "respaldo": format!("{}.bak-megadaemon", ruta_ls().to_string_lossy()),
        "nota": "el manifiesto (runtime/manifest.json) se actualiza con el sha256 nuevo",
    })
}

/// Aplica el parche del binario: copia de seguridad + sustitución + manifiesto.
pub fn aplicar_endpoint(puerto: u16, confirmar: bool) -> Result<Value, String> {
    if !confirmar {
        return Err("confirm_required".to_string());
    }
    if !url_destino_valido(puerto) {
        return Err(format!(
            "url_destino_invalida: {} bytes (se necesitan {})",
            url_destino(puerto).len(),
            CLOUCODE_URL.len()
        ));
    }
    let ruta = ruta_ls();
    let respaldo = ruta.with_extension("so.bak-megadaemon");
    let mut datos = std::fs::read(&ruta).map_err(|e| e.to_string())?;
    let destino = url_destino(puerto);
    let antes = CLOUCODE_URL.as_bytes();
    let despues = destino.as_bytes();

    let mut pos = 0usize;
    let mut cambios = 0usize;
    while pos + antes.len() <= datos.len() {
        if datos[pos..pos + antes.len()] == *antes {
            datos[pos..pos + antes.len()].copy_from_slice(despues);
            cambios += 1;
            pos += antes.len();
        } else {
            pos += 1;
        }
    }
    if cambios == 0 {
        return Err("sin_cambios: la URL original no aparece (¿ya parcheado?)".to_string());
    }

    if !respaldo.exists() {
        std::fs::copy(&ruta, &respaldo).map_err(|e| format!("respaldo: {e}"))?;
    }
    std::fs::write(&ruta, &datos).map_err(|e| format!("escritura: {e}"))?;

    let sha = sha256_hex(&datos);
    let actualizado = actualizar_manifesto(&sha);

    Ok(json!({
        "ok": true,
        "cambios": cambios,
        "ruta": ruta.to_string_lossy(),
        "respaldo": respaldo.to_string_lossy(),
        "sha256_nuevo": sha,
        "manifesto_actualizado": actualizado,
        "url_destino": destino,
    }))
}

/// Actualiza el sha256 del LS en `runtime/manifest.json` (expuesto para
/// que el módulo del "falso" (`antigravity_fake_so`) reutilice ESTA
/// función y no duplique la lógica del manifiesto).
pub fn actualizar_manifesto_publico(sha: &str) -> bool {
    actualizar_manifesto(sha)
}

/// Actualiza el sha256 del LS en `runtime/manifest.json`.
fn actualizar_manifesto(sha: &str) -> bool {
    let ruta = ruta_manifest();
    let texto = match std::fs::read_to_string(&ruta) {
        Ok(t) => t,
        Err(_) => return false,
    };
    let mut v: Value = match serde_json::from_str(&texto) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let ok = v
        .get_mut("files")
        .and_then(|f| f.as_object_mut())
        .map(|f| {
            f.insert(
                "bin/liblanguage_server.so".to_string(),
                Value::String(sha.to_string()),
            );
            true
        })
        .unwrap_or(false);
    if ok {
        match serde_json::to_string_pretty(&v) {
            Ok(s) => {
                let _ = std::fs::write(&ruta, s);
            }
            Err(_) => return false,
        }
    }
    ok
}

/// Plan del parche del gate de la GUI (no escribe nada).
pub fn plan_gate() -> Value {
    let ruta = ruta_gate();
    match std::fs::read_to_string(&ruta) {
        Ok(t) => {
            let occ = t.matches(GATE_CONDICION).count();
            json!({
                "ok": true,
                "ruta": ruta.to_string_lossy(),
                "ocurrencias": occ,
                "de": GATE_CONDICION,
                "a": "!0",
                "nota": "`!0` es true en JS: habilita los modelos personalizados en cualquier SO",
                "riesgo": "bundle web regenerado por la app => puede perder el parche",
            })
        }
        Err(e) => json!({ "ok": false, "error": e.to_string() }),
    }
}

/// Aplica el parche del gate de la GUI (con respaldo).
pub fn aplicar_gate(confirmar: bool) -> Result<Value, String> {
    if !confirmar {
        return Err("confirm_required".to_string());
    }
    let ruta = ruta_gate();
    let respaldo = ruta.with_extension("js.bak-megadaemon");
    let texto = std::fs::read_to_string(&ruta).map_err(|e| e.to_string())?;
    let occ = texto.matches(GATE_CONDICION).count();
    if occ == 0 {
        return Err("sin_cambios: la condición del gate no aparece".to_string());
    }
    let nuevo = texto.replace(GATE_CONDICION, "!0");
    if !respaldo.exists() {
        std::fs::copy(&ruta, &respaldo).map_err(|e| format!("respaldo: {e}"))?;
    }
    std::fs::write(&ruta, nuevo).map_err(|e| format!("escritura: {e}"))?;
    Ok(json!({
        "ok": true,
        "cambios": occ,
        "ruta": ruta.to_string_lossy(),
        "respaldo": respaldo.to_string_lossy(),
    }))
}

/// Restaura los dos ficheros desde sus respaldos.
pub fn restaurar(confirmar: bool) -> Result<Value, String> {
    if !confirmar {
        return Err("confirm_required".to_string());
    }
    let mut restaurados = Vec::new();
    for (orig, bak) in [
        (ruta_ls(), ruta_ls().with_extension("so.bak-megadaemon")),
        (ruta_gate(), ruta_gate().with_extension("js.bak-megadaemon")),
    ] {
        if bak.exists() {
            std::fs::copy(&bak, &orig).map_err(|e| format!("restaurar {}: {e}", orig.to_string_lossy()))?;
            restaurados.push(orig.to_string_lossy().to_string());
        }
    }
    // Tras restaurar, el sha256 vuelve a ser el original: hay que reponerlo
    // en el manifiesto o la app lo consideraría alterado.
    if let Some(sha) = sha256_de(&ruta_ls()) {
        let _ = actualizar_manifesto(&sha);
    }
    Ok(json!({ "ok": true, "restaurados": restaurados }))
}

/// CLI + MCP: `antigravity_endpoint`.
pub fn dispatch(name: &str, args: &Value) -> Option<Value> {
    if name != "antigravity_endpoint" {
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
        .unwrap_or(PUERTO_GATEWAY as u64) as u16;

    let v = match accion.as_str() {
        "estado" => estado(),
        "plan" => plan_endpoint(puerto),
        "plan_gate" => plan_gate(),
        "catalogo" => catalogo_gemini(),
        "aplicar" => match aplicar_endpoint(puerto, confirm) {
            Ok(v) => v,
            Err(e) => json!({ "ok": false, "error": e }),
        },
        "aplicar_gate" => match aplicar_gate(confirm) {
            Ok(v) => v,
            Err(e) => json!({ "ok": false, "error": e }),
        },
        "restaurar" => match restaurar(confirm) {
            Ok(v) => v,
            Err(e) => json!({ "ok": false, "error": e }),
        },
        _ => json!({
            "ok": false,
            "error": "accion_desconocida",
            "acciones": ["estado", "plan", "plan_gate", "catalogo", "aplicar", "aplicar_gate", "restaurar"],
        }),
    };
    Some(json!({ "tool": "antigravity_endpoint", "accion": accion, "resultado": v }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_destino_conserva_longitud() {
        for p in [9090u16, 50999, 8080, 443] {
            assert_eq!(url_destino(p).len(), CLOUCODE_URL.len(), "puerto {p}");
            assert!(url_destino_valido(p));
        }
    }

    #[test]
    fn url_destino_apunta_al_gateway() {
        let u = url_destino(9090);
        assert!(u.starts_with("http://127.0.0.1:9090/v1internal/"));
    }

    #[test]
    fn catalogo_incluye_megadaemon() {
        let c = catalogo_gemini();
        let nombres: Vec<String> = c["models"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["name"].as_str().unwrap_or("").to_string())
            .collect();
        assert!(nombres.contains(&"models/megadaemon".to_string()));
    }

    #[test]
    fn dispatch_rechaza_otros_nombres() {
        assert!(dispatch("otro", &json!({})).is_none());
    }
}

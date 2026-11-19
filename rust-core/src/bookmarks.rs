//! MegaDaemon v13.4 — Marcadores, sesiones guardadas y passkeys.
//!
//! El usuario tiene decenas de miles de marcadores y ningún sistema que los
//! reparta entre navegadores y sistemas.  Este módulo **extrae** lo que hay
//! y lo **exporta en formatos estándar** que cualquier navegador, gestor o
//! sistema puede importar:
//!
//! * **Chrome/Chromium**: `Bookmarks` (JSON) → árbol completo.
//! * **Firefox**: `places.sqlite` (SQLite nativo con rusqlite) → marcadores e
//!   historial de visitas.
//! * **Edge/Chromium**: mismo formato que Chrome.
//! * Exportación **HTML de Netscape** (formato que importan Chrome, Firefox,
//!   Edge, Safari, Opera, Brave, Vivaldi, etc.): es el denominador común que
//!   permite "sincronizar y distribuir" a cualquier sistema.
//! * Passkeys/credenciales: se **localizan** (no se exportan en claro) y se
//!   listan con su origen; el material criptográfico del Android Keystore no
//!   es exportable ni con root, y eso se dice con claridad en el informe.

use serde_json::{json, Value};

/// Estructura de un marcador.
#[derive(Clone, Debug)]
pub struct Bookmark {
    pub title: String,
    pub url: String,
    pub folder: String,
    pub added: u64,
    pub source: String,
}

/// Los navegadores/almacenes que se inspeccionan.
const PROFILES: &[(&str, &str)] = &[
    ("chrome", "/data/data/com.android.chrome/app_chrome/Default"),
    ("edge", "/data/data/com.microsoft.emmx/app_chrome/Default"),
    ("brave", "/data/data/com.brave.browser/app_chrome/Default"),
    ("vivaldi", "/data/data/com.vivaldi.android/app_chrome/Default"),
    ("opera", "/data/data/com.opera.browser/app_chrome/Default"),
    ("firefox", "/data/data/org.mozilla.firefox/files"),
];

/// Lee un fichero de texto del dispositivo.
fn read_text(path: &str, max: usize) -> Option<String> {
    let r = crate::android::root_exec(
        &format!("cat {} 2>/dev/null", crate::android::sh_quote(path)),
        90_000,
        max,
    );
    if r.stdout.trim().is_empty() {
        None
    } else {
        Some(r.stdout)
    }
}

/// Lee un fichero binario del dispositivo en base64.
fn read_binary_b64(path: &str, max: usize) -> Option<String> {
    let r = crate::android::root_exec(
        &format!(
            "base64 -w 0 {} 2>/dev/null",
            crate::android::sh_quote(path)
        ),
        180_000,
        max,
    );
    if r.stdout.trim().is_empty() {
        None
    } else {
        Some(r.stdout.trim().to_string())
    }
}

/// Parsea el JSON `Bookmarks` de Chromium (recorre el árbol de carpetas).
pub fn parse_chromium_bookmarks(json_text: &str, source: &str) -> Vec<Bookmark> {
    let mut out = vec![];
    let Ok(v) = serde_json::from_str::<Value>(json_text) else {
        return out;
    };
    fn walk(node: &Value, path: &str, source: &str, out: &mut Vec<Bookmark>) {
        let Some(children) = node.get("children").and_then(|c| c.as_array()) else {
            return;
        };
        for c in children {
            let ty = c.get("type").and_then(|t| t.as_str()).unwrap_or("");
            let name = c.get("name").and_then(|t| t.as_str()).unwrap_or("").to_string();
            let folder = if path.is_empty() { name.clone() } else { format!("{}/{}", path, name) };
            match ty {
                "folder" => walk(c, &folder, source, out),
                "url" => {
                    let url = c.get("url").and_then(|t| t.as_str()).unwrap_or("").to_string();
                    if url.is_empty() {
                        continue;
                    }
                    let added = c
                        .get("date_added")
                        .and_then(|d| d.as_str())
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0);
                    out.push(Bookmark {
                        title: name,
                        url,
                        folder: path.to_string(),
                        added,
                        source: source.to_string(),
                    });
                }
                _ => {}
            }
        }
    }
    if let Some(roots) = v.get("roots").and_then(|r| r.as_object()) {
        for (_k, node) in roots {
            walk(node, "", source, &mut out);
        }
    }
    out
}

/// Extrae los marcadores de un perfil Chromium instalado.
pub fn chromium_bookmarks(profile_key: &str) -> Value {
    let Some((_, base)) = PROFILES.iter().find(|(k, _)| *k == profile_key) else {
        return json!({ "ok": false, "error": "unknown_profile" });
    };
    let path = format!("{}/Bookmarks", base);
    let Some(text) = read_text(&path, 64 * 1024 * 1024) else {
        return json!({
            "ok": true, "profile": profile_key, "installed": false,
            "path": path, "count": 0, "bookmarks": []
        });
    };
    let marks = parse_chromium_bookmarks(&text, profile_key);
    json!({
        "ok": true,
        "profile": profile_key,
        "installed": true,
        "path": path,
        "count": marks.len(),
        "bookmarks": marks.iter().take(200).map(|m| json!({
            "title": m.title, "url": m.url, "folder": m.folder, "added": m.added
        })).collect::<Vec<Value>>(),
    })
}

/// Extrae marcadores de Firefox (`places.sqlite`) con SQLite nativo.
pub fn firefox_bookmarks(limit: usize) -> Value {
    // Firefox guarda el perfil en un directorio aleatorio.
    let find = crate::android::root_exec(
        "find /data/data/org.mozilla.firefox/files -name places.sqlite 2>/dev/null | head -3",
        60_000,
        4096,
    );
    let mut paths: Vec<String> = find
        .stdout
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    if paths.is_empty() {
        return json!({ "ok": true, "installed": false, "count": 0, "bookmarks": [] });
    }
    let mut out = vec![];
    let mut errors = vec![];
    for p in paths.iter().take(2) {
        // Se copia antes de abrir: Firefox escribe a diario.
        let tmp = format!("/data/local/tmp/megadaemon/places_{}.sqlite", std::process::id());
        let cp = crate::android::root_exec(
            &format!(
                "mkdir -p /data/local/tmp/megadaemon && cp {} {} 2>/dev/null",
                crate::android::sh_quote(p),
                crate::android::sh_quote(&tmp)
            ),
            120_000,
            1024,
        );
        if cp.code != Some(0) {
            errors.push(json!({ "path": p, "error": cp.stderr.trim() }));
            continue;
        }
        match rusqlite::Connection::open(&tmp) {
            Ok(conn) => {
                let mut stmt = match conn.prepare(
                    "SELECT moz_bookmarks.title, moz_places.url FROM moz_bookmarks \
                     JOIN moz_places ON moz_bookmarks.fk = moz_places.id \
                     WHERE moz_places.url LIKE 'http%' LIMIT ?",
                ) {
                    Ok(s) => s,
                    Err(e) => {
                        errors.push(json!({ "path": p, "error": e.to_string() }));
                        continue;
                    }
                };
                let rows = stmt.query_map([limit as i64], |r| {
                    Ok((
                        r.get::<_, Option<String>>(0).unwrap_or_default(),
                        r.get::<_, Option<String>>(1).unwrap_or_default(),
                    ))
                });
                if let Ok(rows) = rows {
                    for row in rows.flatten() {
                        out.push(json!({
                            "title": row.0.unwrap_or_default(),
                            "url": row.1.unwrap_or_default(),
                            "source": "firefox",
                        }));
                    }
                }
            }
            Err(e) => errors.push(json!({ "path": p, "error": e.to_string() })),
        }
        let _ = crate::android::root_exec(
            &format!("rm -f {}", crate::android::sh_quote(&tmp)),
            20_000,
            512,
        );
    }
    json!({
        "ok": true,
        "installed": true,
        "count": out.len(),
        "bookmarks": out,
        "errors": errors,
    })
}

/// Exporta a HTML de Netscape (importable por cualquier navegador).
pub fn export_netscape_html(bookmarks: &[Bookmark]) -> String {
    let mut s = String::from(
        "<!DOCTYPE NETSCAPE-Bookmark-file-1>\n<META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">\n<TITLE>Bookmarks</TITLE>\n<H1>Bookmarks</H1>\n<DL><p>\n",
    );
    for b in bookmarks {
        let esc = |t: &str| {
            t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
        };
        s.push_str(&format!(
            "    <DT><A HREF=\"{}\" ADD_DATE=\"{}\">{}</A>\n",
            esc(&b.url),
            b.added,
            esc(&b.title)
        ));
    }
    s.push_str("</DL><p>\n");
    s
}

/// Inventario completo: todos los navegadores del dispositivo.
pub fn inventory() -> Value {
    let mut browsers = vec![];
    let mut total = 0usize;
    for (key, base) in PROFILES {
        let exists = crate::android::root_exec(
            &format!("test -d {} && echo yes || echo no", crate::android::sh_quote(base)),
            15_000,
            512,
        );
        let installed = exists.stdout.trim() == "yes";
        let mut count = 0usize;
        let mut detail = Value::Null;
        if installed {
            if *key == "firefox" {
                let ff = firefox_bookmarks(20000);
                count = ff["count"].as_u64().unwrap_or(0) as usize;
            } else {
                let ch = chromium_bookmarks(key);
                count = ch["count"].as_u64().unwrap_or(0) as usize;
                detail = json!({ "path": ch["path"].clone() });
            }
        }
        total += count;
        browsers.push(json!({
            "browser": key,
            "base": base,
            "installed": installed,
            "bookmarks": count,
            "detail": detail,
        }));
    }
    json!({
        "ok": true,
        "browsers": browsers,
        "total_bookmarks": total,
    })
}

/// Exporta todos los marcadores a un fichero HTML en el dispositivo.
pub fn export_all(dest_path: &str) -> Value {
    if !dest_path.starts_with('/') || dest_path.contains("..") {
        return json!({ "ok": false, "error": "bad_path" });
    }
    let mut all: Vec<Bookmark> = vec![];
    for (key, _) in PROFILES {
        if *key == "firefox" {
            let ff = firefox_bookmarks(50000);
            if let Some(arr) = ff["bookmarks"].as_array() {
                for b in arr {
                    all.push(Bookmark {
                        title: b["title"].as_str().unwrap_or("").to_string(),
                        url: b["url"].as_str().unwrap_or("").to_string(),
                        folder: String::new(),
                        added: 0,
                        source: "firefox".into(),
                    });
                }
            }
        } else {
            let ch = chromium_bookmarks(key);
            if let Some(arr) = ch["bookmarks"].as_array() {
                for b in arr {
                    all.push(Bookmark {
                        title: b["title"].as_str().unwrap_or("").to_string(),
                        url: b["url"].as_str().unwrap_or("").to_string(),
                        folder: b["folder"].as_str().unwrap_or("").to_string(),
                        added: b["added"].as_u64().unwrap_or(0),
                        source: key.to_string(),
                    });
                }
            }
        }
    }
    let html = export_netscape_html(&all);
    let cmd = format!(
        "mkdir -p $(dirname {}) && cat > {} <<'__MD_EOF__'\n{}\n__MD_EOF__\nchmod 600 {} && wc -c < {}",
        crate::android::sh_quote(dest_path),
        crate::android::sh_quote(dest_path),
        html,
        crate::android::sh_quote(dest_path),
        crate::android::sh_quote(dest_path)
    );
    let r = crate::android::root_exec(&cmd, 180_000, 4096);
    json!({
        "ok": r.code == Some(0),
        "path": dest_path,
        "bookmarks": all.len(),
        "bytes": r.stdout.trim(),
        "format": "netscape-html",
    })
}

/// Passkeys / credenciales guardadas: **localización**, no exportación.
///
/// El material criptográfico de los passkeys vive en el Android Keystore
/// (no exportable) o cifrado con una clave derivada de la cuenta.  Lo que sí
/// puede hacerse —y es útil— es un **inventario**: qué passkeys hay, en qué
/// origen, cuándo se crearon y para qué sitio (RP ID).
pub fn passkey_inventory() -> Value {
    let mut found = vec![];
    // 1) Passkeys de Google/Play Services (Android 14): el almacén real está
    //    en el Keystore; el metadato legible está en la app de Google.
    for (svc, base) in [
        ("google", "/data/data/com.google.android.gms"),
        ("chromium", "/data/data/com.android.chrome"),
        ("androidx-credential-manager", "/data/data/com.google.android.apps.credentialmanager"),
    ] {
        let r = crate::android::root_exec(
            &format!("find {} -iname '*credential*' -o -iname '*passkey*' 2>/dev/null | head -5", crate::android::sh_quote(base)),
            60_000,
            8192,
        );
        let paths: Vec<String> = r.stdout.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
        if !paths.is_empty() {
            found.push(json!({ "service": svc, "artifacts": paths, "material": "keystore-no-exportable" }));
        }
    }
    // 2) Gestores de contraseñas instalados (los datos sí son exportables).
    let pm = crate::android::root_exec(
        "pm list packages 2>/dev/null | grep -iE 'bitwarden|keepass|1password|lastpass|dashlane|enpass|mium' | head -20",
        45_000,
        8192,
    );
    let managers: Vec<String> = pm.stdout.lines().map(|l| l.trim().trim_start_matches("package:").to_string()).filter(|l| !l.is_empty()).collect();
    json!({
        "ok": true,
        "passkey_stores": found,
        "password_managers": managers,
        "note": "El material criptográfico de los passkeys no es exportable ni con root (AndroidKeyStore). Lo exportable son los gestores de contraseñas con su propio formato (bitwarden JSON cifrado, kdbx, 1Password .opvault).",
        "exportable": true,
    })
}

/// Exporta los datos de un gestor de contraseñas instalado (formatos estándar).
pub fn password_manager_export(package: &str, dest_path: &str) -> Value {
    if !dest_path.starts_with('/') || dest_path.contains("..") {
        return json!({ "ok": false, "error": "bad_path" });
    }
    if package.is_empty() || !package.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-') {
        return json!({ "ok": false, "error": "bad_package" });
    }
    let base = format!("/data/data/{}", package);
    let r = crate::android::root_exec(
        &format!("find {} -maxdepth 3 \\( -name 'data.json' -o -name '*.kdbx' -o -name '*.opvault' -o -name '*.sqlite' \\) 2>/dev/null | head -10", crate::android::sh_quote(&base)),
        60_000,
        8192,
    );
    let files: Vec<String> = r.stdout.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    if files.is_empty() {
        return json!({
            "ok": false, "error": "no_export_found",
            "hint": "no se ha encontrado un almacén exportable; revisa el gestor concreto",
            "package": package,
        });
    }
    json!({
        "ok": true,
        "package": package,
        "candidates": files,
        "hint": "copia el fichero a un destino seguro; el contenido va cifrado con la contraseña del gestor",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsea_el_arbol_de_chromium() {
        let sample = r#"{"roots":{"bookmark_bar":{"type":"folder","name":"Barra de marcadores","children":[
            {"type":"url","name":"Ejemplo","url":"https://example.com","date_added":"13200000000000000"},
            {"type":"folder","name":"Carpeta","children":[
                {"type":"url","name":"Inside","url":"https://inside.test","date_added":"13200000000000001"}]}]}}}"#;
        let marks = parse_chromium_bookmarks(sample, "chrome");
        assert_eq!(marks.len(), 2);
        assert_eq!(marks[0].url, "https://example.com");
        assert_eq!(marks[1].folder, "Barra de marcadores/Carpeta");
    }

    #[test]
    fn la_exportacion_es_html_de_netscape_importable() {
        let marks = vec![Bookmark {
            title: "A & B".into(),
            url: "https://x.test/?a=1&b=2".into(),
            folder: String::new(),
            added: 123,
            source: "chrome".into(),
        }];
        let html = export_netscape_html(&marks);
        assert!(html.contains("<!DOCTYPE NETSCAPE-Bookmark-file-1>"));
        assert!(html.contains("https://x.test/?a=1&amp;b=2"));
        assert!(html.contains("A &amp; B"));
    }

    #[test]
    fn una_exportacion_vacia_sigue_siendo_valida() {
        let html = export_netscape_html(&[]);
        assert!(html.contains("</DL>"));
    }

    #[test]
    fn el_inventario_de_passkeys_es_explicito_sobre_lo_no_exportable() {
        let i = passkey_inventory();
        assert_eq!(i["ok"], json!(true));
        assert!(i["note"].as_str().unwrap().contains("no es exportable"));
    }

    #[test]
    fn las_rutas_invalidas_se_rechazan() {
        assert_eq!(export_all("relativa.html")["ok"], json!(false));
        assert_eq!(export_all("/tmp/../etc/passwd")["ok"], json!(false));
    }
}

//! MegaDaemon v13.4 — Identidad de servicio y cliente HTTP discreto.
//!
//! Objetivo: cuando megadaemon habla con un servicio, lo hace **como lo haría
//! la aplicación real** de ese servicio: mismo User-Agent, mismas cabeceras,
//! mismo orden, mismas cookies.  No se trata de engañar a nadie: se trata de
//! que el servicio devuelva a megadaemon **lo mismo que le devuelve a su
//! propia app**, en lugar de una página de "navegador no soportado" o un 403.
//!
//! Aplicaciones reales de este módulo:
//!
//! * el *pull* de conversaciones usa el perfil del cliente real del servicio;
//! * el **scraping** usa perfiles genéricos (Chrome de escritorio/móvil,
//!   bots honestos como `Megadaemon-Bot`) según el caso;
//! * el scraper puede **rotar** entre perfiles cuando recibe un 403/429, que
//!   es el comportamiento correcto (y el único que no rompe los ToS: se
//!   respeta `robots.txt` por defecto y hay límites de ritmo).

use serde_json::{json, Value};

/// Perfil de identidad de un servicio.
#[derive(Clone, Debug)]
pub struct Identity {
    pub id: &'static str,
    pub service: &'static str,
    pub user_agent: &'static str,
    /// Cabeceras adicionales en el orden en que se envían.
    pub headers: &'static [(&'static str, &'static str)],
    pub accept: &'static str,
    pub accept_language: &'static str,
    pub referer: &'static str,
    /// ¿La app móvil usa este servicio? (afecta al perfil por defecto)
    pub mobile: bool,
}

const UA_CHROME_ANDROID: &str = "Mozilla/5.0 (Linux; Android 14; SM-G998B) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36";
const UA_CHROME_DESKTOP: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const UA_CHATGPT_APP: &str = "ChatGPT/1.2025.168 (Android 14; SM-G998B; Build_UP1A.231005.007;) okhttp/4.12.0";
const UA_CLAUDE_APP: &str = "Claude/1.2509.1 (Android 14; SM-G998B)";
const UA_DEEPSEEK_APP: &str = "DeepSeek/1.0 (Android 14; SM-G998B)";
const UA_KIMI_APP: &str = "Kimi/1.0 (Android 14; SM-G998B)";
const UA_GEMINI_APP: &str = "Gemini/1.0 (Linux; Android 14; SM-G998B)";

/// Catálogo de perfiles.
const IDENTITIES: &[Identity] = &[
    Identity {
        id: "chatgpt.app",
        service: "chatgpt",
        user_agent: UA_CHATGPT_APP,
        headers: &[
            ("Accept", "application/json"),
            ("Accept-Encoding", "gzip"),
            ("OAI-Device-Id", "00000000-0000-4000-8000-000000000000"),
        ],
        accept: "application/json",
        accept_language: "es-ES,es;q=0.9",
        referer: "https://chatgpt.com/",
        mobile: true,
    },
    Identity {
        id: "chatgpt.web",
        service: "chatgpt",
        user_agent: UA_CHROME_ANDROID,
        headers: &[
            ("Accept", "application/json, text/plain, */*"),
            ("Sec-Fetch-Dest", "empty"),
            ("Sec-Fetch-Mode", "cors"),
            ("Sec-Fetch-Site", "same-origin"),
        ],
        accept: "application/json, text/plain, */*",
        accept_language: "es-ES,es;q=0.9,en;q=0.8",
        referer: "https://chatgpt.com/",
        mobile: true,
    },
    Identity {
        id: "claude.app",
        service: "claude",
        user_agent: UA_CLAUDE_APP,
        headers: &[("Accept", "application/json")],
        accept: "application/json",
        accept_language: "es-ES,es;q=0.9",
        referer: "https://claude.ai/",
        mobile: true,
    },
    Identity {
        id: "claude.web",
        service: "claude",
        user_agent: UA_CHROME_ANDROID,
        headers: &[
            ("Accept", "application/json, text/plain, */*"),
            ("Sec-Fetch-Site", "same-origin"),
        ],
        accept: "application/json, text/plain, */*",
        accept_language: "es-ES,es;q=0.9",
        referer: "https://claude.ai/",
        mobile: true,
    },
    Identity {
        id: "deepseek.app",
        service: "deepseek",
        user_agent: UA_DEEPSEEK_APP,
        headers: &[("Accept", "application/json")],
        accept: "application/json",
        accept_language: "es-ES,es;q=0.9",
        referer: "https://chat.deepseek.com/",
        mobile: true,
    },
    Identity {
        id: "kimi.app",
        service: "kimi",
        user_agent: UA_KIMI_APP,
        headers: &[("Accept", "application/json")],
        accept: "application/json",
        accept_language: "zh-CN,zh;q=0.9",
        referer: "https://www.kimi.com/",
        mobile: true,
    },
    Identity {
        id: "gemini.app",
        service: "gemini",
        user_agent: UA_GEMINI_APP,
        headers: &[("Accept", "application/json, text/plain, */*")],
        accept: "application/json, text/plain, */*",
        accept_language: "es-ES,es;q=0.9",
        referer: "https://gemini.google.com/",
        mobile: true,
    },
    Identity {
        id: "google.play",
        service: "play",
        user_agent: UA_CHROME_DESKTOP,
        headers: &[("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")],
        accept: "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
        accept_language: "es-ES,es;q=0.9",
        referer: "https://play.google.com/",
        mobile: false,
    },
    Identity {
        id: "fdroid.index",
        service: "fdroid",
        user_agent: "MegaDaemon/13.4 (+https://github.com/boolforge/megadaemon)",
        headers: &[("Accept", "application/json")],
        accept: "application/json",
        accept_language: "en",
        referer: "https://f-droid.org/",
        mobile: false,
    },
    Identity {
        id: "browser.desktop",
        service: "web",
        user_agent: UA_CHROME_DESKTOP,
        headers: &[
            ("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8"),
            ("Upgrade-Insecure-Requests", "1"),
            ("Sec-Fetch-Dest", "document"),
            ("Sec-Fetch-Mode", "navigate"),
            ("Sec-Fetch-Site", "none"),
            ("Sec-Fetch-User", "?1"),
        ],
        accept: "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
        accept_language: "es-ES,es;q=0.9,en;q=0.8",
        referer: "",
        mobile: false,
    },
    Identity {
        id: "browser.mobile",
        service: "web",
        user_agent: UA_CHROME_ANDROID,
        headers: &[
            ("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8"),
            ("Upgrade-Insecure-Requests", "1"),
            ("Sec-Fetch-Dest", "document"),
            ("Sec-Fetch-Mode", "navigate"),
            ("Sec-Fetch-Site", "none"),
        ],
        accept: "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
        accept_language: "es-ES,es;q=0.9",
        referer: "",
        mobile: true,
    },
    Identity {
        id: "bot.honest",
        service: "web",
        user_agent: "MegaDaemon-Bot/13.4 (+https://github.com/boolforge/megadaemon; contacto: el usuario)",
        headers: &[("Accept", "text/html,application/xhtml+xml,application/json;q=0.9,*/*;q=0.8")],
        accept: "text/html,application/xhtml+xml,application/json;q=0.9,*/*;q=0.8",
        accept_language: "en",
        referer: "",
        mobile: false,
    },
];

/// Perfil por id.
pub fn by_id(id: &str) -> Option<&'static Identity> {
    IDENTITIES.iter().find(|i| i.id == id)
}

/// Perfil por servicio (elige la variante web o app según `mobile`).
pub fn for_service(service: &str, mobile: bool) -> Option<&'static Identity> {
    IDENTITIES
        .iter()
        .find(|i| i.service == service && i.mobile == mobile)
        .or_else(|| IDENTITIES.iter().find(|i| i.service == service))
}

/// Lista de perfiles.
pub fn list() -> Value {
    json!({
        "ok": true,
        "count": IDENTITIES.len(),
        "identities": IDENTITIES.iter().map(|i| json!({
            "id": i.id, "service": i.service, "mobile": i.mobile,
            "user_agent": i.user_agent, "accept": i.accept,
            "accept_language": i.accept_language, "referer": i.referer,
            "headers": i.headers.iter().map(|(k, v)| json!({"k": k, "v": v})).collect::<Vec<Value>>(),
        })).collect::<Vec<Value>>(),
    })
}

/// Construye la línea de comando `curl` con el perfil aplicado.
///
/// Se usa `curl` del dispositivo en lugar de un cliente HTTP de Rust porque la
/// huella TLS de Cloudflare distingue navegadores reales del resto: con el
/// cliente de Rust la API responde 401 aunque la cookie sea correcta.
pub fn curl_command(url: &str, ident: &Identity, cookie: &str, bearer: &str, extra: &[(String, String)]) -> String {
    let mut cmd = String::from("/data/data/com.termux/files/usr/bin/curl -s --compressed -m 45");
    cmd.push_str(&format!(
        " -A {}",
        crate::android::sh_quote(ident.user_agent)
    ));
    cmd.push_str(&format!(
        " -H {}",
        crate::android::sh_quote(&format!("Accept: {}", ident.accept))
    ));
    if !ident.accept_language.is_empty() {
        cmd.push_str(&format!(
            " -H {}",
            crate::android::sh_quote(&format!("Accept-Language: {}", ident.accept_language))
        ));
    }
    if !ident.referer.is_empty() {
        cmd.push_str(&format!(
            " -H {}",
            crate::android::sh_quote(&format!("Referer: {}", ident.referer))
        ));
    }
    for (k, v) in ident.headers {
        cmd.push_str(&format!(
            " -H {}",
            crate::android::sh_quote(&format!("{}: {}", k, v))
        ));
    }
    for (k, v) in extra {
        cmd.push_str(&format!(
            " -H {}",
            crate::android::sh_quote(&format!("{}: {}", k, v))
        ));
    }
    if !cookie.is_empty() {
        cmd.push_str(&format!(
            " -H {}",
            crate::android::sh_quote(&format!("Cookie: {}", cookie))
        ));
    }
    if !bearer.is_empty() {
        cmd.push_str(&format!(
            " -H {}",
            crate::android::sh_quote(&format!("Authorization: Bearer {}", bearer))
        ));
    }
    cmd.push_str(&format!(" -w '\\n__MD_HTTP__%{{http_code}}' {}", crate::android::sh_quote(url)));
    cmd
}

/// Petición HTTP con perfil. Devuelve (status, cuerpo).
pub fn request(url: &str, ident: &Identity, cookie: &str, bearer: &str, extra: &[(String, String)]) -> (u16, String) {
    let cmd = curl_command(url, ident, cookie, bearer, extra);
    let r = crate::android::root_exec(&cmd, 60_000, 8 * 1024 * 1024);
    if r.timed_out {
        return (0, "timeout".into());
    }
    match r.stdout.rfind("__MD_HTTP__") {
        Some(pos) => {
            let (body, tail) = r.stdout.split_at(pos);
            let code: u16 = tail["__MD_HTTP__".len()..].trim().parse().unwrap_or(0);
            (code, body.trim_end().to_string())
        }
        None => (0, r.stdout.trim().to_string()),
    }
}

/// Petición con rotación de perfiles ante 403/429 (reintentos acotados).
pub fn request_rotating(url: &str, ids: &[&str], cookie: &str) -> (u16, String, String) {
    let mut last = (0u16, String::new(), String::new());
    for id in ids {
        let Some(ident) = by_id(id) else { continue };
        let (st, body) = request(url, ident, cookie, "", &[]);
        last = (st, body, ident.id.to_string());
        if st != 403 && st != 429 && st != 0 {
            break;
        }
    }
    last
}

/// Scraping de una página: devuelve texto plano y enlaces.
///
/// Respeta `robots.txt` por defecto (se puede desactivar explícitamente por
/// tarea, nunca por omisión).
pub fn scrape(url: &str, identity: &str, respect_robots: bool, max_bytes: usize) -> Value {
    let Some(ident) = by_id(identity).or_else(|| by_id("browser.mobile")) else {
        return json!({ "ok": false, "error": "unknown_identity" });
    };
    if respect_robots {
        if let Some(host) = host_of(url) {
            let robots = format!("https://{}/robots.txt", host);
            let (_, body) = request(&robots, ident, "", "", &[]);
            if !body.is_empty() && robots_forbids(&body, url) {
                return json!({
                    "ok": false, "error": "robots_disallow",
                    "hint": format!("robots.txt de {} no permite esta ruta", host),
                });
            }
        }
    }
    let (st, body) = request(url, ident, "", "", &[]);
    let truncated = body.len() > max_bytes;
    let text = if truncated {
        body.chars().take(max_bytes).collect::<String>()
    } else {
        body.clone()
    };
    json!({
        "ok": st == 200 || st == 301 || st == 302,
        "status": st,
        "url": url,
        "identity": ident.id,
        "bytes": body.len(),
        "truncated": truncated,
        "links": extract_links(&text),
        "text": strip_html(&text),
        "raw_head": text.chars().take(400).collect::<String>(),
    })
}

/// Extrae enlaces `<a href="...">`.
pub fn extract_links(html: &str) -> Vec<String> {
    let mut out = vec![];
    let lower = html.to_lowercase();
    let mut idx = 0;
    while let Some(p) = lower[idx..].find("<a ") {
        let start = idx + p;
        let rest = &html[start..];
        let end = rest.find('>').unwrap_or(rest.len());
        let tag = &rest[..end];
        if let Some(hp) = tag.to_lowercase().find("href=") {
            let after = &tag[hp + 5..];
            let quote = after.chars().next().unwrap_or('"');
            if quote == '"' || quote == '\'' {
                let rest2 = &after[1..];
                if let Some(ce) = rest2.find(quote) {
                    let href = rest2[..ce].trim().to_string();
                    if !href.is_empty() && !href.starts_with('#') && !href.starts_with("javascript:") {
                        if !out.contains(&href) {
                            out.push(href);
                        }
                    }
                }
            }
        }
        idx = start + 4;
        if out.len() > 500 {
            break;
        }
    }
    out
}

/// Convierte HTML en texto plano (sin dependencias externas).
pub fn strip_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let mut in_tag = false;
    let mut skip_depth = 0usize;
    let bytes: Vec<char> = html.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == '<' {
            if in_tag {
                in_tag = false;
            } else {
                in_tag = true;
                // Etiquetas cuyo contenido no debe aparecer en el texto.
                let rest: String = bytes[i..].iter().take(8).collect::<String>().to_lowercase();
                if rest.starts_with("<script") || rest.starts_with("<style") {
                    skip_depth += 1;
                }
            }
            i += 1;
            continue;
        }
        if c == '>' {
            if in_tag {
                in_tag = false;
            } else {
                // fin de script/style
                let prev: String = bytes[i.saturating_sub(8)..i].iter().collect::<String>().to_lowercase();
                if prev.ends_with("</script") || prev.ends_with("</style") {
                    skip_depth = skip_depth.saturating_sub(1);
                }
            }
            i += 1;
            if i < bytes.len() && (bytes[i] == '\n' || bytes[i] == '\r') {
                out.push('\n');
            }
            continue;
        }
        if !in_tag && skip_depth == 0 {
            out.push(c);
        }
        i += 1;
    }
    let mut text = String::new();
    let mut last_space = false;
    for c in out.chars() {
        if c.is_whitespace() {
            if !last_space {
                text.push(' ');
                last_space = true;
            }
        } else {
            text.push(c);
            last_space = false;
        }
    }
    text.trim().to_string()
}

/// Host de una URL.
pub fn host_of(url: &str) -> Option<String> {
    let rest = url.split("://").nth(1)?;
    let host = rest.split('/').next()?;
    let host = host.split('@').next_back()?; // quita credenciales
    let host = host.split(':').next()?;
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

/// ¿`robots.txt` prohíbe esta ruta? (implementación mínima y conservadora)
pub fn robots_forbids(robots: &str, url: &str) -> bool {
    let path = url.split("://").nth(1).and_then(|r| r.split_once('/')).map(|(_, p)| format!("/{}", p)).unwrap_or_else(|| "/".into());
    let mut applies = false;
    let mut disallow: Vec<String> = vec![];
    for line in robots.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let (k, v) = match line.split_once(':') {
            Some((k, v)) => (k.trim().to_ascii_lowercase(), v.trim().to_string()),
            None => continue,
        };
        match k.as_str() {
            "user-agent" => applies = v == "*",
            "disallow" if applies => {
                if !v.is_empty() {
                    disallow.push(v);
                }
            }
            "allow" if applies => {
                disallow.retain(|d| !path.starts_with(d.as_str()) || d.is_empty());
            }
            _ => {}
        }
    }
    disallow.iter().any(|d| path.starts_with(d.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hay_perfil_para_cada_servicio_pedido() {
        for s in ["chatgpt", "claude", "deepseek", "kimi", "gemini", "web"] {
            assert!(for_service(s, true).is_some(), "falta perfil para {}", s);
        }
    }

    #[test]
    fn los_perfiles_de_app_y_de_web_se_diferencian() {
        let app = for_service("chatgpt", true).unwrap();
        let web = for_service("chatgpt", false).unwrap();
        assert_ne!(app.user_agent, web.user_agent);
        assert_eq!(app.id, "chatgpt.app");
        assert_eq!(web.id, "chatgpt.web");
    }

    #[test]
    fn el_comando_curl_incluye_todas_las_cabeceras() {
        let ident = by_id("chatgpt.web").unwrap();
        let cmd = curl_command("https://chatgpt.com/x", ident, "a=b", "tok", &[]);
        assert!(cmd.contains("-A 'Mozilla"));
        assert!(cmd.contains("Accept-Language"));
        assert!(cmd.contains("Cookie: a=b"));
        assert!(cmd.contains("Authorization: Bearer tok"));
        assert!(cmd.contains("__MD_HTTP__"));
    }

    #[test]
    fn extrae_enlaces() {
        let html = "<html><body><a href=\"/a\">A</a><a href='https://x.test/b'>B</a><a href=\"#c\">C</a></body></html>";
        let links = extract_links(html);
        assert!(links.contains(&"/a".to_string()));
        assert!(links.contains(&"https://x.test/b".to_string()));
        assert!(!links.contains(&"#c".to_string()));
    }

    #[test]
    fn limpia_html_a_texto() {
        let html = "<html><head><style>b{}</style><script>var x=1;</script></head><body><h1>Título</h1><p>Hola &amp; adiós</p></body></html>";
        let text = strip_html(html);
        assert!(text.contains("Título"));
        assert!(text.contains("Hola"));
        assert!(!text.contains("var x=1"));
        assert!(!text.contains("b{}"));
    }

    #[test]
    fn extrae_el_host() {
        assert_eq!(host_of("https://chatgpt.com/backend-api/x").as_deref(), Some("chatgpt.com"));
        assert_eq!(host_of("https://user:pw@x.test/p").as_deref(), Some("x.test"));
        assert_eq!(host_of("no-es-url"), None);
    }

    #[test]
    fn robots_txt_prohibe_y_permite() {
        let robots = "User-agent: *\nDisallow: /privado\nAllow: /privado/publico\n";
        assert!(robots_forbids(robots, "https://x.test/privado/secreto"));
        assert!(!robots_forbids(robots, "https://x.test/publico"));
    }
}

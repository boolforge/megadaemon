//! MegaDaemon v15 — **Conector de GitHub**: los repos del usuario como
//! fuente de trabajo del enjambre.
//!
//! # Qué hace
//!
//! * **lista** los repos del usuario (`GET /user/repos`),
//! * **descarga** un repo (tarball por la API, sin `git clone`),
//! * **lee** issues y PRs (`GET /repos/{repo}/issues`, `/pulls`),
//! * **publica el estado del enjambre** como issue o como PR.
//!
//! # Reglas duras
//!
//! * **Ningún secreto en el request que se registra ni en el catálogo.** El
//!   token **nunca** está en la struct [`HttpRequest`]: se lee del fichero
//!   justo antes de enviar y se inyecta en la cabecera.  Si alguien imprime
//!   el request, no ve la credencial.
//! * **Nada se publica sin `confirm = true`**: abrir una issue o un PR en el
//!   repo del usuario es visible para el mundo.
//! * **El cuerpo publicado va saneado**: si el estado del enjambre contuviera
//!   algo con forma de token, [`scrub_secrets`] lo enmascara antes de
//!   subirlo.
//! * **HTTP inyectable**: las funciones puras (construir request, parsear
//!   respuesta, formatear el informe) se testean sin red; el I/O real pasa
//!   por un [`HttpClient`].

pub use crate::util_text::now_iso;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use serde_json::{json, Value};

/// Base por defecto de la API de GitHub.
pub const GITHUB_API: &str = "https://api.github.com";

/// User-Agent que identifica al daemon ante GitHub (obligatorio en la API).
pub const USER_AGENT: &str = "MegaDaemon/15 (+https://github.com/boolforge/megadaemon)";

/// Ruta por defecto del fichero con el token de GitHub en el móvil.
///
/// Es una **ruta**, no el token: el fichero se lee (0600) justo antes de
/// enviar y nunca se copia al catálogo, al tablón ni a las peticiones.
pub const DEFAULT_TOKEN_PATH: &str = "/data/user/0/com.agy/files/megadaemon/state/github.token";

/// Ruta del token en el state dir **canónico** (persistente en el móvil).
///
/// MEDIDO (2026-11-23): el token vivía en el directorio de la app; el
/// state dir canónico (en el móvil `/data/adb/megadaemon/state`) es la ruta
/// persistente que usa el resto del daemon.  La ruta antigua se conserva
/// como alternativa de lectura.
pub fn token_path_por_defecto() -> PathBuf {
    PathBuf::from(crate::state_dir::state_dir()).join("github.token")
}

/// Candidatas para LEER el token: la indicada, la canónica y la heredada.
pub fn token_candidates(explicit: &str) -> Vec<String> {
    let mut v = vec![explicit.to_string()];
    let canon = token_path_por_defecto().to_string_lossy().to_string();
    if !v.contains(&canon) {
        v.push(canon);
    }
    let legado = DEFAULT_TOKEN_PATH.to_string();
    if !v.contains(&legado) {
        v.push(legado);
    }
    v
}

/// Configuración del conector.
///
/// `token_ref` es la **ruta** del fichero con el token (0600).  Nunca el
/// token: si en algún momento se cuela uno en la config, `scrub_secrets` lo
/// limpia antes de publicarlo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GitHubConfig {
    /// Base de la API (`https://api.github.com`).
    pub api_base: String,
    /// Ruta del fichero con el token.
    pub token_ref: String,
    /// User-Agent.
    pub user_agent: String,
    /// Repo por defecto (`owner/name`).
    pub default_repo: String,
}

impl Default for GitHubConfig {
    fn default() -> Self {
        GitHubConfig {
            api_base: GITHUB_API.to_string(),
            token_ref: "md-secrets/github.token".to_string(),
            user_agent: USER_AGENT.to_string(),
            default_repo: String::new(),
        }
    }
}

impl GitHubConfig {
    /// Config con la ruta real del secreto en el móvil.
    pub fn with_token_ref(token_ref: &str) -> GitHubConfig {
        GitHubConfig {
            token_ref: token_ref.to_string(),
            ..Default::default()
        }
    }
}

/// Un repo del usuario.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Repo {
    pub full_name: String,
    pub description: String,
    pub private: bool,
    pub default_branch: String,
    pub html_url: String,
    pub clone_url: String,
    pub language: String,
    pub stars: u32,
    pub forks: u32,
    pub open_issues: u32,
    pub size_kb: u64,
    pub updated_at: String,
    pub pushed_at: String,
    pub archived: bool,
}

/// Una issue o un PR (misma forma en la API de GitHub: los PRs también
/// aparecen en `/issues`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Issue {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub body_excerpt: String,
    pub author: String,
    pub labels: Vec<String>,
    pub comments: u32,
    pub created_at: String,
    pub updated_at: String,
    pub url: String,
    /// `true` si el item es un pull request.
    pub is_pr: bool,
}

/// Petición HTTP **sin credenciales**.
///
/// La cabecera `Authorization` la añade el cliente al enviar: la struct no
/// la guarda, así que imprimarla no filtra nada.
#[derive(Clone, Debug, PartialEq)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    /// Cabeceras no sensibles (Accept, Content-Type...).
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

impl HttpRequest {
    /// Representación imprimible (sin secretos por construcción).
    pub fn to_json(&self) -> Value {
        json!({
            "method": self.method,
            "url": self.url,
            "headers": self.headers.iter().map(|(k, v)| json!({"name": k, "value": v})).collect::<Vec<_>>(),
            "body": self.body,
            "has_body": self.body.is_some(),
        })
    }
}

/// Respuesta HTTP.
#[derive(Clone, Debug, PartialEq)]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
    /// Cuerpo binario (tarball).
    pub bytes: Vec<u8>,
}

impl HttpResponse {
    /// Respuesta de texto con el cuerpo duplicado en `bytes`.
    pub fn text(status: u16, body: &str) -> HttpResponse {
        HttpResponse {
            status,
            body: body.to_string(),
            bytes: body.as_bytes().to_vec(),
        }
    }

    /// ¿Fue un 2xx?
    pub fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// Cliente HTTP inyectable (para que los tests no toquen la red).
pub trait HttpClient {
    fn send(&self, req: &HttpRequest) -> Result<HttpResponse, String>;
}

/// Cliente real: `ureq` + token leído del fichero en el momento del envío.
pub struct UreqClient {
    /// Ruta del fichero con el token.
    pub token_path: String,
    /// Timeout en ms.
    pub timeout_ms: u64,
}

impl UreqClient {
    pub fn new(token_path: &str) -> UreqClient {
        UreqClient {
            token_path: token_path.to_string(),
            timeout_ms: 20_000,
        }
    }

    /// Lee el token del fichero (trim).  Si no hay fichero, se sigue **sin
    /// credencial** (las rutas públicas de la API no la necesitan).
    fn token(&self) -> Option<String> {
        // MEDIDO (2026-11-23): el token vivía en el directorio de la app
        // (volátil en cuanto el daemon se muda al prefijo persistente).  Se
        // prueban la ruta indicada, la canónica y la heredada.
        for p in token_candidates(&self.token_path) {
            if let Ok(s) = std::fs::read_to_string(&p) {
                let t = s.trim().to_string();
                if !t.is_empty() {
                    return Some(t);
                }
            }
        }
        None
    }
}

impl HttpClient for UreqClient {
    fn send(&self, req: &HttpRequest) -> Result<HttpResponse, String> {
        let config = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_millis(self.timeout_ms)))
            .timeout_connect(Some(std::time::Duration::from_millis(
                self.timeout_ms.min(10_000),
            )))
            // Los 4xx/5xx llegan como respuesta (con su cuerpo), no como
            // error de transporte: queremos el mensaje de GitHub.
            .http_status_as_error(false)
            .build();
        let agent = ureq::Agent::new_with_config(config);
        let token = self.token();
        let method = req.method.to_ascii_uppercase();

        let resp = match method.as_str() {
            "GET" | "HEAD" | "DELETE" => {
                let mut r = agent.get(&req.url);
                for (k, v) in &req.headers {
                    r = r.header(k, v);
                }
                if let Some(t) = &token {
                    r = r.header("Authorization", &format!("Bearer {}", t));
                }
                r.call().map_err(|e| format!("error de red: {}", e))?
            }
            "PUT" => {
                let mut r = agent.put(&req.url);
                for (k, v) in &req.headers {
                    r = r.header(k, v);
                }
                if let Some(t) = &token {
                    r = r.header("Authorization", &format!("Bearer {}", t));
                }
                let body = req.body.clone().unwrap_or_default();
                r.send(body.as_bytes())
                    .map_err(|e| format!("error de red: {}", e))?
            }
            _ => {
                let mut r = agent.post(&req.url);
                for (k, v) in &req.headers {
                    r = r.header(k, v);
                }
                if let Some(t) = &token {
                    r = r.header("Authorization", &format!("Bearer {}", t));
                }
                let body = req.body.clone().unwrap_or_default();
                r.send(body.as_bytes())
                    .map_err(|e| format!("error de red: {}", e))?
            }
        };
        let status = resp.status().as_u16();
        let bytes = resp
            .into_body()
            .read_to_vec()
            .map_err(|e| format!("lectura fallida: {}", e))?;
        let body = String::from_utf8_lossy(&bytes).to_string();
        Ok(HttpResponse {
            status,
            body,
            bytes,
        })
    }
}

/// Cliente de mentira (tests): devuelve respuestas en cola.
pub struct MockClient {
    pub responses: std::cell::RefCell<Vec<HttpResponse>>,
    pub requests: std::cell::RefCell<Vec<HttpRequest>>,
}

impl MockClient {
    pub fn new(responses: Vec<HttpResponse>) -> MockClient {
        MockClient {
            responses: std::cell::RefCell::new(responses),
            requests: std::cell::RefCell::new(Vec::new()),
        }
    }
}

impl HttpClient for MockClient {
    fn send(&self, req: &HttpRequest) -> Result<HttpResponse, String> {
        self.requests.borrow_mut().push(req.clone());
        let mut q = self.responses.borrow_mut();
        if q.is_empty() {
            return Err("sin respuestas en cola".into());
        }
        Ok(q.remove(0))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Peticiones
// ─────────────────────────────────────────────────────────────────────────────

fn api_headers(cfg: &GitHubConfig) -> Vec<(String, String)> {
    vec![
        ("Accept".to_string(), "application/vnd.github+json".to_string()),
        ("X-GitHub-Api-Version".to_string(), "2022-11-28".to_string()),
        ("User-Agent".to_string(), cfg.user_agent.clone()),
    ]
}

/// `GET /user/repos` — los repos del usuario.
pub fn list_repos_request(cfg: &GitHubConfig, page: u32, per_page: u32) -> HttpRequest {
    HttpRequest {
        method: "GET".into(),
        url: format!(
            "{}/user/repos?per_page={}&page={}&sort=updated",
            cfg.api_base.trim_end_matches('/'),
            per_page.clamp(1, 100),
            page.max(1)
        ),
        headers: api_headers(cfg),
        body: None,
    }
}

/// `GET /repos/{repo}/issues` — issues (y PRs: la API los mezcla).
pub fn list_issues_request(
    cfg: &GitHubConfig,
    repo: &str,
    state: &str,
    page: u32,
    per_page: u32,
) -> HttpRequest {
    HttpRequest {
        method: "GET".into(),
        url: format!(
            "{}/repos/{}/issues?state={}&per_page={}&page={}",
            cfg.api_base.trim_end_matches('/'),
            repo,
            state,
            per_page.clamp(1, 100),
            page.max(1)
        ),
        headers: api_headers(cfg),
        body: None,
    }
}

/// `GET /repos/{repo}/pulls` — pull requests.
pub fn list_prs_request(
    cfg: &GitHubConfig,
    repo: &str,
    state: &str,
    page: u32,
    per_page: u32,
) -> HttpRequest {
    HttpRequest {
        method: "GET".into(),
        url: format!(
            "{}/repos/{}/pulls?state={}&per_page={}&page={}",
            cfg.api_base.trim_end_matches('/'),
            repo,
            state,
            per_page.clamp(1, 100),
            page.max(1)
        ),
        headers: api_headers(cfg),
        body: None,
    }
}

/// `GET /repos/{repo}/tarball/{ref}` — descarga el repo sin `git clone`.
pub fn download_repo_request(cfg: &GitHubConfig, repo: &str, git_ref: &str) -> HttpRequest {
    HttpRequest {
        method: "GET".into(),
        url: format!(
            "{}/repos/{}/tarball/{}",
            cfg.api_base.trim_end_matches('/'),
            repo,
            if git_ref.trim().is_empty() {
                "HEAD"
            } else {
                git_ref.trim()
            }
        ),
        headers: api_headers(cfg),
        body: None,
    }
}

/// `POST /repos/{repo}/issues` — abrir una issue.
pub fn create_issue_request(
    cfg: &GitHubConfig,
    repo: &str,
    title: &str,
    body: &str,
) -> HttpRequest {
    let mut headers = api_headers(cfg);
    headers.push(("Content-Type".to_string(), "application/json".to_string()));
    HttpRequest {
        method: "POST".into(),
        url: format!("{}/repos/{}/issues", cfg.api_base.trim_end_matches('/'), repo),
        headers,
        body: Some(
            json!({
                "title": scrub_secrets(title),
                "body": scrub_secrets(body),
            })
            .to_string(),
        ),
    }
}

/// `POST /repos/{repo}/pulls` — abrir un pull request.
pub fn create_pr_request(
    cfg: &GitHubConfig,
    repo: &str,
    title: &str,
    body: &str,
    head: &str,
    base: &str,
) -> HttpRequest {
    let mut headers = api_headers(cfg);
    headers.push(("Content-Type".to_string(), "application/json".to_string()));
    HttpRequest {
        method: "POST".into(),
        url: format!("{}/repos/{}/pulls", cfg.api_base.trim_end_matches('/'), repo),
        headers,
        body: Some(
            json!({
                "title": scrub_secrets(title),
                "body": scrub_secrets(body),
                "head": head,
                "base": base,
            })
            .to_string(),
        ),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Parseo
// ─────────────────────────────────────────────────────────────────────────────

/// Convierte la respuesta de `/user/repos` en [`Repo`].
pub fn parse_repos(body: &str) -> Result<Vec<Repo>, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| format!("JSON inválido: {}", e))?;
    let arr = v
        .as_array()
        .ok_or_else(|| "la respuesta no es un array".to_string())?;
    let mut out = Vec::new();
    for r in arr {
        out.push(Repo {
            full_name: str_of(r, "full_name"),
            description: str_of(r, "description"),
            private: r.get("private").and_then(|x| x.as_bool()).unwrap_or(false),
            default_branch: str_of(r, "default_branch"),
            html_url: str_of(r, "html_url"),
            clone_url: str_of(r, "clone_url"),
            language: str_of(r, "language"),
            stars: r.get("stargazers_count").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
            forks: r.get("forks_count").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
            open_issues: r.get("open_issues_count").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
            size_kb: r.get("size").and_then(|x| x.as_u64()).unwrap_or(0),
            updated_at: str_of(r, "updated_at"),
            pushed_at: str_of(r, "pushed_at"),
            archived: r.get("archived").and_then(|x| x.as_bool()).unwrap_or(false),
        });
    }
    Ok(out)
}

/// Convierte la respuesta de `/issues` o `/pulls` en [`Issue`].
///
/// La API de issues incluye los PRs (llevan `"pull_request"` en el objeto):
/// aquí se marcan con `is_pr` y se pueden filtrar.
pub fn parse_issues(body: &str, only_prs: bool) -> Result<Vec<Issue>, String> {
    let v: Value = serde_json::from_str(body).map_err(|e| format!("JSON inválido: {}", e))?;
    let arr = v
        .as_array()
        .ok_or_else(|| "la respuesta no es un array".to_string())?;
    let mut out = Vec::new();
    for it in arr {
        let is_pr = it.get("pull_request").is_some();
        if only_prs != is_pr {
            continue;
        }
        let labels = it
            .get("labels")
            .and_then(|x| x.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|l| {
                        l.get("name")
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string())
                            .or_else(|| l.as_str().map(|s| s.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.push(Issue {
            number: it.get("number").and_then(|x| x.as_u64()).unwrap_or(0),
            title: str_of(it, "title"),
            state: str_of(it, "state"),
            body_excerpt: excerpt(&str_of(it, "body"), 400),
            author: it
                .get("user")
                .and_then(|u| u.get("login"))
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            labels,
            comments: it.get("comments").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
            created_at: str_of(it, "created_at"),
            updated_at: str_of(it, "updated_at"),
            url: str_of(it, "html_url"),
            is_pr,
        });
    }
    Ok(out)
}

fn str_of(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string()
}

fn excerpt(s: &str, n: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= n {
        t.to_string()
    } else {
        t.chars().take(n).collect::<String>() + "…"
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Publicación del estado del enjambre
// ─────────────────────────────────────────────────────────────────────────────

/// Cómo se publica el estado.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PublishKind {
    /// Abrir una issue.
    Issue,
    /// Abrir un pull request (el commit lo pone `git_ops`).
    PullRequest {
        /// Rama con el commit (en el fork o en el repo).
        head: String,
        /// Rama destino.
        base: String,
    },
}

/// Construye el informe Markdown del estado del enjambre.
///
/// `state` es el snapshot del tablón (`mesh::Mesh::snapshot()` o
/// `coordination::TaskBoard::snapshot()`): tareas, agentes, peers.  La
/// función es **pura**: no toca disco ni red, y el texto sale saneado.
pub fn swarm_state_markdown(state: &Value, title: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", title.trim()));
    out.push_str(&format!(
        "_generado el {} por MegaDaemon (autónomo)_\n\n",
        now_iso()
    ));

    // -- tareas -------------------------------------------------------------
    let tasks = state
        .get("tasks")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut done = 0usize;
    let mut open = 0usize;
    let mut lines: Vec<String> = Vec::new();
    for t in &tasks {
        let id = t
            .get("task_id")
            .or_else(|| t.get("id"))
            .and_then(|x| x.as_str())
            .unwrap_or("?");
        let st = t.get("state").and_then(|x| x.as_str()).unwrap_or("?");
        let title = t
            .get("title")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .trim();
        let cap = t
            .get("capability")
            .and_then(|x| x.as_str())
            .unwrap_or("");
        let owner = t
            .get("claimed_by")
            .and_then(|x| x.as_str())
            .unwrap_or("");
        if st == "done" || st == "completed" {
            done += 1;
        } else {
            open += 1;
        }
        lines.push(format!(
            "| `{}` | {} | {} | {} | {} |",
            id,
            st,
            cap,
            if owner.is_empty() { "-" } else { owner },
            title.replace('|', "\\|")
        ));
    }
    out.push_str(&format!(
        "## Tareas ({} abiertas / {} completadas)\n\n",
        open, done
    ));
    if lines.is_empty() {
        out.push_str("_No hay tareas en el tablón._\n\n");
    } else {
        out.push_str("| id | estado | capacidad | dueño | título |\n|---|---|---|---|---|\n");
        for l in lines {
            out.push_str(&l);
            out.push('\n');
        }
        out.push('\n');
    }

    // -- agentes ------------------------------------------------------------
    let agents = state
        .get("agents")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if !agents.is_empty() {
        out.push_str("## Agentes\n\n| agente | rol | estado | tarea | detalle |\n|---|---|---|---|---|\n");
        for a in &agents {
            let id = a
                .get("agent")
                .or_else(|| a.get("node_id"))
                .or_else(|| a.get("id"))
                .and_then(|x| x.as_str())
                .unwrap_or("?");
            let role = a.get("role").and_then(|x| x.as_str()).unwrap_or("-");
            let st = a.get("status").and_then(|x| x.as_str()).unwrap_or("-");
            let task = a
                .get("task")
                .or_else(|| a.get("task_id"))
                .and_then(|x| x.as_str())
                .unwrap_or("-");
            let detail = a
                .get("detail")
                .or_else(|| a.get("note"))
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .replace('|', "\\|");
            out.push_str(&format!(
                "| `{}` | {} | {} | {} | {} |\n",
                id, role, st, task, detail
            ));
        }
        out.push('\n');
    }

    // -- peers --------------------------------------------------------------
    let peers = state
        .get("peers")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if !peers.is_empty() {
        out.push_str("## Nodos\n\n| nodo | instancia | capacidades | último contacto |\n|---|---|---|---|\n");
        for p in &peers {
            let id = p.get("id").and_then(|x| x.as_str()).unwrap_or("?");
            let inst = p.get("hostname").and_then(|x| x.as_str()).unwrap_or("-");
            let caps = p
                .get("capabilities")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|c| c.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            let seen = p.get("last_seen").and_then(|x| x.as_i64()).unwrap_or(0);
            out.push_str(&format!(
                "| `{}` | {} | {} | {} |\n",
                id,
                inst,
                caps.replace('|', "\\|"),
                if seen == 0 {
                    "-".to_string()
                } else {
                    seen.to_string()
                }
            ));
        }
        out.push('\n');
    }

    scrub_secrets(&out)
}

/// Publica el estado del enjambre.
///
/// **Exige `confirm = true`**: publicar en el repo del usuario es visible.
/// Con `confirm = false` devuelve el `dry_run` (lo que se publicaría) sin
/// tocar la red.
pub fn publish_swarm_state(
    cfg: &GitHubConfig,
    repo: &str,
    state: &Value,
    kind: &PublishKind,
    title: &str,
    confirm: bool,
    client: &dyn HttpClient,
) -> Result<Value, String> {
    let repo = if repo.trim().is_empty() {
        cfg.default_repo.clone()
    } else {
        repo.trim().to_string()
    };
    if repo.is_empty() {
        return Err("no hay repo indicado".into());
    }
    let body = swarm_state_markdown(state, title);
    let req = match kind {
        PublishKind::Issue => create_issue_request(cfg, &repo, title, &body),
        PublishKind::PullRequest { head, base } => {
            create_pr_request(cfg, &repo, title, &body, head, base)
        }
    };
    if !confirm {
        return Ok(json!({
            "ok": true,
            "dry_run": true,
            "repo": repo,
            "request": req.to_json(),
            "title": title,
            "body_bytes": body.len(),
        }));
    }
    let resp = client.send(&req)?;
    if !resp.ok() {
        return Err(format!(
            "GitHub respondió {}: {}",
            resp.status,
            excerpt(&resp.body, 300)
        ));
    }
    let v: Value = serde_json::from_str(&resp.body).unwrap_or(Value::Null);
    Ok(json!({
        "ok": true,
        "dry_run": false,
        "repo": repo,
        "kind": match kind {
            PublishKind::Issue => "issue",
            PublishKind::PullRequest { .. } => "pull_request",
        },
        "url": v.get("html_url").and_then(|x| x.as_str()).unwrap_or(""),
        "number": v.get("number").and_then(|x| x.as_u64()).unwrap_or(0),
        "status": resp.status,
    }))
}

/// Lista los repos del usuario.
pub fn list_repos(
    cfg: &GitHubConfig,
    page: u32,
    per_page: u32,
    client: &dyn HttpClient,
) -> Result<Vec<Repo>, String> {
    let req = list_repos_request(cfg, page, per_page);
    let resp = client.send(&req)?;
    if !resp.ok() {
        return Err(format!(
            "GitHub respondió {}: {}",
            resp.status,
            excerpt(&resp.body, 300)
        ));
    }
    parse_repos(&resp.body)
}

/// Lista issues de un repo (o PRs con `only_prs = true`).
pub fn list_issues(
    cfg: &GitHubConfig,
    repo: &str,
    state: &str,
    only_prs: bool,
    client: &dyn HttpClient,
) -> Result<Vec<Issue>, String> {
    let req = if only_prs {
        list_prs_request(cfg, repo, state, 1, 50)
    } else {
        list_issues_request(cfg, repo, state, 1, 50)
    };
    let resp = client.send(&req)?;
    if !resp.ok() {
        return Err(format!(
            "GitHub respondió {}: {}",
            resp.status,
            excerpt(&resp.body, 300)
        ));
    }
    parse_issues(&resp.body, only_prs)
}

/// Descarga el tarball de un repo a `dest_dir`.
///
/// Devuelve la ruta del fichero escrito y su tamaño.  No usa `git clone`:
/// la API basta y en un móvil con poco disco es mucho más ligero.
pub fn download_repo(
    cfg: &GitHubConfig,
    repo: &str,
    git_ref: &str,
    dest_dir: &str,
    client: &dyn HttpClient,
) -> Result<Value, String> {
    let req = download_repo_request(cfg, repo, git_ref);
    let resp = client.send(&req)?;
    if !resp.ok() {
        return Err(format!(
            "GitHub respondió {}: {}",
            resp.status,
            excerpt(&resp.body, 200)
        ));
    }
    let dir = std::path::Path::new(dest_dir);
    std::fs::create_dir_all(dir).map_err(|e| format!("no se pudo crear {}: {}", dest_dir, e))?;
    let safe = repo.replace('/', "_");
    let path = dir.join(format!("{}-{}.tar.gz", safe, now_epoch()));
    std::fs::write(&path, &resp.bytes).map_err(|e| format!("no se pudo escribir: {}", e))?;
    Ok(json!({
        "ok": true,
        "repo": repo,
        "ref": git_ref,
        "path": path.to_string_lossy(),
        "bytes": resp.bytes.len(),
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// Saneado de secretos
// ─────────────────────────────────────────────────────────────────────────────

/// Patrones que delatan una credencial.
const SECRET_PATTERNS: &[&str] = &[
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "ghr_",
    "github_pat_",
    "sk-",
    "mdaemon_sk_",
    "xoxb-",
    "AKIA",
];

/// Enmascara cualquier cosa con **forma** de credencial.
///
/// Se aplica a todo lo que se publica (títulos, cuerpos, informes): mejor
/// un `***` que un token filtrado en un repo público.
pub fn scrub_secrets(text: &str) -> String {
    let mut out = text.to_string();
    for p in SECRET_PATTERNS {
        out = replace_token_like(&out, p);
    }
    // Cabeceras de autorización literales.
    for marker in ["Bearer ", "Authorization: ", "authorization: "] {
        if let Some(idx) = out.find(marker) {
            let start = idx + marker.len();
            let end = out[start..]
                .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ',')
                .map(|i| start + i)
                .unwrap_or(out.len());
            if end > start {
                out = format!("{}***{}", &out[..start], &out[end..]);
            }
        }
    }
    out
}

fn replace_token_like(text: &str, prefix: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(idx) = rest.find(prefix) {
        let (head, tail) = rest.split_at(idx);
        out.push_str(head);
        // Longitud mínima razonable de un token (los prefijos cortos como
        // "sk-" se exigen largos para no enmascarar palabras).
        let min_len = if prefix.len() <= 3 { 16 } else { 8 };
        let token: String = tail
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-' || *c == '.')
            .collect();
        if token.len() < min_len {
            out.push_str(prefix);
            rest = &tail[prefix.len()..];
            continue;
        }
        out.push_str(prefix);
        out.push_str("***");
        rest = &tail[token.len()..];
    }
    out.push_str(rest);
    out
}

/// Segundos desde epoch (para nombres de fichero únicos).
pub fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Hora actual en ISO-8601 UTC (días → fecha civil, algoritmo de Hinnant).

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> GitHubConfig {
        GitHubConfig {
            api_base: GITHUB_API.to_string(),
            token_ref: "/state/github.token".to_string(),
            user_agent: USER_AGENT.to_string(),
            default_repo: "boolforge/megadaemon".to_string(),
        }
    }

    #[test]
    fn las_peticiones_no_llevan_credencial() {
        let c = cfg();
        for req in [
            list_repos_request(&c, 1, 30),
            list_issues_request(&c, "o/r", "open", 1, 30),
            list_prs_request(&c, "o/r", "open", 1, 30),
            download_repo_request(&c, "o/r", "main"),
            create_issue_request(&c, "o/r", "t", "b"),
            create_pr_request(&c, "o/r", "t", "b", "head", "main"),
        ] {
            let j = req.to_json().to_string();
            assert!(!j.contains("Authorization"), "la request lleva credencial: {}", j);
            assert!(!j.contains("Bearer"), "la request lleva credencial: {}", j);
            assert!(!j.contains("ghp_"), "la request lleva credencial: {}", j);
            assert!(req.url.starts_with(GITHUB_API));
        }
    }

    #[test]
    fn las_urls_estan_bien_construidas() {
        let c = cfg();
        assert!(list_repos_request(&c, 2, 50).url.contains("/user/repos?per_page=50&page=2"));
        assert!(list_issues_request(&c, "o/r", "open", 1, 30)
            .url
            .contains("/repos/o/r/issues?state=open"));
        assert!(list_prs_request(&c, "o/r", "closed", 1, 30)
            .url
            .contains("/repos/o/r/pulls?state=closed"));
        assert!(download_repo_request(&c, "o/r", "").url.ends_with("/tarball/HEAD"));
        assert!(download_repo_request(&c, "o/r", "v1").url.ends_with("/tarball/v1"));
    }

    #[test]
    fn parsea_los_repos() {
        let body = r#"[
            {"full_name":"a/b","description":"d","private":true,"default_branch":"main",
             "html_url":"https://github.com/a/b","clone_url":"https://github.com/a/b.git",
             "language":"Rust","stargazers_count":7,"forks_count":2,"open_issues_count":3,
             "size":1234,"updated_at":"2026-11-19T00:00:00Z","pushed_at":"2026-11-18T00:00:00Z",
             "archived":false}
        ]"#;
        let r = parse_repos(body).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].full_name, "a/b");
        assert!(r[0].private);
        assert_eq!(r[0].stars, 7);
        assert_eq!(r[0].size_kb, 1234);
        assert_eq!(r[0].default_branch, "main");
        // JSON roto → error claro.
        assert!(parse_repos("{").is_err());
        assert!(parse_repos("{}").is_err());
    }

    #[test]
    fn parsea_issues_y_distingue_los_prs() {
        let body = r#"[
            {"number":1,"title":"bug","state":"open","body":"cuerpo largo","user":{"login":"ana"},
             "labels":[{"name":"p1"},{"name":"p2"}],"comments":3,"created_at":"2026-11-01",
             "updated_at":"2026-11-02","html_url":"https://github.com/o/r/issues/1"},
            {"number":2,"title":"pr","state":"open","body":"x","user":{"login":"bob"},"labels":[],
             "comments":0,"created_at":"2026-11-03","updated_at":"2026-11-03",
             "html_url":"https://github.com/o/r/pull/2","pull_request":{"url":"..."}}
        ]"#;
        let issues = parse_issues(body, false).unwrap();
        assert_eq!(issues.len(), 1, "en /issues el PR se cuela si se pide solo issues");
        assert_eq!(issues[0].number, 1);
        assert_eq!(issues[0].author, "ana");
        assert_eq!(issues[0].labels, vec!["p1", "p2"]);
        assert!(!issues[0].is_pr);
        let prs = parse_issues(body, true).unwrap();
        assert_eq!(prs.len(), 1);
        assert_eq!(prs[0].number, 2);
        assert!(prs[0].is_pr);
    }

    #[test]
    fn el_informe_del_enjambre_consume_el_snapshot_real() {
        let state = json!({
            "tasks": [
                {"task_id":"t1","title":"hacer X","capability":"reverse","state":"published","claimed_by":""},
                {"task_id":"t2","title":"hacer Y","capability":"npu","state":"done","claimed_by":"cloud"}
            ],
            "agents": [
                {"agent":"megadaemon-voice","role":"voice","status":"running","task":"t1","detail":"planificando"}
            ],
            "peers": [
                {"id":"cloud-sandbox","hostname":"cloud","capabilities":["gpu","re"],"last_seen":1700000000}
            ]
        });
        let md = swarm_state_markdown(&state, "Estado del enjambre");
        assert!(md.contains("# Estado del enjambre"));
        assert!(md.contains("1 abiertas / 1 completadas"));
        assert!(md.contains("`t1`"));
        assert!(md.contains("`megadaemon-voice`"));
        assert!(md.contains("`cloud-sandbox`"));
        assert!(!md.contains("\"task_id\""), "no debe_dumpar el JSON crudo");
    }

    #[test]
    fn publicar_exige_confirmacion() {
        let c = cfg();
        let client = MockClient::new(vec![]);
        let state = json!({"tasks": []});
        let out = publish_swarm_state(
            &c,
            "o/r",
            &state,
            &PublishKind::Issue,
            "Estado",
            false,
            &client,
        )
        .unwrap();
        assert_eq!(out["dry_run"], json!(true));
        // No se ha enviado nada.
        assert!(client.requests.borrow().is_empty());
    }

    #[test]
    fn publicar_abre_la_issue() {
        let c = cfg();
        let client = MockClient::new(vec![HttpResponse::text(
            201,
            r#"{"number":42,"html_url":"https://github.com/o/r/issues/42"}"#,
        )]);
        let state = json!({"tasks": [{"task_id":"t1","title":"x","state":"published","capability":"c"}]});
        let out = publish_swarm_state(
            &c,
            "o/r",
            &state,
            &PublishKind::Issue,
            "Estado del enjambre",
            true,
            &client,
        )
        .unwrap();
        assert_eq!(out["ok"], json!(true));
        assert_eq!(out["number"], json!(42));
        assert_eq!(out["url"], json!("https://github.com/o/r/issues/42"));
        let req = &client.requests.borrow()[0];
        assert_eq!(req.method, "POST");
        assert!(req.url.ends_with("/repos/o/r/issues"));
        let body: Value = serde_json::from_str(req.body.as_ref().unwrap()).unwrap();
        assert!(body["body"].as_str().unwrap().contains("t1"));
    }

    #[test]
    fn publicar_un_pr_usa_el_rama_correcta() {
        let c = cfg();
        let client = MockClient::new(vec![HttpResponse::text(
            201,
            r#"{"number":7,"html_url":"https://github.com/o/r/pull/7"}"#,
        )]);
        let out = publish_swarm_state(
            &c,
            "o/r",
            &json!({"tasks": []}),
            &PublishKind::PullRequest {
                head: "agent/estado".into(),
                base: "main".into(),
            },
            "Estado",
            true,
            &client,
        )
        .unwrap();
        assert_eq!(out["kind"], json!("pull_request"));
        let req = &client.requests.borrow()[0];
        assert!(req.url.ends_with("/repos/o/r/pulls"));
        let body: Value = serde_json::from_str(req.body.as_ref().unwrap()).unwrap();
        assert_eq!(body["head"], json!("agent/estado"));
        assert_eq!(body["base"], json!("main"));
    }

    #[test]
    fn un_error_de_github_no_se_traga() {
        let c = cfg();
        let client = MockClient::new(vec![HttpResponse::text(403, r#"{"message":"Bad credentials"}"#)]);
        let err = publish_swarm_state(
            &c,
            "o/r",
            &json!({"tasks": []}),
            &PublishKind::Issue,
            "t",
            true,
            &client,
        )
        .unwrap_err();
        assert!(err.contains("403"), "error: {}", err);
    }

    #[test]
    fn el_saneado_enmascara_credenciales() {
        let dirty = "token ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345 y sk-0123456789abcdefgh y \
                     Authorization: Bearer abcdef1234567890";
        let clean = scrub_secrets(dirty);
        assert!(!clean.contains("ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345"));
        assert!(!clean.contains("sk-0123456789abcdefgh"));
        assert!(!clean.contains("abcdef1234567890"));
        assert!(clean.contains("***"));
    }

    #[test]
    fn el_saneado_no_destroza_el_texto_normal() {
        let txt = "El informe dice: skip this y sk- no es un token corto";
        let out = scrub_secrets(txt);
        assert!(out.contains("skip this"));
        assert!(out.contains("sk- no es"));
    }

    #[test]
    fn listar_repos_usa_el_cliente_inyectado() {
        let c = cfg();
        let client = MockClient::new(vec![HttpResponse::text(
            200,
            r#"[{"full_name":"a/b","default_branch":"main"}]"#,
        )]);
        let repos = list_repos(&c, 1, 10, &client).unwrap();
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].full_name, "a/b");
        assert!(client.requests.borrow()[0].url.contains("/user/repos"));
    }

    #[test]
    fn descargar_el_repo_escribe_el_tarball() {
        let c = cfg();
        let client = MockClient::new(vec![HttpResponse {
            status: 200,
            body: "binario".into(),
            bytes: b"binario".to_vec(),
        }]);
        let dir = std::env::temp_dir().join(format!("md-gh-{}", now_epoch()));
        let out = download_repo(&c, "o/r", "main", dir.to_str().unwrap(), &client).unwrap();
        assert_eq!(out["ok"], json!(true));
        assert_eq!(out["bytes"], json!(7));
        let p = out["path"].as_str().unwrap();
        assert!(std::path::Path::new(p).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

//! MegaDaemon v13.4 — Operaciones Git autonomousas.
//!
//! El usuario quiere que megadaemon "se identifique" y sepa usar git: para
//! que los ciclos de auto-mejora sean continuos, megadaemon debe poder
//! commitear y pushear sus propios cambios sin intervención manual.
//!
//! Diseño seguro por defecto:
//!
//! * **nunca** se escribe una credencial en el repo ni en la URL del remoto;
//! * se usa el *credential helper* del dispositivo (`gh auth git-credential`)
//!   que ya está configurado;
//! * **nunca** se hace `push --force`: sólo `push` normal;
//! * todo commit lleva una firma identificativa del demonio;
//! * el diff se **inspecciona antes** de commitear y se devuelve en el
//!   resultado, de modo que la decisión final es visible.

use serde_json::{json, Value};

/// Identidad con la que megadaemon firma sus commits.
pub const AGENT_NAME: &str = "MegaDaemon";
pub const AGENT_EMAIL: &str = "megadaemon@agy.internal";

/// Repositorio de trabajo en el dispositivo.
pub const DEVICE_REPO: &str = "/data/data/com.termux/files/home/repos/megadaemon";

fn git(args: &str, timeout_ms: u64) -> (i32, String, String) {
    let cmd = format!(
        "cd {} && export HOME=/data/data/com.termux/files/home && export PATH=/data/data/com.termux/files/usr/bin:/system/bin:/bin && git {}",
        crate::android::sh_quote(DEVICE_REPO),
        args
    );
    let r = crate::android::root_exec(&cmd, timeout_ms, 256 * 1024);
    (r.code.unwrap_or(-1), r.stdout, r.stderr)
}

/// Estado del repo: rama, cambios pendientes, remoto, identidad.
pub fn status() -> Value {
    let (code, out, err) = git("status --porcelain=v1 -b 2>&1", 30_000);
    let (c2, branch_out, _) = git("rev-parse --abbrev-ref HEAD 2>&1", 15_000);
    let (c3, remote_out, _) = git("remote -v 2>&1", 15_000);
    let (c4, log_out, _) = git("log --oneline -3 2>&1", 20_000);
    let mut changed = vec![];
    let mut branch = String::new();
    for line in out.lines() {
        if line.starts_with("##") {
            branch = line.trim_start_matches("## ").trim().to_string();
            continue;
        }
        if line.len() > 3 {
            changed.push(json!({
                "status": line[..2].trim(),
                "path": line[3..].trim(),
            }));
        }
    }
    json!({
        "ok": code == 0,
        "repo": DEVICE_REPO,
        "branch": branch,
        "changed": changed.len(),
        "changed_files": changed,
        "remotes": remote_out.lines().map(|l| l.to_string()).collect::<Vec<String>>(),
        "recent_commits": log_out.lines().map(|l| l.to_string()).collect::<Vec<String>>(),
        "agent_identity": { "name": AGENT_NAME, "email": AGENT_EMAIL },
        "error": if code != 0 { err.trim().to_string() } else { String::new() },
    })
}

/// Identidad configurada de git en el dispositivo (sin credenciales).
pub fn identity() -> Value {
    let (_, name, _) = git("config user.name 2>&1", 10_000);
    let (_, email, _) = git("config user.email 2>&1", 10_000);
    let (_, helper, _) = git("config --get-all credential.helper 2>&1", 10_000);
    let (_, gh, _) = git("--version 2>&1", 10_000);
    json!({
        "ok": true,
        "user_name": name.trim(),
        "user_email": email.trim(),
        "credential_helper": helper.lines().map(|l| l.to_string()).collect::<Vec<String>>(),
        "git_version": gh.trim(),
        "uses_gh_helper": helper.contains("gh auth git-credential"),
    })
}

/// Diff de los cambios pendientes (para inspección previa).
pub fn diff(max_bytes: usize) -> Value {
    let (_, out, err) = git("diff --stat 2>&1", 30_000);
    let (_, full, _) = git("diff 2>&1", 45_000);
    json!({
        "ok": true,
        "stat": out.trim(),
        "diff": full.chars().take(max_bytes).collect::<String>(),
        "truncated": full.len() > max_bytes,
        "error": err.trim(),
    })
}

/// Commit + push. **Exige `confirm = true`** (publicar en un remoto es
/// irreversible en la práctica).
pub fn commit_push(message: &str, paths: Option<Vec<String>>, confirm: bool, push: bool) -> Value {
    if !confirm {
        return json!({
            "ok": false,
            "error": "needs_confirm",
            "hint": "commit/push publica cambios: repítelo con confirm=true",
        });
    }
    if message.trim().is_empty() {
        return json!({ "ok": false, "error": "empty_message" });
    }
    let add_args = match paths {
        Some(p) if !p.is_empty() => format!("add -- {}", p.iter().map(|x| crate::android::sh_quote(x)).collect::<Vec<String>>().join(" ")),
        _ => "add -A".to_string(),
    };
    let (c1, _, e1) = git(&format!("{} 2>&1", add_args), 60_000);
    if c1 != 0 {
        return json!({ "ok": false, "error": "git_add_failed", "detail": e1.trim() });
    }
    let (_, stat, _) = git("diff --cached --stat 2>&1", 30_000);
    let msg = format!("{}\\n\\nCo-Authored-By: {} <{}>\\nAgent-Version: {}", message, AGENT_NAME, AGENT_EMAIL, env!("CARGO_PKG_VERSION"));
    let (c2, out2, e2) = git(
        &format!("commit -m {} 2>&1", crate::android::sh_quote(&msg)),
        60_000,
    );
    let mut push_res = Value::Null;
    if push {
        let (c3, out3, e3) = git("push origin HEAD 2>&1", 180_000);
        push_res = json!({
            "ok": c3 == 0,
            "output": out3.trim().chars().take(600).collect::<String>(),
            "error": e3.trim().chars().take(400).collect::<String>(),
        });
    }
    let (_, head, _) = git("rev-parse HEAD 2>&1", 15_000);
    crate::android::audit("git.commit_push", &json!({ "ok": c2 == 0, "pushed": push }));
    json!({
        "ok": c2 == 0,
        "staged_stat": stat.trim(),
        "commit_output": out2.trim().chars().take(600).collect::<String>(),
        "commit_error": e2.trim().chars().take(300).collect::<String>(),
        "head": head.trim(),
        "push": push_res,
    })
}

/// Historial de commits (resumen).
pub fn log(n: usize) -> Value {
    let (_, out, _) = git(&format!("log --oneline -{} 2>&1", n.clamp(1, 200)), 30_000);
    json!({
        "ok": true,
        "commits": out.lines().map(|l| l.to_string()).collect::<Vec<String>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_exige_confirmacion() {
        let r = commit_push("mensaje", None, false, false);
        assert_eq!(r["ok"], json!(false));
        assert_eq!(r["error"], json!("needs_confirm"));
    }

    #[test]
    fn mensaje_vacio_se_rechaza() {
        let r = commit_push("   ", None, true, false);
        assert_eq!(r["ok"], json!(false));
        assert_eq!(r["error"], json!("empty_message"));
    }

    #[test]
    fn la_identidad_del_agente_es_estable() {
        assert_eq!(AGENT_NAME, "MegaDaemon");
        assert!(AGENT_EMAIL.contains('@'));
        assert!(DEVICE_REPO.contains("megadaemon"));
    }

    #[test]
    fn la_ruta_del_repo_es_absoluta() {
        assert!(DEVICE_REPO.starts_with('/'));
    }
}

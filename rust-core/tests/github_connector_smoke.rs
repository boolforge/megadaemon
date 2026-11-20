//! MegaDaemon v15 (W6) — **Conector de GitHub**: humo del parseo, de la
//! publicación del estado del enjambre y del sellado de secretos.
//!
//! No toca la red: el cliente HTTP es inyectado (MockClient).
//!
//! Ejecutar:
//!   cd rust-core && cargo test --test github_connector_smoke

use megadaemon_core::github_connector::{
    create_issue_request, create_pr_request, download_repo_request, list_issues_request,
    list_prs_request, list_repos_request, parse_issues, parse_repos, publish_swarm_state,
    scrub_secrets, swarm_state_markdown, GitHubConfig, HttpResponse, MockClient, PublishKind,
    GITHUB_API,
};
use serde_json::{json, Value};

fn cfg() -> GitHubConfig {
    GitHubConfig {
        api_base: GITHUB_API.to_string(),
        token_ref: "/state/github.token".to_string(),
        user_agent: "MegaDaemon/15".to_string(),
        default_repo: "boolforge/megadaemon".to_string(),
    }
}

#[test]
fn ninguna_peticion_lleva_la_credencial() {
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
        assert!(!j.contains("Authorization"), "fuga: {}", j);
        assert!(!j.contains("Bearer"), "fuga: {}", j);
        assert!(!j.contains("ghp_"), "fuga: {}", j);
        assert!(req.url.starts_with(GITHUB_API));
    }
}

#[test]
fn parsea_repos_issues_y_prs() {
    let repos = parse_repos(
        r#"[{"full_name":"a/b","private":false,"default_branch":"main","stargazers_count":3,
             "size":42,"html_url":"https://github.com/a/b"}]"#,
    )
    .unwrap();
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0].full_name, "a/b");
    assert_eq!(repos[0].stars, 3);
    assert_eq!(repos[0].size_kb, 42);

    let items = parse_issues(
        r#"[{"number":1,"title":"t","state":"open","user":{"login":"ana"},
             "labels":[{"name":"bug"}],"comments":2,"html_url":"u"},
            {"number":2,"title":"pr","pull_request":{"url":"x"}}]"#,
        false,
    )
    .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].author, "ana");
    assert_eq!(items[0].labels, vec!["bug"]);
    assert!(!items[0].is_pr);
}

#[test]
fn publicar_exige_confirmacion_y_no_toca_la_red() {
    let client = MockClient::new(vec![]);
    let out = publish_swarm_state(
        &cfg(),
        "o/r",
        &json!({"tasks": []}),
        &PublishKind::Issue,
        "Estado",
        false,
        &client,
    )
    .unwrap();
    assert_eq!(out["dry_run"], json!(true));
    assert!(client.requests.borrow().is_empty());
}

#[test]
fn publica_el_estado_del_enjambre_como_issue() {
    let client = MockClient::new(vec![HttpResponse::text(
        201,
        r#"{"number":9,"html_url":"https://github.com/o/r/issues/9"}"#,
    )]);
    let state = json!({
        "tasks": [{"task_id":"t1","title":"revisar el móvil","capability":"device",
                   "state":"published","claimed_by":""}],
        "agents": [{"agent":"megadaemon-voice","role":"voice","status":"running","task":"t1"}],
        "peers": [{"id":"cloud-sandbox","capabilities":["gpu"],"last_seen":4102444800i64}]
    });
    let out = publish_swarm_state(
        &cfg(),
        "o/r",
        &state,
        &PublishKind::Issue,
        "Estado del enjambre",
        true,
        &client,
    )
    .unwrap();
    assert_eq!(out["ok"], json!(true));
    assert_eq!(out["number"], json!(9));
    let req = &client.requests.borrow()[0];
    assert_eq!(req.method, "POST");
    assert!(req.url.ends_with("/repos/o/r/issues"));
    let body: Value = serde_json::from_str(req.body.as_ref().unwrap()).unwrap();
    let texto = body["body"].as_str().unwrap();
    assert!(texto.contains("t1"));
    assert!(texto.contains("megadaemon-voice"));
}

#[test]
fn el_informe_sella_las_credenciales() {
    let state = json!({
        "tasks": [{"task_id":"t1","title":"usar ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345","state":"published"}],
        "agents": []
    });
    let md = swarm_state_markdown(&state, "Estado");
    assert!(md.contains("t1"));
    assert!(!md.contains("ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZ012345"));
    assert!(md.contains("***"));
}

#[test]
fn el_sellado_no_destroza_el_texto() {
    let txt = "hay que hacer skip this y sk- no es un token";
    let out = scrub_secrets(txt);
    assert!(out.contains("skip this"));
    assert!(out.contains("sk- no es"));
}

#[test]
fn un_error_de_github_no_se_traga() {
    let client = MockClient::new(vec![HttpResponse::text(403, r#"{"message":"Bad credentials"}"#)]);
    let err = publish_swarm_state(
        &cfg(),
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

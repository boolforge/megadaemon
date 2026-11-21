//! Arnés de pruebas del módulo `delegate` (P1.3 — delegación parasitaria).
//!
//! `src/lib.rs` es intocable en esta tarea, así que el módulo se incluye
//! aquí directamente con `#[path]`, igual que hacen `tests/signing.rs` y
//! `tests/root_hide_app_intel.rs`.  En cuanto se añada `pub mod delegate;`
//! a `src/lib.rs`, este arnés puede borrarse sin tocar los tests.
//!
//! Al incluirse el módulo, sus propios `#[cfg(test)] mod tests` se compilan
//! y se ejecutan: aquí sólo se añaden las pruebas de integración que
//! necesitan el módulo como externos (coste real, cuota agotada y
//! procedencia de precios).

use megadaemon_core::delegate;

use megadaemon_core::delegate::{price_provenance, pricing_ledger, pricing_report, pricing_stale, DelegateEngine};

fn cuenta(id: &str, tier: &str, model: &str, caps: &[&str], cuota: Option<u32>) -> delegate::ProviderAccount {
    delegate::ProviderAccount {
        id: id.into(),
        provider: id.split('-').next().unwrap_or("x").into(),
        tier: tier.into(),
        model: model.into(),
        capabilities: caps.iter().map(|c| c.to_string()).collect(),
        remaining_quota: cuota,
        daily_budget_usd: 0.0,
        priority: 0,
        enabled: true,
        notes: String::new(),
        // Campos que anadio el bloque de delegacion (MEDIDO el 2026-11-21:
        // sin ellos el arnes no compila, E0063 "missing auth_kind and
        // auth_ref", y el job de calidad caia por esto).
        auth_kind: String::new(),
        auth_ref: String::new(),
    }
}

fn tarea(kind: &str, caps: &[&str]) -> delegate::DelegationTask {
    delegate::DelegationTask {
        id: String::new(),
        prompt: "resume el informe trimestral".into(),
        kind: kind.into(),
        required_capabilities: caps.iter().map(|c| c.to_string()).collect(),
        max_tokens: 1024,
        deadline: None,
        status: "queued".into(),
        attempts: vec![],
        assigned_account: None,
    }
}

/// Requisito P1.3 nº1: enrutar SIEMPRE a la cuenta gratuita adecuada y
/// reservar las de pago como último recurso.
#[test]
fn el_enrutado_reserva_las_cuentas_de_pago() {
    let e = DelegateEngine::new(vec![
        cuenta("paga-openai", "paid", "gpt-5.6-sol", &["research"], Some(999)),
        cuenta("gratis-kimi", "free", "kimi-k3", &["research"], Some(5)),
    ]);
    let elegida = e.pick_account(&tarea("research", &["research"])).unwrap();
    assert_eq!(elegida, "gratis-kimi", "la cuenta gratuita debe ganar siempre");

    // Cuando la gratuita no tiene la capacidad, entra la de pago.
    let e2 = DelegateEngine::new(vec![
        cuenta("paga-openai", "paid", "gpt-5.6-sol", &["code"], Some(999)),
        cuenta("gratis-kimi", "free", "kimi-k3", &["research"], Some(5)),
    ]);
    let elegida2 = e2.pick_account(&tarea("code", &["code"])).unwrap();
    assert_eq!(elegida2, "paga-openai");
}

/// Requisito P1.3 nº3: cuota agotada = cuenta quemada, no recibe tareas.
#[test]
fn una_cuenta_quemada_no_recibe_tareas() {
    let e = DelegateEngine::new(vec![
        cuenta("gratis-kimi", "free", "kimi-k3", &["research"], Some(0)),
        cuenta("gratis-minimax", "free", "minimax-m3", &["research"], Some(3)),
    ]);
    let elegida = e.pick_account(&tarea("research", &["research"])).unwrap();
    assert_eq!(elegida, "gratis-minimax");

    // Si todas están quemadas, el error lo dice explícitamente.
    let e2 = DelegateEngine::new(vec![cuenta(
        "gratis-kimi",
        "free",
        "kimi-k3",
        &["research"],
        Some(0),
    )]);
    let err = e2.pick_account(&tarea("research", &["research"])).unwrap_err();
    assert!(err.contains("cuota"), "error: {}", err);
}

/// Requisito P1.3 nº2: precios reales por modelo, con fecha y fuente.
#[test]
fn cada_precio_documenta_su_fuente_y_su_fecha() {
    for (modelo, entrada, salida, proveedor) in delegate::PRICING_TABLE {
        let p = price_provenance(modelo).unwrap_or_else(|| panic!("{} sin procedencia", modelo));
        assert!(
            p.source.starts_with("https://"),
            "{}: la fuente debe ser la URL oficial: {}",
            modelo,
            p.source
        );
        assert!(!p.verified_on.is_empty(), "{}: falta la fecha", modelo);
        assert_eq!(p.provider, *proveedor);
        assert_eq!(p.input_usd_per_mtok, *entrada);
        assert_eq!(p.output_usd_per_mtok, *salida);
    }
    // El libro tiene una entrada por modelo.
    assert_eq!(pricing_ledger().len(), delegate::PRICING_TABLE.len());
    // El informe JSON lleva la fuente, la fecha y los rancios.
    let r = pricing_report(30);
    assert!(r["ledger"].as_array().unwrap().len() == delegate::PRICING_TABLE.len());
    assert!(r["ledger"][0]["source"].as_str().unwrap().starts_with("https://"));
    assert!(r["stale_models"].as_u64().is_some());
}

/// El ahorro del enjambre es la diferencia entre el coste real y el que
/// habría costado con cuentas de pago (shadow cost).
#[test]
fn el_coste_real_es_cero_en_cuentas_gratuitas_y_el_sombra_no() {
    let mut e = DelegateEngine::new(vec![cuenta(
        "gratis-kimi",
        "free",
        "kimi-k3",
        &["research"],
        Some(10),
    )]);
    let id = e.submit(tarea("research", &["research"]));
    e.record_attempt(
        &id,
        delegate::Attempt {
            account: "gratis-kimi".into(),
            model: "kimi-k3".into(),
            started_at: "2026-11-19T10:00:00Z".into(),
            finished_at: Some("2026-11-19T10:00:05Z".into()),
            ok: true,
            input_tokens: 10_000,
            output_tokens: 2_000,
            error: None,
            quality_score: None,
        },
    );
    let st = e.account_stats();
    assert_eq!(st.len(), 1);
    assert_eq!(st[0].cost_usd, 0.0, "una cuenta gratuita no cuesta dinero");
    assert!(st[0].shadow_cost_usd > 0.0, "el coste sombra sí es el de lista");
    let stats = e.stats();
    assert_eq!(stats["accounts"]["free"].as_u64(), Some(1));
    assert_eq!(stats["accounts"]["paid"].as_u64(), Some(0));
    // El presupuesto se mide: lo que cuesta de verdad frente a lo que
    // habría costado con cuentas de pago.
    assert_eq!(stats["cost_usd"]["estimated"].as_f64(), Some(0.0));
    let sombra = stats["cost_usd"]["shadow_if_paid"].as_f64().unwrap();
    assert!(sombra > 0.0, "el coste sombra debe ser el de lista: {}", sombra);
    assert_eq!(stats["cost_usd"]["savings"].as_f64(), Some(sombra));
}

/// La detección de precios rancios tiene que ser utilizable por el cockpit.
#[test]
fn los_precios_rancios_son_detectables() {
    let ledger = pricing_ledger();
    // "Hoy" = 2026-11-19 (día epoch 20_776): lo verificado hoy no está rancio.
    let hoy = 20_776u64;
    let hoy_rancios = pricing_stale(0, hoy);
    assert!(
        hoy_rancios.iter().all(|p| p.verified_on != delegate::PRICING_VERIFIED_ON),
        "lo verificado hoy no puede estar rancio: {:?}",
        hoy_rancios
    );
    // En el futuro todo caduca: el motor no puede fiarse eternamente de un
    // precio sin re-verificar.
    let futuros = pricing_stale(0, 40_000);
    assert_eq!(futuros.len(), ledger.len());
}

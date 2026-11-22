//! Arnés de pruebas del planificador cron (`src/cron.rs`).
//!
//! Usa la biblioteca (`use megadaemon_core::…;`): los módulos ya están
//! cableados en `src/lib.rs`, así que no hace falta copiar el fichero con
//! `#[path]` (y copiarlo rompería las referencias `crate::`).

use megadaemon_core::cron::{
    self, parse_tz_offset, CatchUp, CronExpr, Job, ResourceLimits, SchedulerState,
};

#[test]
fn una_expresion_cron_encuentra_su_siguiente_ejecucion() {
    // 2026-11-22 12:00:00 UTC = dia 20779 desde el epoch.
    let base = 20_779 * 86_400 + 12 * 3600;
    let e = CronExpr::parse("30 13 * * *").unwrap();
    let n = e.next_after(base, 0).unwrap();
    assert_eq!(cron::civil_from_epoch(n, 0), (2026, 11, 22, 13, 30));

    // Cada 15 minutos, el siguiente múltiplo de 15.
    let e2 = CronExpr::parse("*/15 * * * *").unwrap();
    let n2 = e2.next_after(base, 0).unwrap();
    assert_eq!(cron::civil_from_epoch(n2, 0).4, 15);
}

#[test]
fn el_calendario_es_exacto_en_un_dia_de_cambio_de_mes() {
    // 2026-02-28 23:59:00 UTC -> el siguiente 0 de marzo.
    let base = 20_796 * 86_400 + 23 * 3600 + 59 * 60;
    let e = CronExpr::parse("0 0 * * *").unwrap();
    let n = e.next_after(base, 0).unwrap();
    assert_eq!(cron::civil_from_epoch(n, 0), (2026, 3, 1, 0, 0));
}

#[test]
fn la_forma_de_la_expresion_se_valida() {
    assert!(CronExpr::parse("*/5 * * * *").is_ok());
    assert!(CronExpr::parse("0 0 1 jan mon").is_ok());
    assert!(CronExpr::parse("0 0 * * *").is_ok());
    assert!(CronExpr::parse("no es cron").is_err());
    assert!(CronExpr::parse("0 0 * *").is_err());
    assert!(CronExpr::parse("60 * * * *").is_err());
}

#[test]
fn la_zona_horaria_se_interpreta() {
    assert_eq!(parse_tz_offset("+0200"), Some(120));
    assert_eq!(parse_tz_offset("-0330"), Some(-210));
    assert_eq!(parse_tz_offset("xyz"), None);
}

#[test]
fn el_planificador_genera_y_recupera_ejecuciones() {
    let mut st = SchedulerState {
        tz_offset_min: 0,
        max_catchup: 3,
        ..SchedulerState::default()
    };
    st.jobs.push(Job {
        id: "j".into(),
        name: "cada hora".into(),
        cron: "0 * * * *".into(),
        catch_up: CatchUp::RunAll,
        next_run: Some(20_779 * 86_400),
        ..Job::default()
    });
    let now = 20_779 * 86_400 + 3600 * 5;
    let plan = cron::plan(&st, now);
    assert_eq!(plan.len(), 3); // con tope de catch-up
    assert!(plan.iter().all(|p| p.catch_up));
    assert_eq!(plan[0].scheduled_at, 20_779 * 86_400);
}

#[test]
fn una_tarea_interrumpida_se_detecta_y_se_relanza() {
    let mut st = SchedulerState {
        tz_offset_min: 0,
        ..SchedulerState::default()
    };
    st.jobs.push(Job {
        id: "lenta".into(),
        cron: "0 * * * *".into(),
        running_since: Some(1_000),
        ..Job::default()
    });
    let limits = ResourceLimits {
        max_runtime_secs: 600,
        ..ResourceLimits::default()
    };
    assert_eq!(cron::interrupted(&st, 2_000, &limits), vec!["lenta".to_string()]);
    cron::resume(&mut st, "lenta", 2_000).unwrap();
    assert!(st.jobs[0].running_since.is_none());
}

#[test]
fn los_limites_de_recursos_vetan_tareas_pesadas() {
    let mut st = SchedulerState::default();
    st.jobs.push(Job {
        id: "pesada".into(),
        cron: "0 * * * *".into(),
        heavy: true,
        ..Job::default()
    });
    let limits = ResourceLimits {
        allow_heavy: true,
        min_battery_pct: 40,
        max_concurrent: 1,
        ..ResourceLimits::default()
    };
    assert!(cron::eligible(&st, "pesada", 0, &limits, Some(80)).is_ok());
    assert!(cron::eligible(&st, "pesada", 0, &limits, Some(20)).is_err());
    assert!(cron::eligible(&st, "pesada", 1, &limits, Some(80)).is_err());
}

#[test]
fn el_estado_persiste_en_disco() {
    let dir = std::env::temp_dir().join("md_test_cron_persistencia");
    let _ = std::fs::remove_dir_all(&dir);
    let f = dir.join("state.json");
    let p = f.to_str().unwrap();
    let mut st = SchedulerState {
        tz_offset_min: 120,
        max_concurrent: 3,
        ..SchedulerState::default()
    };
    st.jobs.push(Job {
        id: "j".into(),
        name: "tarea".into(),
        cron: "*/10 * * * *".into(),
        catch_up: CatchUp::RunOnce,
        last_run: Some(99),
        ..Job::default()
    });
    cron::save(p, &st).unwrap();
    let back = cron::load(p);
    assert_eq!(back.jobs.len(), 1);
    assert_eq!(back.jobs[0].catch_up, CatchUp::RunOnce);
    assert_eq!(back.jobs[0].last_run, Some(99));
    assert_eq!(back.tz_offset_min, 120);
    assert_eq!(back.max_concurrent, 3);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn un_reintento_espera_mas_cada_vez() {
    let mut st = SchedulerState {
        tz_offset_min: 0,
        ..SchedulerState::default()
    };
    st.jobs.push(Job {
        id: "j".into(),
        cron: "0 * * * *".into(),
        retry_backoff_secs: 30,
        max_attempts: 3,
        ..Job::default()
    });
    cron::mark_result(&mut st, "j", false, 1_000).unwrap();
    assert_eq!(st.jobs[0].next_run, Some(1_030));
    cron::mark_result(&mut st, "j", false, 2_000).unwrap();
    assert_eq!(st.jobs[0].next_run, Some(2_060));
    cron::mark_result(&mut st, "j", true, 3_000).unwrap();
    assert_eq!(st.jobs[0].failures, 0);
}

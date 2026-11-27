use testproj::{argmax, moving_average, normalize};

#[test]
fn normalize_preserva_la_suma() {
    let r = normalize(&[1.0, 2.0, 1.0]);
    let s: f64 = r.iter().sum();
    assert!((s - 1.0).abs() < 1e-9, "la suma debe ser 1.0, fue {}", s);
}

#[test]
fn normalize_trunca_negativos() {
    let r = normalize(&[2.0, -1.0, 1.0]);
    assert!((r[0] - 2.0 / 3.0).abs() < 1e-9);
    assert!(r[1] >= 0.0);
}

#[test]
fn normalize_todo_no_positivo_es_uniforme() {
    let r = normalize(&[0.0, 0.0]);
    assert!((r[0] - 0.5).abs() < 1e-9 && (r[1] - 0.5).abs() < 1e-9);
}

#[test]
fn argmax_empieza_por_el_primero_si_empata() {
    assert_eq!(argmax(&[2.0, 2.0, 1.0]), Some(0));
}

#[test]
fn moving_average_es_la_media_de_la_ventana() {
    let r = moving_average(&[1.0, 2.0, 3.0, 4.0], 2);
    assert!((r[0] - 1.0).abs() < 1e-9);
    assert!((r[1] - 1.5).abs() < 1e-9);
    assert!((r[2] - 2.5).abs() < 1e-9);
    assert!((r[3] - 3.5).abs() < 1e-9);
}

#[test]
fn moving_average_con_ventana_0_devuelve_copia() {
    let r = moving_average(&[1.0, 2.0], 0);
    assert_eq!(r, vec![1.0, 2.0]);
}

//! Pruebas del motor MBA de MegaDaemon (`re_mba`).
//!
//! Lo que se prueba aquí no es "que el simplificador acierte en estos casos":
//! es que la **prueba de equivalencia** del motor es una demostración, no una
//! heurística.  Concretamente:
//!
//! * toda identidad del catálogo se demuestra por **forma canónica polinómica**
//!   ⇒ la equivalencia vale para las 2^64 entradas, no para las probadas;
//! * una identidad **falsa** se detecta (no se acepta por parecido textual);
//! * el predicado opaco se decide por polinomio, no por muestreo;
//! * el simplificador recupera la forma simple y **adjunta su demostración**.

use megadaemon_core::re_mba::{
    analyze_opaque_predicate, eval, identity_catalog, identidades_falsas_detectadas, parse,
    prove_equivalent, render, simplify, OpaqueVerdict, ProofKind,
};
use std::collections::HashMap;

fn env2(x: u64, y: u64) -> HashMap<String, u64> {
    let mut e = HashMap::new();
    e.insert("x".to_string(), x);
    e.insert("y".to_string(), y);
    e
}

#[test]
fn el_parser_entiende_precedencias() {
    let e = parse("(x ^ y) + 2*(x & y)").unwrap();
    assert_eq!(render(&e), "((x ^ y) + (2 * (x & y)))");
}

#[test]
fn la_forma_canonica_prueba_la_identidad_de_suma() {
    let id = identity_catalog()
        .into_iter()
        .find(|i| i.name == "MBA-ADD-1")
        .unwrap();
    let p = megadaemon_core::re_mba::verify_identity(&id);
    assert!(p.passed, "la identidad debe quedar probada: {p:?}");
    assert_eq!(p.kind, ProofKind::CanonicalForm);
}

#[test]
fn todas_las_identidades_del_catalogo_se_prueban() {
    for id in identity_catalog() {
        let p = megadaemon_core::re_mba::verify_identity(&id);
        assert!(p.passed, "{} no se prueba: {p:?}", id.name);
        assert_eq!(
            p.kind,
            ProofKind::CanonicalForm,
            "{} debe probarse por forma canónica",
            id.name
        );
    }
}

#[test]
fn una_identidad_falsa_se_detecta() {
    let lhs = parse("(x ^ y)").unwrap();
    let rhs = parse("(x + y)").unwrap();
    let p = prove_equivalent(&lhs, &rhs);
    assert!(!p.passed);
}

#[test]
fn el_simplificador_recupera_x_mas_y() {
    let e = parse("(x ^ y) + 2*(x & y)").unwrap();
    let r = simplify(&e);
    assert!(r.changed, "debería simplificar: {r:?}");
    assert_eq!(r.simplified, "(x + y)");
    assert_eq!(r.proof.kind, ProofKind::CanonicalForm);
    assert!(r.proof.passed);
}

#[test]
fn predicado_opaco_detectado() {
    let e = parse("(x ^ x)").unwrap();
    let (v, p) = analyze_opaque_predicate(&e);
    assert_eq!(v, OpaqueVerdict::AlwaysZero);
    assert!(p.passed);
    assert_eq!(p.kind, ProofKind::CanonicalForm);
}

#[test]
fn predicado_no_opaco_no_se_afirma() {
    let e = parse("(x + y)").unwrap();
    let (v, p) = analyze_opaque_predicate(&e);
    assert_eq!(v, OpaqueVerdict::Unknown);
    assert!(!p.passed);
}

#[test]
fn la_evaluacion_usa_la_semantica_de_32_bits() {
    let e = parse("(x ^ y) + 2*(x & y)").unwrap();
    let v = eval(&e, &env2(0xFFFF_FFFF, 1));
    assert_eq!(v, 0);
}

#[test]
fn la_prueba_canonica_detecta_desigualdades_reales() {
    // Mismo polinomio formal, funciones distintas: `(x & y) * 2` frente a `x + y`
    // NO son equivalentes y el motor no debe afirmarlo.
    let a = parse("(x & y) * 2").unwrap();
    let b = parse("(x + y)").unwrap();
    let p = prove_equivalent(&a, &b);
    assert!(!p.passed, "no deben ser equivalentes: {p:?}");
}

#[test]
fn el_motor_encuentra_las_identidades_falsas_de_la_tabla_del_repo() {
    // MEDIDO el 2026-11-22: la tabla de reglas del simplificador antigo
    // (`kaitai_imhex::simplify_mba_expression`) tenía DOS identidades falsas, y su
    // flag `equivalence_verified` no verificaba nada.  Este test fija el hallazgo:
    // si alguien "arregla" el catálogo sin arreglar la tabla vieja, el motor lo
    // seguiría detectando.
    let falsas = identidades_falsas_detectadas();
    assert_eq!(falsas.len(), 2, "se esperaban 2 identidades falsas: {falsas:?}");
    let nombres: Vec<&str> = falsas.iter().map(|f| f.name).collect();
    assert!(nombres.contains(&"MBA-SUB-1"), "{nombres:?}");
    assert!(nombres.contains(&"MBA-SUB-3"), "{nombres:?}");
    for f in &falsas {
        // El contraejemplo debe seguir siendo un contraejemplo (recalculado).
        let le = parse(f.lhs).unwrap();
        let re = parse(f.rhs).unwrap();
        let mut env = HashMap::new();
        env.insert("x".to_string(), f.counterexample.0);
        env.insert("y".to_string(), f.counterexample.1);
        assert_ne!(eval(&le, &env), eval(&re, &env), "{f:?}");
        // Y el motor debe RECHAZAR la equivalencia.
        let p = prove_equivalent(&le, &re);
        assert!(!p.passed, "{} no debería probarse: {p:?}", f.name);
    }
}

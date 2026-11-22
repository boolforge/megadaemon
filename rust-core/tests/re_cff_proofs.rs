//! Pruebas del desaplanado de control (`re_cff`).
//!
//! Lo que se prueba es que el motor (a) **detecta** el dispatcher, (b) **aplana**
//! el CFG y (c) **demuestra** que el resultado se comporta igual que el original
//! con las mismas entradas (mismos efectos, mismos valores finales, misma
//! terminación).

use megadaemon_core::re_cff::{
    analyze_cff, detect_cff, deflatten, prove_deflatten, run, test_inputs, BinOp, Block, FlatProgram,
    Stmt, Terminator,
};
use std::collections::BTreeMap;

/// Construye el programa aplanado de referencia:
///
/// ```text
///   entry:  x = 0; goto disp
///   disp:   switch(state) { 1 => b1, 2 => b2, 3 => b3 }
///   b1:     estado = 2; efecto("A"); goto disp
///   b2:     estado = 3; efecto("B"); goto disp
///   b3:     efecto("C"); halt
/// ```
fn prog_aplanado() -> FlatProgram {
    FlatProgram {
        entry: 0,
        blocks: vec![
            Block {
                id: 0,
                stmts: vec![
                    Stmt::Assign {
                        dst: "state".into(),
                        value: megadaemon_core::re_cff::SVal::Const(1),
                    },
                    Stmt::BinAssign {
                        dst: "acc".into(),
                        op: BinOp::Add,
                        a: megadaemon_core::re_cff::SVal::Var("x".into()),
                        b: megadaemon_core::re_cff::SVal::Const(0),
                    },
                ],
                term: Terminator::Goto(1),
            },
            Block {
                id: 1,
                stmts: vec![],
                term: Terminator::Switch {
                    state_var: "state".into(),
                    cases: vec![(1, 2), (2, 3), (3, 4)],
                    default: None,
                },
            },
            Block {
                id: 2,
                stmts: vec![
                    Stmt::Assign {
                        dst: "state".into(),
                        value: megadaemon_core::re_cff::SVal::Const(2),
                    },
                    Stmt::Effect("A".into()),
                ],
                term: Terminator::Goto(1),
            },
            Block {
                id: 3,
                stmts: vec![
                    Stmt::Assign {
                        dst: "state".into(),
                        value: megadaemon_core::re_cff::SVal::Const(3),
                    },
                    Stmt::Effect("B".into()),
                ],
                term: Terminator::Goto(1),
            },
            Block {
                id: 4,
                stmts: vec![Stmt::Effect("C".into())],
                term: Terminator::Halt,
            },
        ],
    }
}

#[test]
fn detecta_el_dispatcher() {
    let p = prog_aplanado();
    let det = detect_cff(&p).expect("debería detectar el dispatcher");
    assert_eq!(det.dispatcher, 1);
    assert_eq!(det.state_var, "state");
    assert_eq!(det.setters.len(), 3);
    assert_eq!(det.cases.len(), 3);
    assert!(det.confidence > 0.99);
}

#[test]
fn un_programa_normal_no_se_declara_aplanado() {
    let p = FlatProgram {
        entry: 0,
        blocks: vec![
            Block {
                id: 0,
                stmts: vec![Stmt::Effect("A".into())],
                term: Terminator::Goto(1),
            },
            Block {
                id: 1,
                stmts: vec![Stmt::Effect("B".into())],
                term: Terminator::Halt,
            },
        ],
    };
    assert!(detect_cff(&p).is_none());
}

#[test]
fn el_aplanado_equivale_al_original() {
    let p = prog_aplanado();
    let det = detect_cff(&p).unwrap();
    let plano = deflatten(&p, &det);
    // El dispatcher desaparece (ya nadie lo apunta).
    assert!(plano.block(1).is_none());
    // Los bloques van directo a su destino.
    let entradas = test_inputs(16, &["x".to_string()]);
    let prueba = prove_deflatten(&p, &plano, &entradas);
    assert!(prueba.passed, "{prueba:?}");
    assert_eq!(prueba.inputs_tested, 16);
}

#[test]
fn la_prueba_diferencial_detecta_una_equivalencia_falsa() {
    // Se falsea el bloque final del programa **desaplanado**: el aplanado ya
    // no es equivalente al original y la prueba tiene que decirlo.
    //
    // (OJO: falsear el ORIGINAL no serviría como prueba de nada: los dos
    // programas cambiarían a la vez y seguirían siendo equivalentes entre sí.
    // La prueba compara original contra aplanado, así que hay que tocar el
    // aplanado.)
    let p = prog_aplanado();
    let det = detect_cff(&p).unwrap();
    let mut plano = deflatten(&p, &det);
    let b = plano.block_mut(4).expect("bloque final");
    b.stmts = vec![Stmt::Effect("C-MAL".into())];
    let entradas = test_inputs(4, &["x".to_string()]);
    let prueba = prove_deflatten(&p, &plano, &entradas);
    assert!(!prueba.passed);
    assert!(prueba.mismatch.is_some());
}

#[test]
fn el_informe_devuelve_la_prueba() {
    let p = prog_aplanado();
    let rep = analyze_cff(&p, 32);
    assert!(rep.detected);
    let prueba = rep.proof.unwrap();
    assert!(prueba.passed);
    assert_eq!(prueba.inputs_tested, 32);
}

#[test]
fn la_traza_refleja_los_efectos() {
    let p = prog_aplanado();
    let mut inp = BTreeMap::new();
    inp.insert("x".to_string(), 7i64);
    let t = run(&p, &inp);
    assert_eq!(t.effects, vec!["A", "B", "C"]);
    assert!(t.halted);
}

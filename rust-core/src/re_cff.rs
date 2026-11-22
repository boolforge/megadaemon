//! MegaDaemon v13.4 — Control-flow flattening: detección, **desaplanado** y
//! **prueba de equivalencia** por ejecución diferencial.
//!
//! ## QUÉ ES EL CONTROL-FLOW FLATTENING
//!
//! El aplanador convierte el flujo original
//!
//! ```text
//! A -> B -> C -> D
//! ```
//!
//! en una máquina de estados con un *dispatcher*:
//!
//! ```text
//! A -> dispatcher(estado=1) -> B -> dispatcher(estado=2) -> C -> ...
//! ```
//!
//! El resultado es un CFG con un nodo gigante (el dispatcher) al que apuntan
//! casi todos los bloques: grado de entrada altísimo, out-degree bajo.
//!
//! ## POR QUÉ UN LENGUAJE PROPIO Y NO UN CFG DE ASSEMBLER
//!
//! Para *demostrar* que el programa desaplanado se comporta igual que el
//! original hace falta poder **ejecutar** los dos: el aplanado se prueba como
//! una transformación de un programa pequeño (bloques, sentencias y
//! terminadores) y la equivalencia se decide **por ejecución diferencial**
//! (mismas entradas → mismos efectos, mismos valores finales, misma
//! terminación).  Un CFG de assembler no se puede ejecutar sin emularlo.
//!
//! El emparejamiento con el binario real se hace en [`crate::re_ir`], que
//! traduce un desensamblado a este mismo modelo de bloques.
//!
//! ## LAS CUATRO PIEZAS
//!
//! * [`detect_cff`]: heurística de detección (bloque con `Switch` + muchos
//!   bloques que le asignan el estado y saltan a él).
//! * [`deflatten`]: reescribe el programa quitando el dispatcher y saltando
//!   directo al bloque real de cada estado.
//! * [`run`]: interpreta un programa (efectos + valores + terminación).
//! * [`prove_deflatten`]: **la prueba**: ejecuta ambos programas con las
//!   mismas entradas y compara; si algo difiere, dice DÓNDE.

#![allow(dead_code)]

use std::collections::BTreeMap;

/// Valor de una expresión: constante o variable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SVal {
    /// Constante entera.
    Const(i64),
    /// Variable (nombre).
    Var(String),
}

impl SVal {
    /// Evalúa el valor con el entorno dado.
    pub fn eval(&self, vars: &BTreeMap<String, i64>) -> i64 {
        match self {
            SVal::Const(c) => *c,
            SVal::Var(v) => vars.get(v).copied().unwrap_or(0),
        }
    }

    /// Texto legible.
    pub fn render(&self) -> String {
        match self {
            SVal::Const(c) => c.to_string(),
            SVal::Var(v) => v.clone(),
        }
    }
}

/// Operadores binarios soportados por el intérprete.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    /// Suma.
    Add,
    /// Resta.
    Sub,
    /// Producto.
    Mul,
    /// XOR bit a bit.
    Xor,
    /// AND bit a bit.
    And,
    /// OR bit a bit.
    Or,
    /// Desplazamiento a la izquierda.
    Shl,
    /// Desplazamiento a la derecha (aritmético).
    Shr,
}

impl BinOp {
    /// Aplica el operador.
    pub fn apply(self, a: i64, b: i64) -> i64 {
        match self {
            BinOp::Add => a.wrapping_add(b),
            BinOp::Sub => a.wrapping_sub(b),
            BinOp::Mul => a.wrapping_mul(b),
            BinOp::Xor => a ^ b,
            BinOp::And => a & b,
            BinOp::Or => a | b,
            BinOp::Shl => a.wrapping_shl(b as u32),
            BinOp::Shr => a.wrapping_shr(b as u32),
        }
    }

    /// Nombre del operador.
    pub fn name(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Xor => "^",
            BinOp::And => "&",
            BinOp::Or => "|",
            BinOp::Shl => "<<",
            BinOp::Shr => ">>",
        }
    }
}

/// Sentencia de un bloque.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stmt {
    /// `dst = value`.
    Assign {
        /// Variable destino.
        dst: String,
        /// Valor asignado.
        value: SVal,
    },
    /// `dst = a <op> b`.
    BinAssign {
        /// Variable destino.
        dst: String,
        /// Operador.
        op: BinOp,
        /// Operando izquierdo.
        a: SVal,
        /// Operando derecho.
        b: SVal,
    },
    /// Efecto observable (una llamada a una API, una escritura en memoria…).
    Effect(String),
}

/// Cómo termina un bloque.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Terminator {
    /// Salto incondicional a otro bloque.
    Goto(usize),
    /// Salto condicional.
    Cond {
        /// Condición.
        cond: SVal,
        /// Bloque si la condición es distinta de cero.
        then_block: usize,
        /// Bloque si la condición es cero.
        else_block: usize,
    },
    /// `switch (state_var) { … }`: el dispatcher del aplanador.
    Switch {
        /// Variable de estado.
        state_var: String,
        /// Casos: (valor de estado -> bloque destino).
        cases: Vec<(i64, usize)>,
        /// Destino por defecto (si el estado no está en los casos).
        default: Option<usize>,
    },
    /// Fin del programa.
    Halt,
}

/// Bloque básico de un programa plano.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    /// Identificador del bloque.
    pub id: usize,
    /// Sentencias del bloque.
    pub stmts: Vec<Stmt>,
    /// Terminador del bloque.
    pub term: Terminator,
}

/// Programa plano (bloques con identificadores).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlatProgram {
    /// Bloque de entrada.
    pub entry: usize,
    /// Bloques del programa.
    pub blocks: Vec<Block>,
}

impl FlatProgram {
    /// Bloque por identificador.
    pub fn block(&self, id: usize) -> Option<&Block> {
        self.blocks.iter().find(|b| b.id == id)
    }

    /// Bloque por identificador (mutable).
    pub fn block_mut(&mut self, id: usize) -> Option<&mut Block> {
        self.blocks.iter_mut().find(|b| b.id == id)
    }

    /// Identificadores de los bloques.
    pub fn ids(&self) -> Vec<usize> {
        self.blocks.iter().map(|b| b.id).collect()
    }

    /// Estado asignado por un bloque (si asigna la variable `var`).
    pub fn estado_asignado(&self, id: usize, var: &str) -> Option<i64> {
        let b = self.block(id)?;
        for s in &b.stmts {
            match s {
                Stmt::Assign { dst, value } if dst.as_str() == var => {
                    return Some(value.eval(&BTreeMap::new()))
                }
                Stmt::BinAssign { dst, .. } if dst == var => return None,
                _ => {}
            }
        }
        None
    }
}

/// Lo que devuelve la detección de aplanado.
#[derive(Clone, Debug)]
pub struct CffDetection {
    /// Identificador del bloque dispatcher.
    pub dispatcher: usize,
    /// Variable de estado usada por el dispatcher.
    pub state_var: String,
    /// Bloques que asignan el estado (los "setters").
    pub setters: Vec<usize>,
    /// Casos del `switch` del dispatcher.
    pub cases: Vec<(i64, usize)>,
    /// Confianza heurística (0..=1).
    pub confidence: f64,
}

/// Resultado de una ejecución.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trace {
    /// Efectos observables, en orden.
    pub effects: Vec<String>,
    /// ¿Terminó el programa?
    pub halted: bool,
    /// Valores finales de las variables.
    pub vars: BTreeMap<String, i64>,
    /// Pasos ejecutados (protección contra bucles infinitos).
    pub steps: usize,
}

/// Detecta el dispatcher de un programa aplanado.
///
/// Heurística: hay un bloque con terminador `Switch`; ese bloque es el
/// dispatcher.  La confianza es la fracción de bloques que saltan al dispatcher
/// y además **asignan** un estado que está entre los casos: un `switch` que
/// nadie usa no es un dispatcher de aplanado.
pub fn detect_cff(p: &FlatProgram) -> Option<CffDetection> {
    let mut mejor: Option<CffDetection> = None;
    for b in &p.blocks {
        let (state_var, cases, default) = match &b.term {
            Terminator::Switch {
                state_var,
                cases,
                default,
            } => (state_var.clone(), cases.clone(), *default),
            _ => continue,
        };
        if cases.len() < 2 {
            continue;
        }
        // Bloques que saltan al dispatcher.
        let van: Vec<usize> = p
            .blocks
            .iter()
            .filter(|x| x.id != b.id && matches!(&x.term, Terminator::Goto(t) if *t == b.id))
            .map(|x| x.id)
            .collect();
        if van.len() < 2 {
            continue;
        }
        // Setters: bloques que asignan la variable de estado.
        let setters: Vec<usize> = p
            .blocks
            .iter()
            .filter(|x| {
                x.id != b.id
                    && x.stmts.iter().any(|s| match s {
                        Stmt::Assign { dst, .. } => dst == &state_var,
                        Stmt::BinAssign { dst, .. } => dst == &state_var,
                        _ => false,
                    })
            })
            .map(|x| x.id)
            .collect();
        // Confianza: de los que van al dispatcher, ¿cuántos asignan un estado
        // que está entre los casos?
        let claves: Vec<i64> = cases.iter().map(|(k, _)| *k).collect();
        let buenos = van
            .iter()
            .filter(|id| match p.estado_asignado(**id, &state_var) {
                Some(v) => claves.contains(&v),
                None => false,
            })
            .count();
        let confianza = buenos as f64 / van.len() as f64;
        let _ = default;
        let det = CffDetection {
            dispatcher: b.id,
            state_var,
            setters,
            cases,
            confidence: confianza,
        };
        let mejor_conf = mejor.as_ref().map(|d| d.confidence).unwrap_or(-1.0);
        if confianza > mejor_conf {
            mejor = Some(det);
        }
    }
    mejor
}

/// Devuelve el programa **desaplanado**: el dispatcher desaparece y cada bloque
/// salta directamente al bloque real que le corresponde según su estado.
///
/// Los identificadores de los bloques se conservan (menos el del dispatcher),
/// así que las referencias entre bloques siguen siendo válidas.
pub fn deflatten(p: &FlatProgram, det: &CffDetection) -> FlatProgram {
    let mut out = FlatProgram {
        entry: p.entry,
        blocks: Vec::new(),
    };
    let claves: Vec<i64> = det.cases.iter().map(|(k, _)| *k).collect();
    for b in &p.blocks {
        if b.id == det.dispatcher {
            continue;
        }
        let mut nb = b.clone();
        if let Terminator::Goto(t) = nb.term {
            if t == det.dispatcher {
                match p.estado_asignado(b.id, &det.state_var) {
                    Some(v) => {
                        if let Some((_, dest)) = det.cases.iter().find(|(k, _)| *k == v) {
                            nb.term = Terminator::Goto(*dest);
                        } else if let Some(d) = det.cases.first().map(|(_, d)| *d) {
                            nb.term = Terminator::Goto(d);
                        }
                    }
                    None => {
                        // Sin estado asignado: no se puede desenredar; se deja
                        // el salto al dispatcher (que ya no existe) y se avisa
                        // con el `default` del switch si lo había.
                        if let Some(Terminator::Switch { default, .. }) = p
                            .block(det.dispatcher)
                            .map(|b| b.term.clone())
                            .as_ref()
                        {
                            if let Some(d) = default {
                                nb.term = Terminator::Goto(*d);
                            }
                        }
                    }
                }
            }
        }
        out.blocks.push(nb);
    }
    let _ = claves;
    out
}

/// Interpreta un programa y devuelve su traza.
pub fn run(p: &FlatProgram, input: &BTreeMap<String, i64>) -> Trace {
    let mut vars: BTreeMap<String, i64> = input.clone();
    let mut effects: Vec<String> = Vec::new();
    let mut steps = 0usize;
    const MAX_STEPS: usize = 100_000;
    let mut cur = p.entry;
    let mut halted = false;
    while steps < MAX_STEPS {
        steps += 1;
        let b = match p.block(cur) {
            Some(b) => b.clone(),
            None => break,
        };
        for s in &b.stmts {
            match s {
                Stmt::Assign { dst, value } => {
                    vars.insert(dst.clone(), value.eval(&vars));
                }
                Stmt::BinAssign { dst, op, a, b } => {
                    let va = a.eval(&vars);
                    let vb = b.eval(&vars);
                    vars.insert(dst.clone(), op.apply(va, vb));
                }
                Stmt::Effect(e) => effects.push(e.clone()),
            }
        }
        match &b.term {
            Terminator::Goto(t) => cur = *t,
            Terminator::Cond {
                cond,
                then_block,
                else_block,
            } => {
                cur = if cond.eval(&vars) != 0 {
                    *then_block
                } else {
                    *else_block
                };
            }
            Terminator::Switch {
                state_var,
                cases,
                default,
            } => {
                let v = vars.get(state_var).copied().unwrap_or(0);
                match cases.iter().find(|(k, _)| *k == v) {
                    Some((_, t)) => cur = *t,
                    None => match default {
                        Some(d) => cur = *d,
                        None => {
                            halted = true;
                            break;
                        }
                    },
                }
            }
            Terminator::Halt => {
                halted = true;
                break;
            }
        }
    }
    Trace {
        effects,
        halted,
        vars,
        steps,
    }
}

/// Genera `n` entornos de prueba deterministas para las variables dadas.
pub fn test_inputs(n: usize, vars: &[String]) -> Vec<BTreeMap<String, i64>> {
    let mut out = Vec::with_capacity(n);
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    for i in 0..n {
        let mut env = BTreeMap::new();
        for (k, v) in vars.iter().enumerate() {
            // El primer entorno es "todo ceros" y el resto barre valores
            // crecientes: determinista y reproducible.
            let val = if i == 0 {
                0
            } else {
                state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
                let mut z = state;
                z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
                z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
                z ^= z >> 31;
                (z >> (k % 8)) as i64
            };
            env.insert(v.clone(), val);
        }
        out.push(env);
    }
    out
}

/// Diferencia encontrada entre dos trazas.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mismatch {
    /// Entrada con la que se detectó.
    pub input: BTreeMap<String, i64>,
    /// Motivo (`efectos`, `terminacion` o `variables`).
    pub motivo: String,
    /// Traza del original.
    pub original: Trace,
    /// Traza del aplanado.
    pub aplanado: Trace,
}

/// Resultado de la prueba de equivalencia.
#[derive(Clone, Debug)]
pub struct DeflattenProof {
    /// ¿Son equivalentes?
    pub passed: bool,
    /// Cuántas entradas se probaron.
    pub inputs_tested: u64,
    /// Primera diferencia (si la hay).
    pub mismatch: Option<Mismatch>,
    /// Descripción legible del método.
    pub detail: String,
}

/// **La prueba**: ejecuta el original y el aplanado con las mismas entradas y
/// compara efectos, terminación y valores finales.
///
/// No es un muestreo "a ojo": es una comparación de **trazas completas**, y la
/// diferencia se localiza (entrada, motivo y ambas trazas).
pub fn prove_deflatten(
    orig: &FlatProgram,
    plano: &FlatProgram,
    inputs: &[BTreeMap<String, i64>],
) -> DeflattenProof {
    let mut probados = 0u64;
    for env in inputs {
        probados += 1;
        let t1 = run(orig, env);
        let t2 = run(plano, env);
        let mut motivo: Option<String> = None;
        if t1.effects != t2.effects {
            motivo = Some(format!(
                "efectos distintos: {:?} vs {:?}",
                t1.effects, t2.effects
            ));
        } else if t1.halted != t2.halted {
            motivo = Some(format!(
                "terminación distinta: halted={} vs halted={}",
                t1.halted, t2.halted
            ));
        } else {
            let claves: Vec<&String> = t1
                .vars
                .keys()
                .chain(t2.vars.keys())
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            for k in claves {
                let a = t1.vars.get(k).copied().unwrap_or(0);
                let b = t2.vars.get(k).copied().unwrap_or(0);
                if a != b {
                    motivo = Some(format!("variable {k}: {a} vs {b}"));
                    break;
                }
            }
        }
        if let Some(motivo) = motivo {
            return DeflattenProof {
                passed: false,
                inputs_tested: probados,
                mismatch: Some(Mismatch {
                    input: env.clone(),
                    motivo,
                    original: t1,
                    aplanado: t2,
                }),
                detail: format!("diferencia tras {probados} entradas"),
            };
        }
    }
    DeflattenProof {
        passed: true,
        inputs_tested: probados,
        mismatch: None,
        detail: format!(
            "las {} trazas coinciden (efectos, terminación y variables finales)",
            probados
        ),
    }
}

/// Informe completo del aplanado.
#[derive(Clone, Debug)]
pub struct CffReport {
    /// ¿Se detectó aplanado?
    pub detected: bool,
    /// Bloque dispatcher (si lo hay).
    pub dispatcher: Option<usize>,
    /// Variable de estado.
    pub state_var: String,
    /// Cuántos bloques asignan el estado.
    pub setters: usize,
    /// Cuántos casos tiene el switch.
    pub cases: usize,
    /// Confianza de la detección.
    pub confidence: f64,
    /// Cadena de bloques recuperada (orden de ejecución).
    pub cadena: Vec<usize>,
    /// Prueba de equivalencia (si se pudo hacer).
    pub proof: Option<DeflattenProof>,
}

/// Analiza un programa: detecta el aplanado, lo deshace y **prueba** que el
/// resultado se comporta igual que el original con `n` entradas.
pub fn analyze_cff(p: &FlatProgram, n: usize) -> CffReport {
    let det = detect_cff(p);
    let Some(det) = det else {
        return CffReport {
            detected: false,
            dispatcher: None,
            state_var: String::new(),
            setters: 0,
            cases: 0,
            confidence: 0.0,
            cadena: Vec::new(),
            proof: None,
        };
    };
    let plano = deflatten(p, &det);
    let vars: Vec<String> = p
        .blocks
        .iter()
        .flat_map(|b| b.stmts.iter())
        .flat_map(|s| match s {
            Stmt::BinAssign { a, b, .. } => {
                let mut out = Vec::new();
                if let SVal::Var(v) = a {
                    out.push(v.clone());
                }
                if let SVal::Var(v) = b {
                    out.push(v.clone());
                }
                out
            }
            _ => Vec::new(),
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let entradas = test_inputs(n, &vars);
    let proof = prove_deflatten(p, &plano, &entradas);
    // Cadena de bloques: se sigue el flujo desde la entrada.
    let mut cadena = Vec::new();
    let mut cur = p.entry;
    let mut vistas = std::collections::BTreeSet::new();
    while vistas.insert(cur) {
        cadena.push(cur);
        let Some(b) = p.block(cur) else { break };
        let next = match &b.term {
            Terminator::Goto(t) => *t,
            Terminator::Cond {
                then_block,
                else_block,
                ..
            } => *then_block,
            Terminator::Switch { cases, default, .. } => {
                let v = p.estado_asignado(cur, &det.state_var).unwrap_or(0);
                match cases.iter().find(|(k, _)| *k == v) {
                    Some((_, t)) => *t,
                    None => default.unwrap_or(cur),
                }
            }
            Terminator::Halt => break,
        };
        cur = next;
    }
    CffReport {
        detected: true,
        dispatcher: Some(det.dispatcher),
        state_var: det.state_var.clone(),
        setters: det.setters.len(),
        cases: det.cases.len(),
        confidence: det.confidence,
        cadena,
        proof: Some(proof),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prog() -> FlatProgram {
        FlatProgram {
            entry: 0,
            blocks: vec![
                Block {
                    id: 0,
                    stmts: vec![Stmt::Assign {
                        dst: "state".into(),
                        value: SVal::Const(1),
                    }],
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
                            value: SVal::Const(2),
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
                            value: SVal::Const(3),
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
        let d = detect_cff(&prog()).expect("debe detectar");
        assert_eq!(d.dispatcher, 1);
        assert_eq!(d.state_var, "state");
        assert_eq!(d.setters.len(), 3);
        assert_eq!(d.cases.len(), 3);
        assert!(d.confidence > 0.99);
    }

    #[test]
    fn el_programa_normal_no_se_declara_aplanado() {
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
    fn el_desaplanado_es_equivalente() {
        let p = prog();
        let det = detect_cff(&p).unwrap();
        let plano = deflatten(&p, &det);
        assert!(plano.block(1).is_none());
        let entradas = test_inputs(16, &["x".to_string()]);
        let prueba = prove_deflatten(&p, &plano, &entradas);
        assert!(prueba.passed, "{prueba:?}");
        assert_eq!(prueba.inputs_tested, 16);
    }
}

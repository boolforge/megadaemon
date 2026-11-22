//! MegaDaemon v13.4 — Predicados opacos: detección y **plegado** de ramas.
//!
//! ## QUÉ ES UN PREDICADO OPACO
//!
//! Una condición que **siempre** se cumple o **nunca** se cumple, pero que el
//! ofuscador presenta como si dependiera de datos.  Casos típicos:
//!
//! * `(x ^ x) == 0` → siempre cierto;
//! * `(x & ~x) == 0` → siempre cierto;
//! * `x * 0 == 0` → siempre cierto;
//! * una comparación con un valor que el propio bloque deja constante:
//!   `xor eax, eax` seguido de `cmp eax, 0`.
//!
//! ## LOS DOS NIVELES DEL MOTOR
//!
//! * **Expresión** ([`crate::re_mba`]): se decide con la forma canónica BDD de
//!   la expresión ⇒ la equivalencia vale para **todas** las entradas, no para
//!   las probadas.  Se expone aquí como [`analizar_opaco_expresion`].
//! * **Bloque de código** (este módulo): se sigue el valor de los registros
//!   dentro del bloque; si el registro comparado tiene un valor **constante
//!   conocido**, la rama es opaca y se puede plegar.
//!
//! ## LA PRUEBA
//!
//! En el nivel de bloque la decisión **no es una heurística**: el motor sabe
//! el valor exacto del registro en el punto de la comparación, así que la
//! conclusión es una consecuencia lógica, no un muestreo.  El informe lleva el
//! valor, la comparación y la frase que lo demuestra.

#![allow(dead_code)]

use crate::re_ir::{Block, Cfg, Flow, Ins};
use std::collections::BTreeMap;

/// Veredicto sobre una condición.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Veredicto {
    /// La condición se cumple siempre.
    SiempreCierto,
    /// La condición nunca se cumple.
    SiempreFalso,
    /// No se puede decidir con la información del bloque.
    Desconocido,
}

impl Veredicto {
    /// Texto legible.
    pub fn label(&self) -> &'static str {
        match self {
            Veredicto::SiempreCierto => "siempre-cierto",
            Veredicto::SiempreFalso => "siempre-falso",
            Veredicto::Desconocido => "desconocido",
        }
    }
}

/// Un predicado opaco detectado.
#[derive(Clone, Debug)]
pub struct Opaco {
    /// Dirección de la instrucción de salto.
    pub addr: u64,
    /// Mnemónico de la condición (`je`, `jne`…).
    pub cc: String,
    /// Dirección del destino de la rama.
    pub destino: u64,
    /// Valor constante del registro comparado (si se conoce).
    pub valor: Option<u64>,
    /// Inmediato de la comparación.
    pub inmediato: Option<i64>,
    /// Veredicto.
    pub veredicto: Veredicto,
    /// Texto del bloque (evidencia).
    pub bloque: String,
    /// Por qué es opaco (la demostración).
    pub prueba: String,
}

/// Seguimiento de constantes dentro de un bloque.
///
/// Sólo se modelan las operaciones que *dejan un valor conocido*: `mov reg,
/// inm`, `xor reg, reg`, `sub reg, reg`, `and reg, 0` y `mov reg, reg`.  Si el
/// bloque hace cualquier otra cosa con el registro, se pierde la constante
/// (no se inventa).
pub fn constantes_del_bloque(b: &Block) -> BTreeMap<String, u64> {
    let mut vals: BTreeMap<String, u64> = BTreeMap::new();
    for ins in &b.ins {
        match ins.mnem.as_str() {
            "mov" | "movabs" => {
                let dst = ins.op0().to_string();
                if let Some(v) = crate::re_ir::parse_int(ins.op1()) {
                    vals.insert(dst, v as u64);
                } else if vals.contains_key(ins.op1()) {
                    // `mov dst, src` copia el valor conocido de `src`.
                    let v = vals[ins.op1()];
                    vals.insert(dst, v);
                } else {
                    vals.remove(&dst);
                }
            }
            "xor" => {
                if ins.op0() == ins.op1() {
                    vals.insert(ins.op0().to_string(), 0);
                } else {
                    vals.remove(ins.op0());
                }
            }
            "sub" => {
                if ins.op0() == ins.op1() {
                    vals.insert(ins.op0().to_string(), 0);
                } else {
                    vals.remove(ins.op0());
                }
            }
            "and" => {
                if let Some(v) = crate::re_ir::parse_int(ins.op1()) {
                    if v == 0 {
                        vals.insert(ins.op0().to_string(), 0);
                    } else {
                        vals.remove(ins.op0());
                    }
                } else {
                    vals.remove(ins.op0());
                }
            }
            "cmp" | "test" | "jmp" | "je" | "jne" | "jz" | "jnz" | "jg" | "jge" | "jl"
            | "jle" | "ja" | "jae" | "jb" | "jbe" => {}
            _ => {
                // Cualquier otra escritura invalida el valor conocido.
                vals.remove(ins.op0());
            }
        }
    }
    vals
}

/// Decide una comparación con valores conocidos.
///
/// `cmp reg, inm` con `reg = c`:
/// * `je` → cierto sii `c == inm`;
/// * `jne` → cierto sii `c != inm`;
/// * `jg`/`jl` → comparación con signo.
pub fn decide_comparacion(cc: &str, c: u64, inm: i64) -> Veredicto {
    let c = c as i64;
    match cc {
        "je" | "jz" | "jeq" => {
            if c == inm {
                Veredicto::SiempreCierto
            } else {
                Veredicto::SiempreFalso
            }
        }
        "jne" | "jnz" => {
            if c != inm {
                Veredicto::SiempreCierto
            } else {
                Veredicto::SiempreFalso
            }
        }
        "jg" => {
            if c > inm {
                Veredicto::SiempreCierto
            } else {
                Veredicto::SiempreFalso
            }
        }
        "jge" => {
            if c >= inm {
                Veredicto::SiempreCierto
            } else {
                Veredicto::SiempreFalso
            }
        }
        "jl" => {
            if c < inm {
                Veredicto::SiempreCierto
            } else {
                Veredicto::SiempreFalso
            }
        }
        "jle" => {
            if c <= inm {
                Veredicto::SiempreCierto
            } else {
                Veredicto::SiempreFalso
            }
        }
        "ja" => {
            if (c as u64) > (inm as u64) {
                Veredicto::SiempreCierto
            } else {
                Veredicto::SiempreFalso
            }
        }
        "jae" => {
            if (c as u64) >= (inm as u64) {
                Veredicto::SiempreCierto
            } else {
                Veredicto::SiempreFalso
            }
        }
        "jb" => {
            if (c as u64) < (inm as u64) {
                Veredicto::SiempreCierto
            } else {
                Veredicto::SiempreFalso
            }
        }
        "jbe" => {
            if (c as u64) <= (inm as u64) {
                Veredicto::SiempreCierto
            } else {
                Veredicto::SiempreFalso
            }
        }
        _ => Veredicto::Desconocido,
    }
}

/// Detecta los predicados opacos de un bloque.
pub fn detectar_opacos_del_bloque(b: &Block) -> Vec<Opaco> {
    let mut out = Vec::new();
    let vals = constantes_del_bloque(b);
    let Some(ultimo) = b.ins.last() else {
        return out;
    };
    let Flow::Cond { target, cc } = &b.flow else {
        return out;
    };
    // La comparación es la instrucción anterior al salto (o dos antes).
    let mut cmp_ins: Option<&Ins> = None;
    let n = b.ins.len();
    for d in 1..=2usize {
        if n < d {
            break;
        }
        let cand = &b.ins[n - d];
        if cand.mnem == "cmp" || cand.mnem == "test" {
            cmp_ins = Some(cand);
            break;
        }
    }
    let Some(cmp_ins) = cmp_ins else {
        return out;
    };
    let inmediato = crate::re_ir::parse_int(cmp_ins.op1());
    let valor = vals.get(cmp_ins.op0()).copied();
    let veredicto = match (valor, inmediato) {
        (Some(c), Some(i)) => decide_comparacion(cc, c, i),
        _ => Veredicto::Desconocido,
    };
    if veredicto == Veredicto::Desconocido {
        return out;
    }
    let prueba = match (valor, inmediato) {
        (Some(c), Some(i)) => format!(
            "{} = 0x{:x} en este punto y la comparación es `cmp {}, {}` ⇒ la condición {} es {}",
            cmp_ins.op0(),
            c,
            cmp_ins.op0(),
            i,
            cc,
            veredicto.label()
        ),
        _ => "valor constante no conocido".to_string(),
    };
    out.push(Opaco {
        addr: ultimo.addr,
        cc: cc.clone(),
        destino: *target,
        valor,
        inmediato,
        veredicto,
        bloque: b.render(),
        prueba,
    });
    out
}

/// Detecta los predicados opacos de todo un CFG.
pub fn detectar_opacos(cfg: &Cfg) -> Vec<Opaco> {
    let mut out = Vec::new();
    for b in &cfg.blocks {
        out.extend(detectar_opacos_del_bloque(b));
    }
    out
}

/// Plega un bloque con un predicado opaco: la rama muerta desaparece.
///
/// * Condición **siempre cierta** → el bloque termina en salto incondicional
///   al destino.
/// * Condición **siempre falsa** → el bloque termina en caída (el destino es
///   código muerto).
pub fn plegar_bloque(b: &Block, op: &Opaco) -> Block {
    let mut nb = b.clone();
    match op.veredicto {
        Veredicto::SiempreCierto => nb.flow = Flow::Jump(op.destino),
        Veredicto::SiempreFalso => nb.flow = Flow::Fallthrough,
        Veredicto::Desconocido => {}
    }
    nb
}

/// Plega todos los predicados opacos de un CFG y devuelve el CFG plegado.
pub fn plegar(cfg: &Cfg) -> (Cfg, Vec<Opaco>) {
    let ops = detectar_opacos(cfg);
    let mut bloques = Vec::with_capacity(cfg.blocks.len());
    for b in &cfg.blocks {
        match ops.iter().find(|o| o.addr == b.ins.last().map(|i| i.addr).unwrap_or(0)) {
            Some(o) => bloques.push(plegar_bloque(b, o)),
            None => bloques.push(b.clone()),
        }
    }
    (Cfg { blocks: bloques }, ops)
}

/// Analiza un predicado **expresivo** (MBA) con la forma canónica BDD.
///
/// Es el nivel de expresión: la decisión es completa (vale para las 2^w
/// entradas), no un muestreo.
pub fn analizar_opaco_expresion(
    expr: &crate::re_mba::MbaExpr,
) -> (crate::re_mba::OpaqueVerdict, crate::re_mba::MbaProof) {
    crate::re_mba::analyze_opaque_predicate(expr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::re_ir::Ins;

    fn bloque(ins: Vec<Ins>, flow: Flow) -> Block {
        Block {
            addr: ins[0].addr,
            ins,
            flow,
        }
    }

    #[test]
    fn detecta_el_predicado_opaco_por_constante() {
        let b = bloque(
            vec![
                Ins::new(0x1000, "xor", "eax, eax"),
                Ins::new(0x1004, "cmp", "eax, 0"),
                Ins::new(0x1008, "je", "0x1100"),
            ],
            Flow::Cond {
                target: 0x1100,
                cc: "je".into(),
            },
        );
        let ops = detectar_opacos_del_bloque(&b);
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].veredicto, Veredicto::SiempreCierto);
        assert_eq!(ops[0].valor, Some(0));
    }

    #[test]
    fn el_valor_constante_se_seguye_dentro_del_bloque() {
        let b = bloque(
            vec![
                Ins::new(0x1000, "mov", "eax, 5"),
                Ins::new(0x1004, "mov", "ebx, eax"),
                Ins::new(0x1008, "cmp", "ebx, 5"),
                Ins::new(0x100c, "jne", "0x1100"),
            ],
            Flow::Cond {
                target: 0x1100,
                cc: "jne".into(),
            },
        );
        let ops = detectar_opacos_del_bloque(&b);
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].veredicto, Veredicto::SiempreFalso);
    }

    #[test]
    fn una_comparacion_no_constante_no_se_declara_opaca() {
        // El registro NO tiene valor conocido: la condición puede ser cierta o
        // falsa segun los datos ⇒ no es opaca.
        let b = bloque(
            vec![
                Ins::new(0x1000, "cmp", "eax, 7"),
                Ins::new(0x1004, "je", "0x1100"),
            ],
            Flow::Cond {
                target: 0x1100,
                cc: "je".into(),
            },
        );
        assert!(detectar_opacos_del_bloque(&b).is_empty());
    }

    #[test]
    fn plegar_quita_la_rama_muerta() {
        let b = bloque(
            vec![
                Ins::new(0x1000, "xor", "eax, eax"),
                Ins::new(0x1004, "cmp", "eax, 0"),
                Ins::new(0x1008, "je", "0x1100"),
            ],
            Flow::Cond {
                target: 0x1100,
                cc: "je".into(),
            },
        );
        let ops = detectar_opacos_del_bloque(&b);
        let plegado = plegar_bloque(&b, &ops[0]);
        assert_eq!(plegado.flow, Flow::Jump(0x1100));
    }

    #[test]
    fn el_nivel_de_expresion_usa_el_bdd() {
        let e = crate::re_mba::parse("(x ^ x)").unwrap();
        let (v, p) = analizar_opaco_expresion(&e);
        assert_eq!(v, crate::re_mba::OpaqueVerdict::AlwaysZero);
        assert!(p.passed);
        assert_eq!(p.kind, crate::re_mba::ProofKind::CanonicalForm);
    }
}

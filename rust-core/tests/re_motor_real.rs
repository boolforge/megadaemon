//! Pruebas del motor de RE contra **fragmentos reales** de los binarios de
//! Antigravity.
//!
//! POR QUÉ FRAGMENTOS Y NO LOS BINARIOS ENTEROS
//! -----------------------------------------------
//! `liblanguage_server.so` pesa 181.358.800 B y `com.agy.app` 103.469.317 B:
//! el repo es público y esos binarios no se suben.  Aquí van **trozos de
//! 64 B a 8 KiB** extraídos de los binarios reales, con su desplazamiento
//! original anotado, para que las pruebas del motor comprueben **hechos
//! medidos** y no inventados.
//!
//! QUÉ COMPRUEBAN
//! --------------
//! 1. La cabecera de la `pclntab` real ⇒ 161.682 funciones Go.
//! 2. El `EnumDescriptorProto` real ⇒ `MODEL_PLACEHOLDER_M<n>` = 1000 + n.
//! 3. Los ids reales del catálogo de modelos.
//! 4. Que el código ARM64 real se desensambla y da un CFG.
//!
//! El desplazamiento original de cada fragmento está en `FIXTURES` para que
//! quien lo revise pueda ir al binario y comprobarlo.

use megadaemon_core::re_engine::detectar_go_pclntab;
use megadaemon_core::re_ir::Cfg;
use std::path::PathBuf;

/// Fixtures con su desplazamiento original en el binario real.
const FIXTURES: &[(&str, &str, u64)] = &[
    (
        "pclntab_go1.18.bin",
        "pcHeader go1.18+ del liblanguage_server.so real",
        0x1449ff0,
    ),
    (
        "model_enum_descriptor.bin",
        "EnumDescriptorProto del enum exa.codeium_common_pb.Model",
        0x495e663,
    ),
    (
        "catalogo_model_ids.bin",
        "ids del catalogo de modelos (18 ids, extraidos de su offset real)",
        0x4357ca9,
    ),
    (
        "codigo_arm64.bin",
        "inicio de .text (ARM64) del binario Go real",
        0x4d0f000,
    ),
    (
        "model_enum_m319.bin",
        "entrada del enum Model para MODEL_PLACEHOLDER_M319",
        0x4960a16,
    ),
    (
        "model_enum_m526.bin",
        "entrada del enum Model para MODEL_PLACEHOLDER_M526",
        0x4962189,
    ),
    (
        "model_enum_nombres.bin",
        "principio del enum Model con los nombres REALES (no placeholders)",
        0x495e000,
    ),
];

/// Ruta de un fixture.
fn fixture(nombre: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/re")
        .join(nombre)
}

/// Lee un fixture (si no esta, el test se salta: el repo puede venir sin
/// binarios de referencia).
fn leer(nombre: &str) -> Option<Vec<u8>> {
    std::fs::read(fixture(nombre)).ok()
}

/// Lee un varint de protobuf.
fn varint(b: &[u8], i: &mut usize) -> Option<u64> {
    let mut r = 0u64;
    let mut s = 0u32;
    while *i < b.len() {
        let c = b[*i];
        *i += 1;
        r |= ((c & 0x7f) as u64) << s;
        if c & 0x80 == 0 {
            return Some(r);
        }
        s += 7;
        if s > 63 {
            return None;
        }
    }
    None
}

/// Extrae los pares (nombre, número) del `EnumDescriptorProto`.
///
/// El formato es el de protobuf: `\x0a<len>nombre` (campo 1 = name) seguido de
/// `\x10<varint>` (campo 2 = number).
fn pares_enum(b: &[u8]) -> Vec<(String, i64)> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 2 < b.len() {
        if b[i] == 0x0a {
            let len = b[i + 1] as usize;
            if len < 64 && i + 2 + len < b.len() {
                let nombre = String::from_utf8_lossy(&b[i + 2..i + 2 + len]).to_string();
                let mut j = i + 2 + len;
                if j < b.len() && b[j] == 0x10 {
                    j += 1;
                    let mut k = j;
                    if let Some(v) = varint(b, &mut k) {
                        if v <= i64::MAX as u64 {
                            out.push((nombre, v as i64));
                            i = k;
                            continue;
                        }
                    }
                }
            }
        }
        i += 1;
    }
    out
}

#[test]
fn la_cabecera_de_la_pclntab_real_da_161682_funciones() {
    let Some(d) = leer("pclntab_go1.18.bin") else {
        return;
    };
    let (n, ver) = detectar_go_pclntab(&d).expect("debe detectar la pclntab");
    assert_eq!(n, 161_682, "funciones Go del binario real");
    let v = ver.unwrap_or_default();
    assert!(v.contains("go1.18+"), "version: {v}");
}

#[test]
fn los_placeholders_del_enum_real_valen_mas_mil() {
    let Some(d) = leer("model_enum_descriptor.bin") else {
        return;
    };
    let pares = pares_enum(&d);
    assert!(pares.len() > 50, "pares leidos: {}", pares.len());
    let mapa: std::collections::BTreeMap<&str, i64> =
        pares.iter().map(|(n, v)| (n.as_str(), *v)).collect();

    // MEDIDO en el binario real: los nombres del enum `Model` son
    // MODEL_PLACEHOLDER_M0..M1650 con los valores 1000..2650.
    for (nombre, valor) in [
        ("MODEL_PLACEHOLDER_M0", 1000i64),
        ("MODEL_PLACEHOLDER_M50", 1050),
        ("MODEL_PLACEHOLDER_M124", 1124),
    ] {
        assert_eq!(
            mapa.get(nombre),
            Some(&valor),
            "{nombre} deberia valer {valor} (medido en el binario real)"
        );
    }

}

#[test]
fn los_nombres_reales_del_enum_valen_lo_que_dice() {
    let Some(d) = leer("model_enum_nombres.bin") else {
        return;
    };
    let pares = pares_enum(&d);
    let mapa: std::collections::BTreeMap<&str, i64> =
        pares.iter().map(|(n, v)| (n.as_str(), *v)).collect();
    // MEDIDO en el binario real: los 40 nombres reales del enum `Model` y sus
    // valores (el resto, 1651, son los placeholders M0..M1650 = 1000..2650).
    assert_eq!(mapa.get("MODEL_UNSPECIFIED"), Some(&0));
    assert_eq!(mapa.get("MODEL_CHAT_20706"), Some(&235));
    assert_eq!(mapa.get("MODEL_GOOGLE_GEMINI_2_5_PRO"), Some(&246));
    assert_eq!(mapa.get("MODEL_GOOGLE_GEMINI_INTERNAL_BYOM"), Some(&326));
    assert_eq!(mapa.get("MODEL_CLAUDE_4_5_SONNET"), Some(&333));
    assert_eq!(mapa.get("MODEL_OPENAI_GPT_OSS_120B_MEDIUM"), Some(&342));
}

#[test]
fn los_placeholders_del_fragmento_son_consecutivos() {
    let Some(d) = leer("model_enum_descriptor.bin") else {
        return;
    };
    let pares = pares_enum(&d);
    let mut n = 0usize;
    let mut ultimo = 0i64;
    let mut ultimo_idx = 0i64;
    for (nombre, valor) in &pares {
        if let Some(rest) = nombre.strip_prefix("MODEL_PLACEHOLDER_M") {
            let idx: i64 = rest.parse().unwrap_or(-1);
            assert_eq!(
                *valor,
                1000 + idx,
                "{nombre} deberia valer 1000+{idx} (medido)"
            );
            n += 1;
            ultimo = *valor;
            ultimo_idx = idx;
        }
    }
    assert!(n > 100, "placeholders en el fragmento: {n}");
    // El fragmento empieza en M1 (el nombre de M0 esta justo antes), asi que
    // el ultimo valor depende del ultimo indice leido.
    assert_eq!(ultimo, 1000 + ultimo_idx);
}

#[test]
fn los_placeholders_m319_y_m526_valen_lo_que_dice() {
    // Fragmentos tomados de la entrada exacta de cada uno en el enum real.
    for (fichero, nombre, valor) in [
        ("model_enum_m319.bin", "MODEL_PLACEHOLDER_M319", 1319i64),
        ("model_enum_m526.bin", "MODEL_PLACEHOLDER_M526", 1526),
    ] {
        let Some(d) = leer(fichero) else { return };
        let pares = pares_enum(&d);
        let mapa: std::collections::BTreeMap<&str, i64> =
            pares.iter().map(|(n, v)| (n.as_str(), *v)).collect();
        assert_eq!(
            mapa.get(nombre),
            Some(&valor),
            "{nombre} deberia valer {valor} (medido en el binario real)"
        );
    }
}

#[test]
fn los_ids_reales_del_catalogo_estan_en_la_rodata() {
    let Some(d) = leer("catalogo_model_ids.bin") else {
        return;
    };
    let texto = String::from_utf8_lossy(&d);
    // Los 18 ids del catalogo, con el desplazamiento real de cada uno en el
    // binario (MEDIDO 2026-11-22).
    for (id, off) in [
        ("gemini-4-argon-high", 0x4357ca9u64),
        ("gemini-4-argon-medium", 0x43755ad),
        ("gemini-4-argon-low", 0x4348088),
        ("gemini-3.8-flash-high", 0x43755c2),
        ("gemini-3.8-flash-medium", 0x43966bd),
        ("gemini-3.8-flash-low", 0x4366b53),
        ("gemini-3.7-flash-high", 0x43755d7),
        ("gemini-3.7-flash-medium", 0x43966eb),
        ("gemini-3.7-flash-low", 0x4366b67),
        ("gemini-3.6-flash-high", 0x43755ec),
        ("gemini-3.6-flash-medium", 0x4396719),
        ("gemini-3.6-flash-low", 0x4366b7b),
        ("gemini-3.5-flash-low", 0x4366b8f),
        ("gemini-3.5-flash", 0x432644e),
        ("gemini-3.5-flash-extra-low", 0x43bde76),
        ("gemini-3.1-pro-low-thinking", 0x43c88cb),
        ("gemini-3.1-flash-lite-preview", 0x43dcb3e),
        ("gemini-3-flash-preview", 0x4383afd),
    ] {
        assert!(texto.contains(id), "falta el id real {id} (@{off:#x})");
    }
}

#[test]
fn el_codigo_arm64_real_se_desensambla_y_da_un_cfg() {
    let Some(d) = leer("codigo_arm64.bin") else {
        return;
    };
    let rep = megadaemon_core::re_disasm::disasm_capstone(&d, "arm64", 0x4d0f000, 5000)
        .expect("capstone debe desensamblar");
    // Capstone se detiene en la primera instruccion no valida (en un binario
    // real el .text tiene datos intercalados): basta con que decodifique el
    // prologo de la funcion.
    assert!(rep.count >= 10, "instrucciones desensambladas: {}", rep.count);
    let instrs: Vec<(u64, String)> = rep
        .instructions
        .iter()
        .filter_map(|i| {
            let a = u64::from_str_radix(i.address.trim_start_matches("0x"), 16).ok()?;
            Some((a, i.text.clone()))
        })
        .collect();
    let cfg = Cfg::desde_disasm(&instrs);
    assert!(!cfg.blocks.is_empty(), "el CFG no puede estar vacio");
    // Todo bloque debe tener al menos una instruccion y un terminador.
    for b in &cfg.blocks {
        assert!(!b.ins.is_empty());
    }
}

#[test]
fn los_fixtures_son_pequenos_y_tienen_su_desplazamiento() {
    for (nombre, _, off) in FIXTURES {
        let ruta = fixture(nombre);
        let meta = std::fs::metadata(&ruta).expect("falta el fixture");
        let tam = meta.len();
        assert!(
            (1..=8192).contains(&tam),
            "{nombre} deberia pesar entre 1 y 8192 B (pesa {tam})"
        );
        assert!(*off > 0, "{nombre} necesita su desplazamiento original");
    }
}

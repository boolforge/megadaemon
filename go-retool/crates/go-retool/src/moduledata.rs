//! Lectura del `moduledata` de Go en un ELF64 para obtener la base de texto REAL.
//!
//! POR QUE ESTE MODULO EXISTE
//! ---------------------------
//! La base de texto de Go NO es la direccion de la seccion `.text` ni la
//! minima direccion de una seccion ".text*": es `moduledata.text`, el valor
//! que el runtime usa para convertir un `entryoff` de la functab en una VA
//! (`f.entry() == moduledata.text + entryoff`).
//!
//! MEDIDO 2026-11-23 (liblanguage_server.so de Antigravity, Go 1.28 RC02):
//!   * `pcHeader.textStart` = 0  (en una biblioteca compartida el linker no
//!     puede conocer la VA final) => no sirve.
//!   * La heuristica por secciones daba 0x4d06cb8 (`.text.hot`, seccion de
//!     TAMANO 0) => **error de 0x83F8 bytes** en TODAS las direcciones.
//!   * `moduledata.text` = **0x4d0f0b0** (leido de la seccion `.go.module`).
//!
//! COMO SE LEE
//! -----------
//! El linker de Go escribe el `runtime.firstmoduledata` en la seccion
//! `.go.module`.  En un ELF PIE los punteros no estan en el fichero: los pone
//! una tabla de relocalizaciones (`.rela.dyn`) de tipo R_*_RELATIVE, cuyo
//! `r_addend` es el valor de enlace (la VA antes de aplicar el sesgo de
//! carga).  En un ELF no-PIE el valor ya esta escrito en el fichero.
//!
//! El layout de `runtime.moduledata` (Go 1.18+, 64 bits) es:
//!   +0x00 pcHeader *pcHeader | +0x08 funcnametab []byte | +0x20 cutab []uint32
//!   +0x38 filetab []byte | +0x50 pctab []byte | +0x68 pclntable []byte
//!   +0x80 ftab []functab | +0x98 findfunctab | +0xA0 minpc | +0xA8 maxpc
//!   +0xB0 text | +0xB8 etext
//! (los offsets se han CONFIRMADO contra las relocalizaciones reales del
//! binario de Antigravity: +0xA0=minpc, +0xA8=maxpc, +0xB0=text, +0xB8=etext).
#![forbid(unsafe_code)]

/// Offsets de los campos que interesan dentro de `runtime.moduledata`.
pub const OFF_MINPC: u64 = 0xA0;
pub const OFF_MAXPC: u64 = 0xA8;
pub const OFF_TEXT: u64 = 0xB0;
pub const OFF_ETEXT: u64 = 0xB8;

/// Tipos de relocalizacion "relativa": el valor final es `base + r_addend`,
/// asi que `r_addend` es la VA de enlace que buscamos.
const RELATIVE_TYPES: [u32; 4] = [
    8,    // R_X86_64_RELATIVE / R_386_RELATIVE
    1027, // R_AARCH64_RELATIVE
    23,   // R_ARM_RELATIVE
    3,    // R_RISCV_RELATIVE
];

/// La base de texto y sus limites, tal y como los declara el propio binario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moduledata {
    /// `moduledata.text`: base de las VAs de las funciones Go.
    pub text: u64,
    /// `moduledata.etext`: fin del texto.
    pub etext: u64,
    /// `moduledata.minpc`: PC de la primera funcion (== `text` + entryoff[0]).
    pub minpc: u64,
    /// `moduledata.maxpc`: PC de fin de la ultima funcion.
    pub maxpc: u64,
    /// De donde salio el valor (para el informe).
    pub source: String,
}

fn u16le(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(d.get(o..o + 2)?.try_into().ok()?))
}
fn u32le(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(d.get(o..o + 4)?.try_into().ok()?))
}
fn u64le(d: &[u8], o: usize) -> Option<u64> {
    Some(u64::from_le_bytes(d.get(o..o + 8)?.try_into().ok()?))
}

/// Localiza la seccion `.go.module` (el `runtime.firstmoduledata`) y lee
/// `text`/`etext`/`minpc`/`maxpc`.
///
/// Devuelve `None` si el binario no es un ELF64 little-endian, si no tiene
/// `.go.module`, o si los campos no son utilizables (base 0 o incoherente).
pub fn read_moduledata(data: &[u8]) -> Option<Moduledata> {
    if data.get(0..4)? != b"\x7fELF" {
        return None;
    }
    if data.get(4)? != &2 || data.get(5)? != &1 {
        return None; // solo ELF64 little-endian
    }
    let e_shoff = u64le(data, 0x28)? as usize;
    let e_shentsize = u16le(data, 0x3A)? as usize;
    let e_shnum = u16le(data, 0x3C)? as usize;
    let e_shstrndx = u16le(data, 0x3E)? as usize;
    if e_shentsize < 64 || e_shoff == 0 {
        return None;
    }
    let sh = |i: usize| -> Option<[u64; 7]> {
        let b = e_shoff.checked_add(i.checked_mul(e_shentsize)?)?;
        let d = data.get(b..b + e_shentsize)?;
        Some([
            u32le(d, 0)? as u64,          // sh_name
            u32le(d, 4)? as u64,          // sh_type
            u64le(d, 16)?,                // sh_addr
            u64le(d, 24)?,                // sh_offset
            u64le(d, 32)?,                // sh_size
            u64le(d, 40)?,                // sh_link
            u64le(d, 56)?,                // sh_entsize
        ])
    };
    let strtab = sh(e_shstrndx)?;
    let names = data.get(strtab[3] as usize..(strtab[3] as usize + strtab[4] as usize))?;
    let name_at = |off: u64| -> String {
        let o = off as usize;
        let end = names.get(o..).and_then(|s| s.iter().position(|&c| c == 0));
        match end {
            Some(e) => String::from_utf8_lossy(&names[o..o + e]).into_owned(),
            None => String::new(),
        }
    };
    // .go.module = el runtime.firstmoduledata escrito por el linker.
    let mut md = None;
    let mut rela = None;
    for i in 0..e_shnum {
        let s = match sh(i) {
            Some(v) => v,
            None => continue,
        };
        let n = name_at(s[0]);
        if n == ".go.module" {
            md = Some(s);
        }
        if n == ".rela.dyn" {
            rela = Some(s);
        }
    }
    let md = md?;
    let (md_addr, md_off, md_size) = (md[2], md[3], md[4]);
    if md_size < OFF_ETEXT + 8 {
        return None;
    }
    // Valores por campo: primero las relocalizaciones, luego el fichero.
    let mut campos: Vec<(u64, u64)> = Vec::new();
    if let Some(r) = rela {
        let n = (r[4] / 24) as usize;
        for k in 0..n {
            let o = r[3] as usize + k * 24;
            let d = data.get(o..o + 24)?;
            let r_offset = u64le(d, 0)?;
            let r_info = u64le(d, 8)?;
            let r_addend = u64le(d, 16)?;
            let r_type = (r_info & 0xffff_ffff) as u32;
            if !RELATIVE_TYPES.contains(&r_type) {
                continue;
            }
            if r_offset < md_addr || r_offset >= md_addr + md_size {
                continue;
            }
            campos.push((r_offset - md_addr, r_addend));
        }
    }
    let campo = |off: u64| -> Option<u64> {
        if let Some((_, v)) = campos.iter().find(|(k, _)| *k == off) {
            return Some(*v);
        }
        // ELF no-PIE: el valor ya esta en el fichero.
        let o = md_off as usize + off as usize;
        let v = u64le(data, o)?;
        if v == 0 {
            None
        } else {
            Some(v)
        }
    };
    let text = campo(OFF_TEXT)?;
    let etext = campo(OFF_ETEXT).unwrap_or(0);
    let minpc = campo(OFF_MINPC).unwrap_or(text);
    let maxpc = campo(OFF_MAXPC).unwrap_or(0);
    if text == 0 {
        return None;
    }
    if etext != 0 && etext <= text {
        return None;
    }
    let source = if campos.is_empty() {
        "moduledata.text (ELF .go.module, valor en el fichero)".to_string()
    } else {
        "moduledata.text (ELF .go.module + relocalizacion R_*_RELATIVE)".to_string()
    };
    Some(Moduledata {
        text,
        etext,
        minpc,
        maxpc,
        source,
    })
}

/// Comprueba una base de texto contra el `moduledata`.
///
/// Invariantes del runtime de Go:
///   * `base + entryoff[0] == minpc`  (la primera funcion)
///   * `base + sentinel == maxpc`     (la ultima funcion, por su fin)
///
/// MEDIDO 2026-11-23: con la base heuristica 0x4d06cb8 estas dos
/// comprobaciones FALLAN (daban 0x4d06cb8 y 0x71c89d9); con la base real
/// 0x4d0f0b0 pasan las dos.
pub fn verify_text_base(
    md: &Moduledata,
    base: u64,
    first_entryoff: u64,
    last_end_off: u64,
) -> Result<(), String> {
    let primero = base
        .checked_add(first_entryoff)
        .ok_or_else(|| "desbordamiento al sumar base+entryoff".to_string())?;
    if md.minpc != 0 && primero != md.minpc {
        return Err(format!(
            "la primera funcion caeria en 0x{primero:x} pero moduledata.minpc es 0x{:x}",
            md.minpc
        ));
    }
    let ultimo = base
        .checked_add(last_end_off)
        .ok_or_else(|| "desbordamiento al sumar base+sentinel".to_string())?;
    if md.maxpc != 0 && ultimo != md.maxpc {
        return Err(format!(
            "la ultima funcion terminaria en 0x{ultimo:x} pero moduledata.maxpc es 0x{:x}",
            md.maxpc
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Construye un ELF64 minimo con `.go.module` y `.rela.dyn`.
    /// Devuelve (elf, valor_de_text_en_el_fichero).
    fn elf_sintetico(text: u64, con_reloc: bool) -> Vec<u8> {
        const MD_ADDR: u64 = 0x9000;
        const MD_OFF: u64 = 0x1000;
        const RELA_ADDR: u64 = 0xa000;
        const RELA_OFF: u64 = 0x2000;
        const SH_OFF: u64 = 0x3000;
        let mut d = vec![0u8; SH_OFF as usize + 3 * 64];
        d[0..4].copy_from_slice(b"\x7fELF");
        d[4] = 2; // ELFCLASS64
        d[5] = 1; // little-endian
        d[0x28..0x30].copy_from_slice(&SH_OFF.to_le_bytes());
        d[0x3A..0x3C].copy_from_slice(&64u16.to_le_bytes());
        d[0x3C..0x3E].copy_from_slice(&3u16.to_le_bytes()); // 3 secciones
        d[0x3E..0x40].copy_from_slice(&2u16.to_le_bytes()); // shstrndx = 2
        // .go.module: sh_addr=MD_ADDR, offset=MD_OFF, size=0x238
        let mut s0 = vec![0u8; 64];
        s0[0..4].copy_from_slice(&0u32.to_le_bytes()); // sh_name -> ".go.module"
        s0[16..24].copy_from_slice(&MD_ADDR.to_le_bytes());
        s0[24..32].copy_from_slice(&MD_OFF.to_le_bytes());
        s0[32..40].copy_from_slice(&0x238u64.to_le_bytes());
        // .rela.dyn: sh_addr=RELA_ADDR, offset=RELA_OFF, size=24*3
        let mut s1 = vec![0u8; 64];
        s1[0..4].copy_from_slice(&11u32.to_le_bytes()); // sh_name -> ".rela.dyn"
        s1[16..24].copy_from_slice(&RELA_ADDR.to_le_bytes());
        s1[24..32].copy_from_slice(&RELA_OFF.to_le_bytes());
        s1[32..40].copy_from_slice(&(24u64 * 3).to_le_bytes());
        // .shstrtab
        let mut s2 = vec![0u8; 64];
        s2[0..4].copy_from_slice(&21u32.to_le_bytes());
        s2[24..32].copy_from_slice(&(MD_OFF + 0x200).to_le_bytes());
        s2[32..40].copy_from_slice(&64u64.to_le_bytes());
        let base = SH_OFF as usize;
        d[base..base + 64].copy_from_slice(&s0);
        d[base + 64..base + 128].copy_from_slice(&s1);
        d[base + 128..base + 192].copy_from_slice(&s2);
        // shstrtab contenido
        let so = (MD_OFF + 0x200) as usize;
        d[so..so + 10].copy_from_slice(b".go.module");
        d[so + 10] = 0;
        d[so + 11..so + 11 + 9].copy_from_slice(b".rela.dyn");
        d[so + 11 + 9] = 0;
        d[so + 21..so + 21 + 9].copy_from_slice(b".shstrtab");
        d[so + 21 + 9] = 0;
        // moduledata: text/etext/minpc/maxpc
        let m = MD_OFF as usize;
        d[m + OFF_MINPC as usize..m + OFF_MINPC as usize + 8]
            .copy_from_slice(&text.to_le_bytes());
        d[m + OFF_MAXPC as usize..m + OFF_MAXPC as usize + 8]
            .copy_from_slice(&(text + 0x900).to_le_bytes());
        if !con_reloc {
            d[m + OFF_TEXT as usize..m + OFF_TEXT as usize + 8].copy_from_slice(&text.to_le_bytes());
            d[m + OFF_ETEXT as usize..m + OFF_ETEXT as usize + 8]
                .copy_from_slice(&(text + 0xa00).to_le_bytes());
        } else {
            // relocalizacion RELATIVE en el campo text
            let r = RELA_OFF as usize;
            d[r..r + 8].copy_from_slice(&(MD_ADDR + OFF_TEXT).to_le_bytes());
            d[r + 8..r + 16].copy_from_slice(&(1027u64).to_le_bytes()); // R_AARCH64_RELATIVE
            d[r + 16..r + 24].copy_from_slice(&text.to_le_bytes());
        }
        d
    }

    #[test]
    fn lee_text_de_una_relocalizacion_relativa() {
        let d = elf_sintetico(0x4d0f0b0, true);
        let md = read_moduledata(&d).expect("moduledata");
        assert_eq!(md.text, 0x4d0f0b0);
        assert_eq!(md.minpc, 0x4d0f0b0);
        assert_eq!(md.maxpc, 0x4d0f9b0);
        assert!(md.source.contains("relocalizacion"));
    }

    #[test]
    fn lee_text_del_fichero_si_no_hay_relocalizacion() {
        let d = elf_sintetico(0x4d0f0b0, false);
        let md = read_moduledata(&d).expect("moduledata");
        assert_eq!(md.text, 0x4d0f0b0);
        assert_eq!(md.etext, 0x4d0f0b0 + 0xa00);
    }

    #[test]
    fn no_inventa_moduledata_en_un_no_elf() {
        assert!(read_moduledata(b"not an executable").is_none());
    }

    #[test]
    fn la_verificacion_detecta_la_base_equivocada() {
        let d = elf_sintetico(0x4d0f0b0, true);
        let md = read_moduledata(&d).unwrap();
        // base correcta: entryoff[0]=0 y sentinel = 0x900
        assert!(verify_text_base(&md, 0x4d0f0b0, 0, 0x900).is_ok());
        // base heuristica equivocada: falla el primer invariante
        let err = verify_text_base(&md, 0x4d06cb8, 0, 0x900).unwrap_err();
        assert!(err.contains("minpc"), "{}", err);
    }
}

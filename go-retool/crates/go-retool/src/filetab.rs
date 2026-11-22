//! Resolucion de `fichero:linea` de cada funcion Go.
//!
//! MECANISMO (runtime Go >= 1.24; ver `runtime/symtab.go` y
//! `cmd/link/internal/ld/pcln.go`):
//!
//! | que              | donde                                                |
//! |------------------|------------------------------------------------------|
//! | `cuOffset`       | `pcHeader.cuOffset` -> indice base dentro de `cutab`  |
//! | `filetab`        | `pcHeader.filetabOffset` -> bytes con los nombres     |
//! | `pctab`          | `pcHeader.pctabOffset` -> tablas pcvalue              |
//! | tabla pcfile     | `_func.pcfile` -> offset DENTRO de pctab              |
//! | tabla pcln       | `_func.pcln`   -> offset DENTRO de pctab              |
//!
//! Lookup del nombre de fichero (igual que `runtime.funcfile`):
//!   `fileno = pcvalue(pcfile, entrada)` y luego
//!!   `cutab[cuOffset + fileno]` -> offset dentro de `filetab`, y
//!!   `filetab[offset]` -> cadena C terminada en 0.
//!
//! OJO (bug que ya se cometio una vez aqui): `_func.pcfile` y `_func.pcln`
//! son offsets DIRECTOS dentro de pctab, NO indices del array `pcdata[]`
//! (ver `writeFuncs` en el linker: `sb.SetUint32(... pcfile)`).
//!
//! MEDIDO 2026-11-22 (liblanguage_server.so de Antigravity, Go 1.28 RC02):
//! con este mecanismo se reproduce EXACTAMENTE el `fichero:linea` de la
//! tabla de referencia del agente de RE en 161.680 de 161.682 funciones;
//! las 2 que no cuadran (`go:textfipsstart` y `go:textfipsend`) no tienen
//! fichero ni linea en la referencia tampoco.

use crate::reader::Reader;
use crate::{Error, Result};

/// Longitud maxima aceptada para un nombre de fichero.
const MAX_FILE_NAME: u64 = 4096;

/// Resuelve ficheros y lineas a partir de las tablas del pcHeader.
#[derive(Debug, Clone, Copy)]
pub struct SourceResolver {
    /// Base (offset de fichero) de la tabla `cutab` (array de u32).
    pub cutab_base: u64,
    /// Base (offset de fichero) de la tabla `filetab` (cadenas C).
    pub filetab_base: u64,
    /// Base (offset de fichero) de la tabla `pctab` (tablas pcvalue).
    pub pctab_base: u64,
    /// Longitud de la region `pctab` (para acotar las lecturas).
    pub pctab_len: u64,
    /// Cuantum de PC (minLC del pcHeader).
    pub pc_quantum: u8,
}

/// Valor de una tabla pcvalue en la PC de entrada de la funcion.
///
/// Reproduce EXACTAMENTE el primer paso de `runtime.step`: el `uvdelta`
/// (zigzag) se aplica al valor inicial -1 y el `pcdelta` avanza la PC.
/// El valor asi obtenido es el que el runtime devuelve para
/// `pcvalue(f, off, f.entry(), true)`.
fn first_pcvalue(
    r: &Reader,
    table_off: u64,
    _entry_va: u64,
    quantum: u8,
) -> Result<Option<i32>> {
    if quantum == 0 {
        return Err(Error::Invalid("pc quantum is zero"));
    }
    // Varint de hasta 5 bytes (uvarint32 del runtime).
    let mut pos = table_off;
    let mut value: i64 = -1;
    let mut shift = 0u32;
    let mut uv: u32 = 0;
    for _ in 0..5 {
        let b = r.u8(pos)?;
        pos += 1;
        if shift == 28 && (b & 0xf0) != 0 {
            return Err(Error::Invalid("pcvalue varint overflow"));
        }
        uv |= ((b & 0x7f) as u32) << shift;
        if b & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    // Zigzag: el runtime hace `*val += int32(-(uv & 1) ^ (uv >> 1))`.
    let delta = ((uv >> 1) as i64) ^ -((uv & 1) as i64);
    value += delta;
    if value < i32::MIN as i64 || value > i32::MAX as i64 {
        return Err(Error::Invalid("pcvalue value out of range"));
    }
    // Hay que consumir tambien el pcdelta para validar que la tabla existe.
    let mut shift = 0u32;
    let mut pd: u32 = 0;
    for _ in 0..5 {
        let b = r.u8(pos)?;
        pos += 1;
        if shift == 28 && (b & 0xf0) != 0 {
            return Err(Error::Invalid("pcvalue varint overflow"));
        }
        pd |= ((b & 0x7f) as u32) << shift;
        if b & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    let _ = pd;
    if value < 0 {
        return Ok(None);
    }
    Ok(Some(value as i32))
}

impl SourceResolver {
    /// Construye el resolutor a partir de los offsets del pcHeader.
    ///
    /// ATENCION: los cuatro offsets son offsets ABSOLUTOS de fichero (ya
    /// sumados al offset del pcHeader).  `pcln_base` acota la region `pctab`
    /// (en el pcHeader los offsets van en orden: funcname, cutab, filetab,
    /// pctab, pcln).
    pub fn new(
        cutab_off: u64,
        filetab_off: u64,
        pctab_off: u64,
        pcln_off: u64,
        pc_quantum: u8,
    ) -> Self {
        let pctab_len = pcln_off.saturating_sub(pctab_off);
        Self {
            cutab_base: cutab_off,
            filetab_base: filetab_off,
            pctab_base: pctab_off,
            pctab_len,
            pc_quantum,
        }
    }

    /// Nombre del fichero fuente de la funcion, si se puede resolver.
    pub fn file(
        &self,
        r: &Reader,
        cu_offset: u32,
        pcfile_off: u32,
        entry_va: u64,
    ) -> Result<Option<String>> {
        if pcfile_off == 0 {
            return Ok(None);
        }
        let table = self
            .pctab_base
            .checked_add(pcfile_off as u64)
            .ok_or(Error::Overflow("pcfile table offset"))?;
        if table >= self.pctab_base.saturating_add(self.pctab_len) {
            return Ok(None);
        }
        let fileno = match first_pcvalue(r, table, entry_va, self.pc_quantum)? {
            Some(v) => v as u64,
            None => return Ok(None),
        };
        let idx = cu_offset as u64 + fileno;
        let cut = self
            .cutab_base
            .checked_add(idx.checked_mul(4).ok_or(Error::Overflow("cutab index"))?)
            .ok_or(Error::Overflow("cutab offset"))?;
        let file_off = r.u32le(cut)?;
        if file_off == u32::MAX {
            // Entrada invalida: el linker la escribe cuando el fichero fue
            // eliminado por el deadcode eliminator.
            return Ok(None);
        }
        let name_off = self
            .filetab_base
            .checked_add(file_off as u64)
            .ok_or(Error::Overflow("filetab offset"))?;
        let bytes = r.c_string(name_off, MAX_FILE_NAME)?;
        match std::str::from_utf8(bytes) {
            Ok(s) => Ok(Some(s.to_owned())),
            Err(_) => Ok(None),
        }
    }

    /// Numero de linea en la PC de entrada de la funcion, si se puede resolver.
    pub fn line(
        &self,
        r: &Reader,
        pcline_off: u32,
        entry_va: u64,
    ) -> Result<Option<i32>> {
        if pcline_off == 0 {
            return Ok(None);
        }
        let table = self
            .pctab_base
            .checked_add(pcline_off as u64)
            .ok_or(Error::Overflow("pcline table offset"))?;
        if table >= self.pctab_base.saturating_add(self.pctab_len) {
            return Ok(None);
        }
        first_pcvalue(r, table, entry_va, self.pc_quantum)
    }
}

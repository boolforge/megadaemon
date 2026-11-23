//! Lector **ELF64 mínimo** (sin dependencias) para el payload de hooks.
//!
//! Para que el payload **realmente** intercepte, no basta con exportar el
//! símbolo: en Android el enlazador resuelve los símbolos **al cargar**
//! (enlace inmediato) ⇒ las llamadas que ya estaban resueltas seguirían
//! yendo a la implementación original.  La forma correcta de interponer en un
//! proceso vivo es **parchear la ranura del GOT** de cada módulo que importa
//! el símbolo.
//!
//! Para eso hace falta leer la tabla de relocaciones (`RELA`) y la tabla de
//! símbolos dinámicos de cada librería cargada.  Aquí se hace con un lector
//! propio: el payload no puede depender del crate del daemon (es enorme) ni
//! de `goblin` (pesado para una lib que se inyecta en una app).
//!
//! Formato (ELF64, little-endian, el de arm64 y x86_64):
//! * `PT_DYNAMIC` ⇒ `DT_SYMTAB`, `DT_STRTAB`, `DT_SYMENT`, `DT_HASH` /
//!   `DT_GNU_HASH` (para saber cuántos símbolos hay), `DT_RELA`/`DT_RELASZ`
//!   y `DT_JMPREL`/`DT_PLTRELSZ` (las relocaciones).
//! * `Elf64_Sym` = 24 bytes: `st_name`(4) `st_info`(1) `st_other`(1)
//!   `st_shndx`(2) `st_value`(8) `st_size`(8).
//! * `Elf64_Rela` = 24 bytes: `r_offset`(8) `r_info`(8) `r_addend`(8).
//!   El índice del símbolo es `r_info >> 32` y el tipo `r_info & 0xffffffff`.

/// Una relocación de una tabla `RELA`.
#[derive(Debug, Clone)]
pub struct Relocacion {
    /// Offset de la ranura (GOT) dentro de la librería.
    pub offset: u64,
    /// Tipo de relocación (`R_*_JUMP_SLOT`, `R_*_GLOB_DAT`, ...).
    pub tipo: u32,
    /// Nombre del símbolo importado (vacío si no se pudo leer).
    pub nombre: String,
}

/// Librería ELF64 dinámica ya parseada.
pub struct Elf {
    datos: Vec<u8>,
    symtab: usize,
    strtab: usize,
    syment: usize,
    n_simbolos: usize,
    /// Tablas de relocaciones: (offset, tamaño en bytes).
    relas: Vec<(usize, usize)>,
}

fn rd_u16(d: &[u8], off: usize) -> Option<u16> {
    if off + 2 > d.len() {
        return None;
    }
    Some(u16::from_le_bytes([d[off], d[off + 1]]))
}

fn rd_u32(d: &[u8], off: usize) -> Option<u32> {
    if off + 4 > d.len() {
        return None;
    }
    Some(u32::from_le_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]]))
}

fn rd_u64(d: &[u8], off: usize) -> Option<u64> {
    if off + 8 > d.len() {
        return None;
    }
    let mut b = [0u8; 8];
    b.copy_from_slice(&d[off..off + 8]);
    Some(u64::from_le_bytes(b))
}

impl Elf {
    /// Parsea una librería ELF64 dinámica de disco.
    pub fn cargar(ruta: &str) -> Result<Elf, String> {
        let datos = std::fs::read(ruta).map_err(|e| format!("no se puede leer {}: {}", ruta, e))?;
        Self::parsear(datos).map_err(|e| format!("{}: {}", ruta, e))
    }

    /// Parsea una imagen ELF64 dinámica ya en memoria.
    pub fn parsear(datos: Vec<u8>) -> Result<Elf, String> {
        if datos.len() < 64 || &datos[0..4] != b"\x7fELF" {
            return Err("no es un ELF".to_string());
        }
        if datos[4] != 2 {
            return Err("no es ELF64".to_string());
        }
        if datos[5] != 1 {
            return Err("no es little-endian".to_string());
        }
        let e_phoff = rd_u64(&datos, 0x20).unwrap_or(0) as usize;
        let e_phentsize = rd_u16(&datos, 0x36).unwrap_or(0) as usize;
        let e_phnum = rd_u16(&datos, 0x38).unwrap_or(0) as usize;

        // Segmentos: (p_vaddr, p_offset, p_filesz) y la PT_DYNAMIC.
        let mut cargas: Vec<(u64, u64, u64)> = Vec::new();
        let mut dinamica: Option<(usize, usize)> = None;
        for i in 0..e_phnum {
            let ph = e_phoff + i * e_phentsize;
            if ph + 56 > datos.len() {
                break;
            }
            let p_type = rd_u32(&datos, ph).unwrap_or(0);
            let p_offset = rd_u64(&datos, ph + 8).unwrap_or(0);
            let p_vaddr = rd_u64(&datos, ph + 16).unwrap_or(0);
            let p_filesz = rd_u64(&datos, ph + 32).unwrap_or(0);
            if p_type == 1 {
                cargas.push((p_vaddr, p_offset, p_filesz));
            } else if p_type == 2 {
                dinamica = Some((p_offset as usize, p_filesz as usize));
            }
        }
        let (dyn_off, dyn_size) = dinamica.ok_or_else(|| "no tiene PT_DYNAMIC".to_string())?;

        let vaddr_a_offset = |v: u64| -> Option<usize> {
            for (p_vaddr, p_offset, p_filesz) in &cargas {
                if v >= *p_vaddr && v < *p_vaddr + *p_filesz {
                    return Some((p_offset + (v - p_vaddr)) as usize);
                }
            }
            None
        };

        // Tabla dinámica.
        let mut dt_symtab: Option<u64> = None;
        let mut dt_strtab: Option<u64> = None;
        let mut dt_syment: u64 = 24;
        let mut dt_hash: Option<u64> = None;
        let mut dt_gnu_hash: Option<u64> = None;
        // Los tamaños (DT_RELASZ, DT_PLTRELSZ) pueden venir ANTES o DESPUES
        // de su tabla: se guardan aparte y se combinan al final.
        let mut dt_rela_v: Option<u64> = None;
        let mut dt_relasz: u64 = 0;
        let mut dt_jmprel_v: Option<u64> = None;
        let mut dt_pltrelsz: u64 = 0;
        let mut off = dyn_off;
        let limite = dyn_off.saturating_add(dyn_size);
        while off + 16 <= datos.len() && off < limite {
            let tag = rd_u64(&datos, off).unwrap_or(0) as i64;
            let val = rd_u64(&datos, off + 8).unwrap_or(0);
            match tag {
                0 => break, // DT_NULL
                2 => dt_pltrelsz = val,                     // DT_PLTRELSZ
                4 => dt_hash = Some(val),                     // DT_HASH
                5 => dt_strtab = Some(val),                   // DT_STRTAB
                6 => dt_symtab = Some(val),                   // DT_SYMTAB
                7 => dt_rela_v = Some(val),                   // DT_RELA
                8 => dt_relasz = val,                        // DT_RELASZ
                11 => dt_syment = val,                       // DT_SYMENT
                23 => dt_jmprel_v = Some(val),                // DT_JMPREL
                0x6ffffef5 => dt_gnu_hash = Some(val),        // DT_GNU_HASH
                _ => {}
            }
            off += 16;
        }

        let symtab_v = dt_symtab.ok_or_else(|| "no declara DT_SYMTAB".to_string())?;
        let strtab_v = dt_strtab.ok_or_else(|| "no declara DT_STRTAB".to_string())?;
        let symtab = vaddr_a_offset(symtab_v).ok_or_else(|| "DT_SYMTAB fuera de PT_LOAD".to_string())?;
        let strtab = vaddr_a_offset(strtab_v).ok_or_else(|| "DT_STRTAB fuera de PT_LOAD".to_string())?;

        // Número de símbolos: DT_HASH.nchain o el final de la cadena de
        // DT_GNU_HASH (que es lo que usa el cargador de Android).
        let n_simbolos = if let Some(h) = dt_hash {
            let ho = vaddr_a_offset(h).unwrap_or(0);
            rd_u32(&datos, ho + 4).unwrap_or(0) as usize
        } else if let Some(h) = dt_gnu_hash {
            Self::nchain_gnu_hash(&datos, vaddr_a_offset(h).unwrap_or(0)).unwrap_or(0)
        } else {
            0
        };
        if n_simbolos == 0 {
            return Err("no se puede saber el número de símbolos (sin DT_HASH/DT_GNU_HASH)".to_string());
        }

        // Tablas de relocaciones (RELA y JMPREL).
        let mut relas: Vec<(usize, usize)> = Vec::new();
        for (vaddr, tam) in [(dt_rela_v, dt_relasz), (dt_jmprel_v, dt_pltrelsz)] {
            if let (Some(v), t) = (vaddr, tam) {
                if v != 0 && t != 0 {
                    if let Some(off) = vaddr_a_offset(v) {
                        relas.push((off, t as usize));
                    }
                }
            }
        }

        Ok(Elf {
            datos,
            symtab,
            strtab,
            syment: if dt_syment == 0 { 24 } else { dt_syment as usize },
            n_simbolos,
            relas,
        })
    }

    /// Número de símbolos de la tabla dinámica.
    fn nchain_gnu_hash(datos: &[u8], off: usize) -> Option<usize> {
        let nbuckets = rd_u32(datos, off)? as usize;
        let symoffset = rd_u32(datos, off + 4)? as usize;
        let bloom_size = rd_u32(datos, off + 8)? as usize;
        if nbuckets == 0 || nbuckets > 1_000_000 || bloom_size > 1_000_000 {
            return None;
        }
        let buckets = off + 16 + bloom_size * 8;
        let chains = buckets + nbuckets * 4;
        let mut max = 0usize;
        for i in 0..nbuckets {
            let b = rd_u32(datos, buckets + i * 4)? as usize;
            if b < symoffset {
                continue;
            }
            let mut idx = b;
            let mut vueltas = 0usize;
            loop {
                let c = rd_u32(datos, chains + (idx - symoffset) * 4)?;
                max = max.max(idx + 1);
                if c & 1 != 0 || vueltas > 1_000_000 {
                    break;
                }
                idx += 1;
                vueltas += 1;
            }
        }
        Some(max)
    }

    /// Nombre del símbolo `indice` de la tabla dinámica.
    pub fn nombre_simbolo(&self, indice: usize) -> String {
        let s = self.symtab + indice * self.syment;
        if s + 24 > self.datos.len() {
            return String::new();
        }
        let st_name = rd_u32(&self.datos, s).unwrap_or(0) as usize;
        if st_name == 0 {
            return String::new();
        }
        let n = self.strtab + st_name;
        if n >= self.datos.len() {
            return String::new();
        }
        let fin = self.datos[n..]
            .iter()
            .position(|c| *c == 0)
            .map(|k| n + k)
            .unwrap_or(self.datos.len());
        String::from_utf8_lossy(&self.datos[n..fin]).to_string()
    }

    /// Valor (dirección virtual) del símbolo `nombre`.
    pub fn valor_simbolo(&self, nombre: &str) -> Option<u64> {
        for i in 0..self.n_simbolos {
            if self.nombre_simbolo(i) == nombre {
                let s = self.symtab + i * self.syment;
                if s + 24 > self.datos.len() {
                    return None;
                }
                let v = rd_u64(&self.datos, s + 8).unwrap_or(0);
                if v == 0 {
                    return None;
                }
                return Some(v);
            }
        }
        None
    }

    /// Todas las relocaciones con su nombre de símbolo.
    pub fn relocaciones(&self) -> Vec<Relocacion> {
        let mut out = Vec::new();
        for (off, tam) in &self.relas {
            let n = tam / 24;
            for i in 0..n {
                let r = off + i * 24;
                if r + 24 > self.datos.len() {
                    break;
                }
                let r_offset = rd_u64(&self.datos, r).unwrap_or(0);
                let r_info = rd_u64(&self.datos, r + 8).unwrap_or(0);
                let tipo = (r_info & 0xffff_ffff) as u32;
                let indice = (r_info >> 32) as usize;
                let nombre = self.nombre_simbolo(indice);
                out.push(Relocacion {
                    offset: r_offset,
                    tipo,
                    nombre,
                });
            }
        }
        out
    }
}

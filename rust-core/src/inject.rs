//! Inyector **real** por `ptrace`: carga una `.so` dentro de un proceso vivo
//! sin `frida-server` y sin `frida-gum`.
//!
//! POR QUÉ EXISTE
//! --------------
//! `rust-core/src/frida_hooks.rs` genera los scripts de gancho y
//! `frida_dbi.rs` los inspecciona, pero **ninguno de los dos inyecta**: la
//! crate `frida` (Rust) compila `frida-gum` (C enorme) y el móvil **no tiene
//! `frida-server` instalado** (MEDIDO 2026-11-23).  Sin embargo, el móvil es
//! un Samsung SM-G998B con **Magisk**: `root_exec` da `uid=0(root)` y
//! `context=u:r:magisk:s0` (MEDIDO).  Con eso se puede hacer `ptrace`.
//!
//! MECANISMO (el clásico "remote dlopen")
//! --------------------------------------
//! 1. Se localiza el PID leyendo `/proc/*/cmdline`.
//! 2. `PTRACE_ATTACH` + `waitpid` ⇒ el hilo queda parado.
//! 3. Se **resuelve `dlopen` dentro del proceso objetivo**: se lee
//!    `/proc/<pid>/maps`, se localizan las librerías del objetivo y se
//!    **parsea su tabla de símbolos dinámicos** (ELF64: `PT_DYNAMIC` ⇒
//!    `DT_SYMTAB`/`DT_STRTAB`/`DT_HASH`).  La dirección final es
//!    `bias_de_carga + st_value`.
//! 4. Se escribe la **ruta de la `.so`** en la memoria del objetivo (en su
//!    propia pila, por debajo del `SP`) y se **prepara la llamada**: los
//!    registros quedan como si el proceso objetivo fuese a hacer
//!    `dlopen(ruta, RTLD_NOW|RTLD_GLOBAL)` y a volver a una dirección
//!    "mágica" (`0`, no mapeada) que provoca un `SIGSEGV` que el inyector
//!    captura y **se traga** (no se le entrega a la app).
//! 5. Se **restauran los registros originales** y se suelta el proceso
//!    (`PTRACE_CONT` con señal `0`): la app sigue viva exactamente igual.
//! 6. Se **verifica** el resultado leyendo `/proc/<pid>/maps`: si aparece el
//!    mapa de la `.so`, la carga fue real (no un "creo que sí").
//!
//! POR QUÉ `ptrace` Y NO `/proc/<pid>/mem` (decisión medida/documentada)
//! --------------------------------------------------------------------
//! Las dos vías sirven para ESCRIBIR en el proceso objetivo, pero **no son
//! equivalentes**:
//!
//! * `/proc/<pid>/mem` exige `PTRACE_MODE_ATTACH` para **abrir** el fichero
//!   ⇒ necesita el mismo permiso y el mismo `SELinux` que `ptrace`.  La
//!   diferencia real es que `PTRACE_POKEDATA` **no necesita abrir nada**:
//!   el permiso lo concede el propio `PTRACE_ATTACH`.
//! * `PTRACE_POKEDATA`/`PEEKDATA` granularity: palabra de máquina (`long`).
//!   `/proc/<pid>/mem` escribe a byte.  Para una cadena de ruta da igual.
//! * `PTRACE_ATTACH` es **obligatorio** en los dos casos: sin él no se puede
//!   ni parar el proceso ni copiar sus registros (y sin registros no hay
//!   llamada remota posible).
//!
//! ⇒ Decisión: **la vía principal es `ptrace` (`POKEDATA`/`PEEKDATA`)**, y
//! `/proc/<pid>/mem` queda implementada como **vía de escritura alternativa**
//! (`--mem=proc`), porque en Android con `SELinux` en *Enforcing* el
//! `open("/proc/<pid>/mem")` puede estar denegado al dominio `magisk`
//! mientras que `ptrace` sí pasa (o al revés: `ptrace` puede exigir
//! `allow magisk tracer trace:process` y `mem` no).  **Las dos implementadas
//! y conmutables** porque en este móvil el veredicto de `SELinux` **no se ha
//! medido todavía** (ver `docs/INYECCION-HOOKS.md`).
//!
//! CUIDADOS (por qué el diseño es el que es)
//! -----------------------------------------
//! * **La pila del objetivo se usa como memoria auxiliar**: se escribe la
//!   ruta 4 KB por debajo del `SP`.  Por debajo del `SP` no hay datos vivos
//!   (la pila crece hacia abajo) ⇒ no se pisa el estado de la app.  Se
//!   comprueba antes que la dirección esté dentro de una zona escribible.
//! * **La llamada se hace en el hilo parado**, no en un hilo nuevo: `dlopen`
//!   necesita el cerrojo del cargador y el hilo parado es el que el
//!   inyector controla.
//! * **El `SIGSEGV` de la dirección mágica NO se entrega a la app**: se
//!   restauran los registros y se reanuda con señal `0`.  La app no se
//!   entera (medido: el proceso sigue vivo y contesta).
//! * Si la `.so` **no se puede leer** por el proceso objetivo (permisos
//!   POSIX o `SELinux`), `dlopen` devuelve `NULL` y el informe lo dice:
//!   por eso el instalador copia la `.so` al **directorio privado de la app**
//!   (etiqueta `app_data_file`), no a `/data/local/tmp`
//!   (`shell_data_file`, que `untrusted_app` no puede leer).

use std::collections::BTreeMap;
use std::path::Path;

/// Un proceso vivo del objetivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proceso {
    pub pid: i32,
    /// Primer argumento de `cmdline` (el nombre del proceso).
    pub nombre: String,
    /// `cmdline` completo, con los `\0` sustituidos por espacios.
    pub cmdline: String,
}

/// Una línea de `/proc/<pid>/maps`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapa {
    pub inicio: u64,
    pub fin: u64,
    /// Permisos en texto (`r-xp`).
    pub perms: String,
    pub offset: u64,
    /// Ruta del fichero (`` si es anónimo).
    pub ruta: String,
}

impl Mapa {
    /// ¿La zona es escribible?
    pub fn escribible(&self) -> bool {
        self.perms.len() >= 2 && self.perms.as_bytes()[1] == b'w'
    }

    /// ¿La zona es ejecutable?
    pub fn ejecutable(&self) -> bool {
        self.perms.len() >= 3 && self.perms.as_bytes()[2] == b'x'
    }

    /// ¿Contiene la dirección?
    pub fn contiene(&self, addr: u64) -> bool {
        addr >= self.inicio && addr < self.fin
    }
}

// ========================================================================= //
// /proc: cmdline y maps
// ========================================================================= //

/// Lee `/proc/<pid>/cmdline` (el `cmdline` del proceso, con `\0`).
///
/// Devuelve `None` si el proceso ya no existe o no se puede leer (normal en
/// un proceso que muere mientras lo miramos).
pub fn leer_cmdline(pid: i32) -> Option<String> {
    leer_cmdline_en(Path::new("/proc"), pid)
}

/// Igual que [`leer_cmdline`] pero con la raíz de `/proc` parametrizable
/// (para poder probar con directorios de mentira).
pub fn leer_cmdline_en(raiz: &Path, pid: i32) -> Option<String> {
    let ruta = raiz.join(pid.to_string()).join("cmdline");
    let bytes = std::fs::read(ruta).ok()?;
    if bytes.is_empty() {
        // Proceso zombie: `cmdline` está vacío.
        return None;
    }
    Some(String::from_utf8_lossy(&bytes).replace('\0', " ").trim_end().to_string())
}

/// Lista los procesos cuyo nombre o `cmdline` contienen `filtro`.
///
/// Con `filtro` vacío devuelve todos.  La comparación es **sin distinguir
/// mayúsculas** y acepta tanto el nombre del proceso (`com.agy.app`) como el
/// `cmdline` completo.
pub fn listar_procesos(filtro: &str) -> Vec<Proceso> {
    listar_procesos_en(Path::new("/proc"), filtro)
}

/// Igual que [`listar_procesos`] pero con la raíz de `/proc` parametrizable.
pub fn listar_procesos_en(raiz: &Path, filtro: &str) -> Vec<Proceso> {
    let f = filtro.trim().to_lowercase();
    let mut out = Vec::new();
    let entradas = match std::fs::read_dir(raiz) {
        Ok(e) => e,
        Err(_) => return out,
    };
    for e in entradas.flatten() {
        let nombre = e.file_name().to_string_lossy().to_string();
        if nombre.is_empty() || !nombre.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let pid: i32 = match nombre.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let cmdline = match leer_cmdline_en(raiz, pid) {
            Some(c) => c,
            None => continue,
        };
        let proc_nombre = cmdline.split_whitespace().next().unwrap_or("").to_string();
        if !f.is_empty() {
            let coincide = proc_nombre.to_lowercase().contains(&f)
                || cmdline.to_lowercase().contains(&f);
            if !coincide {
                continue;
            }
        }
        out.push(Proceso {
            pid,
            nombre: proc_nombre,
            cmdline,
        });
    }
    out.sort_by_key(|p| p.pid);
    out
}

/// Resuelve un objetivo (PID numérico o nombre) a un PID.
///
/// * Si `objetivo` es un número ⇒ se comprueba que el proceso existe.
/// * Si no ⇒ se busca por nombre; si hay varios, se devuelve el primero
///   (menor PID) y el error lo dice, para que el operador pueda afinar.
pub fn resolver_pid(objetivo: &str) -> Result<i32, String> {
    resolver_pid_en(Path::new("/proc"), objetivo)
}

/// Igual que [`resolver_pid`] pero con la raíz de `/proc` parametrizable.
pub fn resolver_pid_en(raiz: &Path, objetivo: &str) -> Result<i32, String> {
    let o = objetivo.trim();
    if o.is_empty() {
        return Err("no se ha indicado el proceso objetivo".to_string());
    }
    if let Ok(pid) = o.parse::<i32>() {
        if leer_cmdline_en(raiz, pid).is_some() {
            return Ok(pid);
        }
        return Err(format!("no existe el proceso {} en {}", pid, raiz.display()));
    }
    let encontrados = listar_procesos_en(raiz, o);
    match encontrados.len() {
        0 => Err(format!("no hay ningún proceso que case con '{}'", o)),
        1 => Ok(encontrados[0].pid),
        n => {
            let lista: Vec<String> = encontrados
                .iter()
                .map(|p| format!("{} ({})", p.pid, p.nombre))
                .collect();
            Err(format!(
                "'{}' case con {} procesos ({}): usa el PID",
                o,
                n,
                lista.join(", ")
            ))
        }
    }
}

/// Lee `/proc/<pid>/maps`.
pub fn leer_maps(pid: i32) -> Vec<Mapa> {
    leer_maps_en(Path::new("/proc"), pid)
}

/// Igual que [`leer_maps`] pero con la raíz de `/proc` parametrizable.
pub fn leer_maps_en(raiz: &Path, pid: i32) -> Vec<Mapa> {
    let ruta = raiz.join(pid.to_string()).join("maps");
    // MEDIDO (2026-11-26, móvil): `/proc/<pid>/maps` puede contener bytes
    // **NO UTF-8**: los nombres de las regiones `[anon:…]` son arbitrarios
    // (p. ej. `[anon:(??Lq]`).  `read_to_string` falla entonces con
    // `InvalidData` ⇒ el inyector concluía "no se puede leer
    // /proc/<pid>/maps (¿el proceso ha muerto?)" con el proceso VIVO y
    // respondiendo.  Ahora se lee en BYTES y se decodifica con permisión de
    // pérdidas (los nombres no importan para calcular direcciones).
    let bytes = match std::fs::read(&ruta) {
        Ok(b) => b,
        Err(_) => return Vec::new(),
    };
    parse_maps(&String::from_utf8_lossy(&bytes))
}

/// Parsea el texto de `/proc/<pid>/maps`.
pub fn parse_maps(texto: &str) -> Vec<Mapa> {
    let mut out = Vec::new();
    for linea in texto.lines() {
        let l = linea.trim();
        if l.is_empty() {
            continue;
        }
        let mut partes = l.split_whitespace();
        let rango = match partes.next() {
            Some(r) => r,
            None => continue,
        };
        let perms = match partes.next() {
            Some(p) => p.to_string(),
            None => continue,
        };
        let offset = match partes.next() {
            Some(o) => u64::from_str_radix(o, 16).unwrap_or(0),
            None => continue,
        };
        // `dev` e `inode` van después del offset; la ruta es lo que queda
        // (puede ir precedida de espacios y terminar en " (deleted)").
        let resto: Vec<&str> = partes.collect();
        let ruta = if resto.len() > 2 {
            resto[2..].join(" ")
        } else {
            String::new()
        };
        let ruta = ruta.trim_end_matches(" (deleted)").to_string();
        let (inicio, fin) = match rango.split_once('-') {
            Some((a, b)) => (
                u64::from_str_radix(a, 16).unwrap_or(0),
                u64::from_str_radix(b, 16).unwrap_or(0),
            ),
            None => continue,
        };
        out.push(Mapa {
            inicio,
            fin,
            perms,
            offset,
            ruta,
        });
    }
    out
}

/// Devuelve el **bias de carga** de un fichero: la dirección en la que se
/// mapeó su segmento `PT_LOAD` de menor `p_offset`.
///
/// Es `min(inicio - offset)` sobre los mapas de ese fichero: para el primer
/// segmento (`p_vaddr = 0`, `p_offset = 0`) eso da exactamente la base.
pub fn bias_de_carga(mapas: &[Mapa], ruta: &str) -> Option<u64> {
    let mut mejor: Option<u64> = None;
    for m in mapas {
        if m.ruta != ruta {
            continue;
        }
        let v = m.inicio.saturating_sub(m.offset);
        mejor = Some(match mejor {
            Some(b) => b.min(v),
            None => v,
        });
    }
    mejor
}

// ========================================================================= //
// ELF: buscar un símbolo exportado en una librería
// ========================================================================= //

const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const DT_HASH: i64 = 4;
const DT_STRTAB: i64 = 5;
const DT_SYMTAB: i64 = 6;
const DT_SYMENT: i64 = 11;
/// `DT_GNU_HASH`: la tabla de hash estilo GNU.  **Medido (2026-11-26, móvil)**
/// las librerías del runtime de Antigravity (`libc.so.6` de glibc) **sólo**
/// declaran `DT_GNU_HASH` y **no** `DT_HASH`: un parser que exija `DT_HASH`
/// no ve ningún símbolo y falla siempre.
const DT_GNU_HASH: i64 = 0x6ffffef5;

/// Tope de seguridad: no se leen librerías de más de 64 MB.
const MAX_ELF: usize = 64 * 1024 * 1024;

/// Índices de los símbolos **hasheados** de una tabla `DT_GNU_HASH`.
///
/// Estructura de la tabla (ELF gABI, GNU extension):
///
/// ```text
///   nbuckets   (u32)
///   symoffset  (u32)   <- primer símbolo con hash
///   bloom_size (u32)   <- palabras del filtro de Bloom
///   bloom_shift(u32)
///   bloom[bloom_size]
///   buckets[nbuckets]  <- índice del primer símbolo de cada hash
///   chain[]            <- eslabones; el bit 0 marca el último
/// ```
///
/// El símbolo `symoffset + k` está en `chain[k]`: si el bit 0 de la palabra es
/// 1, ése es el último de la cadena.  El bucket 0 se reserva ("vacío"), por
/// eso `symoffset` es siempre ≥ 1.
///
/// Devuelve los índices **en la tabla de símbolos** (relativos a `symtab`).
pub fn indices_gnu_hash(datos: &[u8], gnu_off: usize) -> Vec<usize> {
    let rd_u32 = |off: usize| -> u64 {
        if off + 4 > datos.len() {
            return 0;
        }
        u32::from_le_bytes([datos[off], datos[off + 1], datos[off + 2], datos[off + 3]]) as u64
    };
    if gnu_off + 16 > datos.len() {
        return Vec::new();
    }
    let nbuckets = rd_u32(gnu_off) as usize;
    let symoffset = rd_u32(gnu_off + 4) as usize;
    let bloom_size = rd_u32(gnu_off + 8) as usize;
    // Se salta el filtro de Bloom: `bloom_size` palabras de 8 bytes.
    let buckets_off = gnu_off + 16 + bloom_size * 8;
    let chain_off = buckets_off + nbuckets * 4;
    if buckets_off + nbuckets * 4 > datos.len() {
        return Vec::new();
    }
    let mut vistos: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    for b in 0..nbuckets {
        let bucket = rd_u32(buckets_off + b * 4) as usize;
        // El bucket 0 significa "cadena vacía"; los índices válidos son
        // ≥ symoffset.
        if bucket == 0 || bucket < symoffset {
            continue;
        }
        let mut idx = bucket;
        let mut k = bucket - symoffset;
        loop {
            let coff = chain_off + k * 4;
            if coff + 4 > datos.len() || idx > 10_000_000 {
                break;
            }
            let c = rd_u32(coff) as u32;
            vistos.insert(idx);
            if c & 1 != 0 {
                break; // último eslabón de la cadena
            }
            idx += 1;
            k += 1;
        }
    }
    vistos.into_iter().collect()
}

/// Devuelve la dirección **en el espacio del proceso cargado** del símbolo
/// `nombre` exportado por la librería `ruta` (bias de carga `bias`).
///
/// Se parsea la tabla de símbolos **dinámicos** (la que el cargador usa de
/// verdad), no la de depuración: una `.so` de Android va "stripped" pero
/// `DT_SYMTAB`/`DT_STRTAB` siempre están (son lo que permite resolver
/// `dlsym`).
///
/// El **bias de carga** se calcula con los mapas y con el ELF: el segmento
/// `PT_LOAD` de menor `p_offset` es el base; su `p_vaddr` y el mapa que le
/// corresponde dan `bias = inicio_mapa - p_vaddr`.  (Hacer `min(inicio -
/// offset)` a secas vale sólo si `p_vaddr` del primer segmento es 0, que es
/// lo habitual pero no está garantizado.)
pub fn buscar_simbolo_elf(ruta: &str, mapas: &[Mapa], nombre: &str) -> Result<u64, String> {
    let datos = std::fs::read(ruta).map_err(|e| format!("no se puede leer {}: {}", ruta, e))?;
    if datos.len() > MAX_ELF {
        return Err(format!("{} es demasiado grande para parsearlo", ruta));
    }
    if datos.len() < 64 || &datos[0..4] != b"\x7fELF" {
        return Err(format!("{} no es un ELF", ruta));
    }
    if datos[4] != 2 {
        return Err(format!("{} no es ELF64 (clase {})", ruta, datos[4]));
    }
    if datos[5] != 1 {
        return Err(format!("{} no es little-endian", ruta));
    }

    let rd_u32 = |off: usize| -> u64 {
        if off + 4 > datos.len() {
            return 0;
        }
        u32::from_le_bytes([datos[off], datos[off + 1], datos[off + 2], datos[off + 3]]) as u64
    };
    let rd_u16 = |off: usize| -> u64 {
        if off + 2 > datos.len() {
            return 0;
        }
        u16::from_le_bytes([datos[off], datos[off + 1]]) as u64
    };
    let rd_u64 = |off: usize| -> u64 {
        if off + 8 > datos.len() {
            return 0;
        }
        let mut b = [0u8; 8];
        b.copy_from_slice(&datos[off..off + 8]);
        u64::from_le_bytes(b)
    };

    let e_phoff = rd_u64(0x20) as usize;
    let e_phentsize = rd_u16(0x36) as usize;
    let e_phnum = rd_u16(0x38) as usize;

    // Segmentos con tipo y dirección virtual.
    let mut cargas: Vec<(u64, u64, u64)> = Vec::new(); // (p_vaddr, p_offset, p_filesz)
    let mut dinamica: Option<(u64, u64)> = None; // (offset, tamaño)
    for i in 0..e_phnum {
        let ph = e_phoff + i * e_phentsize;
        if ph + 56 > datos.len() {
            break;
        }
        let p_type = rd_u32(ph) as u32;
        let p_offset = rd_u64(ph + 8);
        let p_vaddr = rd_u64(ph + 16);
        let p_filesz = rd_u64(ph + 32);
        match p_type {
            PT_LOAD => cargas.push((p_vaddr, p_offset, p_filesz)),
            PT_DYNAMIC => dinamica = Some((p_offset, p_filesz)),
            _ => {}
        }
    }
    // Bias de carga: el segmento PT_LOAD de menor p_offset es el base.
    let mut primer_carga: Option<(u64, u64)> = None; // (p_vaddr, p_offset)
    for (p_vaddr, p_offset, _) in &cargas {
        match primer_carga {
            Some((_, off)) if *p_offset >= off => {}
            _ => primer_carga = Some((*p_vaddr, *p_offset)),
        }
    }
    let bias = match primer_carga {
        Some((p_vaddr, p_offset)) => {
            let mapa_base = mapas
                .iter()
                .filter(|m| m.ruta == ruta && m.offset == p_offset)
                .map(|m| m.inicio)
                .min();
            match mapa_base {
                Some(inicio) => inicio.saturating_sub(p_vaddr),
                None => mapas
                    .iter()
                    .filter(|m| m.ruta == ruta)
                    .map(|m| m.inicio.saturating_sub(m.offset))
                    .min()
                    .unwrap_or(0),
            }
        }
        None => 0,
    };

    let (dyn_off, dyn_size) = match dinamica {
        Some(d) => d,
        None => {
            return Err(format!(
                "{} no tiene PT_DYNAMIC (no es una librería dinámica)",
                ruta
            ))
        }
    };

    // Dirección virtual -> offset de fichero.
    let vaddr_a_offset = |v: u64| -> Option<usize> {
        for (p_vaddr, p_offset, p_filesz) in &cargas {
            if v >= *p_vaddr && v < *p_vaddr + *p_filesz {
                return Some((p_offset + (v - p_vaddr)) as usize);
            }
        }
        None
    };

    // Tabla dinámica: se buscan DT_HASH / DT_GNU_HASH / DT_SYMTAB / DT_STRTAB.
    let mut dt_hash: Option<u64> = None;
    let mut dt_gnu_hash: Option<u64> = None;
    let mut dt_symtab: Option<u64> = None;
    let mut dt_strtab: Option<u64> = None;
    let mut dt_syment: u64 = 24;
    let mut off = dyn_off as usize;
    let limite = (dyn_off as usize).saturating_add(dyn_size as usize);
    while off + 16 <= datos.len() && off < limite {
        let tag = rd_u64(off) as i64;
        let val = rd_u64(off + 8);
        if tag == 0 {
            break; // DT_NULL
        }
        match tag {
            DT_HASH => dt_hash = Some(val),
            DT_GNU_HASH => dt_gnu_hash = Some(val),
            DT_SYMTAB => dt_symtab = Some(val),
            DT_STRTAB => dt_strtab = Some(val),
            DT_SYMENT => dt_syment = val,
            _ => {}
        }
        off += 16;
    }

    let symtab_v = dt_symtab.ok_or_else(|| format!("{} no declara DT_SYMTAB", ruta))?;
    let strtab_v = dt_strtab.ok_or_else(|| format!("{} no declara DT_STRTAB", ruta))?;
    let symtab_off = vaddr_a_offset(symtab_v)
        .ok_or_else(|| format!("{}: DT_SYMTAB fuera de los segmentos PT_LOAD", ruta))?;
    let strtab_off = vaddr_a_offset(strtab_v)
        .ok_or_else(|| format!("{}: DT_STRTAB fuera de los segmentos PT_LOAD", ruta))?;

    // Índices de los símbolos a revisar:
    //  * con `DT_HASH` → `nchain` es el número exacto (todos los símbolos);
    //  * sin `DT_HASH` pero con `DT_GNU_HASH` → los símbolos hasheados se
    //    recorren por buckets+chains.  **Es el caso de las librerías del
    //    runtime de Antigravity** (glibc con `--hash-style=gnu`).
    let indices: Vec<usize> = if let Some(h) = dt_hash {
        let ho = vaddr_a_offset(h).unwrap_or(0);
        if ho + 8 <= datos.len() {
            (0..(rd_u32(ho + 4) as usize)).collect()
        } else {
            Vec::new()
        }
    } else if let Some(g) = dt_gnu_hash {
        match vaddr_a_offset(g) {
            Some(go) => indices_gnu_hash(&datos, go),
            None => Vec::new(),
        }
    } else {
        Vec::new()
    };

    if indices.is_empty() {
        return Err(format!(
            "{} no declara DT_HASH ni DT_GNU_HASH: no se puede saber qué símbolos exporta",
            ruta
        ));
    }

    let syment = if dt_syment == 0 { 24 } else { dt_syment as usize };
    for i in indices {
        let s = symtab_off + i * syment;
        if s + 24 > datos.len() {
            break;
        }
        let st_name = rd_u32(s) as usize;
        let st_value = rd_u64(s + 8);
        let st_shndx = rd_u16(s + 6);
        if st_name == 0 || st_value == 0 || st_shndx == 0 {
            continue; // símbolo sin nombre, sin dirección o indefinido (UND)
        }
        let n = strtab_off + st_name;
        if n >= datos.len() {
            continue;
        }
        let fin = datos[n..]
            .iter()
            .position(|c| *c == 0)
            .map(|k| n + k)
            .unwrap_or(datos.len());
        let sym = String::from_utf8_lossy(&datos[n..fin]).to_string();
        if sym == nombre {
            return Ok(bias + st_value);
        }
    }
    Err(format!("{} no exporta '{}'", ruta, nombre))
}

/// Resuelve la dirección de `dlopen` **dentro del proceso `pid`**.
///
/// Se recorren las librerías mapeadas por el objetivo buscando una que
/// exporte `dlopen` (en Android suele ser `libdl.so` o `libc.so`; en
/// glibc, `libc.so.6`).
/// Nombres que se prueban para la carga dinámica.  Todos tienen la **misma
/// firma** `void *f(const char *, int)`, que es la que usa la llamada remota.
pub const NOMBRES_CARGA: [&str; 3] = ["dlopen", "__libc_dlopen", "__libc_dlopen_mode"];

pub fn resolver_dlopen(pid: i32) -> Result<u64, String> {
    resolver_dlopen_en(Path::new("/proc"), pid)
}

/// Igual que [`resolver_dlopen`] pero con la raíz de `/proc` parametrizable.
pub fn resolver_dlopen_en(raiz: &Path, pid: i32) -> Result<u64, String> {
    let mapas = leer_maps_en(raiz, pid);
    if mapas.is_empty() {
        return Err(format!(
            "no se puede leer /proc/{}/maps (¿el proceso ha muerto?)",
            pid
        ));
    }
    let mut probadas: Vec<String> = Vec::new();
    let mut motivos: Vec<String> = Vec::new();
    // Candidatas: las librerías del sistema (dlopen vive en libdl/libc).
    // Se recorren rutas **únicas**: la misma libc sale mapeada en 4 trozos.
    let mut vistas: Vec<String> = Vec::new();
    for m in &mapas {
        if m.ruta.is_empty() || vistas.contains(&m.ruta) {
            continue;
        }
        let base = m.ruta.rsplit('/').next().unwrap_or("");
        let es_candidata = base.contains("libdl.so")
            || base.contains("libc.so")
            || base.starts_with("libc.")
            || base.starts_with("ld-linux");
        if !es_candidata {
            continue;
        }
        vistas.push(m.ruta.clone());
        probadas.push(m.ruta.clone());
        for nombre in NOMBRES_CARGA {
            match buscar_simbolo_elf(&m.ruta, &mapas, nombre) {
                Ok(direccion) => return Ok(direccion),
                Err(e) => {
                    if nombre == NOMBRES_CARGA[0] {
                        // Sólo se guarda el motivo del nombre principal: los
                        // demás son alias y el mensaje se llenaría de ruido.
                        motivos.push(format!("{}: {}", m.ruta, e));
                    }
                }
            }
        }
    }
    Err(format!(
        "no se ha encontrado 'dlopen' en ninguna librería del objetivo (probadas: {}; {})",
        if probadas.is_empty() {
            "ninguna".to_string()
        } else {
            probadas.join(", ")
        },
        if motivos.is_empty() {
            "sin detalles".to_string()
        } else {
            motivos.join(" | ")
        }
    ))
}

/// Agrupa los mapas por fichero (para el informe).
pub fn mapas_por_fichero(mapas: &[Mapa]) -> BTreeMap<String, Vec<Mapa>> {
    let mut out: BTreeMap<String, Vec<Mapa>> = BTreeMap::new();
    for m in mapas {
        out.entry(m.ruta.clone()).or_default().push(m.clone());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn escribir(dir: &Path, pid: i32, nombre: &str, contenido: &[u8]) {
        let d = dir.join(pid.to_string());
        let _ = std::fs::create_dir_all(&d);
        let _ = std::fs::write(d.join(nombre), contenido);
    }

    #[test]
    fn lee_cmdline_y_sustituye_los_nulos() {
        let tmp = std::env::temp_dir().join(format!("md-inject-t1-{}", std::process::id()));
        escribir(&tmp, 4242, "cmdline", b"com.agy.app\0--flag\0");
        let c = leer_cmdline_en(&tmp, 4242).expect("cmdline");
        assert_eq!(c, "com.agy.app --flag");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn lista_procesos_por_filtro_sin_distinguir_mayusculas() {
        let tmp = std::env::temp_dir().join(format!("md-inject-t2-{}", std::process::id()));
        escribir(&tmp, 100, "cmdline", b"com.agy.antigravity\0");
        escribir(&tmp, 200, "cmdline", b"system_server\0");
        escribir(&tmp, 300, "cmdline", b"nota\0");
        let todos = listar_procesos_en(&tmp, "");
        assert_eq!(todos.len(), 3);
        let ant = listar_procesos_en(&tmp, "ANTIGRAVITY");
        assert_eq!(ant.len(), 1);
        assert_eq!(ant[0].pid, 100);
        assert_eq!(ant[0].nombre, "com.agy.antigravity");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn resuelve_pid_numerico_y_por_nombre() {
        let tmp = std::env::temp_dir().join(format!("md-inject-t3-{}", std::process::id()));
        escribir(&tmp, 777, "cmdline", b"com.agy.antigravity\0");
        assert_eq!(resolver_pid_en(&tmp, "777").unwrap_or(-1), 777);
        assert_eq!(
            resolver_pid_en(&tmp, "com.agy.antigravity").unwrap_or(-1),
            777
        );
        let e = resolver_pid_en(&tmp, "no-existe").unwrap_err();
        assert!(e.contains("no hay ningún proceso"), "mensaje: {e}");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn parsea_maps() {
        let texto = "55d0f0a0-55d0f0b0 r--p 00000000 fd:01 1234    /system/lib64/libc.so\n\
                     55d0f0b0-55d10000 r-xp 00001000 fd:01 1234    /system/lib64/libc.so\n\
                     7f000000-7f002000 rw-p 00000000 00:00 0       [anon:dalvik-main space]\n";
        let m = parse_maps(texto);
        assert_eq!(m.len(), 3);
        assert_eq!(m[0].inicio, 0x55d0f0a0);
        assert_eq!(m[0].fin, 0x55d0f0b0);
        assert_eq!(m[0].ruta, "/system/lib64/libc.so");
        assert!(m[0].escribible() == false);
        assert!(m[1].ejecutable());
        assert!(m[2].escribible());
        assert!(m[2].ruta.starts_with("[anon"));
    }

    #[test]
    fn el_bias_de_carga_es_el_menor_inicio_menos_offset() {
        let mapas = parse_maps(
            "1000-2000 r--p 00000000 00:00 0 /a/lib.so\n\
             2000-3000 r-xp 00001000 00:00 0 /a/lib.so\n",
        );
        assert_eq!(bias_de_carga(&mapas, "/a/lib.so"), Some(0x1000));
        assert_eq!(bias_de_carga(&mapas, "/b/otro.so"), None);
    }

    /// MEDIDO (2026-11-26, móvil): `/proc/<pid>/maps` del LS de Antigravity
    /// tiene bytes **NO UTF-8** en los nombres `[anon:…]`.  Con
    /// `read_to_string` la lectura falla (`InvalidData`) y el inyector
    /// decía "no se puede leer /proc/<pid>/maps (¿el proceso ha muerto?)"
    /// con el proceso VIVO.  Este test fija esa regresión.
    #[test]
    fn lee_los_maps_aunque_tengan_bytes_no_utf8() {
        let raiz = std::env::temp_dir().join(format!("md_maps_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&raiz);
        let pid_dir = raiz.join("4242");
        let _ = std::fs::create_dir_all(&pid_dir);
        // Los mismos bytes que se midieron en el móvil: `[anon:(??Lq]`.
        let contenido: &[u8] = b"70ccc00000-70d0000000 ---p 00000000 00:00 0 [anon:(\xf3\x9cLq]\n\
                                    7f000000-7f002000 r-xp 00000000 00:00 0 /system/lib64/libc.so\n";
        std::fs::write(pid_dir.join("maps"), contenido).unwrap();
        let mapas = leer_maps_en(&raiz, 4242);
        assert_eq!(mapas.len(), 2, "los maps deben leerse: {:?}", mapas);
        assert_eq!(mapas[0].inicio, 0x70ccc00000);
        assert!(mapas[1].ruta.contains("libc.so"));
        let _ = std::fs::remove_dir_all(&raiz);
    }

    /// Construye un ELF64 mínimo con tabla de símbolos y una tabla de hash,
    /// para probar el parser sin depender del sistema.
    ///
    /// `con_gnu_hash`/`con_hash` eligen qué tabla declara la sección
    /// dinámica: `true/false` → sólo `DT_GNU_HASH`; `false/true` → sólo
    /// `DT_HASH`; `false/false` → ninguna (el parser debe avisar).
    ///
    /// Exporta `dlopen` en 0x1234 y `dlsym` en 0x5678.
    fn elf_de_prueba(con_gnu_hash: bool, con_hash: bool) -> Vec<u8> {
        let off_datos = 0x200usize;
        let strtab: &[u8] = b"\0dlopen\0dlsym\0";
        let off_str = off_datos;
        let off_sym = off_str + strtab.len();
        let off_gnu = off_sym + 3 * 24; // 3 símbolos: el nulo + los dos reales
        let off_hash = off_gnu + 40;
        let off_dyn = off_hash + 8;
        let total = off_dyn + 5 * 16;

        let mut d = vec![0u8; total];
        let put16 = |d: &mut Vec<u8>, at: usize, v: u16| {
            d[at..at + 2].copy_from_slice(&v.to_le_bytes());
        };
        let put32 = |d: &mut Vec<u8>, at: usize, v: u32| {
            d[at..at + 4].copy_from_slice(&v.to_le_bytes());
        };
        let put64 = |d: &mut Vec<u8>, at: usize, v: u64| {
            d[at..at + 8].copy_from_slice(&v.to_le_bytes());
        };

        // --- Cabecera ELF64 ---
        d[0..4].copy_from_slice(b"\x7fELF");
        d[4] = 2; // ELFCLASS64
        d[5] = 1; // ELFDATA2LSB
        d[6] = 1; // EV_CURRENT
        put16(&mut d, 16, 3); // ET_DYN
        put16(&mut d, 18, 0xb7); // EM_AARCH64
        put32(&mut d, 20, 1); // e_version
        put64(&mut d, 0x20, 0x40); // e_phoff
        put16(&mut d, 0x34, 64); // e_ehsize
        put16(&mut d, 0x36, 56); // e_phentsize
        put16(&mut d, 0x38, 3); // e_phnum

        // --- Tres program headers ---
        let ph = |i: usize| 0x40 + i * 56;
        // PT_LOAD: cubre todo el fichero (vaddr == offset).
        let p = ph(0);
        put32(&mut d, p, 1);
        put32(&mut d, p + 4, 5);
        put64(&mut d, p + 8, 0);
        put64(&mut d, p + 16, 0);
        put64(&mut d, p + 24, 0);
        put64(&mut d, p + 32, total as u64);
        put64(&mut d, p + 40, total as u64);
        put64(&mut d, p + 48, 0x1000);
        // PT_DYNAMIC
        let p = ph(1);
        put32(&mut d, p, 2);
        put32(&mut d, p + 4, 6);
        put64(&mut d, p + 8, off_dyn as u64);
        put64(&mut d, p + 16, off_dyn as u64);
        put64(&mut d, p + 32, (5 * 16) as u64);
        put64(&mut d, p + 40, (5 * 16) as u64);
        put64(&mut d, p + 48, 8);
        // PT_GNU_STACK (ignorado por el parser, pero completa e_phnum)
        let p = ph(2);
        put32(&mut d, p, 0x6474e551);
        put32(&mut d, p + 4, 7);
        put64(&mut d, p + 16, 0);
        put64(&mut d, p + 32, total as u64);
        put64(&mut d, p + 40, total as u64);

        // --- Tabla de nombres ---
        d[off_str..off_str + strtab.len()].copy_from_slice(strtab);
        // --- Tabla de símbolos (2 entradas de 24 bytes) ---
        let entrada = |d: &mut Vec<u8>, at: usize, nombre: u32, valor: u64| {
            put32(d, at, nombre);
            d[at + 4] = 0x12; // GLOBAL | FUNC
            d[at + 5] = 0;
            put16(d, at + 6, 1); // st_shndx = 1 (definido)
            put64(d, at + 8, valor);
            put64(d, at + 16, 0x20);
        };
        entrada(&mut d, off_sym, 0, 0); // entrada nula
        entrada(&mut d, off_sym + 24, 1, 0x1234); // "dlopen"
        entrada(&mut d, off_sym + 48, 8, 0x5678); // "dlsym"
        // --- GNU_HASH: 1 bucket, symoffset=1, cadena de 2 ---
        put32(&mut d, off_gnu, 1); // nbuckets
        put32(&mut d, off_gnu + 4, 1); // symoffset
        put32(&mut d, off_gnu + 8, 1); // bloom_size
        put32(&mut d, off_gnu + 12, 0); // bloom_shift
        put64(&mut d, off_gnu + 16, 0); // bloom[0]
        put32(&mut d, off_gnu + 24, 1); // buckets[0] -> símbolo 1
        put32(&mut d, off_gnu + 28, 0); // chain[0]: sigue
        put32(&mut d, off_gnu + 32, 1); // chain[1]: fin (bit 0)
        // --- DT_HASH clásico ---
        put32(&mut d, off_hash, 1); // nbucket
        put32(&mut d, off_hash + 4, 3); // nchain (nº de símbolos)

        // --- Tabla dinámica ---
        let mut e = 0usize;
        let tag = |d: &mut Vec<u8>, at: usize, t: u64, v: u64| {
            put64(d, at, t);
            put64(d, at + 8, v);
        };
        tag(&mut d, off_dyn + e * 16, 6, off_sym as u64); // DT_SYMTAB
        e += 1;
        tag(&mut d, off_dyn + e * 16, 5, off_str as u64); // DT_STRTAB
        e += 1;
        tag(&mut d, off_dyn + e * 16, 11, 24); // DT_SYMENT
        e += 1;
        if con_gnu_hash {
            tag(&mut d, off_dyn + e * 16, DT_GNU_HASH as u64, off_gnu as u64);
            e += 1;
        }
        if con_hash {
            tag(&mut d, off_dyn + e * 16, DT_HASH as u64, off_hash as u64);
            e += 1;
        }
        tag(&mut d, off_dyn + e * 16, 0, 0); // DT_NULL
        d
    }

    /// MEDIDO (2026-11-26, móvil): la libc del runtime de Antigravity
    /// (`/data/user/0/com.agy/files/runtime/lib/libc.so.6`) declara **sólo**
    /// `DT_GNU_HASH` (no `DT_HASH`) y exporta `dlopen` en 0x871e0.  El
    /// parser exigía `DT_HASH`, así que no veía **ningún** símbolo y la
    /// inyección moría con "no se ha encontrado 'dlopen' en ninguna librería
    /// del objetivo".  Este test fija esa regresión.
    #[test]
    fn encuentra_simbolos_en_elf_con_solo_gnu_hash() {
        let dir = std::env::temp_dir().join(format!("md_elf_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let mapas: Vec<Mapa> = Vec::new();

        let ruta = dir.join("libgnu.so");
        std::fs::write(&ruta, elf_de_prueba(true, false)).unwrap();
        let r = ruta.to_str().unwrap();
        assert_eq!(
            buscar_simbolo_elf(r, &mapas, "dlopen").unwrap(),
            0x1234,
            "con sólo DT_GNU_HASH hay que encontrar dlopen"
        );
        assert_eq!(buscar_simbolo_elf(r, &mapas, "dlsym").unwrap(), 0x5678);

        // El camino clásico (DT_HASH) sigue funcionando.
        let ruta2 = dir.join("libhash.so");
        std::fs::write(&ruta2, elf_de_prueba(false, true)).unwrap();
        assert_eq!(
            buscar_simbolo_elf(ruta2.to_str().unwrap(), &mapas, "dlopen").unwrap(),
            0x1234
        );

        // Sin ninguna tabla de hash el error tiene que decirlo claro.
        let ruta3 = dir.join("libnada.so");
        std::fs::write(&ruta3, elf_de_prueba(false, false)).unwrap();
        let e = buscar_simbolo_elf(ruta3.to_str().unwrap(), &mapas, "dlopen").unwrap_err();
        assert!(e.contains("DT_GNU_HASH"), "el error debe nombrar la tabla: {}", e);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Comprueba el parser de ELF **contra el propio proceso**: la dirección
    /// que devuelve para `dlopen` tiene que coincidir (módulo el bias) con la
    /// que devuelve `dlsym`.  Si el parser se equivoca, este test falla.
    #[test]
    fn el_parser_de_elf_encuentra_dlopen_como_dlsym() {
        let mapas = leer_maps(std::process::id() as i32);
        let ruta = mapas
            .iter()
            .find(|m| {
                m.ruta.contains("libc.so") || m.ruta.contains("libc-") || m.ruta.contains("ld-linux")
            })
            .map(|m| m.ruta.clone())
            .expect("no se encuentra la libc en /proc/self/maps");
        let por_elf = match buscar_simbolo_elf(&ruta, &mapas, "dlopen") {
            Ok(v) => v,
            Err(e) => {
                // En algunas libc `dlopen` no está exportada: no es un fallo
                // del parser, se informa y se sale.
                eprintln!("dlopen no exportada por {}: {}", ruta, e);
                return;
            }
        };
        let por_dlsym =unsafe {
            let sim: *const libc::c_char = b"dlopen\0".as_ptr() as *const libc::c_char;
            libc::dlsym(libc::RTLD_DEFAULT, sim) as u64
        };
        assert_ne!(por_dlsym, 0, "dlsym no devolvió dlopen");
        assert_eq!(
            por_elf,
            por_dlsym,
            "el parser de ELF y dlsym discrepan en {}",
            ruta
        );
    }
}

// ========================================================================= //
// ptrace: registros, memoria y llamada remota
// ========================================================================= //

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Números de `ptrace` (los del kernel).  Se redefinen aquí para no depender
/// de que `libc` los exponga con el mismo tipo en todas las plataformas
/// (`c_uint` en glibc/x86_64, `c_int` en Android y musl).
const PTRACE_PEEKDATA: i32 = 2;
const PTRACE_POKEDATA: i32 = 5;
const PTRACE_CONT: i32 = 7;
const PTRACE_GETREGSET: i32 = 0x4204;
const PTRACE_SETREGSET: i32 = 0x4205;
const PTRACE_ATTACH: i32 = 16;
const PTRACE_DETACH: i32 = 17;
/// Tipo de la nota de `ptrace` que lleva los registros generales.
const NT_PRSTATUS: i32 = 1;

/// `RTLD_NOW | RTLD_GLOBAL`: la `.so` se resuelve **completa** (si le falta un
/// símbolo, falla en vez de dejar un hole) y entra en el **espacio global**
/// de nombres (necesario para que el intercepteo por definición de símbolo
/// tenga efecto en las librerías que se carguen después).
const RTLD_NOW_GLOBAL: u64 = 0x2 | 0x100;

/// Dirección "mágica" a la que vuelve la llamada remota.
///
/// La dirección **0** no está mapeada en ningún proceso (el kernel no mapea
/// nunca la página 0: `mmap_min_addr` es 4096) ⇒ al volver ahí el proceso
/// recibe un `SIGSEGV` que el inyector captura y **se traga** (no se le
/// entrega a la app: la app sigue viva).
const RETORNO_MAGICO: u64 = 0;

/// Distancia bajo el `SP` donde se escribe la ruta de la `.so`.
///
/// 4 KB: por debajo del `SP` no hay datos vivos de la app (la pila crece
/// hacia abajo) y de sobra para la trama de señal que el kernel empuja al
/// parar el proceso (`siginfo`/`ucontext`: ~1 KB en arm64, 512 B en x86_64).
const OFFSET_AUXILIAR: u64 = 0x1000;

/// Separación entre la ruta escrita y la ranura de retorno.
const OFFSET_RANURA: u64 = 0x100;

/// Directorio donde el payload escribe el NDJSON de hooks.
///
/// **Aviso (importante)**: el proceso de una app de terceros **no puede
/// escribir en `/data/local/tmp`** (etiqueta `shell_data_file`, y
/// `untrusted_app` no tiene permiso).  Por eso el payload tiene una lista
/// de rutas candidatas y usa la **primera que puede abrir**; la que haya
/// usado se escribe en la propia primera línea del NDJSON
/// (`"fichero"`).  El daemon (root) las lee todas.
pub const DIR_HOOKS: &str = "/data/local/tmp/megadaemon/state";

/// Ruta principal del NDJSON de hooks.
pub const NDJSON_HOOKS: &str = "/data/local/tmp/megadaemon/state/hooks.ndjson";

/// Vía de escritura en la memoria del proceso objetivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViaMemoria {
    /// `PTRACE_POKEDATA`/`PEEKDATA` (la principal).
    Ptrace,
    /// `write(2)` sobre `/proc/<pid>/mem` (la alternativa).
    ProcMem,
}

impl ViaMemoria {
    /// Nombre corto para los informes.
    pub fn nombre(&self) -> &'static str {
        match self {
            ViaMemoria::Ptrace => "ptrace-pokedata",
            ViaMemoria::ProcMem => "proc-mem",
        }
    }

    /// Convierte el texto de la opción `--mem`.
    pub fn desde_texto(t: &str) -> Option<ViaMemoria> {
        match t.trim().to_lowercase().as_str() {
            "" | "ptrace" | "pokedata" => Some(ViaMemoria::Ptrace),
            "mem" | "procmem" | "proc-mem" | "/proc" => Some(ViaMemoria::ProcMem),
            _ => None,
        }
    }
}

// --- Registros del proceso objetivo ------------------------------------- //

/// Registros generales del proceso objetivo.
///
/// El diseño replica **exactamente** la disposición del kernel
/// (`struct user_pt_regs` en arm64, `struct user_regs_struct` en x86_64):
/// el `PTRACE_GETREGSET` copia bytes crudos, así que el orden de los campos
/// es parte del contrato.
#[cfg(target_arch = "aarch64")]
#[derive(Debug, Clone, Copy, Default)]
pub struct Registros {
    pub x: [u64; 31],
    pub sp: u64,
    pub pc: u64,
    pub pstate: u64,
}

/// Registros generales del proceso objetivo (x86_64).
#[cfg(target_arch = "x86_64")]
#[derive(Debug, Clone, Copy, Default)]
pub struct Registros {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rax: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub orig_rax: u64,
    pub rip: u64,
    pub cs: u64,
    pub eflags: u64,
    pub rsp: u64,
    pub ss: u64,
    pub fs_base: u64,
    pub gs_base: u64,
    pub ds: u64,
    pub es: u64,
    pub fs: u64,
    pub gs: u64,
}

/// `SP` actual del objetivo (el pivote de la llamada remota).
pub fn sp_actual(r: &Registros) -> u64 {
    #[cfg(target_arch = "aarch64")]
    {
        r.sp
    }
    #[cfg(target_arch = "x86_64")]
    {
        r.rsp
    }
}

/// `PC` actual del objetivo.
pub fn pc_actual(r: &Registros) -> u64 {
    #[cfg(target_arch = "aarch64")]
    {
        r.pc
    }
    #[cfg(target_arch = "x86_64")]
    {
        r.rip
    }
}

/// Valor devuelto por la última función llamada (x0 en arm64, rax en x86_64).
pub fn valor_devuelto(r: &Registros) -> u64 {
    #[cfg(target_arch = "aarch64")]
    {
        r.x[0]
    }
    #[cfg(target_arch = "x86_64")]
    {
        r.rax
    }
}

/// Pila de retorno de la llamada (x30 en arm64).  Se informa en el informe
/// pero NO se usa para decidir si la llamada volvió (eso se decide por el PC).
#[allow(dead_code)]
fn pila_de_retorno(r: &Registros) -> u64 {
    #[cfg(target_arch = "aarch64")]
    {
        r.x[30]
    }
    #[cfg(target_arch = "x86_64")]
    {
        0
    }
}

/// La llamada remota que se va a preparar.
///
/// Es una estructura **pura** (no toca nada): se puede construir y comprobar
/// en un test sin proceso detrás.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LlamadaRemota {
    /// Dirección de la función a llamar (`dlopen` en el objetivo).
    pub pc: u64,
    /// Primer argumento (la ruta de la `.so`).
    pub arg0: u64,
    /// Segundo argumento (`RTLD_NOW|RTLD_GLOBAL`).
    pub arg1: u64,
    /// Nuevo `SP`/`RSP` del objetivo.
    pub sp: u64,
    /// Dirección donde hay que escribir la dirección de retorno (x86_64).
    /// En arm64 no hace falta (la dirección de retorno va en `x30`).
    pub ranura_retorno: u64,
    /// Dirección de retorno (mágica: 0, no mapeada).
    pub magico: u64,
}

/// Prepara la llamada remota `dlopen(ruta, flags)` sobre los registros dados.
///
/// * Escribe la ruta en la pila del objetivo (`sp - 4 KB`), que es memoria
///   libre (la pila crece hacia abajo).
/// * Deja el `SP` apuntando a la ranura de retorno.
/// * En x86_64 deja la dirección de retorno en la pila (como un `call`).
/// * En arm64 la deja en `x30` (el `LR`).
pub fn preparar_llamada(r: &Registros, dir_dlopen: u64, dir_ruta: u64) -> LlamadaRemota {
    let sp_original = sp_actual(r);
    let scratch = sp_original.saturating_sub(OFFSET_AUXILIAR) & !0xF;
    let ranura = scratch + OFFSET_RANURA;
    #[cfg(target_arch = "aarch64")]
    let sp = ranura;
    #[cfg(target_arch = "x86_64")]
    let sp = ranura + 8; // tras un `call`, el RSP es ≡ 8 (mod 16)
    LlamadaRemota {
        pc: dir_dlopen,
        arg0: dir_ruta,
        arg1: RTLD_NOW_GLOBAL,
        sp,
        ranura_retorno: {
            #[cfg(target_arch = "aarch64")]
            {
                0
            }
            #[cfg(target_arch = "x86_64")]
            {
                ranura + 8
            }
        },
        magico: RETORNO_MAGICO,
    }
}

impl LlamadaRemota {
    /// Aplica la llamada a los registros del objetivo.
    pub fn aplicar(&self, r: &mut Registros) {
        #[cfg(target_arch = "aarch64")]
        {
            r.x[0] = self.arg0;
            r.x[1] = self.arg1;
            r.x[30] = self.magico;
            r.sp = self.sp;
            r.pc = self.pc;
        }
        #[cfg(target_arch = "x86_64")]
        {
            r.rdi = self.arg0;
            r.rsi = self.arg1;
            r.rax = 0;
            r.rsp = self.sp;
            r.rip = self.pc;
        }
    }
}

// --- Envoltorio de ptrace ------------------------------------------------ //

/// `errno` actual del proceso.
fn errno_actual() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

/// Pone `errno` a 0 (para distinguir "el peek devolvió -1" de "falló").
///
/// MEDIDO (2026-11-23, CI Android): `__errno_location` es de **glibc**; en
/// Android (bionic) la funcion es `__errno` y el crate `libc` no expone la
/// primera => E0425 y el build ARM64 (el binario del movil) no compilaba.
/// Aqui se usa la variante que existe en cada plataforma.
fn limpiar_errno() {
    #[cfg(target_os = "android")]
    unsafe {
        *libc::__errno() = 0;
    }
    #[cfg(not(target_os = "android"))]
    unsafe {
        *libc::__errno_location() = 0;
    }
}

/// Texto legible de un `errno`.
fn texto_errno(e: i32) -> String {
    format!("{} (errno {})", std::io::Error::from_raw_os_error(e), e)
}

/// Llama a `ptrace` con el tipo correcto para cada plataforma.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
unsafe fn ptrace_raw(req: i32, pid: i32, addr: *mut libc::c_void, data: *mut libc::c_void) -> libc::c_long {
    libc::ptrace(req as libc::c_uint, pid, addr, data)
}

/// Llama a `ptrace` con el tipo correcto para cada plataforma.
#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
unsafe fn ptrace_raw(req: i32, pid: i32, addr: *mut libc::c_void, data: *mut libc::c_void) -> libc::c_long {
    libc::ptrace(req as libc::c_int, pid, addr, data)
}

/// Adjunta el proceso al inyector (`PTRACE_ATTACH`) y espera a que pare.
pub fn adjuntar(pid: i32) -> Result<(), String> {
    unsafe {
        limpiar_errno();
        let r = ptrace_raw(
            PTRACE_ATTACH,
            pid,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        if r == -1 {
            let e = errno_actual();
            let pista = match e {
                libc::EPERM => {
                    "EPERM: el objetivo no permite ptrace. En Android con SELinux en Enforcing \
                     prueba: magiskpolicy --live \"allow magisk tracer trace:process\" \
                     (y que el proceso NO sea de otra app con dumpable=0)"
                }
                libc::ESRCH => "ESRCH: el proceso no existe (¿ha muerto?)",
                _ => "",
            };
            return Err(format!(
                "PTRACE_ATTACH(pid={}) falló: {}{}",
                pid,
                texto_errno(e),
                if pista.is_empty() {
                    String::new()
                } else {
                    format!(" — {}", pista)
                }
            ));
        }
    }
    let status = esperar(pid, 5_000)?;
    if !libc::WIFSTOPPED(status) {
        return Err(format!(
            "el proceso {} no se ha parado tras PTRACE_ATTACH (status={})",
            pid, status
        ));
    }
    Ok(())
}

/// Suelta el proceso (lo deja seguir como estaba).
pub fn soltar(pid: i32) {
    unsafe {
        ptrace_raw(
            PTRACE_DETACH,
            pid,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
    }
}

/// Reanuda el proceso **sin** entregarle ninguna señal.
fn continuar(pid: i32) -> Result<(), String> {
    unsafe {
        limpiar_errno();
        let r = ptrace_raw(
            PTRACE_CONT,
            pid,
            std::ptr::null_mut(),
            0usize as *mut libc::c_void,
        );
        if r == -1 {
            return Err(format!(
                "PTRACE_CONT(pid={}) falló: {}",
                pid,
                texto_errno(errno_actual())
            ));
        }
    }
    Ok(())
}

/// Espera a que el proceso se pare otra vez (con tope de tiempo).
fn esperar(pid: i32, timeout_ms: u64) -> Result<i32, String> {
    let limite = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        let mut status: i32 = 0;
        let r = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
        if r == pid {
            return Ok(status);
        }
        if r == -1 {
            let e = errno_actual();
            if e == libc::EINTR {
                continue;
            }
            return Err(format!(
                "waitpid({}) falló: {}",
                pid,
                texto_errno(e)
            ));
        }
        if Instant::now() >= limite {
            return Err(format!(
                "el proceso {} no se ha parado en {} ms",
                pid, timeout_ms
            ));
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Lee los registros generales del objetivo.
pub fn leer_registros(pid: i32) -> Result<Registros, String> {
    let mut regs = Registros::default();
    let mut iov = libc::iovec {
        iov_base: &mut regs as *mut Registros as *mut libc::c_void,
        iov_len: std::mem::size_of::<Registros>(),
    };
    unsafe {
        limpiar_errno();
        let r = ptrace_raw(
            PTRACE_GETREGSET,
            pid,
            NT_PRSTATUS as *mut libc::c_void,
            &mut iov as *mut libc::iovec as *mut libc::c_void,
        );
        if r == -1 {
            return Err(format!(
                "PTRACE_GETREGSET(pid={}) falló: {}",
                pid,
                texto_errno(errno_actual())
            ));
        }
    }
    Ok(regs)
}

/// Escribe los registros generales del objetivo.
pub fn escribir_registros(pid: i32, regs: &Registros) -> Result<(), String> {
    let mut iov = libc::iovec {
        iov_base: regs as *const Registros as *mut libc::c_void,
        iov_len: std::mem::size_of::<Registros>(),
    };
    unsafe {
        limpiar_errno();
        let r = ptrace_raw(
            PTRACE_SETREGSET,
            pid,
            NT_PRSTATUS as *mut libc::c_void,
            &mut iov as *mut libc::iovec as *mut libc::c_void,
        );
        if r == -1 {
            return Err(format!(
                "PTRACE_SETREGSET(pid={}) falló: {}",
                pid,
                texto_errno(errno_actual())
            ));
        }
    }
    Ok(())
}

// --- Memoria del proceso objetivo ---------------------------------------- //

/// Escritor sobre la memoria de otro proceso.
///
/// Hay **dos** vías y se pueden conmutar (ver el módulo):
/// * [`ViaMemoria::Ptrace`]: `PTRACE_POKEDATA`/`PEEKDATA`.
/// * [`ViaMemoria::ProcMem`]: `write(2)`/`read(2)` sobre `/proc/<pid>/mem`.
pub struct Memoria {
    pid: i32,
    via: ViaMemoria,
    fd: Option<std::fs::File>,
}

impl Memoria {
    /// Abre la vía de escritura indicada.
    pub fn abrir(pid: i32, via: ViaMemoria) -> Result<Self, String> {
        let fd = match via {
            ViaMemoria::Ptrace => None,
            ViaMemoria::ProcMem => {
                let ruta = format!("/proc/{}/mem", pid);
                let f = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&ruta)
                    .map_err(|e| format!("no se puede abrir {}: {}", ruta, e))?;
                Some(f)
            }
        };
        Ok(Memoria { pid, via, fd })
    }

    /// La vía usada.
    pub fn via(&self) -> ViaMemoria {
        self.via
    }

    /// Lee `len` bytes de la memoria del objetivo.
    pub fn leer(&mut self, addr: u64, len: usize) -> Result<Vec<u8>, String> {
        if len == 0 {
            return Ok(Vec::new());
        }
        match self.via {
            ViaMemoria::Ptrace => {
                let mut out = Vec::with_capacity(len);
                let mut leidos = 0usize;
                while leidos < len {
                    let base = (addr + leidos as u64) & !0x7;
                    let palabra = leer_palabra(self.pid, base)?;
                    let bytes = palabra.to_le_bytes();
                    let dentro = (addr + leidos as u64) - base;
                    let dentro = dentro as usize;
                    let copiar = std::cmp::min(8 - dentro, len - leidos);
                    out.extend_from_slice(&bytes[dentro..dentro + copiar]);
                    leidos += copiar;
                }
                Ok(out)
            }
            ViaMemoria::ProcMem => {
                use std::os::unix::fs::FileExt;
                let f = self
                    .fd
                    .as_ref()
                    .ok_or_else(|| "el fichero /proc/<pid>/mem no está abierto".to_string())?;
                let mut buf = vec![0u8; len];
                let n = f
                    .read_at(&mut buf, addr)
                    .map_err(|e| format!("read(/proc/{}/mem, 0x{:x}) falló: {}", self.pid, addr, e))?;
                buf.truncate(n);
                Ok(buf)
            }
        }
    }

    /// Escribe `datos` en la memoria del objetivo.
    pub fn escribir(&mut self, addr: u64, datos: &[u8]) -> Result<(), String> {
        if datos.is_empty() {
            return Ok(());
        }
        match self.via {
            ViaMemoria::Ptrace => {
                let mut escritos = 0usize;
                while escritos < datos.len() {
                    let dir = addr + escritos as u64;
                    let base = dir & !0x7;
                    let dentro = (dir - base) as usize;
                    let copiar = std::cmp::min(8 - dentro, datos.len() - escritos);
                    let mut palabra = if dentro == 0 && copiar == 8 {
                        0u64
                    } else {
                        let previo = leer_palabra(self.pid, base)?;
                        previo
                    };
                    let mut bytes = palabra.to_le_bytes();
                    bytes[dentro..dentro + copiar]
                        .copy_from_slice(&datos[escritos..escritos + copiar]);
                    palabra = u64::from_le_bytes(bytes);
                    escribir_palabra(self.pid, base, palabra)?;
                    escritos += copiar;
                }
                Ok(())
            }
            ViaMemoria::ProcMem => {
                use std::os::unix::fs::FileExt;
                let f = self
                    .fd
                    .as_ref()
                    .ok_or_else(|| "el fichero /proc/<pid>/mem no está abierto".to_string())?;
                f.write_at(datos, addr).map_err(|e| {
                    format!(
                        "write(/proc/{}/mem, 0x{:x}, {} B) falló: {}",
                        self.pid,
                        addr,
                        datos.len(),
                        e
                    )
                })?;
                Ok(())
            }
        }
    }
}

/// Lee una palabra (8 bytes) de la memoria del objetivo con `PTRACE_PEEKDATA`.
fn leer_palabra(pid: i32, addr: u64) -> Result<u64, String> {
    unsafe {
        limpiar_errno();
        let v = ptrace_raw(
            PTRACE_PEEKDATA,
            pid,
            addr as *mut libc::c_void,
            std::ptr::null_mut(),
        );
        if v == -1 {
            let e = errno_actual();
            if e != 0 {
                return Err(format!(
                    "PTRACE_PEEKDATA(0x{:x}) falló: {}",
                    addr,
                    texto_errno(e)
                ));
            }
        }
        Ok(v as u64)
    }
}

/// Escribe una palabra en la memoria del objetivo con `PTRACE_POKEDATA`.
fn escribir_palabra(pid: i32, addr: u64, valor: u64) -> Result<(), String> {
    unsafe {
        limpiar_errno();
        let v = ptrace_raw(
            PTRACE_POKEDATA,
            pid,
            addr as *mut libc::c_void,
            valor as usize as *mut libc::c_void,
        );
        if v == -1 {
            let e = errno_actual();
            if e != 0 {
                return Err(format!(
                    "PTRACE_POKEDATA(0x{:x}) falló: {}",
                    addr,
                    texto_errno(e)
                ));
            }
        }
    }
    Ok(())
}

// --- Informe ------------------------------------------------------------- //

/// Informe de una inyección.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Informe {
    /// ¿La carga se ha verificado?
    pub ok: bool,
    pub pid: i32,
    /// Nombre del proceso objetivo.
    pub proceso: String,
    /// Ruta de la `.so` inyectada.
    pub biblioteca: String,
    /// ¿Se pudo adjuntar con `ptrace`?
    pub adjunto: bool,
    /// ¿`dlopen` devolvió un handle distinto de cero?
    pub handle_valido: bool,
    /// ¿La `.so` aparece en `/proc/<pid>/maps`? (verificación real)
    pub cargada: bool,
    /// Dirección de `dlopen` en el objetivo (hex).
    pub dlopen_addr: Option<String>,
    /// Vía de escritura usada en la memoria.
    pub via_memoria: String,
    /// Error exacto (con `errno`) si algo falló.
    pub error: Option<String>,
    /// Traza de los pasos (para depurar en el móvil).
    pub pasos: Vec<String>,
}

impl Informe {
    fn nuevo(pid: i32, proceso: &str, biblioteca: &str, via: ViaMemoria) -> Self {
        Informe {
            ok: false,
            pid,
            proceso: proceso.to_string(),
            biblioteca: biblioteca.to_string(),
            adjunto: false,
            handle_valido: false,
            cargada: false,
            dlopen_addr: None,
            via_memoria: via.nombre().to_string(),
            error: None,
            pasos: Vec::new(),
        }
    }

    fn paso(&mut self, texto: &str) {
        self.pasos.push(texto.to_string());
    }
}

/// Inyecta `biblioteca` en el proceso `pid`.
///
/// Devuelve siempre un informe: el error (si lo hay) va en `error` con el
/// `errno` cuando lo hay.
pub fn inyectar(pid: i32, biblioteca: &str, via: ViaMemoria) -> Informe {
    let mut inf = Informe::nuevo(pid, "", biblioteca, via);
    let proceso = leer_cmdline(pid).unwrap_or_default();
    inf.proceso = proceso.split_whitespace().next().unwrap_or("").to_string();

    if !std::path::Path::new(biblioteca).exists() {
        inf.error = Some(format!("la biblioteca {} no existe", biblioteca));
        inf.paso("la biblioteca no existe");
        return inf;
    }

    // 1) Adjuntar.
    match adjuntar(pid) {
        Ok(()) => {
            inf.adjunto = true;
            inf.paso("PTRACE_ATTACH correcto");
        }
        Err(e) => {
            inf.error = Some(e);
            inf.paso("PTRACE_ATTACH falló");
            return inf;
        }
    }

    // A partir de aquí el proceso está parado: hay que soltarlo siempre.
    //
    // Los registros originales se leen UNA vez y se restauran SIEMPRE al
    // final (éxito o error): si no, la app se queda con el PC en la
    // dirección mágica (0) y se muere con un SIGSEGV al continuar.
    let regs_originales = match leer_registros(pid) {
        Ok(r) => r,
        Err(e) => {
            inf.error = Some(e);
            inf.paso("no se pudieron leer los registros del objetivo");
            let _ = continuar(pid);
            soltar(pid);
            return inf;
        }
    };

    let resultado = inyectar_parado(pid, biblioteca, via, &mut inf, &regs_originales);
    if let Err(e) = resultado {
        if inf.error.is_none() {
            inf.error = Some(e);
        }
    }
    // Restaurar SIEMPRE el contexto original y soltarlo.
    let _ = escribir_registros(pid, &regs_originales);
    let _ = continuar(pid);
    soltar(pid);
    inf
}

/// El núcleo de la inyección: el proceso ya está parado.
fn inyectar_parado(
    pid: i32,
    biblioteca: &str,
    via: ViaMemoria,
    inf: &mut Informe,
    regs_originales: &Registros,
) -> Result<(), String> {
    // 2) Resolver `dlopen` DENTRO del objetivo.
    let dir_dlopen = match resolver_dlopen(pid) {
        Ok(d) => d,
        Err(e) => {
            inf.paso(&format!("no se pudo resolver dlopen: {}", e));
            return Err(e);
        }
    };
    inf.dlopen_addr = Some(format!("0x{:x}", dir_dlopen));
    inf.paso(&format!("dlopen en el objetivo: 0x{:x}", dir_dlopen));

    // 3) Registros originales (los restaura el llamante, siempre).
    let sp_original = sp_actual(regs_originales);

    // 4) Zona auxiliar en la pila del objetivo: tiene que ser escribible.
    let mapas = leer_maps(pid);
    let scratch = sp_original.saturating_sub(OFFSET_AUXILIAR) & !0xF;
    let ranura = scratch + OFFSET_RANURA;
    let zona_ok = mapas
        .iter()
        .any(|m| m.escribible() && m.contiene(scratch) && m.contiene(ranura + 16));
    if !zona_ok {
        let e = format!(
            "no se ha encontrado una zona escribible en la pila del objetivo (sp=0x{:x}, \
             se buscaría 0x{:x}); el mapa de pila puede estar protegido",
            sp_original, scratch
        );
        inf.paso(&e);
        return Err(e);
    }

    // 5) Escribir la ruta de la `.so` (con `\0`) y preparar la llamada.
    let mut memoria = Memoria::abrir(pid, via)?;
    let mut ruta = biblioteca.as_bytes().to_vec();
    ruta.push(0);
    if ruta.len() > OFFSET_RANURA as usize {
        let e = format!("la ruta de la biblioteca es demasiado larga ({} B)", ruta.len());
        inf.paso(&e);
        return Err(e);
    }
    memoria.escribir(scratch, &ruta)?;
    inf.paso(&format!("ruta escrita en 0x{:x} ({} B)", scratch, ruta.len()));

    let llamada = preparar_llamada(regs_originales, dir_dlopen, scratch);
    if llamada.ranura_retorno != 0 {
        memoria.escribir(llamada.ranura_retorno, &llamada.magico.to_le_bytes())?;
    }
    let mut regs = *regs_originales;
    llamada.aplicar(&mut regs);
    escribir_registros(pid, &regs)?;
    inf.paso(&format!(
        "llamada preparada: pc=0x{:x} sp=0x{:x} retorno=0x{:x}",
        llamada.pc, llamada.sp, llamada.magico
    ));

    // 6) Ejecutar: el proceso vuelve de `dlopen` a la dirección mágica (0) y
    //    recibe un SIGSEGV que nos lo señala.
    let status = match esperar(pid, 15_000) {
        Ok(s) => s,
        Err(e) => {
            inf.paso(&format!("la llamada remota no ha vuelto: {}", e));
            return Err(e);
        }
    };
    let regs_vuelta = leer_registros(pid)?;
    let pc_vuelta = pc_actual(&regs_vuelta);
    let senal = if libc::WIFSTOPPED(status) {
        libc::WSTOPSIG(status)
    } else {
        0
    };

    // Ha vuelto a la dirección mágica si el PC está en 0 (el CPU intentó
    // ejecutar ahí y el kernel paró el proceso con un SIGSEGV).
    if pc_vuelta != RETORNO_MAGICO {
        // No es nuestra vuelta: `dlopen` ha petado por dentro.
        let e = format!(
            "la llamada remota no ha vuelto a la dirección mágica (pc=0x{:x}, señal={}): \
             dlopen ha fallado dentro del objetivo",
            pc_vuelta, senal
        );
        inf.paso(&e);
        return Err(e);
    }

    let handle = valor_devuelto(&regs_vuelta);
    inf.handle_valido = handle != 0;
    inf.paso(&format!(
        "dlopen ha vuelto: handle=0x{:x} (señal={})",
        handle, senal
    ));

    // 7) Verificación REAL: la `.so` tiene que aparecer en los mapas.
    let mapas_post = leer_maps(pid);
    let nombre_fichero = std::path::Path::new(biblioteca)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let cargada = mapas_post
        .iter()
        .any(|m| !m.ruta.is_empty() && m.ruta.ends_with(&nombre_fichero));
    inf.cargada = cargada;
    if cargada {
        inf.paso(&format!(
            "VERIFICADO: {} aparece en /proc/{}/maps",
            nombre_fichero, pid
        ));
    } else {
        inf.paso(&format!(
            "AVISO: {} NO aparece en /proc/{}/maps (la carga no se puede dar por buena)",
            nombre_fichero, pid
        ));
    }

    // 8) Restaurar el contexto original (el proceso sigue exactamente igual).
    escribir_registros(pid, regs_originales)?;
    inf.paso("registros originales restaurados");
    inf.ok = cargada && inf.handle_valido;
    if !inf.ok {
        inf.error = Some(
            "la carga no se ha podido verificar: dlopen no ha devuelto handle o la .so no \
             aparece en los mapas del objetivo"
                .to_string(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod pruebas_llamada {
    use super::*;

    /// Registros con un SP **realista**.
    ///
    /// MEDIDO (2026-11-23): `Registros::default()` deja el SP a 0, y con SP a
    /// 0 la comprobacion "la ruta se escribe por debajo del SP" no se puede
    /// cumplir (0x1234 < 0 es falso) => el test era rojo por un motivo
    /// artificial, no por un defecto del inyector.
    fn registros_con_sp() -> Registros {
        let mut r = Registros::default();
        #[cfg(target_arch = "aarch64")]
        {
            r.sp = 0x7fff_0000_0000;
        }
        #[cfg(target_arch = "x86_64")]
        {
            r.rsp = 0x7fff_0000_0000;
        }
        r
    }

    #[test]
    fn la_llamada_es_pura_y_no_toca_nada() {
        let r = registros_con_sp();
        let l = preparar_llamada(&r, 0xdeadbeef, 0x1234);
        assert_eq!(l.pc, 0xdeadbeef);
        assert_eq!(l.arg0, 0x1234);
        assert_eq!(l.arg1, RTLD_NOW_GLOBAL);
        assert_eq!(l.magico, RETORNO_MAGICO);
        // El hueco de trabajo (donde se escribe la ruta) queda POR DEBAJO
        // del SP original, y el nuevo SP apunta a la ranura.
        assert!(l.sp < sp_actual(&r), "el hueco debe quedar bajo el SP original");
        assert!(l.arg0 < sp_actual(&r), "la ruta se escribe bajo el SP");
    }

    #[test]
    fn la_llamada_pisa_los_registros_correctos() {
        let mut r = Registros::default();
        let l = preparar_llamada(&r, 0xdeadbeef, 0x1234);
        l.aplicar(&mut r);
        assert_eq!(pc_actual(&r), 0xdeadbeef);
        assert_eq!(sp_actual(&r), l.sp);
        #[cfg(target_arch = "aarch64")]
        {
            assert_eq!(r.x[0], 0x1234);
            assert_eq!(r.x[1], RTLD_NOW_GLOBAL);
            assert_eq!(r.x[30], RETORNO_MAGICO);
            assert_eq!(l.ranura_retorno, 0);
        }
        #[cfg(target_arch = "x86_64")]
        {
            assert_eq!(r.rdi, 0x1234);
            assert_eq!(r.rsi, RTLD_NOW_GLOBAL);
            assert_eq!(l.ranura_retorno, l.sp);
        }
    }

    #[test]
    fn el_sp_nuevo_esta_alineado() {
        let mut r = Registros::default();
        #[cfg(target_arch = "aarch64")]
        {
            r.sp = 0x7fff_1234_5678;
        }
        #[cfg(target_arch = "x86_64")]
        {
            r.rsp = 0x7fff_1234_5678;
        }
        let l = preparar_llamada(&r, 1, 2);
        #[cfg(target_arch = "aarch64")]
        assert_eq!(l.sp % 16, 0, "el SP de arm64 tiene que estar alineado a 16");
        #[cfg(target_arch = "x86_64")]
        assert_eq!(l.sp % 16, 8, "tras un call, el RSP de x86_64 es ≡ 8 (mod 16)");
    }

    #[test]
    fn las_vias_de_memoria_se_convierten_desde_texto() {
        assert_eq!(ViaMemoria::desde_texto("ptrace"), Some(ViaMemoria::Ptrace));
        assert_eq!(ViaMemoria::desde_texto(""), Some(ViaMemoria::Ptrace));
        assert_eq!(ViaMemoria::desde_texto("mem"), Some(ViaMemoria::ProcMem));
        assert_eq!(ViaMemoria::desde_texto("PROC-MEM"), Some(ViaMemoria::ProcMem));
        assert_eq!(ViaMemoria::desde_texto("xyz"), None);
    }
}

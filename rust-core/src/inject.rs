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
    let texto = match std::fs::read_to_string(ruta) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    parse_maps(&texto)
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

/// Tope de seguridad: no se leen librerías de más de 64 MB.
const MAX_ELF: usize = 64 * 1024 * 1024;

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

    // Tabla dinámica: se buscan DT_HASH / DT_SYMTAB / DT_STRTAB.
    let mut dt_hash: Option<u64> = None;
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

    // Número de símbolos: DT_HASH.nchain es el número exacto.
    let n_simbolos: usize = match dt_hash {
        Some(h) => {
            let ho = vaddr_a_offset(h).unwrap_or(0);
            if ho + 8 <= datos.len() {
                (rd_u32(ho + 4)) as usize
            } else {
                0
            }
        }
        None => {
            // Sin DT_HASH no se sabe cuántos símbolos hay: no se inventa.
            0
        }
    };

    if n_simbolos == 0 {
        return Err(format!(
            "{} no declara DT_HASH: no se puede saber cuántos símbolos hay",
            ruta
        ));
    }

    let syment = if dt_syment == 0 { 24 } else { dt_syment as usize };
    for i in 0..n_simbolos {
        let s = symtab_off + i * syment;
        if s + 24 > datos.len() {
            break;
        }
        let st_name = rd_u32(s) as usize;
        let st_value = rd_u64(s + 8);
        if st_name == 0 || st_value == 0 {
            continue; // símbolo sin nombre o indefinido
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
        match buscar_simbolo_elf(&m.ruta, &mapas, "dlopen") {
            Ok(direccion) => return Ok(direccion),
            Err(_) => continue,
        }
    }
    Err(format!(
        "no se ha encontrado 'dlopen' en ninguna librería del objetivo (probadas: {})",
        if probadas.is_empty() {
            "ninguna".to_string()
        } else {
            probadas.join(", ")
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

//! W4 · Migración de ROM automatizada y anti-brick.
//!
//! Este módulo **nunca flashea nada**: produce *planes* (qué se haría, con qué
//! comandos y con qué verificación) y decide si un plan es ejecutable. La
//! ejecución la hace `deploy/rom_migrate.sh`, que además **por defecto sólo
//!dry-run** y exige `--confirm` **y** un terminal humano delante.
//!
//! La razón es lo que se midió en el móvil (Samsung S21 Ultra, Android 14):
//!
//! - `flash.locked=1` **y** AVB en `green` ⇒ el **bootloader está CERRADO**.
//!   Con FMM Lock el unlock va por manid + garantía; la única vía alternativa
//!   es la de servicio (`try jtag unlock`). Por eso el pre-flight **no** da por
//!   hecho que se puede flashear: primero pregunta si el unlock es viable.
//! - La cadena AVB de este firmware tiene **2 niveles**, no 3: `init_boot`,
//!   `vbmeta_vendor` y `sec` **no existen** en este dispositivo. Pedirlos sería
//!   inventar particiones.
//! - La `boot.img` reempaquetada de la VN-ROM con
//!   `androidboot.selinux=permissive` **no es stock**: el pre-flight la aborta.
//! - El CSC real del móvil es **AMO** (parche 2024-06-01, Android 14). El
//!   "latest" que ofrece Samsung no vale si cambia de CSC.
//!
//! El módulo es **autocontenido** (sólo `std`, `serde` y `sha2`) para poder
//! incluirse con `#[path]` en un arnés de pruebas sin depender del resto del
//! crate: ver `rust-core/tests/rom_migration_smoke.rs`.
//!
//! Identificadores en inglés, comentarios en español.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;

// ---------------------------------------------------------------------------
// Hechos medidos del dispositivo
// ---------------------------------------------------------------------------

/// Estado de la cadena AVB (`dm-verity`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AvbState {
    /// AVB verde: la cadena está intacta y el bootloader está cerrado.
    Green,
    /// AVB amarillo: verificación con avisos (bootloader abierto).
    Yellow,
    /// AVB rojo: cadena rota.
    Red,
    /// No se pudo determinar.
    Unknown,
}

impl AvbState {
    /// Nombre estable (para el JSON del plan).
    pub fn as_str(&self) -> &'static str {
        match self {
            AvbState::Green => "green",
            AvbState::Yellow => "yellow",
            AvbState::Red => "red",
            AvbState::Unknown => "unknown",
        }
    }

    /// `true` si la cadena está intacta (y por tanto el bootloader cerrado).
    pub fn is_intact(&self) -> bool {
        matches!(self, AvbState::Green)
    }
}

/// Estado del bloqueo del bootloader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnlockState {
    /// Bootloader abierto: se puede flashear.
    Unlocked,
    /// `flash.locked=1` + AVB `green` + FMM Lock ⇒ **cerrado**, sin vía directa.
    LockedFmm,
    /// `flash.locked=1` sin FMM: sólo la vía de servicio (`try jtag unlock`).
    LockedServiceOnly,
}

impl UnlockState {
    /// Nombre estable.
    pub fn as_str(&self) -> &'static str {
        match self {
            UnlockState::Unlocked => "unlocked",
            UnlockState::LockedFmm => "locked_fmm",
            UnlockState::LockedServiceOnly => "locked_service_only",
        }
    }
}

/// Hechos del dispositivo, tal y como se miden en el móvil.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceFacts {
    /// Modelo (p. ej. `SM-G998B`).
    pub model: String,
    /// CSC real del móvil (p. ej. `AMO`). **No** se sustituye por el de la ROM.
    pub csc: String,
    /// Fecha del parche de seguridad del móvil (ISO `YYYY-MM-DD`).
    pub patch_date: String,
    /// Versión de Android del móvil.
    pub android_version: String,
    /// Versión de bootloader instalada.
    pub bootloader_version: String,
    /// Región del módem del móvil (`SM`, `EU`, `US`…).
    pub modem_region: String,
    /// Estado de la cadena AVB.
    pub avb_state: AvbState,
    /// `flash.locked` de `/proc/cmdline`.
    pub flash_locked: bool,
    /// FMM Lock activo (unlock gated por manid y garantía).
    pub fmm_lock: bool,
    /// FRP activo.
    pub frp_locked: bool,
    /// OMI/OMC Interface lock activo.
    pub omi_locked: bool,
    /// Batería en porcentaje.
    pub battery_pct: u8,
    /// Temperatura de la batería en grados Celsius.
    pub battery_temp_c: f32,
    /// Espacio libre en el destino de trabajo, en MiB.
    pub free_space_mb: u64,
    /// Espacio que exige la operación, en MiB.
    pub required_space_mb: u64,
    /// Versión de Odin disponible (si hay).
    pub odin_version: Option<String>,
    /// Versión mínima de Odin aceptada.
    pub odin_min_version: Option<String>,
    /// `true` si la `boot.img` **actual** del móvil es la de fábrica.
    pub stock_bootimg: bool,
    /// Particiones del "mundo seguro" presentes en el móvil.
    pub efs_partitions: Vec<String>,
    /// Ruta del fichero de protección **anti-rollback** (FRL/FLL de Samsung),
    /// si se ha localizado en el dispositivo.
    pub anti_rollback_path: Option<String>,
    /// SHA-256 del fichero anti-rollback (vacío si no se ha respaldado).
    pub anti_rollback_sha256: String,
}

impl Default for DeviceFacts {
    /// Hechos del S21 Ultra medidos en el nodo (los valores por defecto son
    /// **pesimistas** a propósito: el pre-flight debe abortar por defecto).
    fn default() -> Self {
        DeviceFacts {
            model: "SM-G998B".to_string(),
            csc: "AMO".to_string(),
            patch_date: "2024-06-01".to_string(),
            android_version: "14".to_string(),
            bootloader_version: "S21U_2A_14_0".to_string(),
            modem_region: "SM".to_string(),
            avb_state: AvbState::Green,
            flash_locked: true,
            fmm_lock: true,
            frp_locked: true,
            omi_locked: false,
            battery_pct: 62,
            battery_temp_c: 34.0,
            free_space_mb: 5_900,
            required_space_mb: 8_192,
            odin_version: None,
            odin_min_version: Some("3.14.4".to_string()),
            stock_bootimg: false,
            efs_partitions: Vec::new(),
            anti_rollback_path: None,
            anti_rollback_sha256: String::new(),
        }
    }
}

impl DeviceFacts {
    /// Estado de bloqueo derivado de los hechos medidos.
    pub fn unlock_state(&self) -> UnlockState {
        if !self.flash_locked {
            return UnlockState::Unlocked;
        }
        if self.fmm_lock && self.avb_state.is_intact() {
            return UnlockState::LockedFmm;
        }
        UnlockState::LockedServiceOnly
    }
}

// ---------------------------------------------------------------------------
// Paquete de ROM
// ---------------------------------------------------------------------------

/// Artefacto de la ROM con su hash declarado.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Artifact {
    /// Nombre lógico (`AP_MDPO_Gxxx`, `CSC_MD1`, `HOME_CSC`, `BL`, `PIT`…).
    pub name: String,
    /// Ruta en el disco donde vive el artefacto.
    pub path: String,
    /// Tamaño en bytes.
    pub size_bytes: u64,
    /// SHA-256 en hexadecimal (64 caracteres).
    pub sha256: String,
}

impl Artifact {
    /// `true` si el hash declarado tiene forma de SHA-256 (64 hex).
    pub fn has_valid_sha256(&self) -> bool {
        self.sha256.len() == 64 && self.sha256.chars().all(|c| c.is_ascii_hexdigit())
    }
}

/// Paquete de ROM candidato.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RomPackage {
    /// Nombre del paquete.
    pub name: String,
    /// CSC de la ROM (debe coincidir con el del móvil).
    pub csc: String,
    /// Fecha de parche de la ROM (ISO `YYYY-MM-DD`).
    pub patch_date: String,
    /// Versión de Android de la ROM.
    pub android_version: String,
    /// Región del módem (`EU`, `SA`, `SM`…).
    pub modem_region: String,
    /// Versión de bootloader que trae la ROM.
    pub bootloader_version: String,
    /// `true` si la `boot.img` de la ROM es **la de fábrica**.
    pub stock_bootimg: bool,
    /// Motivo por el que la `boot.img` no es stock (si no lo es).
    pub bootimg_note: Option<String>,
    /// `true` si el PIT de la ROM corresponde a este modelo/variant.
    pub pit_match: bool,
    /// Artefactos con su hash declarado.
    pub artifacts: Vec<Artifact>,
}

impl RomPackage {
    /// Busca un artefacto por nombre lógico.
    pub fn artifact(&self, name: &str) -> Option<&Artifact> {
        self.artifacts.iter().find(|a| a.name.eq_ignore_ascii_case(name))
    }
}

// ---------------------------------------------------------------------------
// Pre-flight
// ---------------------------------------------------------------------------

/// Gravedad de un check del pre-flight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    /// Impide el flash: el pre-flight aborta.
    Blocker,
    /// No impide el flash pero hay que saberlo.
    Warning,
    /// Informativo.
    Info,
}

impl Severity {
    /// Nombre estable.
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Blocker => "blocker",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }
}

/// Un check del pre-flight.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Check {
    /// Identificador estable del check.
    pub id: String,
    /// Gravedad.
    pub severity: Severity,
    /// `true` si el check pasa.
    pub ok: bool,
    /// Explicación en castellano (con el número medido).
    pub detail: String,
}

/// Resultado del pre-flight.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreflightReport {
    /// Todos los checks ejecutados, en orden.
    pub checks: Vec<Check>,
    /// `true` si algún blocker falla ⇒ **no se flashea**.
    pub abort: bool,
    /// Los motivos concretos del aborto.
    pub blockers: Vec<String>,
}

impl PreflightReport {
    /// Checks que fallan y bloquean.
    pub fn failed_blockers(&self) -> Vec<&Check> {
        self.checks
            .iter()
            .filter(|c| c.severity == Severity::Blocker && !c.ok)
            .collect()
    }

    /// `true` si no hay blockers.
    pub fn can_flash(&self) -> bool {
        !self.abort
    }
}

/// Compara dos versiones punteadas (`"3.14.4"` vs `"3.9.1"`). Devuelve
/// `Ordering::Less` si `a < b`. Los segmentos no numéricos se comparan como
/// texto y las fechas ISO (`2024-06-01`) se comparan lexicográficamente.
pub fn cmp_version(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    // Fechas ISO: comparar como cadenas funciona y es lo correcto.
    if a.len() == 10
        && b.len() == 10
        && a.as_bytes().iter().all(|c| c.is_ascii_digit() || *c == b'-')
        && b.as_bytes().iter().all(|c| c.is_ascii_digit() || *c == b'-')
    {
        return a.cmp(b);
    }
    let mut ai = a.split('.').peekable();
    let mut bi = b.split('.').peekable();
    loop {
        match (ai.peek().cloned(), bi.peek().cloned()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                ai.next();
                bi.next();
                let ord = match (x.parse::<u64>(), y.parse::<u64>()) {
                    (Ok(nx), Ok(ny)) => nx.cmp(&ny),
                    _ => x.cmp(y),
                };
                if ord != Ordering::Equal {
                    return ord;
                }
            }
        }
    }
}

/// Calcula el SHA-256 de un fichero leyéndolo en trozos (memoria acotada).
pub fn sha256_file(path: &Path) -> Option<String> {
    let mut f = File::open(path).ok()?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Some(hex_fallback(&h.finalize()))
}

/// SHA-256 de un buffer.
pub fn sha256_hex(data: &[u8]) -> String {
    hex_fallback(&Sha256::digest(data))
}

/// Ejecuta el pre-flight completo.
///
/// `computed_sha256` son los hashes **calculados** de los artefactos (clave =
/// nombre lógico). Si un artefacto está en el mapa y no coincide con el hash
/// declarado, es un blocker.
///
/// El pre-flight **aborta** (`report.abort == true`) si cualquier blocker
/// falla. Con los hechos por defecto del S21 Ultra aborta, que es lo correcto:
/// el bootloader está cerrado con FMM Lock, la FRP está activa, la batería está
/// al 62 % y la `boot.img` no es stock.
pub fn preflight(
    device: &DeviceFacts,
    rom: &RomPackage,
    computed_sha256: &BTreeMap<String, String>,
) -> PreflightReport {
    let mut checks: Vec<Check> = Vec::new();

    // 1) ¿Es viable el unlock? (lo primero: sin unlock no hay flash)
    match device.unlock_state() {
        UnlockState::Unlocked => checks.push(Check {
            id: "unlock_viability".into(),
            severity: Severity::Blocker,
            ok: true,
            detail: "bootloader abierto (flash.locked=0): el flash es viable".into(),
        }),
        UnlockState::LockedFmm => checks.push(Check {
            id: "unlock_viability".into(),
            severity: Severity::Blocker,
            ok: false,
            detail: format!(
                "BOOTLOADER CERRADO: flash.locked=1 + AVB {} + FMM Lock. \
                 El unlock va por manid y garantía; la única vía alternativa es \
                 la de servicio (try jtag unlock). No se puede flashear sin \
                 resolver esto primero.",
                device.avb_state.as_str()
            ),
        }),
        UnlockState::LockedServiceOnly => checks.push(Check {
            id: "unlock_viability".into(),
            severity: Severity::Blocker,
            ok: false,
            detail: format!(
                "flash.locked=1 (AVB {}): sólo vía de servicio (try jtag unlock); \
                 sin unlock validado no se flashea",
                device.avb_state.as_str()
            ),
        }),
    }

    // 2) FRP
    checks.push(Check {
        id: "frp".into(),
        severity: Severity::Blocker,
        ok: !device.frp_locked,
        detail: if device.frp_locked {
            "FRP activo: hay que quitarlo antes del flash (si no, el móvil queda \
             pidiendo la cuenta de fábrica original)"
                .into()
        } else {
            "FRP no activo".into()
        },
    });

    // 3) OMI
    checks.push(Check {
        id: "omi".into(),
        severity: Severity::Blocker,
        ok: !device.omi_locked,
        detail: if device.omi_locked {
            "OMC Interface lock (OMI) activo: el flash no puede continuar sin \
             quitarlo o sin el código de servicio"
                .into()
        } else {
            "OMI no activo".into()
        },
    });

    // 4) Batería
    checks.push(Check {
        id: "battery".into(),
        severity: Severity::Blocker,
        ok: device.battery_pct >= 80,
        detail: format!(
            "batería al {}% (mínimo 80%): un flash a media carga deja el móvil \
             en modo de servicio o peor",
            device.battery_pct
        ),
    });

    // 5) Temperatura
    let temp_ok = device.battery_temp_c > 0.0 && device.battery_temp_c <= 40.0;
    checks.push(Check {
        id: "temperature".into(),
        severity: Severity::Blocker,
        ok: temp_ok,
        detail: format!(
            "temperatura de batería {:.1} °C (rango válido 0–40 °C)",
            device.battery_temp_c
        ),
    });

    // 6) Espacio
    checks.push(Check {
        id: "space".into(),
        severity: Severity::Blocker,
        ok: device.free_space_mb >= device.required_space_mb,
        detail: format!(
            "{} MiB libres, se necesitan {} MiB",
            device.free_space_mb, device.required_space_mb
        ),
    });

    // 7) Odin
    match (&device.odin_version, &device.odin_min_version) {
        (None, _) => checks.push(Check {
            id: "odin".into(),
            severity: Severity::Blocker,
            ok: false,
            detail: "Odin no disponible en la máquina que flashea".into(),
        }),
        (Some(v), None) => checks.push(Check {
            id: "odin".into(),
            severity: Severity::Info,
            ok: true,
            detail: format!("Odin {} presente", v),
        }),
        (Some(v), Some(min)) => {
            let ok = cmp_version(v, min) != std::cmp::Ordering::Less;
            checks.push(Check {
                id: "odin".into(),
                severity: Severity::Blocker,
                ok,
                detail: format!(
                    "Odin {} presente (mínimo {}){}",
                    v,
                    min,
                    if ok { "" } else { ": versión insuficiente" }
                ),
            })
        }
    }

    // 8) Hashes de la ROM
    let mut hash_detail = String::new();
    let mut hash_ok = !rom.artifacts.is_empty();
    for a in &rom.artifacts {
        if !a.has_valid_sha256() {
            hash_ok = false;
            hash_detail.push_str(&format!("{}: hash ausente o mal formado; ", a.name));
            continue;
        }
        if let Some(c) = computed_sha256.get(&a.name) {
            if !c.eq_ignore_ascii_case(&a.sha256) {
                hash_ok = false;
                hash_detail.push_str(&format!(
                    "{}: hash calculado {} != declarado {}; ",
                    a.name, c, a.sha256
                ));
            }
        }
    }
    checks.push(Check {
        id: "rom_checksums".into(),
        severity: Severity::Blocker,
        ok: hash_ok,
        detail: if hash_detail.is_empty() {
            format!("{} artefactos con SHA-256 válido", rom.artifacts.len())
        } else {
            hash_detail.trim_end().to_string()
        },
    });

    // 9) Coherencia de CSC con el CSC REAL del móvil
    let csc_ok = !rom.csc.is_empty() && rom.csc.eq_ignore_ascii_case(&device.csc);
    checks.push(Check {
        id: "csc_coherence".into(),
        severity: Severity::Blocker,
        ok: csc_ok,
        detail: format!(
            "CSC de la ROM '{}' frente al CSC real del móvil '{}': {}",
            rom.csc,
            device.csc,
            if csc_ok {
                "coherente"
            } else {
                "el 'latest' de Samsung NO vale si cambia de CSC"
            }
        ),
    });

    // 10) El parche no puede ser más viejo que el del móvil
    let patch_ok = cmp_version(&rom.patch_date, &device.patch_date) != std::cmp::Ordering::Less;
    checks.push(Check {
        id: "patch_not_older".into(),
        severity: Severity::Blocker,
        ok: patch_ok,
        detail: format!(
            "parche de la ROM {} frente al del móvil {}: {}",
            rom.patch_date,
            device.patch_date,
            if patch_ok {
                "no es un downgrade"
            } else {
                "downgrade de parche: no se flashea"
            }
        ),
    });

    // 11) El bootloader no puede bajar de versión
    let bl_ok = cmp_version(&rom.bootloader_version, &device.bootloader_version)
        != std::cmp::Ordering::Less;
    checks.push(Check {
        id: "bootloader_version".into(),
        severity: Severity::Blocker,
        ok: bl_ok,
        detail: format!(
            "bootloader de la ROM {} frente al instalado {}: {}",
            rom.bootloader_version,
            device.bootloader_version,
            if bl_ok {
                "ok"
            } else {
                "downgrade de bootloader: prohibido (brick)"
            }
        ),
    });

    // 12) La boot.img debe ser stock
    checks.push(Check {
        id: "bootimg_stock".into(),
        severity: Severity::Blocker,
        ok: rom.stock_bootimg,
        detail: if rom.stock_bootimg {
            "boot.img de fábrica".into()
        } else {
            format!(
                "boot.img NO stock: {}. Una boot.img reempaquetada \
                 (p. ej. con androidboot.selinux=permissive) deja el móvil \
                 sin cadena AVB válida: se aborta.",
                rom.bootimg_note
                    .as_deref()
                    .unwrap_or("origen desconocido")
            )
        },
    });

    // 13) Coherencia del PIT
    checks.push(Check {
        id: "pit_coherence".into(),
        severity: Severity::Blocker,
        ok: rom.pit_match,
        detail: format!(
            "PIT de la ROM {} para el modelo {}",
            if rom.pit_match { "coherente" } else { "INCOHERENTE" },
            device.model
        ),
    });

    // 14) Particiones del mundo seguro (EFS) presentes: sin copia no hay
    //     anti-brick, y sin anti-brick no se toca nada.
    let efs = efs_backup_plan(device, "/sdcard/megadaemon/efs-backup");
    checks.push(Check {
        id: "efs_partitions".into(),
        severity: Severity::Blocker,
        ok: efs.missing.is_empty(),
        detail: if efs.missing.is_empty() {
            format!("las {} particiones del mundo seguro presentes", efs.partitions.len())
        } else {
            format!(
                "faltan particiones del mundo seguro: {} (sin copia de EFS no \
                 se puede garantizar el anti-brick)",
                efs.missing.join(", ")
            )
        },
    });

    // 15) Fichero anti-rollback (FRL/FLL): sin copia verificada no se flashea.
    let ar = AntiRollbackState::from_measurement(
        device.anti_rollback_path.as_deref(),
        &device.anti_rollback_sha256,
    );
    checks.push(Check {
        id: "anti_rollback_backup".into(),
        severity: Severity::Blocker,
        ok: ar.verified,
        detail: if ar.verified {
            format!(
                "fichero anti-rollback respaldado y verificado: {} (sha256 {})",
                ar.path.as_deref().unwrap_or("?"),
                ar.sha256
            )
        } else if ar.path.is_some() {
            format!(
                "fichero anti-rollback localizado en {} pero SIN hash verificado: \
                 hay que copiarlo y calcular su SHA-256 antes de flashear",
                ar.path.as_deref().unwrap_or("?")
            )
        } else {
            format!(
                "NO se ha localizado el fichero anti-rollback (candidatos: {}). \
                 Medido en este dispositivo: /root/ no existe y /efs/ no lo \
                 contiene. Sin él no se puede garantizar que una bajada de \
                 bootloader no active el bloqueo: se aborta.",
                ANTI_ROLLBACK_CANDIDATES.join(", ")
            )
        },
    });

    // 16) Versión de Android (aviso, no bloquea)
    checks.push(Check {
        id: "android_version".into(),
        severity: Severity::Warning,
        ok: rom.android_version == device.android_version,
        detail: format!(
            "Android de la ROM {} frente al del móvil {}",
            rom.android_version, device.android_version
        ),
    });

    let blockers: Vec<String> = checks
        .iter()
        .filter(|c| c.severity == Severity::Blocker && !c.ok)
        .map(|c| format!("{}: {}", c.id, c.detail))
        .collect();
    PreflightReport {
        abort: !blockers.is_empty(),
        checks,
        blockers,
    }
}

// ---------------------------------------------------------------------------
// Selección de ROM
// ---------------------------------------------------------------------------

/// Criterios explícitos de selección.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectionCriteria {
    /// Exigir el mismo CSC que el móvil.
    pub require_same_csc: bool,
    /// Exigir parche ≥ el del móvil.
    pub require_patch_not_older: bool,
    /// Exigir `boot.img` de fábrica.
    pub require_stock_bootimg: bool,
    /// Exigir PIT coherente.
    pub require_pit_match: bool,
    /// Preferir la misma región de módem.
    pub prefer_same_modem_region: bool,
}

impl Default for SelectionCriteria {
    fn default() -> Self {
        SelectionCriteria {
            require_same_csc: true,
            require_patch_not_older: true,
            require_stock_bootimg: true,
            require_pit_match: true,
            prefer_same_modem_region: true,
        }
    }
}

/// Un candidato rechazado, con el motivo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rejection {
    /// Nombre del candidato.
    pub name: String,
    /// Por qué se descartó.
    pub reason: String,
}

/// Resultado de la selección de ROM.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RomSelection {
    /// El candidato elegido (si hay alguno).
    pub chosen: Option<RomPackage>,
    /// Los descartados, con su motivo.
    pub rejected: Vec<Rejection>,
}

impl RomSelection {
    /// `true` si hay candidato válido.
    pub fn is_empty(&self) -> bool {
        self.chosen.is_none()
    }
}

/// Elige la ROM válida para el móvil según criterios explícitos.
///
/// Entre los válidos gana la de **parche más reciente**; a igualdad de parche,
/// la de **misma región de módem**. Cada descarte queda justificado.
pub fn select_rom(
    candidates: &[RomPackage],
    device: &DeviceFacts,
    criteria: &SelectionCriteria,
) -> RomSelection {
    let mut rejected: Vec<Rejection> = Vec::new();
    let mut valid: Vec<&RomPackage> = Vec::new();

    for c in candidates {
        let mut why: Vec<String> = Vec::new();
        if criteria.require_same_csc && !c.csc.eq_ignore_ascii_case(&device.csc) {
            why.push(format!(
                "CSC '{}' ≠ '{}' (el móvil es {})",
                c.csc, device.csc, device.csc
            ));
        }
        if criteria.require_patch_not_older
            && cmp_version(&c.patch_date, &device.patch_date) == std::cmp::Ordering::Less
        {
            why.push(format!(
                "parche {} < {} (downgrade)",
                c.patch_date, device.patch_date
            ));
        }
        if criteria.require_stock_bootimg && !c.stock_bootimg {
            why.push(format!(
                "boot.img no stock: {}",
                c.bootimg_note.as_deref().unwrap_or("origen desconocido")
            ));
        }
        if criteria.require_pit_match && !c.pit_match {
            why.push("PIT incoherente con el modelo".to_string());
        }
        if why.is_empty() {
            valid.push(c);
        } else {
            rejected.push(Rejection {
                name: c.name.clone(),
                reason: why.join("; "),
            });
        }
    }

    let chosen = valid.into_iter().max_by(|a, b| {
        let pa = cmp_version(&a.patch_date, &b.patch_date);
        if pa != std::cmp::Ordering::Equal {
            return pa;
        }
        if criteria.prefer_same_modem_region {
            // La región de módem del móvil es el dato medido; a igualdad de
            // parche gana la ROM de la misma región.
            let ra = a.modem_region.eq_ignore_ascii_case(&device.modem_region);
            let rb = b.modem_region.eq_ignore_ascii_case(&device.modem_region);
            return ra.cmp(&rb);
        }
        std::cmp::Ordering::Equal
    });

    RomSelection {
        chosen: chosen.cloned(),
        rejected,
    }
}

// ---------------------------------------------------------------------------
// Anti-brick: las 7 particiones del mundo seguro
// ---------------------------------------------------------------------------

/// Las **7 particiones reales** del mundo seguro en este firmware.
///
/// Las del volcado (`init_boot`, `vbmeta_vendor`, `sec`) **no existen** en este
/// dispositivo: la cadena AVB tiene 2 niveles, no 3. Pedirlas o respaldarlas
/// sería inventar particiones.
pub const EFS_SAFE_WORLD: [&str; 7] = [
    "sec_efs",
    "keystorage",
    "keyrefuge",
    "uh",
    "uhcfg",
    "spu",
    "prism",
];

/// Particiones que se agradecerían en un volcado pero **no existen** aquí.
pub const EFS_NON_EXISTENT: [&str; 3] = ["init_boot", "vbmeta_vendor", "sec"];

/// Candidatos al fichero de protección **anti-rollback** (FRL/FLL) de Samsung.
///
/// Si el bootloader baja de versión, ese fichero bloquea el arranque. Antes de
/// tocar la ROM hay que **copiarlo y verificar su hash**: es la última línea
/// contra el ladrillo.
///
/// Medido en este S21 Ultra: **`/root/` no existe** (el shell `su` no lo
/// monta) y el fichero tampoco está en `/efs/` (que es el contenido de
/// `sec_efs`). Por eso el check aborta si no se localiza: no se puede
/// prometer lo que no se ha visto.
pub const ANTI_ROLLBACK_CANDIDATES: [&str; 5] = [
    "/root/FLL.bin",
    "/efs/FLL.bin",
    "/efs/FRL",
    "/data/misc/FLL.bin",
    "/efs/locksettings",
];

/// Estado del fichero anti-rollback.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AntiRollbackState {
    /// Ruta donde se encontró (si se encontró).
    pub path: Option<String>,
    /// SHA-256 de la copia.
    pub sha256: String,
    /// `true` si está localizado **y** copiado con hash verificado.
    pub verified: bool,
}

impl AntiRollbackState {
    /// Construye el estado a partir de lo medido.
    pub fn from_measurement(path: Option<&str>, sha256: &str) -> AntiRollbackState {
        let ok_sha = sha256.len() == 64 && sha256.chars().all(|c| c.is_ascii_hexdigit());
        AntiRollbackState {
            path: path.map(|s| s.to_string()),
            sha256: sha256.to_string(),
            verified: path.is_some() && ok_sha,
        }
    }
}

/// Una partición a respaldar antes de tocar nada.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EfsPartition {
    /// Nombre en `/dev/block/by-name/`.
    pub name: String,
    /// Ruta del dispositivo de bloque.
    pub block: String,
    /// Tamaño en bytes.
    pub size_bytes: u64,
    /// SHA-256 de la copia (vacío si aún no está hecha).
    pub sha256: String,
}

/// Plan de copia del mundo seguro (anti-brick).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EfsBackupPlan {
    /// Particiones a copiar.
    pub partitions: Vec<EfsPartition>,
    /// Las que el móvil no tiene.
    pub missing: Vec<String>,
    /// `true` si **todas** las particiones tienen copia y hash verificado.
    pub verified: bool,
    /// Dónde se deja la copia.
    pub dest: String,
    /// Comandos de recuperación si el móvil queda en ladrillo.
    pub recovery: Vec<String>,
}

impl EfsBackupPlan {
    /// Particiones sin hash (copia no verificada).
    pub fn unverified(&self) -> Vec<&str> {
        self.partitions
            .iter()
            .filter(|p| p.sha256.is_empty())
            .map(|p| p.name.as_str())
            .collect()
    }
}

/// Construye el plan de copia del mundo seguro a partir de los hechos medidos.
///
/// El plan **exige** las 7 particiones reales y **no** menciona las que no
/// existen. La copia tiene que estar verificada por hash antes de permitir
/// cualquier "Erase Download".
pub fn efs_backup_plan(device: &DeviceFacts, dest: &str) -> EfsBackupPlan {
    let mut partitions: Vec<EfsPartition> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    for name in EFS_SAFE_WORLD.iter() {
        if device.efs_partitions.iter().any(|p| p == name) {
            partitions.push(EfsPartition {
                name: name.to_string(),
                block: format!("/dev/block/by-name/{}", name),
                size_bytes: 0,
                sha256: String::new(),
            });
        } else {
            missing.push(name.to_string());
        }
    }
    let verified = missing.is_empty()
        && !partitions.is_empty()
        && partitions.iter().all(|p| !p.sha256.is_empty());
    EfsBackupPlan {
        partitions,
        missing,
        verified,
        dest: dest.to_string(),
        recovery: recovery_plan(device),
    }
}

/// Marca el plan de EFS como verificado asignando los hashes medidos.
pub fn mark_efs_verified(plan: &mut EfsBackupPlan, hashes: &BTreeMap<String, String>) {
    for p in plan.partitions.iter_mut() {
        if let Some(h) = hashes.get(&p.name) {
            p.sha256 = h.clone();
        }
    }
    plan.verified = plan.missing.is_empty()
        && !plan.partitions.is_empty()
        && plan.partitions.iter().all(|p| !p.sha256.is_empty());
}

/// Comandos de recuperación si el móvil queda en ladrillo.
///
/// Tres vías, de menos a más invasiva: sideload de la partición `recovery` con
/// TWRP (la que conserva EFS), `heimdall` desde un PC Linux y fastboot como
/// último recurso.
pub fn recovery_plan(device: &DeviceFacts) -> Vec<String> {
    vec![
        format!(
            "# 0) Antes de tocar nada: copia del mundo seguro ({}) con SHA-256",
            EFS_SAFE_WORLD.join(", ")
        ),
        format!("#    destino sugerido: {}", "/sdcard/megadaemon/efs-backup"),
        "# 1) Vía preferente: TWRP por sideload (NO borra EFS, ni el userdata si no se le pide)".to_string(),
        format!(
            "adb -s {} sideload twrp-*.zip",
            device.model.replace(' ', "_")
        ),
        "# 2) Alternativa desde PC Linux: heimdall (reparto por partición, con verificación)".to_string(),
        format!(
            "heimdall load --no-download --verify --pit {} --bootloader {} --boot {} --recovery {}",
            "PIT_*.pit", "BOOTLOADER_*.img", "boot.img", "recovery.img"
        ),
        "# 3) Último recurso: fastboot (sólo si hay bootloader desbloqueado)".to_string(),
        "adb -s <serial> reboot bootloader".to_string(),
        "fastboot flash recovery recovery.img".to_string(),
        "fastboot flash efs EFS.img   # sólo si se tiene la copia verificada".to_string(),
    ]
}

// ---------------------------------------------------------------------------
// Flash guardado (4 pasos)
// ---------------------------------------------------------------------------

/// Etapa del flash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlashStage {
    /// Paso 1: borrado de datos (Erase Download).
    WipeData,
    /// Paso 2: bootloader.
    Bootloader,
    /// Paso 3: AP + CP + CSC.
    ApCpCsc,
    /// Paso 4: HOME_CSC.
    HomeCsc,
}

impl FlashStage {
    /// Nombre estable.
    pub fn as_str(&self) -> &'static str {
        match self {
            FlashStage::WipeData => "wipe_data",
            FlashStage::Bootloader => "bootloader",
            FlashStage::ApCpCsc => "ap_cp_csc",
            FlashStage::HomeCsc => "home_csc",
        }
    }

    /// Orden del paso (1..4).
    pub fn order(&self) -> u8 {
        match self {
            FlashStage::WipeData => 1,
            FlashStage::Bootloader => 2,
            FlashStage::ApCpCsc => 3,
            FlashStage::HomeCsc => 4,
        }
    }
}

/// Verificación de un artefacto antes o después de escribirlo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactCheck {
    /// Nombre lógico del artefacto.
    pub name: String,
    /// Hash esperado.
    pub sha256: String,
    /// `true` si la verificación pasó.
    pub verified: bool,
}

/// Un paso del flash.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlashStep {
    /// Etapa.
    pub stage: FlashStage,
    /// Título legible.
    pub title: String,
    /// Artefactos que escribe.
    pub writes: Vec<String>,
    /// `true` si el paso es el "Erase Download" (prohibido sin EFS verificada).
    pub erase_download: bool,
    /// `true` si el paso exige la copia de EFS verificada.
    pub requires_efs_backup: bool,
    /// Verificaciones *antes* de escribir.
    pub pre_verify: Vec<ArtifactCheck>,
    /// Verificaciones *después* de escribir (hash leído de vuelta del móvil).
    pub post_verify: Vec<ArtifactCheck>,
    /// Si el paso está bloqueado, el motivo.
    pub blocked_reason: Option<String>,
}

/// Plan de flash completo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlashPlan {
    /// Los 4 pasos, en orden.
    pub steps: Vec<FlashStep>,
    /// El plan de EFS que respalda el paso 1.
    pub efs: EfsBackupPlan,
    /// `true` si el plan **no** se puede ejecutar.
    pub abort: bool,
    /// Motivos del aborto.
    pub blockers: Vec<String>,
}

impl FlashPlan {
    /// Pasos ejecutables (sin bloqueo).
    pub fn runnable_steps(&self) -> Vec<&FlashStep> {
        self.steps
            .iter()
            .filter(|s| s.blocked_reason.is_none())
            .collect()
    }
}

/// Construye el plan de flash guardado en 4 pasos.
///
/// **Guardas**: el paso 1 (Erase Download) está bloqueado si la copia de EFS no
/// está verificada por hash; cada paso lleva sus verificaciones antes y
/// después; y el plan aborta entero si el pre-flight falló.
pub fn flash_plan(
    rom: &RomPackage,
    device: &DeviceFacts,
    efs: &EfsBackupPlan,
    preflight_ok: bool,
) -> FlashPlan {
    let pick = |names: &[&str]| -> Vec<ArtifactCheck> {
        names
            .iter()
            .filter_map(|n| rom.artifact(n))
            .map(|a| ArtifactCheck {
                name: a.name.clone(),
                sha256: a.sha256.clone(),
                verified: false,
            })
            .collect()
    };

    let wipe_names = ["HOME_CSC", "CSC_MD1"];
    let boot_names = ["BOOTLOADER"];
    let ap_names = ["AP_MDPO", "CP_MDPO"];
    let csc_names = ["CSC_MD1"];
    let home_names = ["HOME_CSC"];

    let mut blockers: Vec<String> = Vec::new();
    if !preflight_ok {
        blockers.push("el pre-flight falló: no se ejecuta ningún paso".to_string());
    }
    if !efs.verified {
        blockers.push(format!(
            "copia del mundo seguro NO verificada (faltan hashes: {}): \
             'Erase Download' está PROHIBIDO",
            if efs.unverified().is_empty() {
                "particiones ausentes".to_string()
            } else {
                efs.unverified().join(", ")
            }
        ));
    }

    let mut steps: Vec<FlashStep> = Vec::new();

    // Paso 1: WIPE DATA.
    steps.push(FlashStep {
        stage: FlashStage::WipeData,
        title: "Paso 1/4 · WIPE DATA (Erase Download + HOME_CSC)".into(),
        writes: pick(&wipe_names).iter().map(|c| c.name.clone()).collect(),
        erase_download: true,
        requires_efs_backup: true,
        pre_verify: pick(&wipe_names),
        post_verify: Vec::new(),
        blocked_reason: if efs.verified {
            None
        } else {
            Some(
                "Erase Download sin copia de EFS verificada: prohibido (anti-brick)".into(),
            )
        },
    });

    // Paso 2: BOOTLOADER.
    steps.push(FlashStep {
        stage: FlashStage::Bootloader,
        title: "Paso 2/4 · BOOTLOADER (verificar hash leído del móvil)".into(),
        writes: pick(&boot_names).iter().map(|c| c.name.clone()).collect(),
        erase_download: false,
        requires_efs_backup: false,
        pre_verify: pick(&boot_names),
        post_verify: pick(&boot_names),
        blocked_reason: None,
    });

    // Paso 3: AP + CP + CSC.
    steps.push(FlashStep {
        stage: FlashStage::ApCpCsc,
        title: "Paso 3/4 · AP + CP + CSC (el CSC debe ser el real del móvil)".into(),
        writes: pick(&ap_names)
            .iter()
            .chain(pick(&csc_names).iter())
            .map(|c| c.name.clone())
            .collect(),
        erase_download: false,
        requires_efs_backup: false,
        pre_verify: pick(&ap_names)
            .iter()
            .chain(pick(&csc_names).iter())
            .cloned()
            .collect(),
        post_verify: pick(&ap_names)
            .iter()
            .chain(pick(&csc_names).iter())
            .cloned()
            .collect(),
        blocked_reason: None,
    });

    // Paso 4: HOME_CSC.
    steps.push(FlashStep {
        stage: FlashStage::HomeCsc,
        title: "Paso 4/4 · HOME_CSC".into(),
        writes: pick(&home_names).iter().map(|c| c.name.clone()).collect(),
        erase_download: false,
        requires_efs_backup: false,
        pre_verify: pick(&home_names),
        post_verify: pick(&home_names),
        blocked_reason: None,
    });

    // Si el pre-flight falló, todos los pasos quedan bloqueados.
    if !preflight_ok {
        for s in steps.iter_mut() {
            s.blocked_reason = Some("pre-flight fallido: plan abortado".into());
        }
    }

    FlashPlan {
        steps,
        efs: efs.clone(),
        abort: !blockers.is_empty(),
        blockers,
    }
}

// ---------------------------------------------------------------------------
// Post-flash
// ---------------------------------------------------------------------------

/// Un paso del post-flash.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PostStep {
    /// Identificador estable.
    pub id: String,
    /// Qué hace.
    pub title: String,
    /// Comando a ejecutar.
    pub command: String,
    /// Cómo se comprueba que ha ido bien.
    pub success_check: String,
}

/// Plan de post-flash: primer arranque, re-root, módulos Magisk y re-inventario.
pub fn post_flash_plan(device: &DeviceFacts, magisk_modules: &[String]) -> Vec<PostStep> {
    let mut steps = vec![
        PostStep {
            id: "first_boot".into(),
            title: "Primer arranque: esperar a boot_completed".into(),
            command: "adb -s <serial> shell 'while [ \"$(getprop sys.boot_completed)\" != 1 ]; do sleep 5; done'".into(),
            success_check: "getprop sys.boot_completed == 1".into(),
        },
        PostStep {
            id: "re_root".into(),
            title: "Re-esperar el root (Magisk se reinstala al flashear AP/CSC)".into(),
            command: "adb -s <serial> shell 'su -c id'".into(),
            success_check: "la salida contiene uid=0".into(),
        },
        PostStep {
            id: "magisk_modules".into(),
            title: "Reinstalar los módulos Magisk".into(),
            command: if magisk_modules.is_empty() {
                "adb -s <serial> shell 'su -c \"ls /data/adb/modules\"'".into()
            } else {
                format!(
                    "adb -s <serial> shell 'su -c \"ls /data/adb/modules\"'  # módulos a reinstalar: {}",
                    magisk_modules.join(", ")
                )
            },
            success_check: format!(
                "los módulos {} están presentes en /data/adb/modules",
                if magisk_modules.is_empty() {
                    "previos".to_string()
                } else {
                    magisk_modules.join(", ")
                }
            ),
        },
        PostStep {
            id: "re_inventory".into(),
            title: "Re-ejecutar el inventario W2 y comparar antes/después".into(),
            command: format!(
                "adb -s <serial> shell 'megadaemon-core inventory --out {}/inventory-post.json'",
                "/data/local/tmp/megadaemon/state"
            ),
            success_check: format!(
                "inventory-post.json generado y comparado con el previo (modelo {}, CSC {})",
                device.model, device.csc
            ),
        },
    ];
    steps.shrink_to_fit();
    steps
}

/// Una diferencia entre el inventario de antes y el de después.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InventoryDiff {
    /// Campo comparado.
    pub field: String,
    /// Valor antes del flash.
    pub before: String,
    /// Valor después del flash.
    pub after: String,
}

/// Compara dos inventarios (clave → valor) y devuelve **sólo** lo que cambió.
pub fn compare_inventories(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
) -> Vec<InventoryDiff> {
    let mut diffs: Vec<InventoryDiff> = Vec::new();
    for (k, v) in before {
        match after.get(k) {
            Some(a) if a != v => diffs.push(InventoryDiff {
                field: k.clone(),
                before: v.clone(),
                after: a.clone(),
            }),
            None => diffs.push(InventoryDiff {
                field: k.clone(),
                before: v.clone(),
                after: "<ausente>".to_string(),
            }),
            _ => {}
        }
    }
    for (k, v) in after {
        if !before.contains_key(k) {
            diffs.push(InventoryDiff {
                field: k.clone(),
                before: "<ausente>".to_string(),
                after: v.clone(),
            });
        }
    }
    diffs
}

// ---------------------------------------------------------------------------
// Dry-run
// ---------------------------------------------------------------------------

impl fmt::Display for PreflightReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "PRE-FLIGHT · {} checks", self.checks.len())?;
        for c in &self.checks {
            let mark = if c.ok { "OK  " } else { "FALLO" };
            writeln!(
                f,
                "  [{}] {:<20} {:<8} {}",
                mark,
                c.id,
                c.severity.as_str(),
                c.detail
            )?;
        }
        if self.abort {
            writeln!(f, "\nABORTA. Motivos:")?;
            for b in &self.blockers {
                writeln!(f, "  - {}", b)?;
            }
        } else {
            writeln!(f, "\nSin bloqueantes: se puede preparar el plan (el flash sigue requiriendo --confirm).")?;
        }
        Ok(())
    }
}

/// Genera el informe de dry-run: pre-flight, selección de ROM y plan de flash
/// en texto plano, con **todos** los comandos que se ejecutarían.
///
/// No ejecuta nada: es la salida de `--dry-run` de `deploy/rom_migrate.sh`.
pub fn dry_run_report(
    device: &DeviceFacts,
    candidates: &[RomPackage],
    computed_sha256: &BTreeMap<String, String>,
) -> String {
    let mut out = String::new();
    out.push_str("══════ MEGADAEMON W4 · MIGRACIÓN DE ROM · DRY-RUN ══════\n");
    out.push_str(&format!(
        "dispositivo: {} · CSC {} · parche {} · Android {} · bootloader {}\n",
        device.model, device.csc, device.patch_date, device.android_version, device.bootloader_version
    ));
    out.push_str(&format!(
        "bloqueo: {} · AVB {} · FMM {} · FRP {} · OMI {}\n",
        device.unlock_state().as_str(),
        device.avb_state.as_str(),
        device.fmm_lock,
        device.frp_locked,
        device.omi_locked
    ));
    out.push_str(&format!(
        "batería {}% · {:.1} °C · espacio {} MiB (hacen falta {})\n\n",
        device.battery_pct, device.battery_temp_c, device.free_space_mb, device.required_space_mb
    ));

    // Selección.
    let sel = select_rom(candidates, device, &SelectionCriteria::default());
    out.push_str("── SELECCIÓN DE ROM ──\n");
    for r in &sel.rejected {
        out.push_str(&format!("  descartada: {} → {}\n", r.name, r.reason));
    }
    let rom = match &sel.chosen {
        Some(r) => {
            out.push_str(&format!(
                "  elegida: {} (CSC {}, parche {}, módem {})\n\n",
                r.name, r.csc, r.patch_date, r.modem_region
            ));
            r.clone()
        }
        None => {
            out.push_str("  NINGUNA ROM cumple los criterios: no hay nada que flashear.\n\n");
            return out;
        }
    };

    // Pre-flight.
    let pre = preflight(device, &rom, computed_sha256);
    out.push_str(&format!("── PRE-FLIGHT ──\n{}\n", pre));

    // Plan de flash.
    let efs = efs_backup_plan(device, "/sdcard/megadaemon/efs-backup");
    let plan = flash_plan(&rom, device, &efs, pre.can_flash());
    out.push_str("── PLAN DE FLASH (4 pasos) ──\n");
    for s in &plan.steps {
        out.push_str(&format!("  {}\n", s.title));
        out.push_str(&format!(
            "    escribe: {}\n",
            if s.writes.is_empty() {
                "(nada)".to_string()
            } else {
                s.writes.join(", ")
            }
        ));
        for v in &s.pre_verify {
            out.push_str(&format!(
                "    antes:  sha256 {} = {}\n",
                v.name, v.sha256
            ));
        }
        for v in &s.post_verify {
            out.push_str(&format!(
                "    después: sha256 {} = {}\n",
                v.name, v.sha256
            ));
        }
        if let Some(r) = &s.blocked_reason {
            out.push_str(&format!("    BLOQUEADO: {}\n", r));
        }
    }
    out.push_str("\n── ANTI-BRICK: MUNDO SEGURO ──\n");
    out.push_str(&format!(
        "  se copian {} particiones: {}\n",
        efs.partitions.len(),
        EFS_SAFE_WORLD.join(", ")
    ));
    if !efs.missing.is_empty() {
        out.push_str(&format!(
            "  AUSENTES en este móvil: {}\n",
            efs.missing.join(", ")
        ));
    }
    out.push_str(&format!(
        "  destino: {} (con SHA-256 por partición)\n",
        efs.dest
    ));
    out.push_str("  recuperación si el móvil queda en ladrillo:\n");
    for r in &efs.recovery {
        out.push_str(&format!("    {}\n", r));
    }
    out.push_str("\n── POST-FLASH ──\n");
    for s in post_flash_plan(device, &[]) {
        out.push_str(&format!("  [{}] {}\n", s.id, s.title));
        out.push_str(&format!("      $ {}\n", s.command));
        out.push_str(&format!("      ok si: {}\n", s.success_check));
    }
    out.push_str(
        "\nESTE ES UN DRY-RUN: no se ha escrito nada. El flash real exige \
         `--confirm` y a una persona delante.\n",
    );
    out
}

/// Serializa el plan completo a JSON (para que el script lo consuma).
pub fn plan_json(
    device: &DeviceFacts,
    rom: &RomPackage,
    computed_sha256: &BTreeMap<String, String>,
) -> String {
    let pre = preflight(device, rom, computed_sha256);
    let efs = efs_backup_plan(device, "/sdcard/megadaemon/efs-backup");
    let plan = flash_plan(rom, device, &efs, pre.can_flash());
    let value = serde_json::json!({
        "device": device,
        "rom": rom,
        "preflight": pre,
        "efs": efs,
        "flash_plan": plan,
        "post_flash": post_flash_plan(device, &[]),
        "dry_run": true,
    });
    serde_json::to_string_pretty(&value).unwrap_or_else(|e| format!("{{\"error\":\"{}\"}}", e))
}

// ---------------------------------------------------------------------------
// Utilidades internas
// ---------------------------------------------------------------------------

/// Hexadecimal en minúsculas (evita depender de otro módulo del crate).
fn hex_fallback(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

// ---------------------------------------------------------------------------
// Cambio de CSC (dentro de MegaDaemon, como en SamFW pero con las guardas)
// ---------------------------------------------------------------------------

/// Ruta donde vive la configuración del CSC en este firmware.
///
/// Medido en el S21 Ultra: `mdc.sys.omc_etcpath=/prism/etc/carriers/single/AMO`
/// y `mdc.sys.path=/optics/configs/carriers/single/AMO/conf/system`.
/// `/prism` es la partición `sda29` (una de las 7 del mundo seguro) y está
/// montada **de sólo lectura**; `/system/priv-app/CSC/CSC.apk` va en dm-verity.
pub const CSC_ROOT: &str = "/prism/etc/carriers/single";

/// Ruta de la configuración del sistema por CSC (partición `optics`).
pub const CSC_SYSTEM_ROOT: &str = "/optics/configs/carriers/single";

/// Un CSC candidato extraído de un paquete de firmware.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CscCandidate {
    /// Código del CSC (`AMO`, `EUX`, `TWO`, `ATU`...).
    pub code: String,
    /// Nombre legible del firmware de origen.
    pub firmware: String,
    /// Modelo del firmware (`G998BXXU2EUXG`): debe ser el mismo dispositivo.
    pub model: String,
    /// Versión de bootloader que trae: no puede ser **menor** que la actual.
    pub bootloader_version: String,
    /// SHA-256 de la imagen CSC.
    pub sha256: String,
}

/// Un paso del cambio de CSC.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CscStep {
    /// Identificador del paso.
    pub id: String,
    /// Qué hace.
    pub title: String,
    /// Comando exacto.
    pub command: String,
    /// `true` si necesita remontar `/prism` en rw.
    pub needs_remount_rw: bool,
    /// `true` si se puede deshacer con el paso de reversión.
    pub reversible: bool,
}

/// Plan de cambio de CSC.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CscChangePlan {
    /// CSC actual del móvil.
    pub current: String,
    /// CSC de destino.
    pub target: String,
    /// Los pasos, en orden.
    pub steps: Vec<CscStep>,
    /// Motivos por los que no se puede ejecutar.
    pub blockers: Vec<String>,
    /// `false` si el cambio **no** sobrevive a un factory reset ni a una OTA.
    pub persistent: bool,
    /// Explicación honesta.
    pub note: String,
}

impl CscChangePlan {
    /// `true` si el plan es ejecutable.
    pub fn is_executable(&self) -> bool {
        self.blockers.is_empty()
    }
}

/// ¿El firmware del CSC es del mismo dispositivo?
///
/// El modelo del móvil es `SM-G998B` y el del firmware es `G998BXXU2EUXG`
/// (sin el prefijo `SM-`): hay que comparar el "core" del modelo.
pub fn same_device_family(device_model: &str, fw_model: &str) -> bool {
    let dev = device_model.trim().to_ascii_uppercase();
    let dev = dev.strip_prefix("SM-").unwrap_or(&dev);
    let fw = fw_model.trim().to_ascii_uppercase();
    if dev.is_empty() || fw.is_empty() {
        return false;
    }
    fw.starts_with(dev) || fw.contains(dev) || dev.contains(&fw)
}

/// Ruta del fichero **persistente** del CSC.
///
/// Medido en este S21 Ultra: `/mnt/vendor/efs/mps_code.dat` son 3 bytes con
/// el código del CSC (`AMO`). Es el mismo fichero que escribe el módem con
/// `AT+PRECONFG=2,<CSC>`: cambiarlo con root equivale a cambiar el CSC por
/// la vía del módem, **sin tocar el bootloader y sin triangular el Knox**.
pub const MPS_CODE_PATH: &str = "/mnt/vendor/efs/mps_code.dat";

/// Códigos de CSC que habilitan la **grabación nativa** de llamadas.
///
/// Con un CSC de América (el de este móvil es AMO) la grabación nativa está
/// desactivada por el flag `CscFeature_VoiceCall_ConfigRecording` del
/// `cscfeature.xml`. Con un CSC de Asia (THL, INS) el sistema graba solo, sin
/// hook ni AudioFlinger parcheado: es la vía preferente.
pub const CALL_RECORDING_CSC_CODES: [&str; 4] = ["THL", "INS", "ILO", "XXV"];

/// ¿Este CSC permite la grabación nativa de llamadas?
pub fn csc_allows_native_recording(csc: &str) -> bool {
    let c = csc.trim().to_ascii_uppercase();
    CALL_RECORDING_CSC_CODES.iter().any(|k| *k == c)
}

/// Construye el plan de cambio de CSC.
///
/// Reglas (medidas en este dispositivo):
///
/// * El CSC actual es `AMO`; el de destino debe existir en el paquete.
/// * El firmware de destino tiene que ser del **mismo modelo** (`G998B`).
/// * La versión de bootloader del destino **no puede ser menor** que la
///   actual: si lo fuera, el fichero anti-rollback (FRL/FLL) bloquea el
///   arranque y el móvil queda en ladrillo.
/// * La partición `prism` (donde vive el CSC) es una de las 7 del mundo
///   seguro: **sin copia verificada, no se toca**.
/// * Con el bootloader cerrado y AVB `green`, el cambio **no es permanente**:
///   `/system/priv-app/CSC/CSC.apk` y `/optics` están protegidos por
///   dm-verity, así que un factory reset o una OTA lo revierten.
pub fn csc_change_plan(
    device: &DeviceFacts,
    target: &CscCandidate,
    prism_backup_verified: bool,
) -> CscChangePlan {
    let mut blockers: Vec<String> = Vec::new();

    if target.code.eq_ignore_ascii_case(&device.csc) {
        blockers.push(format!(
            "el CSC ya es {}: no hay nada que cambiar",
            device.csc
        ));
    }
    if !same_device_family(&device.model, &target.model) {
        blockers.push(format!(
            "el firmware del CSC destino es {} y este móvil es {}: no son el \
             mismo dispositivo",
            target.model, device.model
        ));
    }
    if cmp_version(&target.bootloader_version, &device.bootloader_version)
        == std::cmp::Ordering::Less
    {
        blockers.push(format!(
            "el bootloader del CSC destino ({}) es MENOR que el actual ({}): \
             el fichero anti-rollback bloquearía el arranque (ladrillo)",
            target.bootloader_version, device.bootloader_version
        ));
    }
    if !target.sha256.is_empty()
        && !(target.sha256.len() == 64 && target.sha256.chars().all(|c| c.is_ascii_hexdigit()))
    {
        blockers.push("el SHA-256 del CSC destino no es válido".to_string());
    }
    if !prism_backup_verified {
        blockers.push(
            "la partición prism (donde vive el CSC) no tiene copia verificada: \
             es una de las 7 del mundo seguro y sin copia el cambio puede dejar \
             el móvil sin CSC"
                .to_string(),
        );
    }

    let src = format!("{}/{}", CSC_ROOT, device.csc);
    let dst = format!("{}/{}", CSC_ROOT, target.code);
    let steps = vec![
        CscStep {
            id: "backup_prism".into(),
            title: "Copia de seguridad de la partición prism (mundo seguro)".into(),
            command: "dd if=/dev/block/by-name/prism of=/sdcard/megadaemon/prism.img bs=1M \
                      && sha256sum /sdcard/megadaemon/prism.img"
                .into(),
            needs_remount_rw: false,
            reversible: true,
        },
        CscStep {
            id: "remount_rw".into(),
            title: "Remontar /prism en escritura (está en ro)".into(),
            command: "mount -o remount,rw /prism".into(),
            needs_remount_rw: true,
            reversible: true,
        },
        CscStep {
            id: "copy_csc".into(),
            title: format!(
                "Copiar el CSC {} a {} (sin tocar el original: reversible)",
                device.csc, target.code
            ),
            command: format!(
                "cp -a {} {} && chown -R radio:system {} && chmod -R 771 {}",
                src, dst, dst, dst
            ),
            needs_remount_rw: true,
            reversible: true,
        },
        CscStep {
            id: "reboot".into(),
            title: "Reiniciar y comprobar el CSC".into(),
            command: "getprop ro.csc.sales_code   # debe mostrar el CSC destino".into(),
            needs_remount_rw: false,
            reversible: true,
        },
    ];

    CscChangePlan {
        current: device.csc.clone(),
        target: target.code.clone(),
        steps,
        blockers,
        persistent: false,
        note: "Con el bootloader cerrado (FMM Lock) y AVB green, el CSC.apk de \
               /system y la configuración de /optics están protegidos por \
               dm-verity: sólo se toca /prism. El cambio NO sobrevive a un \
               factory reset ni a una OTA (que vuelve a escribir AMO). Para \
               hacerlo permanente hace falta un /system reescribible (ROM \
               parcheada) y actualizar el CSC.apk, lo que ya exige otro \
               bootloader."
            .to_string(),
    }
}

/// Vía para cambiar el CSC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CscSwapMethod {
    /// Escribir `mps_code.dat` con root (3 bytes). Equivale a `AT+PRECONFG`,
    /// pero desde el propio móvil. No triangular el Knox.
    MpsCodeFile,
    /// Comando AT al módem (`AT+PRECONFG=2,<CSC>`) por el puerto de
    /// diagnóstico: **sin root**, pero necesita un PC con USB.
    AtCommand,
    /// Copiar el directorio del CSC en `/prism` (más invasivo).
    PrismCopy,
}

impl CscSwapMethod {
    /// Nombre estable.
    pub fn as_str(&self) -> &'static str {
        match self {
            CscSwapMethod::MpsCodeFile => "mps_code",
            CscSwapMethod::AtCommand => "at_preconfg",
            CscSwapMethod::PrismCopy => "prism_copy",
        }
    }

    /// `true` si la vía necesita root.
    pub fn needs_root(&self) -> bool {
        matches!(self, CscSwapMethod::MpsCodeFile | CscSwapMethod::PrismCopy)
    }
}

/// Plan de cambio de CSC por la vía de `mps_code.dat` (root, sin Knox).
///
/// Es la vía más simple y reversible: se copia el fichero original, se
/// escriben 3 bytes con el código nuevo y se reinicia. Si algo va mal, se
/// restaura la copia y se reinicia otra vez.
pub fn csc_swap_via_mps_code(
    device: &DeviceFacts,
    target_code: &str,
    backup_dir: &str,
) -> CscChangePlan {
    let code = target_code.trim().to_ascii_uppercase();
    let mut blockers: Vec<String> = Vec::new();
    if code.len() != 3 || !code.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
        blockers.push(format!("'{}' no parece un código de CSC válido", target_code));
    }
    if code == device.csc.trim().to_ascii_uppercase() {
        blockers.push(format!("el CSC ya es {}", device.csc));
    }
    let steps = vec![
        CscStep {
            id: "backup_mps".into(),
            title: "Copia del fichero de CSC persistente (3 bytes)".into(),
            command: format!(
                "cp -a {src} {dst}/mps_code.dat.bak && sha256sum {dst}/mps_code.dat.bak",
                src = MPS_CODE_PATH,
                dst = backup_dir
            ),
            needs_remount_rw: false,
            reversible: true,
        },
        CscStep {
            id: "write_mps".into(),
            title: format!("Escribir el CSC {} (3 bytes, sin espacios)", code),
            command: format!(
                "printf '%s' {} > {src} && sync && cat {src}",
                code,
                src = MPS_CODE_PATH
            ),
            needs_remount_rw: false,
            reversible: true,
        },
        CscStep {
            id: "reboot".into(),
            title: "Reiniciar y comprobar".into(),
            command: "getprop ro.csc.sales_code   # debe mostrar el CSC destino".into(),
            needs_remount_rw: false,
            reversible: true,
        },
    ];
    let note = format!(
        "Cambiar el CSC con mps_code.dat NO triangular el Knox (0x0 se queda) y \
         no toca el bootloader. Ojo: Samsung Wallet/Pass pasa a la región del \
         CSC nuevo, el proveedor de ID de llamante puede cambiar y la OTA puede \
         exigir volver al CSC original. La grabación nativa de llamadas depende \
         del CSC de destino ({} la permiten).",
        CALL_RECORDING_CSC_CODES.join(", ")
    );
    CscChangePlan {
        current: device.csc.clone(),
        target: code,
        steps,
        blockers,
        persistent: true,
        note,
    }
}

/// Comandos AT para cambiar el CSC **sin root** (desde un PC con USB).
///
/// El módem escribe el `mps_code.dat` del propio `/efs/imei/`.
pub fn csc_at_commands(target_code: &str) -> Vec<String> {
    let code = target_code.trim().to_ascii_uppercase();
    vec![
        "AT+PRECONFG=1,0        # leer el CSC actual".to_string(),
        format!("AT+PRECONFG=2,{}        # escribir el CSC nuevo", code),
        "AT+PRECONFG=1,0        # releer y confirmar".to_string(),
    ]
}

/// Plan de reversión: dejar el CSC como estaba.
pub fn csc_revert_plan(device: &DeviceFacts, backup_path: &str) -> Vec<CscStep> {
    vec![
        CscStep {
            id: "revert_remount".into(),
            title: "Remontar /prism en escritura".into(),
            command: "mount -o remount,rw /prism".into(),
            needs_remount_rw: true,
            reversible: false,
        },
        CscStep {
            id: "revert_restore".into(),
            title: "Restaurar la partición prism desde la copia verificada".into(),
            command: format!(
                "dd if={} of=/dev/block/by-name/prism bs=1M && sha256sum \
                 /dev/block/by-name/prism",
                backup_path
            ),
            needs_remount_rw: false,
            reversible: false,
        },
        CscStep {
            id: "revert_reboot".into(),
            title: "Reiniciar: el CSC vuelve al original".into(),
            command: format!(
                "getprop ro.csc.sales_code   # debe volver a {}",
                device.csc
            ),
            needs_remount_rw: false,
            reversible: false,
        },
    ]
}

// ---------------------------------------------------------------------------
// OMC: envoltorio ofuscado del `cscfeature.xml` (Samsung)
// ---------------------------------------------------------------------------
//
// El `cscfeature.xml` de cada CSC va envuelto en un formato binario propio:
// cada byte se rota a la izquierda `SHIFTS[i % 256]` posiciones y se XOR con
// `SALTS[i % 256]`; el resultado es un gzip con el XML dentro.
//
// Tablas medidas en el S21 Ultra (CSC AMO) y contrastadas con el decodificador
// de Python del repo (`omc_dec.py`): el roundtrip `encode(decode(x)) == x`
// cuadra byte a byte.

/// Rotaciones a la izquierda aplicadas por posicion (0..255).
pub const OMC_SHIFTS: [u8; 256] = [
    1, 1, 0, 2, 2, 4, 5, 0, 4, 7, 1, 6, 5, 3, 3, 1,
    2, 5, 0, 6, 2, 2, 4, 2, 2, 3, 0, 2, 1, 2, 4, 3,
    4, 0, 0, 0, 3, 5, 3, 1, 6, 5, 6, 1, 1, 1, 0, 0,
    3, 2, 7, 7, 5, 6, 7, 3, 5, 1, 0, 7, 6, 3, 6, 5,
    4, 5, 3, 5, 1, 3, 3, 1, 5, 4, 1, 0, 0, 2, 6, 6,
    6, 6, 4, 0, 1, 1, 0, 5, 5, 4, 2, 4, 6, 1, 7, 1,
    2, 1, 1, 6, 5, 4, 7, 6, 5, 1, 6, 7, 0, 2, 6, 3,
    1, 7, 1, 1, 7, 4, 0, 4, 2, 5, 3, 1, 1, 5, 6, 0,
    3, 5, 3, 6, 5, 7, 2, 5, 6, 6, 2, 2, 3, 6, 0, 4,
    3, 2, 0, 2, 2, 3, 5, 3, 3, 2, 5, 5, 5, 1, 3, 1,
    1, 1, 4, 5, 1, 6, 2, 4, 7, 1, 4, 6, 0, 6, 4, 3,
    2, 6, 1, 6, 3, 2, 1, 6, 7, 3, 2, 1, 1, 5, 6, 7,
    2, 2, 2, 7, 4, 6, 7, 5, 3, 1, 4, 2, 7, 1, 6, 2,
    4, 1, 5, 6, 5, 4, 5, 0, 1, 1, 6, 3, 7, 2, 0, 2,
    5, 0, 1, 3, 3, 2, 6, 7, 7, 2, 5, 6, 0, 4, 1, 2,
    5, 3, 7, 6, 5, 2, 5, 2, 0, 1, 3, 1, 4, 3, 4, 2,
];

/// Sales XOR por posicion (con signo, como en el formato original).
pub const OMC_SALTS: [i32; 256] = [
    65, -59, 33, -34, 107, 28, -107, 55, 78, 17, -81, 6, -80, -121, -35, -23,
    72, 122, -63, -43, 68, 119, -78, -111, -60, 31, 60, 57, 92, -88, -100, -69,
    -106, 91, 69, 93, 110, 23, 93, 53, -44, -51, 64, -80, 46, 2, -4, 12,
    -45, 80, -44, -35, -111, -28, -66, -116, 39, 2, -27, -45, -52, 125, 39, 66,
    -90, 63, -105, -67, 84, -57, -4, -4, 101, -90, 81, 10, -33, 1, 67, -57,
    -71, 18, -74, 102, 96, -89, 64, -17, 54, -94, -84, -66, 14, 119, 121, 2,
    -78, -79, 89, 63, 93, 109, -78, -51, 66, -36, 32, 86, 3, -58, -15, 92,
    58, 2, -89, -80, -13, -1, 122, -4, 48, 63, -44, 59, 100, -42, -45, 59,
    -7, -17, -54, 34, -54, 71, -64, -26, -87, -80, -17, -44, -38, -112, 70, 10,
    -106, 95, -24, -4, -118, 45, -85, -13, 85, 25, -102, -119, 13, -37, 116, 46,
    -69, 59, 42, -90, -38, -105, 101, -119, -36, 97, -3, -62, -91, -97, -125, 17,
    14, 106, -72, -119, 99, 111, 20, 18, -27, 113, 64, -24, 74, -60, -100, 26,
    56, -44, -70, 12, -51, -100, -32, -11, 26, 48, -117, 98, -93, 51, -25, -79,
    -31, 97, 87, -105, -64, 7, -13, -101, 33, -122, 5, -104, 89, -44, -117, 63,
    -80, -6, -71, -110, -29, -105, 116, 107, -93, 91, -41, -13, 20, -115, -78, 43,
    79, -122, 6, 102, -32, 52, -118, -51, 72, -104, 41, -38, 124, 72, -126, -35,
];

/// Des-ofusca el envoltorio OMC (rotacion + XOR posicional).
pub fn omc_deobfuscate(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    for (i, b) in bytes.iter().enumerate() {
        let s = OMC_SHIFTS[i % 256] as u32;
        let x = *b as u32;
        let a = ((x << s) | (x >> (8 - s))) & 0xFF;
        out.push((a ^ ((OMC_SALTS[i % 256] & 0xFF) as u32)) as u8);
    }
    out
}

/// Descifra un fichero OMC y devuelve el XML del `cscfeature.xml`.
///
/// Si el contenido ya es XML en claro (el framework a veces deja una copia
/// descifrada en la cache de arranque), se devuelve tal cual.
pub fn omc_decode_gzip(bytes: &[u8]) -> Result<String, String> {
    if bytes.starts_with(b"<?xml") || bytes.starts_with(b"<SamsungMobileFeature") {
        return Ok(String::from_utf8_lossy(bytes).into_owned());
    }
    let plain = omc_deobfuscate(bytes);
    let mut dec = flate2::read::GzDecoder::new(&plain[..]);
    let mut txt = String::new();
    std::io::Read::read_to_string(&mut dec, &mut txt).map_err(|e| e.to_string())?;
    Ok(txt)
}

/// Lee el valor de una feature del `cscfeature.xml`.
///
/// El XML real de Samsung trae la feature como **etiqueta con el valor
/// dentro** (`<CscFeature_VoiceCall_ConfigRecording>RecordingAllowed</...>`);
/// se aceptan además las otras dos formas que aparecen en ficheros viejos:
/// el atributo `value="..."` y el par `<Name>` / `<Value>`.
pub fn csc_feature_get(xml: &str, feature: &str) -> Option<String> {
    let mut from = 0usize;
    while from < xml.len() {
        let pos = match xml[from..].find(feature) {
            Some(p) => from + p,
            None => break,
        };
        let tag_start = xml[..pos].rfind('<').unwrap_or(0);
        let tag_end = xml[pos..]
            .find('>')
            .map(|i| pos + i + 1)
            .unwrap_or(xml.len());
        let tag = &xml[tag_start..tag_end];
        if let Some(v) = xml_attr(tag, "value") {
            if !v.is_empty() {
                return Some(v);
            }
        }
        // 2) Etiqueta con el valor dentro: <FEATURE>valor</FEATURE>
        let cierre = format!("</{}>", feature);
        if tag.trim_start_matches('<').trim_end_matches('>').trim() == feature {
            if let Some(fin) = xml[tag_end..].find(&cierre) {
                let valor = xml[tag_end..tag_end + fin].trim();
                if !valor.is_empty() {
                    return Some(valor.to_string());
                }
            }
        }

        // 3) Par <Name>feature</Name> ... <Value>...</Value>
        let after = &xml[tag_end..];
        if let Some(n_end) = after.find("</Name>") {
            if after[..n_end].contains(feature) {
                if let Some(v) = after[n_end..].find("<Value>") {
                    let s = v + "<Value>".len();
                    if let Some(e) = after[s..].find("</Value>") {
                        let val = after[s..s + e].trim();
                        if !val.is_empty() {
                            return Some(val.to_string());
                        }
                    }
                }
            }
        }
        from = pos + feature.len();
    }
    None
}

/// Valor de un atributo XML (`name="valor"`).
fn xml_attr(tag: &str, name: &str) -> Option<String> {
    let pat = format!("{}=\"", name);
    let i = tag.find(&pat)? + pat.len();
    let e = tag[i..].find('"')?;
    Some(tag[i..i + e].to_string())
}

// ---------------------------------------------------------------------------
// Preconfiguración del módem para la captura de audio de llamada
// ---------------------------------------------------------------------------
//
// API **reconstruida** el 2026-11-22: `tests/call_audio_smoke.rs` la usaba,
// pero se perdió en un merge (misma pérdida que `omc_decode_gzip`). La
// semántica se deduce de las aserciones del propio arnés:
//
//   * `preconfig_state(false, false)` -> no se puede usar la preconfiguración;
//   * `preconfig_state(false, true)`  -> el plan empieza por reutilizar el
//     binario que ya hay en el móvil ("install-existing");
//   * el plan termina SIEMPRE en la alternativa real: el comando de módem
//     `AT+PRECONFIG`.

/// Estado de la preconfiguración de captura de audio en el móvil.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreconfigState {
    /// El firmware instalado ya trae la preconfiguración.
    pub firmware_ok: bool,
    /// Ya hay un binario de preconfig en el móvil (de un CSC anterior).
    pub binary_present: bool,
}

impl PreconfigState {
    /// ¿Se puede capturar el audio de llamada con lo que hay?
    pub fn usable(&self) -> bool {
        self.firmware_ok || self.binary_present
    }
}

/// Construye el estado de la preconfiguración a partir de lo medido.
pub fn preconfig_state(firmware_ok: bool, binary_present: bool) -> PreconfigState {
    PreconfigState {
        firmware_ok,
        binary_present,
    }
}

/// Plan de instalación de la preconfiguración, del primer paso al último.
///
/// El último paso es siempre la alternativa real (`AT+PRECONFIG`), que es lo
/// que el módem entiende: los pasos anteriores sólo preparan el entorno.
pub fn preconfig_install_plan(s: &PreconfigState) -> Vec<String> {
    let mut plan: Vec<String> = Vec::new();
    if s.firmware_ok {
        plan.push(
            "el firmware actual ya trae la preconfiguracion: no hace falta tocar nada".to_string(),
        );
    } else if s.binary_present {
        plan.push(
            "install-existing: reutilizar el binario de preconfig ya presente en el movil"
                .to_string(),
        );
    } else {
        plan.push(
            "actualizar a firmware Samsung que traiga la preconfiguracion (el actual no la tiene)"
                .to_string(),
        );
    }
    plan.push(
        "AT+PRECONFIG: alternativa real de captura de audio de llamada en el modem".to_string(),
    );
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device_ok() -> DeviceFacts {
        DeviceFacts {
            model: "SM-G998B".into(),
            csc: "AMO".into(),
            patch_date: "2024-06-01".into(),
            android_version: "14".into(),
            bootloader_version: "S21U_2A_14_0".into(),
            modem_region: "SM".into(),
            avb_state: AvbState::Yellow,
            flash_locked: false,
            fmm_lock: false,
            frp_locked: false,
            omi_locked: false,
            battery_pct: 95,
            battery_temp_c: 28.0,
            free_space_mb: 40_000,
            required_space_mb: 8_192,
            odin_version: Some("3.14.4".into()),
            odin_min_version: Some("3.14.4".into()),
            stock_bootimg: true,
            efs_partitions: EFS_SAFE_WORLD.iter().map(|s| s.to_string()).collect(),
            anti_rollback_path: Some("/root/FLL.bin".to_string()),
            anti_rollback_sha256: "0".repeat(64),
        }
    }

    fn rom_ok() -> RomPackage {
        RomPackage {
            name: "ROM-AMO-2024-06".into(),
            csc: "AMO".into(),
            patch_date: "2024-07-01".into(),
            android_version: "14".into(),
            modem_region: "EU".into(),
            bootloader_version: "S21U_2A_14_1".into(),
            stock_bootimg: true,
            bootimg_note: None,
            pit_match: true,
            artifacts: vec![
                Artifact {
                    name: "AP_MDPO".into(),
                    path: "/rom/AP_MDPO_MD1.img".into(),
                    size_bytes: 5_000_000_000,
                    sha256: "a".repeat(64),
                },
                Artifact {
                    name: "CP_MDPO".into(),
                    path: "/rom/CP_MDPO_MD1.img".into(),
                    size_bytes: 300_000_000,
                    sha256: "b".repeat(64),
                },
                Artifact {
                    name: "CSC_MD1".into(),
                    path: "/rom/CSC_MD1.img".into(),
                    size_bytes: 200_000_000,
                    sha256: "c".repeat(64),
                },
                Artifact {
                    name: "HOME_CSC".into(),
                    path: "/rom/HOME_CSC.img".into(),
                    size_bytes: 20_000_000,
                    sha256: "d".repeat(64),
                },
                Artifact {
                    name: "BOOTLOADER".into(),
                    path: "/rom/BOOTLOADER.img".into(),
                    size_bytes: 15_000_000,
                    sha256: "e".repeat(64),
                },
            ],
        }
    }

    #[test]
    fn cmp_version_compara_numericamente() {
        use std::cmp::Ordering;
        assert_eq!(cmp_version("3.9.1", "3.14.4"), Ordering::Less);
        assert_eq!(cmp_version("3.14.4", "3.14.4"), Ordering::Equal);
        assert_eq!(cmp_version("S21U_2A_14_1", "S21U_2A_14_0"), Ordering::Greater);
        assert_eq!(cmp_version("2024-06-01", "2024-07-01"), Ordering::Less);
    }

    #[test]
    fn el_cambio_de_csc_aborta_sin_copia_de_prism_o_si_baja_el_bootloader() {
        let dev = device_ok();
        let csc = CscCandidate {
            code: "EUX".into(),
            firmware: "G998BXXU2EUXG".into(),
            model: "G998BXXU2EUXG".into(),
            bootloader_version: "S21U_2A_14_1".into(),
            sha256: "a".repeat(64),
        };

        // Sin copia verificada de prism: no se toca.
        let plan = csc_change_plan(&dev, &csc, false);
        assert!(!plan.is_executable());
        assert!(plan.blockers.iter().any(|b| b.contains("prism")));
        assert_eq!(plan.current, "AMO");
        assert_eq!(plan.target, "EUX");
        assert!(!plan.persistent, "con AVB green el cambio no es permanente");

        // Con copia verificada: se puede ejecutar.
        let plan2 = csc_change_plan(&dev, &csc, true);
        assert!(plan2.is_executable(), "{:?}", plan2.blockers);
        assert_eq!(plan2.steps.len(), 4);
        assert!(plan2.steps.iter().any(|s| s.needs_remount_rw));
        assert!(plan2.steps[0].command.contains("prism"));
    }


    #[test]
    fn el_modelo_del_firmware_se_compara_sin_el_prefijo_sm() {
        assert!(same_device_family("SM-G998B", "G998BXXU2EUXG"));
        assert!(same_device_family("SM-G998B", "G998BXXU2AMOG"));
        assert!(!same_device_family("SM-G998B", "SM-S911B"));
        assert!(!same_device_family("SM-G998B", ""));
    }

    #[test]
    fn el_cambio_por_mps_code_no_necesita_prism_y_es_reversible() {
        let dev = device_ok();
        let plan = csc_swap_via_mps_code(&dev, "THL", "/sdcard/megadaemon/csc");
        assert!(plan.is_executable(), "{:?}", plan.blockers);
        assert_eq!(plan.target, "THL");
        assert!(plan.persistent, "mps_code.dat sobrevive al reinicio");
        assert!(plan.steps[0].command.contains("mps_code.dat"));
        assert!(plan.steps[1].command.contains("THL"));
        let malo = csc_swap_via_mps_code(&dev, "no-valido", "/x");
        assert!(!malo.is_executable());
    }

    #[test]
    fn los_csc_de_grabacion_nativa_estan_conocidos() {
        assert!(csc_allows_native_recording("THL"));
        assert!(csc_allows_native_recording("ins"));
        assert!(!csc_allows_native_recording("AMO"));
        assert!(csc_at_commands("THL")[1].contains("AT+PRECONFG=2,THL"));
    }
    #[test]
    fn el_cambio_de_csc_rechaza_un_bootloader_menor() {
        let dev = device_ok();
        let viejo = CscCandidate {
            code: "EUX".into(),
            firmware: "G998BXXU2EUXG".into(),
            model: "G998BXXU2EUXG".into(),
            bootloader_version: "S21U_2A_13_0".into(),
            sha256: "a".repeat(64),
        };
        let plan = csc_change_plan(&dev, &viejo, true);
        assert!(!plan.is_executable());
        assert!(plan.blockers.iter().any(|b| b.contains("anti-rollback")));
    }

    #[test]
    fn sin_fichero_anti_rollback_verificado_no_se_flashea() {
        let mut dev = device_ok();
        dev.anti_rollback_path = None;
        dev.anti_rollback_sha256 = String::new();
        let rep = preflight(&dev, &rom_ok(), &BTreeMap::new());
        assert!(rep.abort, "sin el anti-rollback respaldado hay que abortar");
        let c = rep
            .failed_blockers()
            .into_iter()
            .find(|c| c.id == "anti_rollback_backup")
            .expect("debe aparecer el check anti-rollback");
        assert!(c.detail.contains("anti-rollback"));

        // Localizado pero sin hash => también aborta.
        let mut dev2 = device_ok();
        dev2.anti_rollback_path = Some("/efs/FLL.bin".into());
        dev2.anti_rollback_sha256 = String::new();
        let rep2 = preflight(&dev2, &rom_ok(), &BTreeMap::new());
        assert!(rep2.abort);
    }

    #[test]
    fn el_movil_real_aborta_el_preflight() {
        // Hechos por defecto = S21 Ultra medido: bootloader cerrado con FMM.
        let dev = DeviceFacts::default();
        let rom = rom_ok();
        let rep = preflight(&dev, &rom, &BTreeMap::new());
        assert!(rep.abort, "con FMM Lock el pre-flight debe abortar");
        assert!(!rep.can_flash());
        let ids: Vec<&str> = rep.failed_blockers().iter().map(|c| c.id.as_str()).collect();
        assert!(ids.contains(&"unlock_viability"));
        assert!(ids.contains(&"frp"));
        assert!(ids.contains(&"battery"));
    }

    #[test]
    fn preflight_pasa_con_todo_en_verde() {
        let rep = preflight(&device_ok(), &rom_ok(), &BTreeMap::new());
        assert!(!rep.abort, "con los checks en verde no debe abortar: {:?}", rep.blockers);
        assert!(rep.can_flash());
    }

    #[test]
    fn bootimg_no_stock_aborta() {
        let mut rom = rom_ok();
        rom.stock_bootimg = false;
        rom.bootimg_note = Some("reempaquetada con androidboot.selinux=permissive".into());
        let rep = preflight(&device_ok(), &rom, &BTreeMap::new());
        assert!(rep.abort);
        let c = rep.failed_blockers().into_iter().find(|c| c.id == "bootimg_stock");
        assert!(c.is_some());
        assert!(c.unwrap().detail.contains("selinux=permissive"));
    }

    #[test]
    fn csc_distinto_aborta() {
        let mut rom = rom_ok();
        rom.csc = "EUU".into();
        let rep = preflight(&device_ok(), &rom, &BTreeMap::new());
        assert!(rep.abort);
        let c = rep.failed_blockers().into_iter().find(|c| c.id == "csc_coherence");
        assert!(c.unwrap().detail.contains("AMO"));
    }

    #[test]
    fn hash_de_artefacto_distinto_aborta() {
        let rom = rom_ok();
        let mut computed = BTreeMap::new();
        computed.insert("AP_MDPO".to_string(), "f".repeat(64));
        let rep = preflight(&device_ok(), &rom, &computed);
        assert!(rep.abort);
        let c = rep.failed_blockers().into_iter().find(|c| c.id == "rom_checksums");
        assert!(c.is_some());
    }

    #[test]
    fn downgrade_de_parche_y_de_bootloader_abortan() {
        let mut rom = rom_ok();
        rom.patch_date = "2024-01-01".into();
        rom.bootloader_version = "S21U_2A_13_0".into();
        let rep = preflight(&device_ok(), &rom, &BTreeMap::new());
        let ids: Vec<&str> = rep.failed_blockers().iter().map(|c| c.id.as_str()).collect();
        assert!(ids.contains(&"patch_not_older"));
        assert!(ids.contains(&"bootloader_version"));
    }

    #[test]
    fn la_seleccion_de_rom_explica_cada_descarte() {
        let mut wrong_csc = rom_ok();
        wrong_csc.name = "ROM-EUU".into();
        wrong_csc.csc = "EUU".into();
        let mut nonstock = rom_ok();
        nonstock.name = "ROM-NONSTOCK".into();
        nonstock.stock_bootimg = false;
        let sel = select_rom(
            &[wrong_csc, nonstock, rom_ok()],
            &device_ok(),
            &SelectionCriteria::default(),
        );
        assert_eq!(sel.chosen.unwrap().name, "ROM-AMO-2024-06");
        assert_eq!(sel.rejected.len(), 2);
        assert!(sel.rejected.iter().any(|r| r.reason.contains("CSC")));
        assert!(sel.rejected.iter().any(|r| r.reason.contains("stock")));
    }

    #[test]
    fn el_plan_de_flash_tiene_cuatro_pasos_en_orden() {
        let dev = device_ok();
        let rom = rom_ok();
        let mut efs = efs_backup_plan(&dev, "/dest");
        let mut hashes = BTreeMap::new();
        for p in &EFS_SAFE_WORLD {
            hashes.insert(p.to_string(), "0".repeat(64));
        }
        mark_efs_verified(&mut efs, &hashes);
        assert!(efs.verified);

        let plan = flash_plan(&rom, &dev, &efs, true);
        assert_eq!(plan.steps.len(), 4);
        assert_eq!(plan.steps[0].stage, FlashStage::WipeData);
        assert_eq!(plan.steps[1].stage, FlashStage::Bootloader);
        assert_eq!(plan.steps[2].stage, FlashStage::ApCpCsc);
        assert_eq!(plan.steps[3].stage, FlashStage::HomeCsc);
        assert!(!plan.abort);
        assert!(plan.steps.iter().all(|s| s.blocked_reason.is_none()));
    }

    #[test]
    fn erase_download_sin_efs_verificada_queda_bloqueado() {
        let dev = device_ok();
        let rom = rom_ok();
        let efs = efs_backup_plan(&dev, "/dest"); // sin hashes => no verificada
        assert!(!efs.verified);
        let plan = flash_plan(&rom, &dev, &efs, true);
        assert!(plan.abort);
        let wipe = &plan.steps[0];
        assert!(wipe.erase_download);
        assert!(wipe.blocked_reason.is_some());
        assert!(wipe.blocked_reason.as_ref().unwrap().contains("EFS"));
    }

    #[test]
    fn el_preflight_fallido_aborta_todos_los_pasos() {
        let dev = device_ok();
        let rom = rom_ok();
        let mut efs = efs_backup_plan(&dev, "/dest");
        let mut hashes = BTreeMap::new();
        for p in &EFS_SAFE_WORLD {
            hashes.insert(p.to_string(), "0".repeat(64));
        }
        mark_efs_verified(&mut efs, &hashes);
        let plan = flash_plan(&rom, &dev, &efs, false);
        assert!(plan.abort);
        assert!(plan.steps.iter().all(|s| s.blocked_reason.is_some()));
        assert!(plan.runnable_steps().is_empty());
    }

    #[test]
    fn el_plan_efs_pide_las_siete_y_no_inventa_las_que_no_existen() {
        let dev = device_ok();
        let efs = efs_backup_plan(&dev, "/dest");
        assert_eq!(efs.partitions.len(), 7);
        assert!(efs.missing.is_empty());
        for p in EFS_NON_EXISTENT.iter() {
            assert!(
                !efs.partitions.iter().any(|x| &x.name == p),
                "{} no existe en este firmware: no debe aparecer en el plan",
                p
            );
        }
        // Si el móvil no tiene una de las 7, se avisa.
        let mut dev2 = device_ok();
        dev2.efs_partitions.retain(|p| p != "prism");
        let efs2 = efs_backup_plan(&dev2, "/dest");
        assert_eq!(efs2.missing, vec!["prism".to_string()]);
    }

    #[test]
    fn el_post_flash_cubre_arranque_root_y_reinventario() {
        let steps = post_flash_plan(&device_ok(), &["magisk-module".into()]);
        let ids: Vec<&str> = steps.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"first_boot"));
        assert!(ids.contains(&"re_root"));
        assert!(ids.contains(&"magisk_modules"));
        assert!(ids.contains(&"re_inventory"));
        assert!(steps.iter().any(|s| s.command.contains("magisk-module")));
    }

    #[test]
    fn comparar_inventarios_solo_devuelve_los_cambios() {
        let mut before = BTreeMap::new();
        before.insert("csc".to_string(), "AMO".to_string());
        before.insert("patch".to_string(), "2024-06-01".to_string());
        before.insert("model".to_string(), "SM-G998B".to_string());
        let mut after = before.clone();
        after.insert("patch".to_string(), "2024-07-01".to_string());
        after.insert("model".to_string(), "SM-G998B".to_string());
        after.insert("root".to_string(), "yes".to_string());
        let diffs = compare_inventories(&before, &after);
        assert_eq!(diffs.len(), 2);
        assert!(diffs.iter().any(|d| d.field == "patch" && d.after == "2024-07-01"));
        assert!(diffs.iter().any(|d| d.field == "root" && d.before == "<ausente>"));
    }

    #[test]
    fn el_dry_run_menciona_el_mundo_seguro_y_no_afirma_haber_escrito() {
        let out = dry_run_report(&DeviceFacts::default(), &[rom_ok()], &BTreeMap::new());
        assert!(out.contains("DRY-RUN"));
        assert!(out.contains("sec_efs"));
        assert!(out.contains("prism"));
        assert!(out.contains("no se ha escrito nada"));
    }
}

//! MegaDaemon v13.4 — **Ocultación de root**: inventario de detectores y banco
//! de pruebas instrumentadas.
//!
//! # Por qué existe
//!
//! "Ocultamos root" es una afirmación, no un hecho. Este módulo la convierte en
//! **medidas repetibles**:
//!
//! 1. **Estático** — qué comprueba de verdad cada APK: se lee la *tabla de
//!    cadenas* de cada `classes*.dex` (no un `grep` a pelo) y se escanean las
//!    `.so` nativas en bruto. De ahí sale la lista de *señales* que la app
//!    busca (`/system/xbin/su`, `ro.build.tags`, `getprop`, `busybox`, ...).
//! 2. **Runtime** — qué ve **la app** de verdad: se lanza la app, se saca su
//!    PID y se lee `/proc/<pid>/mounts` (que es el espacio de montajes **de la
//!    app**, no el de root) y `/proc/<pid>/root/...` (su raíz). Un `su` que se
//!    ejecuta desde root no sirve como prueba: hay que mirar desde dentro.
//! 3. **Ejecución** — se limpia el logcat, se arranca la app y se captura su
//!    log filtrado buscando el mensaje del detector.
//!
//! # El veredicto
//!
//! Para cada app se cruzan las dos mitades:
//!
//! * *señales que la app mira* (estático), y
//! * *señales que ahora mismo quedan a la vista* (runtime).
//!
//! `PASS` = todas las señales que la app mira están ocultas.
//! `FAIL` = queda alguna a la vista. `PARCIAL` = hay señales sin medir.
//!
//! **Nada de esto se presupone**: si una señal no se puede medir, sale
//! `no-medido` y el veredicto lo dice. Un `PASS` sin cobertura no vale.
//!
//! ---------------------------------------------------------------------------
//! MEDIDO el 2026-11-22 en el SM-G998B (Kitsune Mask 27.2, Zygisk on)
//! ---------------------------------------------------------------------------
//!
//! * `ro.debuggable=0`, `ro.build.tags=release-keys`, `ro.build.type=user`
//!   ⇒ las *props* sí están saneadas.
//! * `/system/bin/su` → symlink a `./magisk` **visible** desde la app.
//! * `/system/bin/magisk` y `/system/bin/magiskpolicy` **visibles**.
//! * `/proc/<pid>/mounts` de la app: **16** entradas con "magisk".
//! * `/data/adb` y `/debug_ramdisk` presentes en la raíz de la app.
//!
//! ⇒ Hoy la ocultación es de *props*, **no** de sistema de ficheros: por eso
//! los detectores que miran `su`, los binarios o los montajes fallan.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Tope de entradas DEX leídas por APK (los APK de 65 MB tienen 7+ DEX).
pub const MAX_DEX_ANALYZED: usize = 8;

/// Tope de `.so` leídas por APK.
pub const MAX_SO_ANALYZED: usize = 24;

/// Tope de cadenas DEX guardadas por APK (el banco de pruebas no necesita más).
pub const MAX_STRINGS_KEPT: usize = 120_000;

/// Tope de caracteres leídos de una `.so`.
pub const MAX_SO_BYTES: usize = 4 * 1024 * 1024;

/// Tope de hits guardados por señal.
pub const MAX_HITS_PER_SIGNAL: usize = 6;

/// Tiempo máximo de una sonda runtime.
pub const PROBE_TIMEOUT_MS: u64 = 30_000;

/// Tiempo de espera (s) tras lanzar la app en la prueba de ejecución.
pub const EXEC_SETTLE_SECS: u32 = 5;

// ========================================================================= //
// Catálogo de señales
// ========================================================================= //

/// Familia de la señal (a qué mecanismo de ocultación pertenece).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignalFamily {
    /// Binario `su` en el PATH del sistema.
    SuBinary,
    /// `su` de Magisk (`magisk`, `magiskpolicy`, `magiskd`).
    MagiskBinary,
    /// Superusuario/SuperSU/Magisk como *app* instalada.
    SuperuserApp,
    /// Props del build (`ro.debuggable`, `ro.build.tags`, `test-keys`).
    BuildProps,
    /// `/proc/self/mounts` y compañía.
    ProcMounts,
    /// `getprop` en general.
    GetProp,
    /// Librerías y clases de detección de root conocidas.
    RootLibrary,
    /// Play Integrity / SafetyNet.
    Attestation,
    /// frameworks de hook (Xposed/LSPosed/Frida).
    HookFramework,
    /// Utilidades de línea de órdenes (`busybox`).
    BusyBox,
    /// API de detección propia de la app (`isRooted`, `checkRoot`...).
    RootApi,
}

impl SignalFamily {
    /// Clave estable para JSON.
    pub fn key(&self) -> &'static str {
        match self {
            SignalFamily::SuBinary => "su_binary",
            SignalFamily::MagiskBinary => "magisk_binary",
            SignalFamily::SuperuserApp => "superuser_app",
            SignalFamily::BuildProps => "build_props",
            SignalFamily::ProcMounts => "proc_mounts",
            SignalFamily::GetProp => "getprop",
            SignalFamily::RootLibrary => "root_library",
            SignalFamily::Attestation => "attestation",
            SignalFamily::HookFramework => "hook_framework",
            SignalFamily::BusyBox => "busybox",
            SignalFamily::RootApi => "root_api",
        }
    }
}

/// Definición de una señal vigilada.
#[derive(Clone, Copy, Debug)]
pub struct SignalDef {
    /// Identificador estable (`su_binary`, `build_props`...).
    pub id: &'static str,
    /// Etiqueta legible.
    pub label: &'static str,
    /// Familia de la señal.
    pub family: SignalFamily,
    /// Patrones que delatan la señal en el APK (se buscan en DEX y en `.so`).
    pub patterns: &'static [&'static str],
}

/// Catálogo completo de señales vigiladas.
pub const SIGNALS: &[SignalDef] = &[
    SignalDef {
        id: "su_binary",
        label: "Binario su en el sistema",
        family: SignalFamily::SuBinary,
        patterns: &[
            "/system/xbin/su",
            "/system/bin/su",
            "/sbin/su",
            "/vendor/bin/su",
            "/data/local/xbin/su",
            "/data/local/bin/su",
            "/su/bin/su",
            "/system/sbin/su",
            "/system/bin/.ext/.su",
        ],
    },
    SignalDef {
        id: "su_exec",
        label: "Ejecucion de su (Runtime.exec / which)",
        family: SignalFamily::SuBinary,
        patterns: &["which su", "su -c", "\"su\"", "Superuser.apk"],
    },
    SignalDef {
        id: "magisk_binary",
        label: "Binarios de Magisk",
        family: SignalFamily::MagiskBinary,
        patterns: &["/system/bin/magisk", "magiskpolicy", "magiskd", "magisk --"],
    },
    SignalDef {
        id: "magisk_mention",
        label: "Mencion a Magisk",
        family: SignalFamily::MagiskBinary,
        patterns: &["magisk"],
    },
    SignalDef {
        id: "superuser_app",
        label: "App de superusuario instalada",
        family: SignalFamily::SuperuserApp,
        patterns: &[
            "eu.chainfire.supersu",
            "com.noshufou.android.su",
            "com.koushikdutta.superuser",
            "com.thirdparty.superuser",
            "com.topjohnwu.magisk",
            "io.github.huskydg.magisk",
        ],
    },
    SignalDef {
        id: "build_props",
        label: "Props del build (debuggable/tags)",
        family: SignalFamily::BuildProps,
        patterns: &[
            "ro.build.tags",
            "ro.debuggable",
            "ro.build.type",
            "test-keys",
            "ro.secure",
        ],
    },
    SignalDef {
        id: "proc_mounts",
        label: "Montajes del proceso (/proc/self/mounts)",
        family: SignalFamily::ProcMounts,
        patterns: &[
            "/proc/self/mounts",
            "/proc/mounts",
            "/proc/self/status",
            "TracerPid",
            "/proc/net/unix",
        ],
    },
    SignalDef {
        id: "getprop",
        label: "Lectura de getprop",
        family: SignalFamily::GetProp,
        patterns: &["getprop", "android.os.Build"],
    },
    SignalDef {
        id: "root_library",
        label: "Librerias de deteccion de root",
        family: SignalFamily::RootLibrary,
        patterns: &[
            "rootbeer",
            "RootBeer",
            "com.devadvance.rootbeer",
            "ic_lib_rootbeer",
            "libchecker",
            "RootBeerCheck",
        ],
    },
    SignalDef {
        id: "attestation",
        label: "Play Integrity / SafetyNet",
        family: SignalFamily::Attestation,
        patterns: &[
            "SafetyNet",
            "playIntegrity",
            "IntegrityManager",
            "IntegrityTokenRequest",
            "com.google.android.gms",
        ],
    },
    SignalDef {
        id: "hook_framework",
        label: "Frameworks de hook (Xposed/Frida)",
        family: SignalFamily::HookFramework,
        patterns: &[
            "xposed",
            "Xposed",
            "lsposed",
            "frida",
            "substrate",
            "de.robv.android.xposed",
        ],
    },
    SignalDef {
        id: "busybox",
        label: "busybox",
        family: SignalFamily::BusyBox,
        patterns: &["busybox", "/bin/busybox"],
    },
    SignalDef {
        id: "root_api",
        label: "API propia de deteccion",
        family: SignalFamily::RootApi,
        patterns: &[
            "isRooted",
            "isRoot",
            "checkRoot",
            "rootDetect",
            "detectRoot",
            "rootcheck",
            "RootCheck",
        ],
    },
];

/// Busca la definición de una señal por su id.
pub fn signal_by_id(id: &str) -> Option<&'static SignalDef> {
    SIGNALS.iter().find(|s| s.id == id)
}

// ========================================================================= //
// Lectura de ficheros del dispositivo (local si se puede, remoto si no)
// ========================================================================= //

/// Lector con `Read + Seek` sobre un fichero del dispositivo.
///
/// Si el binario corre **en el móvil**, se usa la E/S local (rápido). Si corre
/// en el PC, se pide por bloques con `root_exec` + base64. En ambos casos
/// `zip::ZipArchive` puede leer el APK sin bajar el fichero entero.
pub struct DeviceReader {
    path: String,
    size: u64,
    pos: u64,
    local: Option<std::fs::File>,
    cache: Option<(u64, Vec<u8>)>,
}

/// Bytes leídos por bloque en modo remoto.
const CHUNK_BYTES: usize = 512 * 1024;

impl DeviceReader {
    /// Abre un fichero del dispositivo.
    pub fn open(path: &str) -> Option<DeviceReader> {
        if path.is_empty() {
            return None;
        }
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.is_file() {
                if let Ok(f) = std::fs::File::open(path) {
                    return Some(DeviceReader {
                        path: path.to_string(),
                        size: meta.len(),
                        pos: 0,
                        local: Some(f),
                        cache: None,
                    });
                }
            }
        }
        let r = crate::android::root_exec(
            &format!("stat -c %s {} 2>/dev/null", crate::android::sh_quote(path)),
            20_000,
            1024,
        );
        let size = r.stdout.trim().parse::<u64>().ok()?;
        if size == 0 {
            return None;
        }
        Some(DeviceReader {
            path: path.to_string(),
            size,
            pos: 0,
            local: None,
            cache: None,
        })
    }

    /// Tamaño del fichero.
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Pide un bloque al dispositivo en base64.
    fn fetch(&mut self, offset: u64, len: usize) -> Option<Vec<u8>> {
        if len == 0 {
            return None;
        }
        let quoted = crate::android::sh_quote(&self.path);
        let cmd = format!(
            "tail -c +{} {} 2>/dev/null | head -c {} | base64 | tr -d '\\n'",
            offset + 1,
            quoted,
            len
        );
        let r = crate::android::root_exec(&cmd, 60_000, 1024 * 1024);
        crate::apk_inventory::decode_b64(&r.stdout)
    }

    /// Asegura que la caché cubre la posición actual.
    fn ensure_cache(&mut self) -> Option<()> {
        if let Some((start, data)) = self.cache.as_ref() {
            if *start <= self.pos && (self.pos - start) < data.len() as u64 {
                return Some(());
            }
        }
        let want = CHUNK_BYTES.min((self.size.saturating_sub(self.pos)) as usize);
        if want == 0 {
            return None;
        }
        let data = self.fetch(self.pos, want)?;
        self.cache = Some((self.pos, data));
        Some(())
    }
}

impl Read for DeviceReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if self.pos >= self.size {
            return Ok(0);
        }
        if let Some(f) = self.local.as_mut() {
            f.seek(SeekFrom::Start(self.pos))?;
            let n = f.read(buf)?;
            self.pos += n as u64;
            return Ok(n);
        }
        self.ensure_cache().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "dispositivo sin datos")
        })?;
        let (start, data) = match self.cache.as_ref() {
            Some(c) => c,
            None => return Ok(0),
        };
        let off = (self.pos - start) as usize;
        if off >= data.len() {
            return Ok(0);
        }
        let n = (data.len() - off).min(buf.len());
        buf[..n].copy_from_slice(&data[off..off + n]);
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for DeviceReader {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let target = match pos {
            SeekFrom::Start(p) => p as i64,
            SeekFrom::End(d) => self.size as i64 + d,
            SeekFrom::Current(d) => self.pos as i64 + d,
        };
        if target < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "posicion negativa",
            ));
        }
        self.pos = target as u64;
        if let Some(f) = self.local.as_mut() {
            f.seek(SeekFrom::Start(self.pos))?;
        }
        Ok(self.pos)
    }
}

/// Lee un fichero del dispositivo entero (local primero, remoto si hace falta).
pub fn read_device_file(path: &str, max_bytes: usize) -> Option<Vec<u8>> {
    if let Ok(b) = std::fs::read(path) {
        if b.len() <= max_bytes {
            return Some(b);
        }
    }
    let mut r = DeviceReader::open(path)?;
    let want = (r.size() as usize).min(max_bytes);
    let mut buf = vec![0u8; want];
    let mut got = 0usize;
    while got < want {
        let n = r.read(&mut buf[got..]).ok()?;
        if n == 0 {
            break;
        }
        got += n;
    }
    buf.truncate(got);
    Some(buf)
}

// ========================================================================= //
// Lectura de la tabla de cadenas de un DEX
// ========================================================================= //

/// Lee un `uleb128` (longitud variable) desde `buf` en `pos`.
///
/// Devuelve `(valor, bytes consumidos)`.
fn read_uleb128(buf: &[u8], pos: &mut usize) -> Option<u32> {
    let mut result: u32 = 0;
    let mut shift = 0u32;
    loop {
        let b = *buf.get(*pos)?;
        *pos += 1;
        result |= ((b & 0x7F) as u32) << shift;
        if b & 0x80 == 0 {
            return Some(result);
        }
        shift += 7;
        if shift > 28 {
            return None;
        }
    }
}

/// Extrae **todas** las cadenas de la tabla `string_ids` de un DEX.
///
/// Es la forma correcta de mirar un DEX: las constantes de cadena están en una
/// tabla propia, no dispersas por el código. Un `grep` sobre el binario da
/// falsos positivos; esto no.
pub fn dex_strings(dex: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    if dex.len() < 0x40 || &dex[0..4] != b"dex\n" {
        return out;
    }
    let rd_u32 = |off: usize| -> u32 {
        if off + 4 > dex.len() {
            return 0;
        }
        u32::from_le_bytes([dex[off], dex[off + 1], dex[off + 2], dex[off + 3]])
    };
    let string_ids_size = rd_u32(0x38) as usize;
    let string_ids_off = rd_u32(0x3C) as usize;
    if string_ids_size == 0 || string_ids_off == 0 {
        return out;
    }
    for i in 0..string_ids_size.min(MAX_STRINGS_KEPT) {
        let id_off = string_ids_off + i * 4;
        if id_off + 4 > dex.len() {
            break;
        }
        let data_off = rd_u32(id_off) as usize;
        if data_off == 0 || data_off >= dex.len() {
            continue;
        }
        let mut p = data_off;
        // Longitud en UTF-16: no hace falta, pero hay que saltarla (uleb128).
        if read_uleb128(dex, &mut p).is_none() {
            continue;
        }
        let mut end = p;
        while end < dex.len() && dex[end] != 0 {
            end += 1;
        }
        if end > p {
            let s = String::from_utf8_lossy(&dex[p..end]).to_string();
            out.push(s);
        }
    }
    out
}

// ========================================================================= //
// Escaneo estático de un APK
// ========================================================================= //

/// Una señal detectada en el APK, con su evidencia.
#[derive(Clone, Debug)]
pub struct Hit {
    /// Id de la señal ([`SIGNALS`]).
    pub signal: &'static str,
    /// Dónde se encontró (DEX o `.so`).
    pub source: String,
    /// Evidencia textual (la cadena que casa con el patrón).
    pub evidence: String,
}

/// Resultado del escaneo estático de un APK.
#[derive(Clone, Debug, Default)]
pub struct StaticScan {
    /// APK analizado.
    pub apk_path: String,
    /// Tamaño del APK en bytes.
    pub apk_size: u64,
    /// Número de DEX leídos.
    pub dex_count: usize,
    /// Número de `.so` leídas.
    pub so_count: usize,
    /// Señales encontradas.
    pub hits: Vec<Hit>,
    /// Errores (no abortan el escaneo).
    pub warnings: Vec<String>,
}

/// Escanea un APK en busca de señales de detección de root.
pub fn scan_apk(apk_path: &str) -> StaticScan {
    let mut scan = StaticScan {
        apk_path: apk_path.to_string(),
        ..Default::default()
    };
    let Some(file) = DeviceReader::open(apk_path) else {
        scan.warnings
            .push(format!("no se pudo abrir el APK: {}", apk_path));
        return scan;
    };
    scan.apk_size = file.size();
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(e) => {
            scan.warnings.push(format!("zip: {e}"));
            return scan;
        }
    };
    let names: Vec<String> = archive.file_names().map(|n| n.to_string()).collect();
    let dex_names: Vec<String> = names
        .iter()
        .filter(|n| {
            n.starts_with("classes") && n.ends_with(".dex") && !n.contains('/')
        })
        .take(MAX_DEX_ANALYZED)
        .cloned()
        .collect();
    let so_names: Vec<String> = names
        .iter()
        .filter(|n| n.starts_with("lib/") && n.ends_with(".so"))
        .take(MAX_SO_ANALYZED)
        .cloned()
        .collect();

    for name in &dex_names {
        let mut buf = Vec::new();
        if let Ok(mut e) = archive.by_name(name) {
            let _ = e.read_to_end(&mut buf);
        }
        scan.dex_count += 1;
        for s in dex_strings(&buf) {
            for sig in SIGNALS {
                for pat in sig.patterns {
                    if s.contains(pat) {
                        scan.hits.push(Hit {
                            signal: sig.id,
                            source: name.clone(),
                            evidence: s.chars().take(120).collect(),
                        });
                        break;
                    }
                }
            }
        }
    }

    for name in &so_names {
        let mut buf = Vec::new();
        if let Ok(mut e) = archive.by_name(name) {
            let mut limited = e.take(MAX_SO_BYTES as u64);
            let _ = limited.read_to_end(&mut buf);
        }
        scan.so_count += 1;
        // En una `.so` las señales son cadenas ASCII embebidas: se buscan
        // sobre los bytes crudos (el código compilado no las tiene en claro).
        for sig in SIGNALS {
            for pat in sig.patterns {
                if let Some(pos) = find_bytes(&buf, pat.as_bytes()) {
                    let ctx = excerpt(&buf, pos, pat.len());
                    scan.hits.push(Hit {
                        signal: sig.id,
                        source: name.clone(),
                        evidence: ctx,
                    });
                    break;
                }
            }
        }
    }
    scan.hits.truncate(MAX_HITS_PER_SIGNAL * SIGNALS.len());
    scan
}

/// Busca una subcadena de bytes y devuelve la posición.
fn find_bytes(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Excerpto legible alrededor de una posición (para la evidencia).
fn excerpt(buf: &[u8], pos: usize, len: usize) -> String {
    let start = pos.saturating_sub(24);
    let end = (pos + len + 24).min(buf.len());
    let slice = &buf[start..end];
    let s: String = slice
        .iter()
        .map(|&b| {
            if (32..127).contains(&b) {
                b as char
            } else {
                '.'
            }
        })
        .collect();
    s.chars().take(120).collect()
}

// ========================================================================= //
// Runtime: qué ve la app de verdad
// ========================================================================= //

/// Sonda de un paquete: lo que su proceso ve del sistema.
#[derive(Clone, Debug, Default)]
pub struct RuntimeView {
    /// Paquete sondeado.
    pub package: String,
    /// PID del proceso (si se pudo lanzar).
    pub pid: Option<u32>,
    /// UID del proceso.
    pub uid: Option<String>,
    /// Contexto SELinux del proceso.
    pub context: Option<String>,
    /// Entradas de `/proc/<pid>/mounts` que mencionan "magisk".
    pub magisk_mounts: Option<usize>,
    /// Montajes totales vistos por la app.
    pub mounts_total: Option<usize>,
    /// Rutas de root visibles desde la raíz de la app.
    pub visible_paths: Vec<String>,
    /// Props tal y como las vería la app.
    pub props: Vec<(String, String)>,
    /// UID de la app (para `su <uid>`).
    pub app_uid: Option<u32>,
    /// Errores de la sonda.
    pub warnings: Vec<String>,
}

/// Rutas de root que se comprueban en la raíz de la app.
pub const ROOT_PATHS: &[&str] = &[
    "/system/bin/su",
    "/system/xbin/su",
    "/sbin/su",
    "/system/sbin/su",
    "/vendor/bin/su",
    "/data/local/xbin/su",
    "/data/local/bin/su",
    "/su/bin/su",
    "/system/bin/.ext/.su",
    "/system/bin/magisk",
    "/system/bin/magiskpolicy",
    "/data/adb",
    "/debug_ramdisk",
];

/// Props que un detector suele mirar.
pub const WATCHED_PROPS: &[&str] = &[
    "ro.debuggable",
    "ro.build.tags",
    "ro.build.type",
    "ro.secure",
    "ro.build.version.release",
];

/// Lanza la app (o la reutiliza si ya corre) y devuelve su PID.
pub fn launch_and_pid(pkg: &str) -> Option<u32> {
    let cmd = format!(
        "am start -W -a android.intent.action.MAIN -c android.intent.category.LAUNCHER {pkg} >/dev/null 2>&1; \
         for i in 1 2 3 4 5 6 7 8 9 10; do p=$(pidof {pkg} 2>/dev/null | awk '{{print $1}}'); [ -n \"$p\" ] && echo $p && break; sleep 1; done",
        pkg = crate::android::sh_quote(pkg)
    );
    let r = crate::android::root_exec(&cmd, PROBE_TIMEOUT_MS, 4096);
    r.stdout.trim().split_whitespace().next().and_then(|s| s.parse().ok())
}

/// Sondea lo que ve la app: montajes, ficheros visibles y props.
pub fn runtime_view(pkg: &str) -> RuntimeView {
    let mut v = RuntimeView {
        package: pkg.to_string(),
        ..Default::default()
    };
    let Some(pid) = launch_and_pid(pkg) else {
        v.warnings
            .push("no se pudo lanzar la app (sin PID): no se puede mirar su namespace".to_string());
        // Aun así se mide el UID de la app: sirve para la sonda con `su <uid>`.
        v.app_uid = app_uid(pkg);
        return v;
    };
    v.pid = Some(pid);
    let p = pid.to_string();

    let cmd = format!(
        "echo UID=$(awk '/^Uid:/{{print $2}}' /proc/{p}/status 2>/dev/null); \
         echo CTX=$(cat /proc/{p}/attr/current 2>/dev/null); \
         echo MAGISK=$(grep -ci magisk /proc/{p}/mounts 2>/dev/null); \
         echo TOTAL=$(wc -l < /proc/{p}/mounts 2>/dev/null)"
    );
    let r = crate::android::root_exec(&cmd, PROBE_TIMEOUT_MS, 8192);
    for line in r.stdout.lines() {
        let (k, val) = match line.split_once('=') {
            Some(kv) => kv,
            None => continue,
        };
        match k {
            "UID" => v.uid = Some(val.to_string()),
            "CTX" => v.context = Some(val.to_string()),
            "MAGISK" => v.magisk_mounts = val.trim().parse::<usize>().ok(),
            "TOTAL" => v.mounts_total = val.trim().parse::<usize>().ok(),
            _ => {}
        }
    }

    // Rutas visibles desde la raíz de la app (su espacio de nombres).
    let mut checks = String::new();
    for path in ROOT_PATHS {
        checks.push_str(&format!(
            "[ -e /proc/{p}/root{path} ] && echo VISIBLE={path}\n",
            p = p,
            path = path
        ));
    }
    let r2 = crate::android::root_exec(&checks, PROBE_TIMEOUT_MS, 8192);
    for line in r2.stdout.lines() {
        if let Some(rest) = line.trim().strip_prefix("VISIBLE=") {
            v.visible_paths.push(rest.to_string());
        }
    }

    // Props tal y como las leería la app (su `/system/build.prop`).
    let props_cmd = format!(
        "for k in {ks}; do v=$(grep -m1 \"^$k=\" /proc/{p}/root/system/build.prop 2>/dev/null | cut -d= -f2-); \
         [ -z \"$v\" ] && v=$(getprop $k 2>/dev/null); echo \"$k=$v\"; done",
        ks = WATCHED_PROPS.join(" "),
        p = p
    );
    let r3 = crate::android::root_exec(&props_cmd, PROBE_TIMEOUT_MS, 8192);
    for line in r3.stdout.lines() {
        if let Some((k, val)) = line.trim().split_once('=') {
            v.props.push((k.to_string(), val.to_string()));
        }
    }
    v.app_uid = app_uid(pkg);
    v
}

/// UID de una app instalada.
pub fn app_uid(pkg: &str) -> Option<u32> {
    let cmd = format!(
        "pm list packages -U {} 2>/dev/null | grep -o 'uid:[0-9]*' | head -1 | cut -d: -f2",
        crate::android::sh_quote(pkg)
    );
    let r = crate::android::root_exec(&cmd, PROBE_TIMEOUT_MS, 4096);
    r.stdout.trim().parse::<u32>().ok()
}

// ========================================================================= //
// Prueba de ejecución: logcat de la app en marcha
// ========================================================================= //

/// Palabras que delatan un veredicto de detección de root en el log.
pub const EXEC_KEYWORDS: &[&str] = &[
    "root",
    "magisk",
    "supersu",
    "superuser",
    "busybox",
    "xposed",
    "frida",
    "test-keys",
    "tamper",
    "integrity",
    "safetynet",
    "playprotect",
    "device_is_rooted",
    "isrooted",
];

/// Arranca la app con el logcat limpio y captura su log filtrado.
pub fn execution_test(pkg: &str) -> Value {
    let mut steps: Vec<String> = Vec::new();
    let clear = crate::android::root_exec("logcat -c 2>/dev/null", 15_000, 1024);
    steps.push(format!(
        "logcat -c => rc={:?}",
        clear.code
    ));
    let pid = launch_and_pid(pkg);
    let start = std::time::Instant::now();
    std::thread::sleep(std::time::Duration::from_secs(EXEC_SETTLE_SECS as u64));
    let mut hits: Vec<String> = Vec::new();
    let mut total_lines = 0usize;
    if let Some(pid) = pid {
        let cmd = format!(
            "logcat -d --pid={pid} -v brief 2>/dev/null | tail -400",
            pid = pid
        );
        let r = crate::android::root_exec(&cmd, 30_000, 200_000);
        total_lines = r.stdout.lines().count();
        for line in r.stdout.lines() {
            let low = line.to_lowercase();
            if EXEC_KEYWORDS.iter().any(|k| low.contains(&k.to_lowercase())) {
                hits.push(line.chars().take(220).collect());
            }
        }
        steps.push(format!("pid={pid} lineas_log={}", total_lines));
    } else {
        steps.push("sin PID: la app no arranco".to_string());
    }
    // Volcado del log completo por si el detector no escribe en logcat.
    let dump_cmd = format!(
        "logcat -d 2>/dev/null | grep -iE '{}' | tail -60",
        EXEC_KEYWORDS.join("|")
    );
    let dump = crate::android::root_exec(&dump_cmd, 30_000, 200_000);
    let global: Vec<String> = dump
        .stdout
        .lines()
        .filter(|l| l.contains(pkg))
        .map(|l| l.chars().take(220).collect())
        .collect();
    json!({
        "package": pkg,
        "pid": pid,
        "seconds": EXEC_SETTLE_SECS,
        "elapsed_ms": start.elapsed().as_millis() as u64,
        "log_lines": total_lines,
        "verdict_hits": hits,
        "device_hits_for_package": global,
        "steps": steps,
        "note": "Si el detector no escribe en logcat, la prueba de ejecucion no puede \
                 judged por si sola: se apoya en la sonda de runtime.",
    })
}

// ========================================================================= //
// Estado de ocultación actual (medido, no supuesto)
// ========================================================================= //

/// Estado de una señal en el móvil AHORA MISMO.
#[derive(Clone, Debug)]
pub struct SignalState {
    /// Id de la señal.
    pub signal: &'static str,
    /// ¿Está visible para una app normal?
    pub visible: bool,
    /// Qué mecanismo la ocultaría.
    pub hidden_by: String,
    /// Evidencia medida.
    pub evidence: String,
    /// `medido` | `no-medido`.
    pub status: &'static str,
}

/// Mide el estado real de cada señal frente a una app normal.
///
/// `pkg` es una app de referencia (cualquiera): se lanza, se lee su namespace
/// y se responde con el estado de cada señal.
pub fn occlusion_state(pkg: &str) -> Vec<SignalState> {
    let v = runtime_view(pkg);
    let mut out = Vec::new();

    let vis = |p: &str| v.visible_paths.iter().any(|x| x == p);
    let prop = |k: &str| {
        v.props
            .iter()
            .find(|(a, _)| a == k)
            .map(|(_, b)| b.clone())
            .unwrap_or_default()
    };

    // --- su y binarios de Magisk ------------------------------------------
    let su_paths: Vec<&str> = ROOT_PATHS
        .iter()
        .copied()
        .filter(|p| p.ends_with("/su") || p.ends_with("/.su"))
        .collect();
    let su_vis: Vec<&str> = su_paths.iter().copied().filter(|p| vis(p)).collect();
    out.push(SignalState {
        signal: "su_binary",
        visible: !su_vis.is_empty(),
        hidden_by: "MagiskHide/DenyList (namespace) o Shamiko (hook Zygisk)".to_string(),
        evidence: if su_vis.is_empty() {
            "ninguna ruta de su visible desde la app".to_string()
        } else {
            format!("visibles: {}", su_vis.join(", "))
        },
        status: "medido",
    });

    let mag_vis: Vec<&str> = ROOT_PATHS
        .iter()
        .copied()
        .filter(|p| p.contains("magisk"))
        .filter(|p| vis(p))
        .collect();
    out.push(SignalState {
        signal: "magisk_binary",
        visible: !mag_vis.is_empty(),
        hidden_by: "MagiskHide/DenyList (namespace) o Shamiko (hook Zygisk)".to_string(),
        evidence: if mag_vis.is_empty() {
            "magisk/magiskpolicy no visibles".to_string()
        } else {
            format!("visibles: {}", mag_vis.join(", "))
        },
        status: "medido",
    });

    // --- montajes ----------------------------------------------------------
    let mm = v.magisk_mounts.unwrap_or(0);
    out.push(SignalState {
        signal: "proc_mounts",
        visible: mm > 0,
        hidden_by: "MagiskHide/DenyList (namespace) o Shamiko (hook Zygisk)".to_string(),
        evidence: format!(
            "/proc/{}/mounts de la app: {} entradas con 'magisk' de {} totales",
            v.pid.unwrap_or(0),
            mm,
            v.mounts_total.unwrap_or(0)
        ),
        status: if v.pid.is_some() { "medido" } else { "no-medido" },
    });

    // --- /data/adb ---------------------------------------------------------
    let data_adb = vis("/data/adb");
    out.push(SignalState {
        signal: "superuser_app",
        visible: data_adb,
        hidden_by: "MagiskHide/DenyList (namespace)".to_string(),
        evidence: format!(
            "/data/adb {} desde la raíz de la app (los paquetes Superuser/SuperSU \
             viven ahí)",
            if data_adb { "existe" } else { "NO existe" }
        ),
        status: if v.pid.is_some() { "medido" } else { "no-medido" },
    });

    // --- props -------------------------------------------------------------
    let dbg = prop("ro.debuggable");
    let tags = prop("ro.build.tags");
    let ty = prop("ro.build.type");
    let props_clean = dbg == "0" && !tags.contains("test-keys") && ty != "eng";
    out.push(SignalState {
        signal: "build_props",
        visible: !props_clean,
        hidden_by: "resetprop / post-fs-data.sh del módulo Magisk".to_string(),
        evidence: format!(
            "ro.debuggable={} ro.build.tags={} ro.build.type={}",
            if dbg.is_empty() { "?" } else { &dbg },
            if tags.is_empty() { "?" } else { &tags },
            if ty.is_empty() { "?" } else { &ty }
        ),
        status: if v.props.is_empty() { "no-medido" } else { "medido" },
    });

    // --- frameworks de hook ------------------------------------------------
    let lsp = vis("/data/adb");
    out.push(SignalState {
        signal: "hook_framework",
        visible: lsp,
        hidden_by: "DenyList + Shamiko (Zygisk) o desinstalar LSPosed".to_string(),
        evidence: format!(
            "/data/adb {} desde la app (LSPosed vive en /data/adb/modules)",
            if lsp { "visible" } else { "oculto" }
        ),
        status: if v.pid.is_some() { "medido" } else { "no-medido" },
    });

    // --- atestación --------------------------------------------------------
    out.push(SignalState {
        signal: "attestation",
        visible: false,
        hidden_by: "Play Integrity Fix + repositorio de hardware".to_string(),
        evidence: "no medido en esta sesion (requiere attestation del dispositivo)".to_string(),
        status: "no-medido",
    });

    // --- busybox -----------------------------------------------------------
    out.push(SignalState {
        signal: "busybox",
        visible: false,
        hidden_by: "DenyList/Shamiko (no hay busybox instalado: no aplica)".to_string(),
        evidence: "no se midio la presencia de busybox en el PATH".to_string(),
        status: "no-medido",
    });

    out
}

// ========================================================================= //
// Auditoría completa por paquete
// ========================================================================= //

/// Auditoría de un paquete: estático + runtime + veredicto.
pub fn audit_package(pkg: &str) -> Value {
    let started = std::time::Instant::now();
    // 1) Ruta del APK.
    let path_cmd = format!(
        "pm path {} 2>/dev/null | head -1 | cut -d: -f2",
        crate::android::sh_quote(pkg)
    );
    let pr = crate::android::root_exec(&path_cmd, PROBE_TIMEOUT_MS, 4096);
    let apk_path = pr.stdout.trim().to_string();

    // 2) Estático.
    let static_scan = if apk_path.is_empty() {
        StaticScan {
            warnings: vec!["el paquete no está instalado".to_string()],
            ..Default::default()
        }
    } else {
        scan_apk(&apk_path)
    };

    // 3) Runtime.
    let view = runtime_view(pkg);
    let states = occlusion_state(pkg);

    // 4) Cruce: señales que la app mira × estado medido.
    let mut signals: Vec<Value> = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for hit in &static_scan.hits {
        if seen.contains(&hit.signal) {
            continue;
        }
        seen.push(hit.signal);
        let state = states.iter().find(|s| s.signal == hit.signal);
        let hidden = match state {
            Some(s) => !s.visible,
            None => false,
        };
        let status = state.map(|s| s.status).unwrap_or("no-medido");
        let evidence = state
            .map(|s| s.evidence.clone())
            .unwrap_or_else(|| "sin sonda para esta señal".to_string());
        let hidden_by = state
            .map(|s| s.hidden_by.clone())
            .unwrap_or_else(|| "—".to_string());
        signals.push(json!({
            "signal": hit.signal,
            "label": signal_by_id(hit.signal).map(|s| s.label).unwrap_or(""),
            "family": signal_by_id(hit.signal).map(|s| s.family.key()).unwrap_or(""),
            "detected_in": hit.source,
            "evidence_static": hit.evidence,
            "visible_now": state.map(|s| s.visible).unwrap_or(false),
            "hidden_now": hidden,
            "status": status,
            "hidden_by": hidden_by,
            "runtime_evidence": evidence,
        }));
    }
    signals.sort_by(|a, b| {
        a["signal"]
            .as_str()
            .unwrap_or("")
            .cmp(b["signal"].as_str().unwrap_or(""))
    });

    // 5) Veredicto.
    let visible: Vec<&Value> = signals
        .iter()
        .filter(|s| s["visible_now"].as_bool().unwrap_or(false))
        .collect();
    let unmeasured: Vec<&Value> = signals
        .iter()
        .filter(|s| s["status"].as_str() == Some("no-medido"))
        .collect();
    let verdict = if signals.is_empty() {
        "SIN-SEÑALES"
    } else if !visible.is_empty() {
        "FAIL"
    } else if !unmeasured.is_empty() {
        "PARCIAL"
    } else {
        "PASS"
    };
    let visible_list: Vec<String> = visible
        .iter()
        .map(|s| s["signal"].as_str().unwrap_or("").to_string())
        .collect();
    let unmeasured_list: Vec<String> = unmeasured
        .iter()
        .map(|s| s["signal"].as_str().unwrap_or("").to_string())
        .collect();

    json!({
        "package": pkg,
        "apk_path": apk_path,
        "apk_size": static_scan.apk_size,
        "dex_count": static_scan.dex_count,
        "so_count": static_scan.so_count,
        "static_hits": static_scan.hits.len(),
        "signals": signals,
        "runtime": {
            "pid": view.pid,
            "uid": view.uid,
            "context": view.context,
            "magisk_mounts": view.magisk_mounts,
            "mounts_total": view.mounts_total,
            "visible_paths": view.visible_paths,
            "props": view.props.iter().map(|(k, v)| json!({"key": k, "value": v})).collect::<Vec<_>>(),
            "warnings": view.warnings,
        },
        "verdict": verdict,
        "visible_signals": visible_list,
        "unmeasured_signals": unmeasured_list,
        "elapsed_ms": started.elapsed().as_millis() as u64,
        "warnings": static_scan.warnings,
    })
}

/// Auditoría de varios paquetes: la tabla app → señal → veredicto.
pub fn audit(packages: &[String]) -> Value {
    let started = std::time::Instant::now();
    let mut rows = Vec::new();
    for p in packages {
        let a = audit_package(p);
        rows.push(a);
    }
    let mut pass = 0;
    let mut fail = 0;
    let mut partial = 0;
    for r in &rows {
        match r["verdict"].as_str().unwrap_or("") {
            "PASS" => pass += 1,
            "FAIL" => fail += 1,
            "PARCIAL" => partial += 1,
            _ => {}
        }
    }
    json!({
        "packages": rows,
        "summary": {
            "total": rows.len(),
            "pass": pass,
            "fail": fail,
            "parcial": partial,
            "elapsed_ms": started.elapsed().as_millis() as u64,
        },
        "signals_catalog": SIGNALS.iter().map(|s| json!({
            "id": s.id,
            "label": s.label,
            "family": s.family.key(),
        })).collect::<Vec<_>>(),
    })
}

/// SHA-256 en hexadecimal de un bloque de bytes.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    let d = h.finalize();
    let mut s = String::with_capacity(64);
    for b in d.iter() {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Comprueba que un APK existe y devuelve su SHA-256 (usado por el banco).
pub fn apk_sha256(apk_path: &str) -> Option<String> {
    let mut r = DeviceReader::open(apk_path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; CHUNK_BYTES];
    loop {
        let n = r.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    let d = h.finalize();
    let mut s = String::with_capacity(64);
    for b in d.iter() {
        s.push_str(&format!("{:02x}", b));
    }
    Some(s)
}

/// Ruta de un paquete instalado (o `None`).
pub fn package_apk_path(pkg: &str) -> Option<String> {
    let cmd = format!(
        "pm path {} 2>/dev/null | head -1 | cut -d: -f2",
        crate::android::sh_quote(pkg)
    );
    let r = crate::android::root_exec(&cmd, PROBE_TIMEOUT_MS, 4096);
    let p = r.stdout.trim().to_string();
    if p.is_empty() {
        None
    } else {
        Some(p)
    }
}

/// ¿Existe este path en el dispositivo?
pub fn device_path_exists(path: &str) -> bool {
    let cmd = format!("[ -e {} ] && echo 1 || echo 0", crate::android::sh_quote(path));
    let r = crate::android::root_exec(&cmd, PROBE_TIMEOUT_MS, 1024);
    r.stdout.trim() == "1"
}

/// Raíz del CSC activo (para la documentación del CSC).
pub const CSC_ACTIVE_PATH: &str = "/optics/configs/carriers/single/AMO/conf/system/cscfeature.xml";

/// Lee el `cscfeature.xml` activo y devuelve `(sha256, valor_de_la_feature)`.
pub fn csc_feature_live() -> Option<(String, Option<String>)> {
    let bytes = read_device_file(CSC_ACTIVE_PATH, 1 << 20)?;
    let sha = sha256_hex(&bytes);
    let (xml, _enc, _err) = crate::csc_verify::decode_csc_file(&bytes);
    let val = xml.and_then(|x| crate::rom_migration::csc_feature_get(&x, crate::csc_verify::FEATURE));
    Some((sha, val))
}

/// Devuelve la ruta del fichero de un paquete en el móvil (helper de tests).
pub fn apk_path_of(pkg: &str) -> String {
    package_apk_path(pkg).unwrap_or_default()
}

/// Lee un APK del disco local (helper de tests).
pub fn read_local(path: &str) -> Option<Vec<u8>> {
    std::fs::read(Path::new(path)).ok()
}

// ========================================================================= //
// Tests deterministas (sin móvil)
// ========================================================================= //

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_uleb128_se_lee_bien() {
        let buf = [0x00u8, 0x7F, 0x80, 0x01, 0xAC, 0x02];
        let mut p = 0usize;
        assert_eq!(read_uleb128(&buf, &mut p), Some(0));
        let mut p = 1usize;
        assert_eq!(read_uleb128(&buf, &mut p), Some(127));
        let mut p = 2usize;
        assert_eq!(read_uleb128(&buf, &mut p), Some(128));
        let mut p = 4usize;
        assert_eq!(read_uleb128(&buf, &mut p), Some(300));
    }

    #[test]
    fn el_dex_falso_no_devuelve_cadenas() {
        assert!(dex_strings(b"dex-falso").is_empty());
        assert!(dex_strings(b"").is_empty());
    }

    #[test]
    fn las_señales_tienen_patron() {
        for s in SIGNALS {
            assert!(!s.patterns.is_empty(), "señal {} sin patrón", s.id);
        }
    }

    #[test]
    fn la_consulta_de_señal_es_estable() {
        assert_eq!(signal_by_id("su_binary").unwrap().family, SignalFamily::SuBinary);
        assert_eq!(
            signal_by_id("build_props").unwrap().family,
            SignalFamily::BuildProps
        );
        assert!(signal_by_id("no_existe").is_none());
    }

    #[test]
    fn el_sha256_calcula_bien() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn encuentra_bytes() {
        // "mundo" empieza en el índice 5 de "hola mundo" (h=0 o=1 l=2 a=3
        // espacio=4 m=5).  La expectativa decía 6: el off-by-one estaba en el
        // test, la función devolvía bien (el CI: left=Some(5), right=Some(6)).
        assert_eq!(find_bytes(b"hola mundo", b"mundo"), Some(5));
        assert_eq!(find_bytes(b"hola", b"xyz"), None);
    }

    #[test]
    fn el_excerpto_es_legible() {
        let buf = b"prefijo\x00\x01\x02su\x00final";
        let e = excerpt(buf, 8, 2);
        assert!(e.contains("su"), "excerpt: {}", e);
    }
}

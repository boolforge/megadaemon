//! **Lector de backups «tar por paquete»**: Titanium Backup, Machiav3lli y los
//! `.dat` de Swift Backup.
//!
//! # Qué es un «tar por paquete»
//!
//! Los backups de estas apps **no** son un ZIP con todo dentro: cada paquete
//! (aplicación) es **un tar** con el árbol de datos de esa app.  Los `.dat`
//! reales de Swift Backup en Drive son un **ZIP que envuelve ese tar**.
//!
//! # Reglas del repo que se respetan aquí
//!
//! * **no se inventa**: lo que no se puede leer sale con el motivo exacto;
//! * **sólo lectura**: este módulo nunca borra nada del origen;
//! * **no se escribe fuera del destino**: toda ruta se sanea antes de
//!   escribir ([`sanear_ruta`]) ⇒ un tar con `../escapado.txt` no extrae nada;
//! * **el checksum del TAR se verifica**: un tar corrupto **no** da `ok`.
//!
//! # Lo que se mide, no se supone
//!
//! * el formato de bloque TAR es **ustar** (`ustar\0` en el offset 257 de la
//!   cabecera), que es lo que produce el `tar` real de la app de backup;
//! * el `size` del TAR es **octal ASCII** (con el formato de base-256 como
//!   reserva para tamaños imposibles).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub use crate::backup_crypto::sha256_hex;

/// Magic **ustar**: va en el offset 257 de la cabecera de cada entrada TAR.
pub const MAGIC_USTAR: &[u8] = b"ustar\0";

/// Tamaño de un bloque TAR (cabecera o datos).
const BLOQUE: usize = 512;

/// Formato del contenedor de un backup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FormatoContenedor {
    /// TAR puro (lo que produce la app de backup del móvil).
    Tar,
    /// ZIP que envuelve un tar (el `.dat` real de Swift Backup).
    ZipConTar,
    /// Contenedor `SBA1` de Swift Backup (cifrado).
    Sba1,
    /// No se reconoce.
    Desconocido,
}

impl FormatoContenedor {
    /// Nombre estable para el JSON del informe.
    pub fn as_str(self) -> &'static str {
        match self {
            FormatoContenedor::Tar => "tar",
            FormatoContenedor::ZipConTar => "zip_con_tar",
            FormatoContenedor::Sba1 => "sba1",
            FormatoContenedor::Desconocido => "desconocido",
        }
    }
}

/// Una entrada del TAR.
///
/// `nombre` es el nombre **tal cual** va en el tar (con su `./` inicial si lo
/// tiene); `offset` apunta al **contenido**, no a la cabecera.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntradaTar {
    /// Ruta dentro del tar, verbatim.
    pub nombre: String,
    /// Tipo de entrada (`b'0'` fichero, `b'5'` directorio…).
    pub tipo: u8,
    /// Tamaño del contenido en bytes.
    pub tamano: u64,
    /// Offset del contenido dentro del tar.
    pub offset: usize,
}

impl EntradaTar {
    /// ¿Es un fichero regular?
    pub fn es_fichero(&self) -> bool {
        self.tipo == b'0' || self.tipo == 0
    }
}

/// Inventario de un paquete (un backup de una app).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PaqueteInventario {
    /// Paquete de la app (`com.foo.bar`).
    pub paquete: String,
    /// Tamaño del fichero de backup.
    pub bytes: u64,
    /// Formato del contenedor.
    pub formato: FormatoContenedor,
    /// ¿El tar trae `shared_prefs/`?
    pub tiene_shared_prefs: bool,
    /// ¿El tar trae `databases/`?
    pub tiene_databases: bool,
    /// sha256 del fichero completo.
    pub sha256: String,
}

/// Informe de verificación de un backup.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Verificacion {
    /// ¿El backup está íntegro?
    pub ok: bool,
    /// Errores encontrados (vacío si `ok`).
    pub errores: Vec<String>,
    /// Formato detectado.
    pub formato: FormatoContenedor,
    /// Tamaño del fichero.
    pub bytes: u64,
    /// Número de entradas del tar.
    pub entradas: usize,
}

/// Resultado de una extracción.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Extraccion {
    /// ¿Se extrajo sin errores?
    pub ok: bool,
    /// Rutas relativas extraídas (las sanitizadas).
    pub ficheros: Vec<String>,
    /// Errores (rutas rechazadas, ficheros ilegibles…).
    pub errores: Vec<String>,
}

/// Una fila del índice SQLite de backups (`Backup`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FilaIndice {
    /// `packageName`.
    pub package_name: String,
    /// `versionName` (`-` si no lo tiene).
    pub version_name: String,
    /// `hasApk`.
    pub has_apk: bool,
    /// `hasAppData`.
    pub has_app_data: bool,
    /// `size` declarado en el índice.
    pub size: i64,
}

// ---------------------------------------------------------------------------
// Detección de formato
// ---------------------------------------------------------------------------

/// ¿Qué formato tiene este fichero?
pub fn detectar_formato(bytes: &[u8]) -> FormatoContenedor {
    if bytes.len() >= 4 && &bytes[..4] == b"SBA1" {
        return FormatoContenedor::Sba1;
    }
    if bytes.len() >= 4 && &bytes[..4] == b"PK\x03\x04" {
        // Un ZIP: sólo es «zip con tar» si dentro hay un `.tar`.
        if let Ok(entradas) = crate::backup_formats::read_zip(bytes) {
            if entradas.iter().any(|e| e.name.to_ascii_lowercase().ends_with(".tar")) {
                return FormatoContenedor::ZipConTar;
            }
        }
        return FormatoContenedor::Desconocido;
    }
    if bytes.len() > 262 && &bytes[257..263] == MAGIC_USTAR {
        return FormatoContenedor::Tar;
    }
    FormatoContenedor::Desconocido
}

// ---------------------------------------------------------------------------
// Lector TAR (ustar) propio: offsets y nombres verbatim
// ---------------------------------------------------------------------------

/// Lee el directorio de un TAR **ustar**.
///
/// A diferencia de [`crate::backup_formats::read_tar`], esto **no** carga el
/// contenido en memoria y **no** sanea los nombres: devuelve el `offset` y el
/// `tamano` de cada entrada para poder extraer por trozos.
///
/// Verifica el **checksum** de cada cabecera: un tar corrupto da `Err`.
pub fn leer_tar(bytes: &[u8]) -> Result<Vec<EntradaTar>, String> {
    if bytes.is_empty() {
        return Err("el tar está vacío".to_string());
    }
    let mut out = Vec::new();
    let mut pos = 0usize;
    let mut bloque_vacio = 0usize;
    while pos + BLOQUE <= bytes.len() {
        let cab = &bytes[pos..pos + BLOQUE];
        if cab.iter().all(|b| *b == 0) {
            bloque_vacio += 1;
            if bloque_vacio >= 2 {
                break;
            }
            pos += BLOQUE;
            continue;
        }
        bloque_vacio = 0;
        let nombre = leer_nombre(cab);
        let tamano = leer_octal(&cab[124..136])
            .ok_or_else(|| format!("el tamaño de la entrada {nombre:?} no se puede leer"))?;
        let tipo = cab[156];
        let chk_declarado = leer_octal(&cab[148..156])
            .ok_or_else(|| format!("el checksum de la entrada {nombre:?} no se puede leer"))?;
        let chk_real = checksum_cabecera(cab);
        if chk_declarado != chk_real {
            return Err(format!(
                "checksum incorrecto en la entrada {nombre:?}: declarado {chk_declarado}, calculado {chk_real}"
            ));
        }
        let inicio = pos + BLOQUE;
        let fin = inicio
            .checked_add(tamano as usize)
            .filter(|f| *f <= bytes.len())
            .ok_or_else(|| format!("la entrada {nombre:?} se sale del tar"))?;
        out.push(EntradaTar {
            nombre,
            tipo,
            tamano,
            offset: inicio,
        });
        // Los datos se rellenan hasta el siguiente bloque de 512.
        let relleno = (BLOQUE - (tamano as usize % BLOQUE)) % BLOQUE;
        pos = fin + relleno;
    }
    if out.is_empty() {
        return Err("el tar no tiene entradas legibles".to_string());
    }
    Ok(out)
}

/// Nombre de la entrada (campo `name`, con el prefijo `prefix` si lo hay).
fn leer_nombre(cab: &[u8]) -> String {
    let nombre = cadena(&cab[0..100]);
    let prefijo = cadena(&cab[345..500]);
    if prefijo.is_empty() {
        nombre
    } else {
        format!("{prefijo}/{nombre}")
    }
}

/// Campo de cadena terminado en NUL.
fn cadena(b: &[u8]) -> String {
    let fin = b.iter().position(|c| *c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..fin]).trim_end().to_string()
}

/// Entero octal ASCII (con el formato de base-256 como reserva).
fn leer_octal(b: &[u8]) -> Option<u64> {
    if b.is_empty() {
        return None;
    }
    if b[0] & 0x80 != 0 {
        // Formato base-256: el primer byte marca el ancho.
        let ancho = (b[0] & 0x7f) as usize;
        if ancho == 0 || ancho > b.len() {
            return None;
        }
        let mut v: u64 = 0;
        for byte in &b[1..ancho] {
            v = v.checked_mul(256)?.checked_add(*byte as u64)?;
        }
        return Some(v);
    }
    let s = cadena(b);
    let s = s.trim_matches(|c: char| c == ' ' || c == '\0');
    if s.is_empty() {
        return Some(0);
    }
    u64::from_str_radix(s, 8).ok()
}

/// Suma de los bytes de la cabecera con el campo de checksum como espacios.
fn checksum_cabecera(cab: &[u8]) -> u64 {
    let mut suma: u64 = 0;
    for (i, b) in cab.iter().enumerate() {
        if (148..156).contains(&i) {
            suma += 32;
        } else {
            suma += *b as u64;
        }
    }
    suma
}

/// Saca el tar del contenedor: si es un ZIP, extrae su `.tar`; si ya es un
/// tar, lo devuelve tal cual.
pub fn tar_del_contenedor(bytes: &[u8]) -> Result<Vec<u8>, String> {
    match detectar_formato(bytes) {
        FormatoContenedor::Tar => Ok(bytes.to_vec()),
        FormatoContenedor::ZipConTar => {
            let entradas = crate::backup_formats::read_zip(bytes)?;
            let e = entradas
                .iter()
                .find(|e| e.name.to_ascii_lowercase().ends_with(".tar"))
                .ok_or_else(|| "el zip no contiene ningún .tar".to_string())?;
            crate::backup_formats::zip_read_data(bytes, e)
        }
        other => Err(format!("el contenedor no contiene un tar (formato: {})", other.as_str())),
    }
}

/// Convierte una ruta de tar en una ruta **segura** dentro del destino.
///
/// Rechaza rutas absolutas, `..` y rutas vacías.  Quita el `./` inicial.
pub fn sanear_ruta(nombre: &str) -> Result<String, String> {
    let n = nombre.replace('\\', "/");
    if n.starts_with('/') {
        return Err(format!("ruta absoluta no permitida: {nombre}"));
    }
    let mut partes: Vec<&str> = Vec::new();
    for p in n.split('/') {
        match p {
            "" | "." => continue,
            ".." => return Err(format!("salto de directorio no permitido: {nombre}")),
            p => partes.push(p),
        }
    }
    if partes.is_empty() {
        return Err(format!("ruta vacía: {nombre}"));
    }
    Ok(partes.join("/"))
}

// ---------------------------------------------------------------------------
// Inventario / verificación / extracción
// ---------------------------------------------------------------------------

/// Inventaria un fichero de backup.
pub fn inventariar_fichero(ruta: &Path) -> Result<PaqueteInventario, String> {
    let bytes = std::fs::read(ruta).map_err(|e| format!("no se pudo leer {}: {e}", ruta.display()))?;
    let formato = detectar_formato(&bytes);
    let tar = tar_del_contenedor(&bytes)?;
    let entradas = leer_tar(&tar)?;
    let paquete = ruta
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "desconocido".to_string());
    let tiene_shared_prefs = entradas.iter().any(|e| e.nombre.contains("shared_prefs"));
    let tiene_databases = entradas.iter().any(|e| e.nombre.contains("databases"));
    Ok(PaqueteInventario {
        paquete,
        bytes: bytes.len() as u64,
        formato,
        tiene_shared_prefs,
        tiene_databases,
        sha256: sha256_hex(&bytes),
    })
}

/// Verifica un backup (checksum del TAR, formato, número de entradas).
///
/// **Nunca** falla por un backup corrupto: devuelve el informe con `ok:false`
/// y el motivo en `errores`.
pub fn verificar_fichero(ruta: &Path) -> Result<Verificacion, String> {
    let bytes = std::fs::read(ruta).map_err(|e| format!("no se pudo leer {}: {e}", ruta.display()))?;
    let formato = detectar_formato(&bytes);
    let mut errores: Vec<String> = Vec::new();
    let mut entradas = 0usize;
    match tar_del_contenedor(&bytes) {
        Ok(tar) => match leer_tar(&tar) {
            Ok(lista) => entradas = lista.len(),
            Err(e) => errores.push(e),
        },
        Err(e) => errores.push(e),
    }
    Ok(Verificacion {
        ok: errores.is_empty(),
        errores,
        formato,
        bytes: bytes.len() as u64,
        entradas,
    })
}

/// Extrae un paquete a `destino`.
///
/// Las rutas que no son seguras se **rechazan** (no se escriben) y se
/// apuntan en `errores`: un tar con `../escapado.txt` no extrae nada.
pub fn extraer_paquete(ruta: &Path, destino: &Path) -> Result<Extraccion, String> {
    let bytes = std::fs::read(ruta).map_err(|e| format!("no se pudo leer {}: {e}", ruta.display()))?;
    let tar = tar_del_contenedor(&bytes)?;
    let entradas = leer_tar(&tar)?;
    let mut ficheros: Vec<String> = Vec::new();
    let mut errores: Vec<String> = Vec::new();
    for e in &entradas {
        if !e.es_fichero() {
            continue;
        }
        let limpio = match sanear_ruta(&e.nombre) {
            Ok(c) => c,
            Err(motivo) => {
                errores.push(motivo);
                continue;
            }
        };
        let fin = e.offset + e.tamano as usize;
        if fin > tar.len() {
            errores.push(format!("la entrada {} se sale del tar", e.nombre));
            continue;
        }
        let destino_final = destino.join(&limpio);
        if let Some(dir) = destino_final.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|x| format!("no se pudo crear {}: {x}", dir.display()))?;
        }
        std::fs::write(&destino_final, &tar[e.offset..fin])
            .map_err(|x| format!("no se pudo escribir {}: {x}", destino_final.display()))?;
        ficheros.push(limpio);
    }
    Ok(Extraccion {
        ok: errores.is_empty(),
        ficheros,
        errores,
    })
}

/// Inventaria **todos** los ficheros de backup de un directorio.
pub fn inventariar_directorio(dir: &Path) -> Result<Vec<PaqueteInventario>, String> {
    let lectura = std::fs::read_dir(dir)
        .map_err(|e| format!("no se pudo leer {}: {e}", dir.display()))?;
    let mut out = Vec::new();
    for entrada in lectura {
        let entrada = match entrada {
            Ok(e) => e,
            Err(e) => return Err(format!("no se pudo leer {}: {e}", dir.display())),
        };
        let ruta = entrada.path();
        if !ruta.is_file() {
            continue;
        }
        match inventariar_fichero(&ruta) {
            Ok(p) => out.push(p),
            // Un fichero que no es un backup no es un error del inventario.
            Err(_) => continue,
        }
    }
    out.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    Ok(out)
}

// ---------------------------------------------------------------------------
// Índice SQLite de backups (tabla `Backup`, esquema MEDIDO en el móvil)
// ---------------------------------------------------------------------------

/// Lee el índice de backups (tabla `Backup` de un `main.db` de Titanium).
///
/// Si la tabla no existe devuelve un vector **vacío** (no es un error: hay
/// bases de datos de backup que no llevan índice).
pub fn leer_indice_sqlite(ruta: &Path) -> Result<Vec<FilaIndice>, String> {
    let conn = rusqlite::Connection::open_with_flags(
        ruta,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(|e| format!("no se pudo abrir {}: {e}", ruta.display()))?;
    let mut st = match conn.prepare(
        "SELECT packageName, versionName, hasApk, hasAppData, size FROM \"Backup\"",
    ) {
        Ok(s) => s,
        // Sin tabla `Backup` no hay índice: **vacío**, no error (hay bases de
        // datos de backup que no lo llevan).
        Err(_) => return Ok(Vec::new()),
    };
    let filas = st
        .query_map([], |f| {
            Ok(FilaIndice {
                package_name: f.get(0)?,
                version_name: f.get::<_, Option<String>>(1)?.unwrap_or_else(|| "-".to_string()),
                has_apk: f.get::<_, i64>(2)? != 0,
                has_app_data: f.get::<_, i64>(3)? != 0,
                size: f.get::<_, Option<i64>>(4)?.unwrap_or(0),
            })
        })
        .map_err(|e| format!("no se pudo leer el índice: {e}"))?;
    let mut out = Vec::new();
    for f in filas.flatten() {
        out.push(f);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Superficie MCP
// ---------------------------------------------------------------------------

/// Definiciones de las herramientas MCP de este bloque.
pub fn tool_definitions_mcp() -> Value {
    json!([
        {
            "name": "titanium_backup",
            "description": "Backups 'tar por paquete' (Titanium/Machiav3lli/Swift .dat): inventaria un directorio de backups, verifica el checksum de un TAR, extrae un paquete a un destino seguro y lee el indice SQLite de backups.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["list", "verify", "extract", "index"] },
                    "dir": { "type": "string", "description": "directorio con los backups (action=list)" },
                    "file": { "type": "string", "description": "fichero de backup (action=verify|extract)" },
                    "dest": { "type": "string", "description": "directorio de salida (action=extract)" },
                    "db": { "type": "string", "description": "main.db del indice (action=index)" }
                },
                "required": ["action"]
            }
        },
        {
            "name": "backup_catalog",
            "description": "Catalogo de backups de una carpeta de Google Drive: clasifica cada fichero por formato (Swift .dat/.app/.extra/.splits/.extdat/.med, Titanium .tar/.tib, SBA1) y resume bytes por clase SIN descargar nada.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["drive"] },
                    "folder": { "type": "string", "description": "id de la carpeta de Drive" },
                    "state_dir": { "type": "string", "description": "directorio de estado del daemon (credenciales)" },
                    "recursivo": { "type": "boolean", "description": "bajar tambien las subcarpetas" }
                },
                "required": ["action", "folder"]
            }
        }
    ])
}

/// Despacha las herramientas MCP de este bloque.
///
/// Devuelve `None` si el nombre no es de este bloque (para que el dispatcher
/// siga con su catálogo normal).
pub fn dispatch_mcp(name: &str, args: &Value) -> Option<Value> {
    let s = |k: &str| -> String {
        args.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string()
    };
    let r = match name {
        "titanium_backup" => {
            let action = s("action");
            let salida = match action.as_str() {
                "list" => {
                    let dir = s("dir");
                    match inventariar_directorio(Path::new(&dir)) {
                        Ok(v) => json!({"ok": true, "paquetes": v, "total": v.len()}),
                        Err(e) => json!({"ok": false, "error": e}),
                    }
                }
                "verify" => {
                    let file = s("file");
                    match verificar_fichero(Path::new(&file)) {
                        Ok(v) => json!({"ok": v.ok, "verificacion": v}),
                        Err(e) => json!({"ok": false, "error": e}),
                    }
                }
                "extract" => {
                    let file = s("file");
                    let dest = s("dest");
                    match extraer_paquete(Path::new(&file), Path::new(&dest)) {
                        Ok(v) => json!({"ok": v.ok, "resumen": v}),
                        Err(e) => json!({"ok": false, "error": e}),
                    }
                }
                "index" => {
                    let db = s("db");
                    match leer_indice_sqlite(Path::new(&db)) {
                        Ok(v) => json!({"ok": true, "entradas": v.len(), "indice": v}),
                        Err(e) => json!({"ok": false, "error": e}),
                    }
                }
                "" => json!({"ok": false, "error": "falta 'action'"}),
                otra => json!({"ok": false, "error": format!("acción desconocida: {otra}")}),
            };
            Some(salida)
        }
        "backup_catalog" => {
            let folder = s("folder");
            let state = args.get("state_dir").and_then(|v| v.as_str());
            let recursivo = args
                .get("recursivo")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let resultado = if recursivo {
                crate::backup_catalog::listar_drive_recursivo(&folder, state, 200, true)
            } else {
                crate::backup_catalog::listar_drive(&folder, state, 200)
            };
            Some(match resultado {
                Ok(v) => {
                    let r = crate::backup_catalog::resumir(&v);
                    json!({
                        "ok": true,
                        "carpeta": folder,
                        "ficheros": r.ficheros,
                        "bytes": r.bytes,
                        "bytes_legibles": crate::backup_catalog::bytes_legibles(r.bytes),
                        "por_clase": r.por_clase,
                    })
                }
                Err(e) => json!({"ok": false, "error": e}),
            })
        }
        _ => return None,
    };
    Some(r)
}

/// Ruta de un fichero dentro de un paquete ya extraído (comodidad).
pub fn ruta_extraida(destino: &Path, relativo: &str) -> PathBuf {
    destino.join(relativo)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detecta_el_formato_por_el_magic() {
        let mut tar = vec![0u8; 512];
        tar[257..263].copy_from_slice(MAGIC_USTAR);
        assert_eq!(detectar_formato(&tar), FormatoContenedor::Tar);
        assert_eq!(detectar_formato(b"PK\x03\x04no"), FormatoContenedor::Desconocido);
        assert_eq!(detectar_formato(b"nada"), FormatoContenedor::Desconocido);
    }

    #[test]
    fn sanea_rutas_y_rechaza_saltos() {
        assert_eq!(sanear_ruta("./databases/fixture.db").unwrap(), "databases/fixture.db");
        assert_eq!(sanear_ruta("files/leeme.txt").unwrap(), "files/leeme.txt");
        assert!(sanear_ruta("../escapado.txt").is_err());
        assert!(sanear_ruta("/absoluto").is_err());
        assert!(sanear_ruta("./").is_err());
    }

    #[test]
    fn lee_octal_y_base256() {
        assert_eq!(leer_octal(b"00000000001\0"), Some(1));
        assert_eq!(leer_octal(b"00000000000\0"), Some(0));
        assert_eq!(leer_octal(b"       "), Some(0));
    }
}

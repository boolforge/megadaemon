//! Consultas SQLite de solo lectura sobre bases de datos del dispositivo.
//!
//! Se usa para leer la base de datos de la app Magisk (Kitsune guarda ahi
//! la deny list y las politicas por paquete) sin depender de que exista un
//! `sqlite3` en el PATH del movil.

use rusqlite::OpenFlags;

/// Abre una base de datos en modo SOLO LECTURA. Nunca escribe.
fn open_ro(db: &str) -> Result<rusqlite::Connection, String> {
    rusqlite::Connection::open_with_flags(
        db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("no se pudo abrir {}: {}", db, e))
}

/// Devuelve la primera columna de cada fila como texto.
pub fn query_column(db: &str, sql: &str) -> Result<Vec<String>, String> {
    let conn = open_ro(db)?;
    let mut stmt = conn.prepare(sql).map_err(|e| format!("prepare: {}", e))?;
    let rows = stmt
        .query_map([], |r| r.get::<_, Option<String>>(0))
        .map_err(|e| format!("query: {}", e))?;
    let mut out = Vec::new();
    for r in rows.flatten() {
        if let Some(v) = r {
            out.push(v);
        }
    }
    Ok(out)
}

/// Devuelve pares de las dos primeras columnas de cada fila.
pub fn query_pairs(db: &str, sql: &str) -> Result<Vec<(String, String)>, String> {
    let conn = open_ro(db)?;
    let mut stmt = conn.prepare(sql).map_err(|e| format!("prepare: {}", e))?;
    let rows = stmt
        .query_map([], |r| {
            let a: Option<String> = r.get(0)?;
            let b: Option<String> = r.get(1)?;
            Ok((a.unwrap_or_default(), b.unwrap_or_default()))
        })
        .map_err(|e| format!("query: {}", e))?;
    let mut out = Vec::new();
    for r in rows.flatten() {
        out.push(r);
    }
    Ok(out)
}

/// Lista las tablas de una base de datos (util para descubrir el esquema).
pub fn tables(db: &str) -> Result<Vec<String>, String> {
    query_column(
        db,
        "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name",
    )
}

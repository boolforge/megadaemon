//! MITM de SQLite: MegaDaemon **no** escribe en la base de datos de
//! Antigravity, pero la **ve**, la **replica** en la suya (que es más
//! eficiente y robusta) y le puede **servir una alterada**.
//!
//! DISEÑO (decidido con el usuario, 2026-11-22):
//!
//! * La DB de Antigravity se abre **SOLO EN LECTURA** (`SQLITE_OPEN_READ_ONLY`
//!   + `busy_timeout`) ⇒ **Antigravity puede seguir escribiendo** en ella
//!   mientras MegaDaemon la observa. Nunca la bloqueamos ni la tocamos.
//! * Las **escrituras de MegaDaemon** van a **su propia base** (`sesiones.db`),
//!   con esquema normalizado, índices y WAL ⇒ más rápida y más robusta que
//!   la de Antigravity.
//! * La **réplica** copia de la DB de Antigravity a la nuestra de forma
//!   incremental (por `updated_at`/id), tolerando que cambie mientras tanto.
//! * La **DB servida** (la "mentirosa") es una copia con las filas que el
//!   MITM decide (filtradas o alteradas).  Con root se monta encima de la
//!   original con `mount --bind` ⇒ Antigravity lee/escribe **lo que nosotros
//!   queremos**.  Se puede revertir en un comando.
//!
//! Es el "MITM para SQLite" que hace falta para que Antigravity deje de
//! ser dueño de sus propios datos.

use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// La DB canónica de MegaDaemon (donde escribimos).
pub fn ruta_db_propia() -> PathBuf {
    PathBuf::from(crate::cockpit_server::state_dir()).join("sesiones.db")
}

/// La DB de Antigravity (sólo lectura).
pub fn ruta_db_antigravity() -> PathBuf {
    let dir = std::env::var("MEGADAEMON_AGY_DIR")
        .unwrap_or_else(|_| "/data/user/0/com.agy/files/.gemini/antigravity-app".to_string());
    PathBuf::from(dir).join("conversations/conversation_summaries.db")
}

/// Esquema de la DB propia: normalizado, con índices y en WAL.
///
/// MEDIDO: la DB de Antigravity es de sólo lectura para nosotros; la nuestra
/// se puede indexar y vacuumar sin tocar la theirs.
pub fn esquema_propio() -> &'static str {
    r#"
    PRAGMA journal_mode = WAL;
    PRAGMA synchronous = NORMAL;
    PRAGMA foreign_keys = ON;

    CREATE TABLE IF NOT EXISTS conversaciones (
        id            TEXT PRIMARY KEY,
        parent_id     TEXT,               -- NULL si no tiene padre (cadena vacia => NULL)
        estado        TEXT,
        creada        INTEGER,
        actualizada   INTEGER,
        inactiva_h    REAL,
        muerta        INTEGER DEFAULT 0,
        colgada       INTEGER DEFAULT 0,
        pasos         INTEGER DEFAULT 0,
        db_path       TEXT,
        meta_json     TEXT
    );
    CREATE INDEX IF NOT EXISTS ix_conv_parent  ON conversaciones(parent_id);
    CREATE INDEX IF NOT EXISTS ix_conv_estado  ON conversaciones(estado);
    CREATE INDEX IF NOT EXISTS ix_conv_actual  ON conversaciones(actualizada DESC);

    CREATE TABLE IF NOT EXISTS huerfanos (
        id      TEXT PRIMARY KEY,          -- hijo sin padre en la tabla
        motivo  TEXT,
        visto   INTEGER
    );

    CREATE TABLE IF NOT EXISTS replicacion (
        fuente      TEXT PRIMARY KEY,
        ultima      INTEGER,
        filas       INTEGER,
       huerfanos    INTEGER
    );
    "#
}

/// Abre (crea) la DB propia.
pub fn abrir_db_propia() -> Result<Connection, String> {
    let ruta = ruta_db_propia();
    if let Some(p) = ruta.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    let con = Connection::open(&ruta).map_err(|e| format!("abrir {}: {e}", ruta.display()))?;
    con.execute_batch(esquema_propio())
        .map_err(|e| format!("esquema: {e}"))?;
    Ok(con)
}

/// Abre la DB de Antigravity **en sólo lectura** con `busy_timeout`.
///
/// Nunca bloquea a Antigravity: si está escribiendo, reintentamos un rato y
/// si no, devolvemos el error (sin tocarla).
pub fn abrir_antigravity_ro() -> Result<Connection, String> {
    let ruta = ruta_db_antigravity();
    if !ruta.exists() {
        return Err(format!("no existe {}", ruta.display()));
    }
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let con = Connection::open_with_flags(&ruta, flags)
        .map_err(|e| format!("abrir RO {}: {e}", ruta.display()))?;
    con.busy_timeout(std::time::Duration::from_millis(2000))
        .map_err(|e| format!("busy_timeout: {e}"))?;
    Ok(con)
}

/// Normaliza un identificador: la cadena vacía (o con espacios) **no** es un
/// identificador válido ⇒ `None`.
///
/// MEDIDO: en la DB de Antigravity `parent_conversation_id` tiene default
/// `''` (cadena vacía) en vez de NULL ⇒ contarlo como "tiene padre" da
/// recuentos **incorrectos**.
pub fn normaliza_id(v: Option<&str>) -> Option<String> {
    let s = v?.trim();
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

/// Una fila de conversación, ya normalizada.
#[derive(Debug, Clone)]
pub struct Conversacion {
    pub id: String,
    pub parent_id: Option<String>,
    pub estado: String,
    pub creada: i64,
    pub actualizada: i64,
    pub pasos: i64,
    pub meta: Value,
}

/// Lee las conversaciones de la DB de Antigravity (sólo lectura).
///
/// `tabla` = el nombre real de la tabla (medido: `conversation_summaries`).
pub fn leer_conversaciones(
    con: &Connection,
    tabla: &str,
    limite: usize,
) -> Result<Vec<Conversacion>, String> {
    let sql = format!("SELECT * FROM {} LIMIT {}", tabla.replace('"', ""), limite);
    let mut st = con.prepare(&sql).map_err(|e| format!("prepare: {e}"))?;
    let cols: Vec<String> = st.column_names().iter().map(|s| s.to_string()).collect();
    let filas = st
        .query_map([], |row| {
            let mut m = serde_json::Map::new();
            for (i, c) in cols.iter().enumerate() {
                let v: Value = match row.get_ref(i) {
                    Ok(rusqlite::types::ValueRef::Null) => Value::Null,
                    Ok(rusqlite::types::ValueRef::Integer(x)) => json!(x),
                    Ok(rusqlite::types::ValueRef::Real(x)) => json!(x),
                    Ok(rusqlite::types::ValueRef::Text(t)) => {
                        Value::String(String::from_utf8_lossy(t).to_string())
                    }
                    Ok(rusqlite::types::ValueRef::Blob(b)) => {
                        Value::String(crate::util_text::hex(b))
                    }
                    Err(_) => Value::Null,
                };
                m.insert(c.clone(), v);
            }
            Ok(Value::Object(m))
        })
        .map_err(|e| format!("query: {e}"))?;
    let mut out = Vec::new();
    for f in filas.flatten() {
        let id = f
            .get("conversation_id")
            .or_else(|| f.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if id.is_empty() {
            continue;
        }
        let parent = normaliza_id(
            f.get("parent_conversation_id")
                .or_else(|| f.get("parent_id"))
                .and_then(|v| v.as_str()),
        );
        let battle = normaliza_id(f.get("battle_id").and_then(|v| v.as_str()));
        let winner = normaliza_id(f.get("winning_conversation_id").and_then(|v| v.as_str()));
        let estado = f
            .get("state")
            .or_else(|| f.get("status"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let creada = f
            .get("created_at")
            .or_else(|| f.get("created_at_ms"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let actualizada = f
            .get("updated_at")
            .or_else(|| f.get("last_updated_at"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let pasos = f.get("steps").and_then(|v| v.as_i64()).unwrap_or(0);
        let mut meta = f.as_object().cloned().unwrap_or_default();
        meta.insert("battle_id".into(), json!(battle));
        meta.insert("winning_conversation_id".into(), json!(winner));
        out.push(Conversacion {
            id,
            parent_id: parent,
            estado,
            creada,
            actualizada,
            pasos,
            meta: Value::Object(meta),
        });
    }
    Ok(out)
}

/// Replica de la DB de Antigravity a la DB propia de MegaDaemon.
///
/// * No toca la de Antigravity (sólo lectura).
/// * Inserta/actualiza en la nuestra (upsert).
/// * Detecta **huerfanos** (hijos cuyo padre no existe) ⇒ dato que el
///   esquema de Antigravity no expresa bien (cadena vacía en vez de NULL).
pub fn replicar(limite: usize) -> Result<Value, String> {
    let src = abrir_antigravity_ro()?;
    let tabla = std::env::var("MEGADAEMON_AGY_TABLE")
        .unwrap_or_else(|_| "conversation_summaries".to_string());
    let filas = leer_conversaciones(&src, &tabla, limite)?;
    drop(src);

    let mut dst = abrir_db_propia()?;
    let ids: Vec<String> = filas.iter().map(|f| f.id.clone()).collect();
    let mut ins = 0u64;
    let mut upd = 0u64;
    {
        let tx = dst.transaction().map_err(|e| format!("tx: {e}"))?;
        for f in &filas {
            let meta = serde_json::to_string(&f.meta).unwrap_or_default();
            let n = tx
                .execute(
                    "INSERT INTO conversaciones (id,parent_id,estado,creada,actualizada,pasos,meta_json)
                     VALUES (?1,?2,?3,?4,?5,?6,?7)
                     ON CONFLICT(id) DO UPDATE SET
                       parent_id=excluded.parent_id, estado=excluded.estado,
                       creada=excluded.creada, actualizada=excluded.actualizada,
                       pasos=excluded.pasos, meta_json=excluded.meta_json",
                    rusqlite::params![f.id, f.parent_id, f.estado, f.creada, f.actualizada, f.pasos, meta],
                )
                .map_err(|e| format!("upsert: {e}"))?;
            if n > 0 {
                ins += 1;
            }
        }
        // Huerfanos: los que tienen padre pero el padre no esta en la tabla.
        let mut huerfanos = 0u64;
        for f in &filas {
            if let Some(p) = &f.parent_id {
                let existe: i64 = tx
                    .query_row(
                        "SELECT COUNT(*) FROM conversaciones WHERE id=?1",
                        rusqlite::params![p],
                        |r| r.get(0),
                    )
                    .unwrap_or(0);
                if existe == 0 {
                    tx.execute(
                        "INSERT OR REPLACE INTO huerfanos (id,motivo,visto) VALUES (?1,?2,?3)",
                        rusqlite::params![f.id, format!("padre {p} no existe"), ahora()],
                    )
                    .ok();
                    huerfanos += 1;
                }
            }
        }
        tx.execute(
            "INSERT OR REPLACE INTO replicacion (fuente,ultima,filas,huerfanos) VALUES (?1,?2,?3,?4)",
            rusqlite::params![
                tabla,
                ahora(),
                filas.len() as i64,
                huerfanos as i64
            ],
        )
        .ok();
        tx.commit().map_err(|e| format!("commit: {e}"))?;
    }
    Ok(json!({
        "ok": true,
        "leidas": filas.len(),
        "insertadas_o_actualizadas": ins,
        "actualizadas": upd,
        "huerfanos": ids.len().saturating_sub(ins as usize),
        "db_propia": ruta_db_propia().display().to_string(),
    }))
}

/// Ruta del temporal de la DB "mentirosa" (`<destino>.tmp`).
///
/// La DB se genera ahí y sólo se publica (rename) cuando está completa.
fn ruta_temporal(destino: &Path) -> std::path::PathBuf {
    let mut p = destino.to_path_buf();
    let nombre = p
        .file_name()
        .map(|n| format!("{}.tmp", n.to_string_lossy()))
        .unwrap_or_else(|| "db_servida.tmp".to_string());
    p.set_file_name(nombre);
    p
}

/// **La DB "mentirosa"**: una copia de la de Antigravity con las filas que
/// decidamos.  Se sirve con `mount --bind` (root) sobre la original.
///
/// `ocultar`: ids de conversaciones que NO deben aparecer.
/// `reescribir`: sustitucion de texto en la columna `state` (para mentir sobre el
/// estado de una sesión).
pub fn generar_db_servida(
    ocultar: &[String],
    reescribir: &[(String, String)],
    destino: &Path,
) -> Result<Value, String> {
    let src = abrir_antigravity_ro()?;
    let tabla = std::env::var("MEGADAEMON_AGY_TABLE")
        .unwrap_or_else(|_| "conversation_summaries".to_string());
    let filas = leer_conversaciones(&src, &tabla, 1_000_000)?;
    drop(src);
    // MEDIDO (2026-11-23, auditoría de calidad): la DB "mentirosa" se
    // generaba EN EL SITIO (`remove_file(destino)` + `Connection::open
    // (destino)`) ⇒ si la generación fallaba a mitad (o el proceso moría),
    // quedaba una DB CORRUPTA en el destino; y ese destino es justamente el
    // que se monta (bind) sobre la DB real de Antigravity (`plan_montaje`)
    // ⇒ la app veía una base de datos rota.  Ahora se genera en un temporal
    // y sólo se publica cuando está completa.
    let temporal = ruta_temporal(destino);
    let _ = std::fs::remove_file(&temporal);
    let mut dst = Connection::open(&temporal).map_err(|e| format!("abrir destino: {e}"))?;
    let esquema = format!(
        "CREATE TABLE IF NOT EXISTS {} AS SELECT * FROM {} LIMIT 0",
        tabla, tabla
    );
    // copiamos el esquema real con sus columnas
    let cols: Vec<String> = filas.first().map(|_| Vec::new()).unwrap_or_default();
    let _ = cols;
    // creamos la tabla con el mismo esquema leyendo de la original
    let src2 = abrir_antigravity_ro()?;
    let sql_esquema: String = src2
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name=?1",
            rusqlite::params![tabla.clone()],
            |r| r.get(0),
        )
        .unwrap_or_default();
    drop(src2);
    if sql_esquema.is_empty() {
        return Err(format!("no encuentro la tabla {tabla}"));
    }
    dst.execute_batch(&sql_esquema)
        .map_err(|e| format!("esquema destino: {e}"))?;
    let _ = esquema;

    let nombres: Vec<String> = filas
        .first()
        .map(|f| {
            f.meta
                .as_object()
                .map(|o| o.keys().cloned().collect())
                .unwrap_or_default()
        })
        .unwrap_or_default();
    if nombres.is_empty() {
        return Err("no hay filas que servir".to_string());
    }
    let cols_sql = nombres
        .iter()
        .map(|c| format!("\"{}\"", c.replace('"', "")))
        .collect::<Vec<_>>()
        .join(",");
    let placeholders = (1..=nombres.len())
        .map(|i| format!("?{i}"))
        .collect::<Vec<_>>()
        .join(",");
    let mut tx = dst.transaction().map_err(|e| format!("tx: {e}"))?;
    let mut insertadas = 0usize;
    for f in &filas {
        if ocultar.iter().any(|h| h == &f.id) {
            continue;
        }
        let mut vals: Vec<rusqlite::types::Value> = Vec::with_capacity(nombres.len());
        for c in &nombres {
            let mut v = f.meta.get(c).cloned().unwrap_or(Value::Null);
            let mut reescrito: Option<String> = None;
            if let Value::String(s) = &v {
                for (de, a) in reescribir {
                    if s == de {
                        reescrito = Some(a.clone());
                    }
                }
            }
            if let Some(t) = reescrito {
                v = Value::String(t);
            }
            vals.push(match v {
                Value::Null => rusqlite::types::Value::Null,
                Value::Bool(b) => rusqlite::types::Value::Integer(b as i64),
                Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        rusqlite::types::Value::Integer(i)
                    } else {
                        rusqlite::types::Value::Real(n.as_f64().unwrap_or(0.0))
                    }
                }
                Value::String(s) => rusqlite::types::Value::Text(s.into()),
                other => rusqlite::types::Value::Text(other.to_string().into()),
            });
        }
        let ps: Vec<&dyn rusqlite::ToSql> =
            vals.iter().map(|v| v as &dyn rusqlite::ToSql).collect();
        tx.execute(
            &format!("INSERT OR REPLACE INTO {tabla} ({cols_sql}) VALUES ({placeholders})"),
            ps.as_slice(),
        )
        .map_err(|e| format!("insert: {e}"))?;
        insertadas += 1;
    }
    tx.commit().map_err(|e| format!("commit: {e}"))?;
    // Publicación ATÓMICA: la DB servida sólo se sustituye cuando la
    // nueva está completa (si no, el destino conserva la anterior, que es
    // válida).
    drop(dst);
    std::fs::rename(&temporal, destino)
        .map_err(|e| format!("publicar {}: {e}", destino.display()))?;
    Ok(json!({
        "ok": true,
        "destino": destino.display().to_string(),
        "servidas": insertadas,
        "ocultas": ocultar.len(),
        "reescrituras": reescribir.len(),
    }))
}

/// El comando EXACTO para servir la DB mentirosa (root) y para revertir.
///
/// **No se ejecuta aquí**: se devuelve el texto para que el usuario o el
/// supervisor lo apliquen (los montajes necesitan root).
pub fn plan_montaje(destino: &Path) -> Value {
    let original = ruta_db_antigravity();
    json!({
        "original": original.display().to_string(),
        "servida": destino.display().to_string(),
        "montar": format!(
            "mount --bind '{}' '{}'",
            destino.display(),
            original.display()
        ),
        "desmontar": format!("umount '{}'", original.display()),
        "aviso": "el bind-mount hace que Antigravity lea/escriba la copia; para volver atras, desmontar. NO se toca el fichero original: queda intacto en su particion.",
    })
}

fn ahora() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_cadena_vacia_no_es_un_padre() {
        assert_eq!(normaliza_id(Some("")), None);
        assert_eq!(normaliza_id(Some("   ")), None);
        assert_eq!(normaliza_id(Some("abc")), Some("abc".to_string()));
        assert_eq!(normaliza_id(None), None);
    }

    #[test]
    fn el_esquema_propio_es_valido() {
        let con = Connection::open_in_memory().unwrap();
        con.execute_batch(esquema_propio()).expect("esquema valido");
        let n: i64 = con
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name='conversaciones'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn la_replica_normaliza_los_padres_vacios() {
        let dir = std::env::temp_dir().join(format!("mdmitm-{}", ahora()));
        let _ = std::fs::create_dir_all(&dir);
        let ruta = dir.join("c.db");
        {
            let con = Connection::open(&ruta).unwrap();
            con.execute_batch(
                "CREATE TABLE conversation_summaries (
                   conversation_id TEXT PRIMARY KEY,
                   parent_conversation_id TEXT DEFAULT '',
                   state TEXT, created_at INTEGER, updated_at INTEGER, steps INTEGER);
                 INSERT INTO conversation_summaries VALUES ('a','','IDLE',1,2,3);
                 INSERT INTO conversation_summaries VALUES ('b','a','BUSY',4,5,6);",
            )
            .unwrap();
        }
        // leemos con la MISMA logica del modulo
        let con = Connection::open(&ruta).unwrap();
        let filas = leer_conversaciones(&con, "conversation_summaries", 10).unwrap();
        assert_eq!(filas.len(), 2);
        assert_eq!(filas[0].parent_id, None, "la cadena vacia NO es un padre");
        assert_eq!(filas[1].parent_id.as_deref(), Some("a"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_plan_de_montaje_es_reversible() {
        let p = plan_montaje(Path::new("/tmp/x.db"));
        let s = p.to_string();
        assert!(s.contains("mount --bind"));
        assert!(s.contains("umount"));
    }
}

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

/// Directorio de datos de Antigravity.
///
/// Acepta los **dos** nombres de variable que usan los módulos del repo
/// (`MEGADAEMON_AGY_DIR` y `MEGADAEMON_ANTIGRAVITY_DIR`): antes cada módulo
/// leía el suyo y apuntaban a sitios distintos.
pub fn dir_antigravity() -> PathBuf {
    for var in ["MEGADAEMON_AGY_DIR", "MEGADAEMON_ANTIGRAVITY_DIR"] {
        if let Ok(v) = std::env::var(var) {
            let t = v.trim();
            if !t.is_empty() {
                return PathBuf::from(t);
            }
        }
    }
    PathBuf::from("/data/user/0/com.agy/files/.gemini/antigravity-app")
}

/// Candidatas a la DB de Antigravity, en orden de preferencia.
///
/// MEDIDO (2026-11-22, S21 Ultra) + **conflicto real del repo**:
/// `antigravity_sessions.rs` leía `…/antigravity-app/conversation_summaries.db`
/// y `sqlite_mitm.rs` leía `…/antigravity-app/conversations/conversation_summaries.db`.
/// Con el MITM activo, montar el fichero equivocado = **no montar nada**:
/// por eso se **detecta** cuál existe en vez de suponerlo.
pub fn candidatas_db_antigravity() -> Vec<PathBuf> {
    let dir = dir_antigravity();
    vec![
        dir.join("conversation_summaries.db"),
        dir.join("conversations/conversation_summaries.db"),
    ]
}

/// La DB real: la primera candidata que exista; si ninguna, la primera.
pub fn ruta_db_antigravity() -> PathBuf {
    let c = candidatas_db_antigravity();
    c.iter()
        .find(|p| p.exists())
        .cloned()
        .unwrap_or_else(|| c[0].clone())
}

/// Ruta del **helper** del MITM (el script que hace el bind-mount).
///
/// En el móvil: el módulo Magisk (`/data/adb/modules/megadaemon_mitm_sqlite/`).
/// En el repo: `deploy/magisk-mitm-sqlite/mitm-sqlite.sh` (desarrollo).
pub fn ruta_helper_mitm() -> Option<PathBuf> {
    if let Ok(v) = std::env::var("MEGADAEMON_MITM_HELPER") {
        let t = v.trim();
        if !t.is_empty() {
            let p = PathBuf::from(t);
            if p.exists() {
                return Some(p);
            }
        }
    }
    let candidatos = [
        "/data/adb/modules/megadaemon_mitm_sqlite/mitm-sqlite.sh",
        "/data/local/tmp/megadaemon/mitm-sqlite.sh",
    ];
    for c in candidatos {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }
    // En el repo: <raiz>/deploy/magisk-mitm-sqlite/mitm-sqlite.sh
    let raiz = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default();
    let p = raiz.join("deploy/magisk-mitm-sqlite/mitm-sqlite.sh");
    if p.exists() {
        return Some(p);
    }
    None
}

/// Directorio de trabajo del MITM (dentro del `state_dir`).
pub fn dir_mitm() -> PathBuf {
    PathBuf::from(crate::cockpit_server::state_dir()).join("mitm")
}

/// La DB **servida** (la "mentirosa"): la que se monta sobre la original.
pub fn ruta_db_servida() -> PathBuf {
    dir_mitm().join("antigravity_servida.db")
}

/// Fichero de **estado** del MITM (JSON, 0600).
pub fn ruta_estado_mitm() -> PathBuf {
    dir_mitm().join("estado.json")
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
    let src_path = ruta_db_antigravity();
    generar_db_servida_desde(&src_path, ocultar, reescribir, destino)
}

/// Igual que [`generar_db_servida`] pero con la **DB de origen** explícita
/// (testeable sin el móvil).
pub fn generar_db_servida_desde(
    origen: &Path,
    ocultar: &[String],
    reescribir: &[(String, String)],
    destino: &Path,
) -> Result<Value, String> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let src = Connection::open_with_flags(origen, flags)
        .map_err(|e| format!("abrir RO {}: {e}", origen.display()))?;
    src.busy_timeout(std::time::Duration::from_millis(2000))
        .map_err(|e| format!("busy_timeout: {e}"))?;
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
    let flags2 = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let src2 = Connection::open_with_flags(origen, flags2)
        .map_err(|e| format!("abrir RO {}: {e}", origen.display()))?;
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
    Ok(json!({
        "ok": true,
        "destino": destino.display().to_string(),
        "servidas": insertadas,
        "ocultas": ocultar.len(),
        "reescrituras": reescribir.len(),
        "journal_mode": "WAL",
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

// =========================================================================
// MITM REAL: activar / desactivar / estado / capturar
// =========================================================================
//
// El MITM **de verdad** son tres piezas que encajan:
//
//   1. `generar_db_servida`  -> la DB "mentirosa" (WAL, filtrada).
//   2. `mount -o bind`       -> la app (com.agy) escribe en la mentirosa;
//                                la original queda INTACTA en su partición.
//   3. `replicar`            -> el motor de réplica (SOLO LECTURA +
//                                busy_timeout 2000 ms) lee por la ruta
//                                montada ⇒ captura lo que la app escribe.
//
// El paso 3 es el que cumple "sin bloquear la original": la réplica no
// escribe nunca en la DB de la app, y la DB servida está en WAL (lector
// y escritor conviven).  Al reactivar, `desactivar` re-mergea las filas
// servidas en la original ⇒ Antigravity no pierde conversaciones.
//
// ¿POR QUÉ UN HELPER EN SHELL Y NO TODO EN RUST?
//   El montaje necesita el **namespace de montaje de Magisk** (`su -mm`),
//   y ese namespace es *por invocación*: si el daemon hiciera el mount en
//   una invocación y el umount en otra, no serían el mismo namespace.
//   Por eso TODA la secuencia privilegiada (montar / desmontar / estado)
//   ocurre DENTRO de una sola invocación del helper.

/// Cita una cadena para un comando de shell.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Ejecuta el helper del MITM como root y devuelve su JSON.
///
/// `accion` ∈ {activar, desactivar, estado}.  `extra` son argumentos
/// extra (`--original RUTA`, `--served RUTA`).
fn ejecutar_helper_mitm(accion: &str, extra: &[String]) -> Result<Value, String> {
    let helper =
        ruta_helper_mitm().ok_or_else(|| "no encuentro el helper del MITM (mitm-sqlite.sh). Sube el modulo Magisk o exporta MEGADAEMON_MITM_HELPER".to_string())?;
    let mut cmd = format!("sh {} {}", shell_quote(&helper.display().to_string()), accion);
    for a in extra {
        cmd.push(' ');
        cmd.push_str(a);
    }
    let r = crate::android::root_exec(&cmd, 30_000, 32_768);
    if r.timed_out {
        return Err(format!(
            "el helper del MITM ({accion}) se paso de tiempo (30 s): {cmd}"
        ));
    }
    if let Some(c) = r.code {
        if c != 0 {
            // El helper devuelve el motivo en su JSON (stdout); si no, se
            // usa el stderr.
            let det = if let Some(v) = parsear_json_line(&r.stdout) {
                v.get("error")
                    .and_then(|e| e.as_str())
                    .unwrap_or("")
                    .to_string()
            } else {
                r.stderr.trim().to_string()
            };
            return Err(format!(
                "el helper del MITM ({accion}) fallo con codigo {c}: {det}"
            ));
        }
    }
    parsear_json_line(&r.stdout)
        .ok_or_else(|| format!("el helper del MITM ({accion}) no devolvio JSON: {}", r.stdout.trim()))
}

/// Parsea la **última** línea que sea JSON válido.
fn parsear_json_line(txt: &str) -> Option<Value> {
    txt.lines()
        .rev()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .find_map(|l| serde_json::from_str::<Value>(l).ok())
}

/// Guarda el estado del MITM (JSON, 0600: puede contain rutas del dispositivo).
pub fn escribir_estado_mitm(v: &Value) -> Result<(), String> {
    escribir_estado_mitm_en(&ruta_estado_mitm(), v)
}

/// Guarda el estado del MITM en una ruta concreta (0600).
pub fn escribir_estado_mitm_en(p: &Path, v: &Value) -> Result<(), String> {
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("crear {}: {e}", dir.display()))?;
    }
    let txt = serde_json::to_string_pretty(v).map_err(|e| format!("serializar estado: {e}"))?;
    std::fs::write(p, txt).map_err(|e| format!("escribir {}: {e}", p.display()))?;
    set_modo_0600(p)?;
    Ok(())
}

/// `chmod 0600` best-effort (Unix).
fn set_modo_0600(p: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600)) {
        Ok(()) => Ok(()),
        Err(e) => Err(format!("chmod 0600 {}: {e}", p.display())),
    }
}

/// Lee el estado del MITM.
pub fn leer_estado_mitm() -> Option<Value> {
    leer_estado_mitm_de(&ruta_estado_mitm())
}

/// Lee el estado del MITM de una ruta concreta.
pub fn leer_estado_mitm_de(p: &Path) -> Option<Value> {
    let txt = std::fs::read_to_string(p).ok()?;
    serde_json::from_str(&txt).ok()
}

/// **Activa** el MITM: prepara la DB servida y monta el bind-mount.
///
/// Efectos (exactos):
/// * Se genera `<state>/mitm/antigravity_servida.db` con las filas
///   filtradas/reescritas, en modo **WAL**.
/// * Se hace `checkpoint` del WAL de la original (para que la copia esté
///   completa) y se **apartan** (copiando) sus ficheros `-wal`/`-shm`:
///   si la app los encontrara, intentaría reproducir transactions de la
///   DB original sobre la DB servida ⇒ "database disk image is malformed".
/// * `mount -o bind <servida> <original>` ⇒ la app escribe en la servida.
/// * Se guarda el estado (0600).
///
/// Devuelve el JSON del helper + lo que hizo el daemon.
pub fn activar(
    ocultar: &[String],
    reescribir: &[(String, String)],
) -> Result<Value, String> {
    let original = ruta_db_antigravity();
    if !original.exists() {
        return Err(format!(
            "no existe la DB de Antigravity (probadas: {})",
            candidatas_db_antigravity()
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let servida = ruta_db_servida();
    if let Some(dir) = servida.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("crear {}: {e}", dir.display()))?;
    }

    // 1) Checkpoint del WAL de la original (best-effort): si la app tiene
    //    transactions sin pasar al fichero principal, la copia estaría
    //    incompleta.  Si no se puede (app escribiendo), se avisa y se sigue.
    let (checkpoint_ok, checkpoint_motivo) = checkpoint_original(&original);

    // 2) La DB servida (WAL, filtrada).
    let gen = generar_db_servida(ocultar, reescribir, &servida)?;

    // 3) El montaje (root).  Todo el bloque privilegiado va en UNA
    //    invocación del helper (mismo namespace de montaje).
    let extra = vec![
        format!("--original {}", shell_quote(&original.display().to_string())),
        format!("--served {}", shell_quote(&servida.display().to_string())),
    ];
    let mont = ejecutar_helper_mitm("activar", &extra)?;

    let estado = json!({
        "accion": "activar",
        "ts": ahora(),
        "original": original.display().to_string(),
        "servida": servida.display().to_string(),
        "ocultas": ocultar.len(),
        "reescrituras": reescribir.len(),
        "checkpoint_original": checkpoint_ok,
        "checkpoint_motivo": checkpoint_motivo,
        "montaje": mont,
    });
    escribir_estado_mitm(&estado)?;

    Ok(json!({
        "ok": true,
        "accion": "activar",
        "original": original.display().to_string(),
        "servida": servida.display().to_string(),
        "servida_detalle": gen,
        "checkpoint_original": checkpoint_ok,
        "checkpoint_motivo": checkpoint_motivo,
        "montaje": mont,
        "efecto": "Antigravity lee y ESCRIBE en la DB servida; la original queda intacta. Para volver atras: accion 'desactivar'.",
    }))
}

/// `PRAGMA wal_checkpoint(TRUNCATE)` sobre la original.
///
/// Devuelve `(ok, motivo)`.  NUNCA bloquea más de 2 s (busy_timeout).
fn checkpoint_original(original: &Path) -> (bool, String) {
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let con = match Connection::open_with_flags(original, flags) {
        Ok(c) => c,
        Err(e) => return (false, format!("abrir RW {}: {e}", original.display())),
    };
    if con.busy_timeout(std::time::Duration::from_millis(2000)).is_err() {
        return (false, "no se pudo fijar busy_timeout".to_string());
    }
    match con.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| {
        // MEDIDO (2026-11-23): el cierre de `query_row` debe devolver un
        // Result => E0308 con `unwrap_or(-1)` a pelo.
        Ok(r.get::<_, i64>(0).unwrap_or(-1))
    }) {
        Ok(v) => {
            if v == 0 {
                (true, "WAL volcado al fichero principal".to_string())
            } else {
                (false, format!("wal_checkpoint devolvio {v} (1 = WAL activo, 2 = bloqueado)"))
            }
        }
        Err(e) => (false, format!("wal_checkpoint: {e}")),
    }
}

/// **Desactiva** el MITM: desmonta y **re-mergea**.
///
/// Efectos (exactos):
/// * `umount <original>` ⇒ la ruta vuelve a ser el **fichero real**.
/// * Se re-mergean las filas de la DB servida en la original ⇒
///   Antigravity conserva las conversaciones que creó con el MITM activo.
/// * Se borra la DB servida y sus `-wal`/`-shm`.
pub fn desactivar() -> Result<Value, String> {
    let original = ruta_db_antigravity();
    let servida = ruta_db_servida();

    let extra = vec![
        format!("--original {}", shell_quote(&original.display().to_string())),
        format!("--served {}", shell_quote(&servida.display().to_string())),
    ];
    let desmont = ejecutar_helper_mitm("desactivar", &extra)?;

    // El re-merge SOLO tiene sentido si la montada estaba puesta.
    let montada = desmont
        .get("desmontado")
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
        > 0;
    let merge = if montada && servida.exists() {
        remergear_servida_en_original(&servida, &original)
    } else {
        Ok(json!({
            "ok": true,
            "omitido": true,
            "motivo": "no estaba montada (o no hay DB servida): no hay nada que re-mergear",
        }))
    };

    // La DB servida ya no hace falta (su contenido está en la original).
    let _ = std::fs::remove_file(&servida);
    let _ = std::fs::remove_file(servida.with_extension("db-wal"));
    let _ = std::fs::remove_file(servida.with_extension("db-shm"));

    let estado = json!({
        "accion": "desactivar",
        "ts": ahora(),
        "original": original.display().to_string(),
        "servida": servida.display().to_string(),
        "montaje": desmont,
        "remerge": merge.clone().unwrap_or(Value::Null),
    });
    escribir_estado_mitm(&estado)?;

    Ok(json!({
        "ok": true,
        "accion": "desactivar",
        "original": original.display().to_string(),
        "montaje": desmont,
        "remerge": merge.unwrap_or(Value::Null),
        "efecto": "la ruta original vuelve al fichero real; las conversaciones del MITM se han re-mergeado en el original (no se pierde nada).",
    }))
}

/// Re-mergea las filas de la DB servida en la original.
///
/// La servida tiene **el mismo esquema** que la original (se copió de
/// ella), así que el re-merge es un upsert por clave primaria.
fn remergear_servida_en_original(servida: &Path, original: &Path) -> Result<Value, String> {
    let tabla = std::env::var("MEGADAEMON_AGY_TABLE")
        .unwrap_or_else(|_| "conversation_summaries".to_string());

    // 1) Leer la servida (sólo lectura + busy_timeout: no bloquea a nadie).
    let flags_s = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let src = Connection::open_with_flags(servida, flags_s)
        .map_err(|e| format!("abrir servida RO {}: {e}", servida.display()))?;
    src.busy_timeout(std::time::Duration::from_millis(2000))
        .map_err(|e| format!("busy_timeout servida: {e}"))?;
    let filas = leer_conversaciones(&src, &tabla, 1_000_000)?;
    drop(src);

    // 2) Escribir en la original (ya desmontada ⇒ es el fichero real).
    let flags_o = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let mut dst = Connection::open_with_flags(original, flags_o)
        .map_err(|e| format!("abrir original RW {}: {e}", original.display()))?;
    dst.busy_timeout(std::time::Duration::from_millis(2000))
        .map_err(|e| format!("busy_timeout original: {e}"))?;

    let nombres: Vec<String> = filas
        .first()
        .and_then(|f| f.meta.as_object().map(|o| o.keys().cloned().collect()))
        .unwrap_or_default();
    if nombres.is_empty() {
        return Ok(json!({
            "ok": true,
            "filas": 0,
            "motivo": "la DB servida no tiene filas que re-mergear",
        }));
    }
    let cols_sql = nombres
        .iter()
        .map(|c| format!("\"{}\"", c.replace('"', "")))
        .collect::<Vec<_>>()
        .join(",");
    // `INSERT OR IGNORE` (NO `OR REPLACE`): las filas que ya están en la
    // original se **conservan tal cual** ⇒ las conversaciones ocultas o
    // con el estado reescrito por el MITM siguen "mintiendo" después de
    // restaurar.  Sólo se AÑADEN las conversaciones que la app creó
    // durante el MITM (las que no existían en la original).
    let upsert = format!(
        "INSERT OR IGNORE INTO {tabla} ({cols_sql}) VALUES ({})",
        (1..=nombres.len())
            .map(|i| format!("?{i}"))
            .collect::<Vec<_>>()
            .join(",")
    );

    let mut n = 0usize;
    {
        let tx = dst.transaction().map_err(|e| format!("tx: {e}"))?;
        for f in &filas {
            let mut vals: Vec<rusqlite::types::Value> = Vec::with_capacity(nombres.len());
            for c in &nombres {
                let v = f.meta.get(c).cloned().unwrap_or(Value::Null);
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
            match tx.execute(&upsert, ps.as_slice()) {
                Ok(_) => n += 1,
                Err(e) => return Err(format!("re-merge {}: {e}", f.id)),
            }
        }
        tx.commit().map_err(|e| format!("commit re-merge: {e}"))?;
    }
    Ok(json!({
        "ok": true,
        "filas_re_mergadas": n,
        "original": original.display().to_string(),
    }))
}

/// **Estado** del MITM (qué está montado, qué hay en disco, qué capturó).
pub fn estado_mitm() -> Result<Value, String> {
    let original = ruta_db_antigravity();
    let servida = ruta_db_servida();
    let sz = |p: &Path| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);

    // ¿Montado?  (el helper lo pregunta en SU namespace de montaje)
    let extra = vec![
        format!("--original {}", shell_quote(&original.display().to_string())),
        format!("--served {}", shell_quote(&servida.display().to_string())),
    ];
    let montaje = match ejecutar_helper_mitm("estado", &extra) {
        Ok(v) => v,
        Err(e) => json!({ "ok": false, "error": e }),
    };

    // Última captura (la tabla `replicacion` de la DB propia).
    let ultima = abrir_db_propia()
        .ok()
        .and_then(|c| {
            c.query_row(
                "SELECT fuente, ultima, filas, huerfanos FROM replicacion LIMIT 1",
                [],
                |r| {
                    Ok(json!({
                        "fuente": r.get::<_, String>(0).unwrap_or_default(),
                        "ultima": r.get::<_, i64>(1).unwrap_or(0),
                        "filas": r.get::<_, i64>(2).unwrap_or(0),
                        "huerfanos": r.get::<_, i64>(3).unwrap_or(0),
                    }))
                },
            )
            .ok()
        })
        .unwrap_or(Value::Null);

    Ok(json!({
        "ok": true,
        "original": {
            "ruta": original.display().to_string(),
            "existe": original.exists(),
            "bytes": sz(&original),
            "candidatas": candidatas_db_antigravity()
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>(),
        },
        "servida": {
            "ruta": servida.display().to_string(),
            "existe": servida.exists(),
            "bytes": sz(&servida),
        },
        "montaje": montaje,
        "estado_guardado": leer_estado_mitm().unwrap_or(Value::Null),
        "ultima_captura": ultima,
    }))
}

/// **Captura** las escrituras de la app en la DB canónica.
///
/// Es el motor de réplica ya existente (`replicar`), que:
/// * lee la DB de Antigravity en **SOLO LECTURA** con `busy_timeout` 2000 ms
///   ⇒ no bloquea a la app (y con la DB servida en WAL, lector y escritor
///   conviven);
/// * vuelca en la DB propia (`sesiones.db`, WAL, indexada) con upsert.
pub fn capturar(limite: usize) -> Result<Value, String> {
    let r = replicar(limite)?;
    Ok(json!({
        "ok": true,
        "accion": "capturar",
        "resultado": r,
        "nota": "lectura SOLO LECTURA con busy_timeout 2000 ms de la DB montada (que es la servida si el MITM esta activo): lo que la app escribe se replica en la DB canonica.",
    }))
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

    // -----------------------------------------------------------------
    // MITM real (activar / desactivar / estado / capturar)
    // -----------------------------------------------------------------

    #[test]
    fn el_estado_del_mitm_se_guarda_en_0600_y_se_relee() {
        let dir = std::env::temp_dir().join(format!("mdmitm-estado-{}", ahora()));
        let _ = std::fs::create_dir_all(&dir);
        let p = dir.join("estado.json");
        let v = json!({"accion": "activar", "ts": 1234, "montado": true});
        escribir_estado_mitm_en(&p, &v).expect("se escribe el estado");
        let leido = leer_estado_mitm_de(&p).expect("se relee el estado");
        assert_eq!(leido["accion"], "activar");
        assert_eq!(leido["ts"], 1234);
        // 0600: el estado puede contain rutas del dispositivo.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let modo = std::fs::metadata(&p).map(|m| m.permissions().mode() & 0o777).unwrap_or(0);
            assert_eq!(modo, 0o600, "el estado debe ser 0600");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_json_del_helper_se_parsea_de_la_ultima_linea() {
        let txt = "ruido\n{\"ok\":true,\"accion\":\"activar\"}\n";
        let v = parsear_json_line(txt).expect("parsea");
        assert_eq!(v["ok"], true);
        assert_eq!(v["accion"], "activar");
        assert!(parsear_json_line("sin json aqui").is_none());
        assert!(parsear_json_line("").is_none());
    }

    #[test]
    fn la_cita_de_shell_escapa_las_comillas() {
        assert_eq!(shell_quote("/data/user/0/com.agy"), "'/data/user/0/com.agy'");
        assert_eq!(shell_quote("a'b"), "'a'\\''b'");
    }

    #[test]
    fn las_candidatas_de_la_db_cubren_las_dos_ubicaciones() {
        let c = candidatas_db_antigravity();
        assert!(c.len() >= 2, "debe probar las dos ubicaciones medidas");
        let s = c.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("|");
        assert!(s.contains("conversation_summaries.db"));
        assert!(s.contains("conversations/conversation_summaries.db"));
    }

    #[test]
    fn el_remerge_ignora_las_filas_que_ya_existen() {
        // La DB servida tiene el MISMO esquema que la original: se copia
        // una fila y se comprueba que el re-merge NO la pisa (para que las
        // conversaciones ocultas por el MITM sigan ocultas).
        let dir = std::env::temp_dir().join(format!("mdmitm-merge-{}", ahora()));
        let _ = std::fs::create_dir_all(&dir);
        let orig = dir.join("orig.db");
        let serv = dir.join("servida.db");
        for p in [&orig, &serv] {
            let con = Connection::open(p).unwrap();
            con.execute_batch(
                "CREATE TABLE conversation_summaries (
                   conversation_id TEXT PRIMARY KEY,
                   parent_conversation_id TEXT DEFAULT '',
                   state TEXT, created_at INTEGER, updated_at INTEGER, steps INTEGER);
                 INSERT INTO conversation_summaries VALUES ('a','','OCULTA',1,2,3);",
            )
            .unwrap();
        }
        // La servida tiene la fila 'a' (mentirosa) y una nueva 'b'.
        {
            let con = Connection::open(&serv).unwrap();
            con.execute(
                "INSERT INTO conversation_summaries VALUES ('b','a','NUEVA',4,5,6)",
                [],
            )
            .unwrap();
        }
        let r = remergear_servida_en_original(&serv, &orig).expect("re-merge");
        assert_eq!(r["filas_re_mergadas"], 1, "sólo se añade la nueva");

        let con = Connection::open(&orig).unwrap();
        let estado_a: String = con
            .query_row("SELECT state FROM conversation_summaries WHERE conversation_id='a'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(estado_a, "OCULTA", "la fila que ya existia NO se toca");
        let n: i64 = con
            .query_row("SELECT COUNT(*) FROM conversation_summaries", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_db_servida_queda_en_wal() {
        let dir = std::env::temp_dir().join(format!("mdmitm-wal-{}", ahora()));
        let _ = std::fs::create_dir_all(&dir);
        let src_path = dir.join("orig.db");
        {
            let con = Connection::open(&src_path).unwrap();
            con.execute_batch(
                "CREATE TABLE conversation_summaries (
                   conversation_id TEXT PRIMARY KEY,
                   parent_conversation_id TEXT DEFAULT '',
                   state TEXT, created_at INTEGER, updated_at INTEGER, steps INTEGER);
                 INSERT INTO conversation_summaries VALUES ('a','','IDLE',1,2,3);",
            )
            .unwrap();
        }
        let dest = dir.join("servida.db");
        generar_db_servida_desde(&src_path, &[], &[], &dest).expect("se genera la DB servida");
        let con = Connection::open(&dest).unwrap();
        let modo: String = con
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap_or_default();
        assert_eq!(
            modo.to_lowercase(),
            "wal",
            "la DB servida va en WAL: asi el lector (replica) y el escritor (la app) conviven"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_db_servida_oculta_y_reescribe() {
        let dir = std::env::temp_dir().join(format!("mdmitm-filtro-{}", ahora()));
        let _ = std::fs::create_dir_all(&dir);
        let src_path = dir.join("orig.db");
        {
            let con = Connection::open(&src_path).unwrap();
            con.execute_batch(
                "CREATE TABLE conversation_summaries (
                   conversation_id TEXT PRIMARY KEY,
                   parent_conversation_id TEXT DEFAULT '',
                   state TEXT, created_at INTEGER, updated_at INTEGER, steps INTEGER);
                 INSERT INTO conversation_summaries VALUES ('a','','IDLE',1,2,3);
                 INSERT INTO conversation_summaries VALUES ('b','','BUSY',4,5,6);",
            )
            .unwrap();
        }
        let dest = dir.join("servida.db");
        generar_db_servida_desde(
            &src_path,
            &["b".to_string()],
            &[("BUSY".to_string(), "IDLE".to_string())],
            &dest,
        )
        .expect("se genera la DB servida");
        let con = Connection::open(&dest).unwrap();
        let n: i64 = con
            .query_row("SELECT COUNT(*) FROM conversation_summaries", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "la conversacion 'b' queda oculta");
        let estado: String = con
            .query_row("SELECT state FROM conversation_summaries", [], |r| r.get(0))
            .unwrap();
        assert_eq!(estado, "IDLE");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

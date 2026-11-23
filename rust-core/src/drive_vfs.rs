//! Google Drive Virtual Filesystem (VFS) for MegaDaemon.
//!
//! Exposes Google Drive paths (`drive:/Folder/Sub/file.bin`) as a structured VFS
//! with SQLite metadata caching (`drive_vfs_cache.db`), directory listing,
//! glob/name search, range reads, atomic writes, mkdir, and delete.

use crate::drive_client::{DriveTarget, DRIVE_API};
use crate::drive_native::{agent, backoff_with_jitter, ensure_folder, is_transient_status, upload_file_retry};
use crate::google_auth;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VfsEntry {
    pub id: String,
    pub name: String,
    pub mime_type: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified_time: String,
}

fn drive_get_retry(url: &str, state_dir: Option<&str>) -> Result<Vec<u8>, String> {
    let ag = agent();
    let mut last_err = String::new();
    for attempt in 0..5u32 {
        let (ak, av) = google_auth::auth_header(state_dir)?;
        match ag.get(url).header(ak, av).call() {
            Ok(resp) => {
                let st = resp.status().as_u16();
                let body = resp.into_body().read_to_vec().unwrap_or_default();
                if st == 200 {
                    return Ok(body);
                }
                if st == 401 {
                    google_auth::invalidate();
                    last_err = "401 unauthorized".to_string();
                    backoff_with_jitter(attempt);
                    continue;
                }
                if is_transient_status(st) && attempt < 4 {
                    last_err = format!("HTTP {}", st);
                    backoff_with_jitter(attempt);
                    continue;
                }
                return Err(format!("drive vfs GET HTTP {}", st));
            }
            Err(e) if attempt < 4 => {
                last_err = e.to_string();
                backoff_with_jitter(attempt);
            }
            Err(e) => return Err(format!("drive vfs GET: {}", e)),
        }
    }
    Err(last_err)
}

/// Moves a Drive file/folder from `old_parent_id` to `new_parent_id` with backoff/retry.
pub fn move_item(
    file_id: &str,
    old_parent_id: &str,
    new_parent_id: &str,
    state_dir: Option<&str>,
) -> Result<(), String> {
    let ag = agent();
    let url = format!(
        "{}/{}?addParents={}&removeParents={}&supportsAllDrives=true&fields=id,parents",
        DRIVE_API, file_id, new_parent_id, old_parent_id
    );
    let mut last_err = String::new();
    for attempt in 0..5u32 {
        let (ak, av) = google_auth::auth_header(state_dir)?;
        match ag.patch(&url).header(ak, av).send(&[] as &[u8]) {
            Ok(resp) => {
                let st = resp.status().as_u16();
                if st == 200 {
                    return Ok(());
                }
                if st == 401 {
                    google_auth::invalidate();
                    backoff_with_jitter(attempt);
                    continue;
                }
                if is_transient_status(st) && attempt < 4 {
                    last_err = format!("HTTP {}", st);
                    backoff_with_jitter(attempt);
                    continue;
                }
                return Err(format!("drive vfs move HTTP {}", st));
            }
            Err(e) if attempt < 4 => {
                last_err = e.to_string();
                backoff_with_jitter(attempt);
            }
            Err(e) => return Err(format!("drive vfs move: {}", e)),
        }
    }
    Err(last_err)
}

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn cache_upsert(state_dir: Option<&str>, path: &str, entry: &VfsEntry) {
    let Some(sd) = state_dir else { return };
    let db = Path::new(sd).join("drive_vfs_cache.db");
    let Ok(conn) = Connection::open(&db) else { return };
    let _ = conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS vfs_cache (
            path TEXT PRIMARY KEY,
            file_id TEXT NOT NULL,
            name TEXT NOT NULL,
            mime_type TEXT NOT NULL,
            is_dir INTEGER NOT NULL,
            size INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );",
    );
    let _ = conn.execute(
        "INSERT OR REPLACE INTO vfs_cache (path, file_id, name, mime_type, is_dir, size, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            path,
            entry.id,
            entry.name,
            entry.mime_type,
            if entry.is_dir { 1i64 } else { 0i64 },
            entry.size as i64,
            now_epoch()
        ],
    );
}

/// Normalizes `drive:/a/b` or `/a/b` into path segments `["a", "b"]`.
pub fn split_vfs_path(vfs_path: &str) -> Vec<String> {
    let trimmed = vfs_path
        .trim()
        .trim_start_matches("drive://")
        .trim_start_matches("drive:/")
        .trim_start_matches('/');
    trimmed
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .map(|s| s.to_string())
        .collect()
}

/// Parent folder id of a file/folder (needed to move it: Drive's `move`
/// takes `addParents` + `removeParents`).
pub fn parent_de(file_id: &str, state_dir: Option<&str>) -> Result<String, String> {
    let url = format!(
        "{}/{}?fields=parents&supportsAllDrives=true",
        DRIVE_API, file_id
    );
    let body = drive_get_retry(&url, state_dir)?;
    let v: Value = serde_json::from_slice(&body)
        .map_err(|e| format!("parent_de: JSON invalido: {}", e))?;
    v["parents"]
        .as_array()
        .and_then(|a| a.first())
        .and_then(|p| p.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "parent_de: el elemento no tiene padre".to_string())
}

/// Lists children of a Drive folder ID (`"root"` for My Drive root).
pub fn list_folder_id(folder_id: &str, state_dir: Option<&str>, limit: usize) -> Result<Vec<VfsEntry>, String> {
    let q = format!("'{}' in parents and trashed = false", folder_id);
    let url = format!(
        "{}?q={}&pageSize={}&fields=files(id,name,mimeType,size,modifiedTime)&orderBy=folder,name&supportsAllDrives=true&includeItemsFromAllDrives=true",
        DRIVE_API,
        urlencode(&q),
        limit.clamp(1, 1000)
    );
    let body = drive_get_retry(&url, state_dir)?;
    let j: Value = serde_json::from_slice(&body).map_err(|e| format!("vfs json: {}", e))?;
    let mut out = Vec::new();
    if let Some(arr) = j["files"].as_array() {
        for f in arr {
            let mime = f["mimeType"].as_str().unwrap_or("").to_string();
            let size = f["size"]
                .as_str()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            out.push(VfsEntry {
                id: f["id"].as_str().unwrap_or("").to_string(),
                name: f["name"].as_str().unwrap_or("").to_string(),
                is_dir: mime == "application/vnd.google-apps.folder",
                mime_type: mime,
                size,
                modified_time: f["modifiedTime"].as_str().unwrap_or("").to_string(),
            });
        }
    }
    Ok(out)
}

/// Resolves a VFS path like `drive:/MakeMKV-RE-v6-backup` to its `VfsEntry`.
pub fn stat_path(vfs_path: &str, state_dir: Option<&str>) -> Result<VfsEntry, String> {
    let segs = split_vfs_path(vfs_path);
    if segs.is_empty() {
        return Ok(VfsEntry {
            id: "root".to_string(),
            name: "/".to_string(),
            mime_type: "application/vnd.google-apps.folder".to_string(),
            is_dir: true,
            size: 0,
            modified_time: String::new(),
        });
    }
    let mut cur_id = "root".to_string();
    let mut last_entry: Option<VfsEntry> = None;
    for seg in &segs {
        let escaped = seg.replace('\\', "\\\\").replace('\'', "\\'");
        let q = format!("'{}' in parents and name = '{}' and trashed = false", cur_id, escaped);
        let url = format!(
            "{}?q={}&pageSize=5&fields=files(id,name,mimeType,size,modifiedTime)&supportsAllDrives=true&includeItemsFromAllDrives=true",
            DRIVE_API,
            urlencode(&q)
        );
        let body = drive_get_retry(&url, state_dir)?;
        let j: Value = serde_json::from_slice(&body).map_err(|e| format!("stat json: {}", e))?;
        let first = j["files"]
            .as_array()
            .and_then(|a| a.first())
            .ok_or_else(|| format!("VFS path not found at segment '{}'", seg))?;
        let mime = first["mimeType"].as_str().unwrap_or("").to_string();
        let entry = VfsEntry {
            id: first["id"].as_str().unwrap_or("").to_string(),
            name: first["name"].as_str().unwrap_or("").to_string(),
            is_dir: mime == "application/vnd.google-apps.folder",
            mime_type: mime,
            size: first["size"]
                .as_str()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0),
            modified_time: first["modifiedTime"].as_str().unwrap_or("").to_string(),
        };
        cur_id = entry.id.clone();
        last_entry = Some(entry);
    }
    let final_entry = last_entry.unwrap();
    cache_upsert(state_dir, vfs_path, &final_entry);
    Ok(final_entry)
}

/// Lists directory contents for `drive:/...`.
pub fn ls(vfs_path: &str, state_dir: Option<&str>, limit: usize) -> Result<Vec<VfsEntry>, String> {
    let entry = stat_path(vfs_path, state_dir)?;
    if !entry.is_dir {
        return Ok(vec![entry]);
    }
    list_folder_id(&entry.id, state_dir, limit)
}

/// Reads up to `max_bytes` from a Drive file at `vfs_path` (supports Range requests).
pub fn read_file_bytes(vfs_path: &str, max_bytes: usize, state_dir: Option<&str>) -> Result<Vec<u8>, String> {
    let entry = stat_path(vfs_path, state_dir)?;
    if entry.is_dir {
        return Err(format!("'{}' is a directory", vfs_path));
    }
    let url = format!("{}?alt=media&supportsAllDrives=true", format!("{}/{}", DRIVE_API, entry.id));
    let (ak, av) = google_auth::auth_header(state_dir)?;
    let end = max_bytes.saturating_sub(1);
    let resp = agent()
        .get(&url)
        .header(ak, av)
        .header("Range", &format!("bytes=0-{}", end))
        .call()
        .map_err(|e| format!("drive vfs read: {}", e))?;
    let bytes = resp.into_body().read_to_vec().map_err(|e| format!("read body: {}", e))?;
    Ok(bytes)
}

/// Searches Drive files by substring query (`name contains '...'`).
pub fn find_by_name(query: &str, state_dir: Option<&str>, limit: usize) -> Result<Vec<VfsEntry>, String> {
    let escaped = query.replace('\\', "\\\\").replace('\'', "\\'");
    let q = format!("name contains '{}' and trashed = false", escaped);
    let url = format!(
        "{}?q={}&pageSize={}&fields=files(id,name,mimeType,size,modifiedTime)&supportsAllDrives=true&includeItemsFromAllDrives=true",
        DRIVE_API,
        urlencode(&q),
        limit.clamp(1, 200)
    );
    let (ak, av) = google_auth::auth_header(state_dir)?;
    let resp = agent()
        .get(&url)
        .header(ak, av)
        .call()
        .map_err(|e| format!("drive vfs find: {}", e))?;
    let body = resp.into_body().read_to_vec().unwrap_or_default();
    let j: Value = serde_json::from_slice(&body).map_err(|e| format!("find json: {}", e))?;
    let mut out = Vec::new();
    if let Some(arr) = j["files"].as_array() {
        for f in arr {
            let mime = f["mimeType"].as_str().unwrap_or("").to_string();
            out.push(VfsEntry {
                id: f["id"].as_str().unwrap_or("").to_string(),
                name: f["name"].as_str().unwrap_or("").to_string(),
                is_dir: mime == "application/vnd.google-apps.folder",
                mime_type: mime,
                size: f["size"]
                    .as_str()
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0),
                modified_time: f["modifiedTime"].as_str().unwrap_or("").to_string(),
            });
        }
    }
    Ok(out)
}

/// Creates a nested folder path `drive:/a/b/c` and returns its final folder ID.
pub fn mkdir_p(vfs_path: &str, state_dir: Option<&str>) -> Result<String, String> {
    let segs = split_vfs_path(vfs_path);
    let mut cur = String::new();
    for seg in segs {
        cur = ensure_folder(&seg, &cur, state_dir)?;
    }
    Ok(if cur.is_empty() { "root".to_string() } else { cur })
}

/// Uploads a local file directly to a Drive VFS directory path.
pub fn write_local_file(local_path: &Path, vfs_dir: &str, remote_name: &str, state_dir: Option<&str>) -> Result<Value, String> {
    let folder_id = mkdir_p(vfs_dir, state_dir)?;
    let target = DriveTarget::folder(&folder_id);
    let up = upload_file_retry(local_path, remote_name, &target, state_dir, 3)?;
    Ok(json!({
        "ok": true,
        "file_id": up.file_id,
        "name": up.name,
        "bytes": up.bytes,
        "folder_id": folder_id,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_drive_vfs_paths_cleanly() {
        assert_eq!(
            split_vfs_path("drive:/MakeMKV-RE-v6-backup/src"),
            vec!["MakeMKV-RE-v6-backup", "src"]
        );
        assert_eq!(
            split_vfs_path("drive:///playground-sdk-research/output/"),
            vec!["playground-sdk-research", "output"]
        );
        assert!(split_vfs_path("drive:/").is_empty());
    }
}

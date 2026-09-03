use crate::db::{self, DbState};
use crate::DbPath;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    pub theme: String,
    pub data_dir: Option<String>,
    pub sidebar_width: Option<i32>,
    pub task_editor_width: Option<i32>,
}

#[tauri::command]
pub fn get_settings(state: State<DbState>) -> Result<Settings, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT key, value FROM settings")
        .map_err(|e| e.to_string())?;
    let map: HashMap<String, String> = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(Settings {
        theme: map.get("theme").cloned().unwrap_or_else(|| "system".to_string()),
        // Not read from the settings table: the pointer file is the single
        // source of truth for which database is actually open.
        data_dir: db::read_data_dir().map(|p| p.to_string_lossy().to_string()),
        sidebar_width: map.get("sidebar_width").and_then(|v| v.parse().ok()),
        task_editor_width: map.get("task_editor_width").and_then(|v| v.parse().ok()),
    })
}

#[tauri::command]
pub fn set_setting(state: State<DbState>, key: String, value: String) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO settings(key, value) VALUES(?1,?2)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// True when `path` is a database that already holds user content. Used to
/// decide between adopting the database at the destination and overwriting it —
/// overwriting one with real data would wipe whatever another machine put there.
/// A missing, empty or non-SQLite file counts as "no data".
fn holds_user_data(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    match rusqlite::Connection::open(path) {
        Ok(conn) => conn
            .query_row(
                "SELECT (SELECT COUNT(*) FROM lists) + (SELECT COUNT(*) FROM tasks)",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0)
            > 0,
        Err(_) => false,
    }
}

#[tauri::command]
pub fn change_data_dir(
    state: State<DbState>,
    db_path: State<DbPath>,
    new_path: String,
) -> Result<(), String> {
    let mut conn = state.0.lock().map_err(|e| e.to_string())?;
    let mut current = db_path.0.lock().map_err(|e| e.to_string())?;

    let new_dir = PathBuf::from(&new_path);
    std::fs::create_dir_all(&new_dir).map_err(|e| e.to_string())?;

    let src_db = PathBuf::from(current.clone());
    let dst_db = new_dir.join(db::DB_FILE);

    if src_db == dst_db {
        db::write_data_dir(&new_dir).map_err(|e| e.to_string())?;
        return Ok(());
    }

    if holds_user_data(&dst_db) {
        // Another machine already put data here — adopt it, don't clobber it.
    } else {
        // Flush the WAL first, otherwise the copy misses recent writes.
        db::checkpoint(&conn).map_err(|e| e.to_string())?;
        std::fs::copy(&src_db, &dst_db).map_err(|e| e.to_string())?;
        // Drop any stale sidecars at the destination; applied against the
        // database we just copied they would resurrect old or foreign writes.
        for ext in ["-wal", "-shm"] {
            let mut sidecar = dst_db.clone().into_os_string();
            sidecar.push(ext);
            let _ = std::fs::remove_file(PathBuf::from(sidecar));
        }
    }

    let dst_str = dst_db.to_string_lossy().to_string();
    *conn = db::open(&dst_str).map_err(|e| e.to_string())?;
    *current = dst_str;
    db::write_data_dir(&new_dir).map_err(|e| e.to_string())?;
    Ok(())
}

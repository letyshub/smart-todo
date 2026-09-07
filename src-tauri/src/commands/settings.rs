use crate::db::{self, DbState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    pub theme: String,
    /// Where the database file lives. Always local, and not user-configurable:
    /// hosting it in a cloud-synced folder is what corrupted it. Sharing data
    /// between machines is the sync folder's job instead.
    pub database_path: String,
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
        database_path: db::resolve_db_path().to_string_lossy().to_string(),
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

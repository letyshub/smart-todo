use crate::db::DbState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct Settings {
    pub theme: String,
    pub data_dir: Option<String>,
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
        data_dir: map.get("data_dir").cloned(),
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

#[tauri::command]
pub fn change_data_dir(state: State<DbState>, new_path: String) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let current_path: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key='data_dir'", [], |r| r.get(0))
        .ok();
    if let Some(src) = current_path {
        let src_db = std::path::Path::new(&src).join("data.db");
        let dst_db = std::path::Path::new(&new_path).join("data.db");
        if src_db.exists() {
            std::fs::create_dir_all(&new_path).map_err(|e| e.to_string())?;
            std::fs::copy(&src_db, &dst_db).map_err(|e| e.to_string())?;
        }
    }
    conn.execute(
        "INSERT INTO settings(key,value) VALUES('data_dir',?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        rusqlite::params![new_path],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

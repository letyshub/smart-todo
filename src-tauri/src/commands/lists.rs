use crate::db::DbState;
use tauri::State;

#[tauri::command]
pub fn get_lists(_state: State<DbState>) -> Result<Vec<()>, String> { Ok(vec![]) }
#[tauri::command]
pub fn create_list(_state: State<DbState>, _title: String, _color: Option<String>) -> Result<(), String> { Ok(()) }
#[tauri::command]
pub fn update_list(_state: State<DbState>, _id: i64, _title: Option<String>, _color: Option<String>, _position: Option<i64>) -> Result<(), String> { Ok(()) }
#[tauri::command]
pub fn delete_list(_state: State<DbState>, _id: i64) -> Result<(), String> { Ok(()) }

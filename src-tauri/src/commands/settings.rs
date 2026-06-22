use crate::db::DbState;
use tauri::State;

#[tauri::command]
pub fn get_settings(_state: State<DbState>) -> Result<(), String> { Ok(()) }
#[tauri::command]
pub fn set_setting(_state: State<DbState>, _key: String, _value: String) -> Result<(), String> { Ok(()) }
#[tauri::command]
pub fn change_data_dir(_state: State<DbState>, _new_path: String) -> Result<(), String> { Ok(()) }

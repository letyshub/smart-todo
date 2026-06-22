use crate::db::DbState;
use tauri::State;

#[tauri::command]
pub fn get_tasks(_state: State<DbState>, _list_id: i64) -> Result<Vec<()>, String> { Ok(vec![]) }
#[tauri::command]
pub fn create_task(_state: State<DbState>, _list_id: i64, _title: String, _priority: Option<String>, _due_date: Option<String>, _description: Option<String>) -> Result<(), String> { Ok(()) }
#[tauri::command]
pub fn update_task(_state: State<DbState>, _id: i64, _title: Option<String>, _description: Option<String>, _priority: Option<String>, _due_date: Option<String>, _completed: Option<bool>, _position: Option<i64>) -> Result<(), String> { Ok(()) }
#[tauri::command]
pub fn delete_task(_state: State<DbState>, _id: i64) -> Result<(), String> { Ok(()) }
#[tauri::command]
pub fn get_dashboard_tasks(_state: State<DbState>) -> Result<(), String> { Ok(()) }
#[tauri::command]
pub fn set_task_tags(_state: State<DbState>, _task_id: i64, _tag_names: Vec<String>) -> Result<Vec<()>, String> { Ok(vec![]) }
#[tauri::command]
pub fn get_all_tags(_state: State<DbState>) -> Result<Vec<()>, String> { Ok(vec![]) }

use crate::db::DbState;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use tauri::State;

pub struct TimerEntry {
    pub start_instant: Instant,
    pub started_at: String,
}
pub struct TimerState(pub Mutex<HashMap<i64, TimerEntry>>);

#[tauri::command]
pub fn start_timer(_task_id: i64, _timer_state: State<TimerState>, _db_state: State<DbState>) -> Result<(), String> { Ok(()) }
#[tauri::command]
pub fn stop_timer(_task_id: i64, _timer_state: State<TimerState>, _db_state: State<DbState>) -> Result<(), String> { Ok(()) }
#[tauri::command]
pub fn get_active_timers(_timer_state: State<TimerState>) -> Result<Vec<()>, String> { Ok(vec![]) }
#[tauri::command]
pub fn get_timer_sessions(_task_id: i64, _state: State<DbState>) -> Result<Vec<()>, String> { Ok(vec![]) }

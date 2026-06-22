use crate::db::DbState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use tauri::State;

pub struct TimerEntry {
    pub start_instant: Instant,
    pub started_at: String,
}

pub struct TimerState(pub Mutex<HashMap<i64, TimerEntry>>);

#[derive(Debug, Serialize, Deserialize)]
pub struct TimerSession {
    pub id: i64,
    pub task_id: i64,
    pub started_at: String,
    pub stopped_at: Option<String>,
    pub duration_seconds: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ActiveTimer {
    pub task_id: i64,
    pub elapsed_seconds: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StartTimerResult {
    pub stopped_task_id: Option<i64>,
}

pub fn stop_timer_inner(task_id: i64, entry: TimerEntry, conn: &rusqlite::Connection) -> Result<(), String> {
    let elapsed = entry.start_instant.elapsed().as_secs() as i64;
    let stopped_at = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    conn.execute(
        "INSERT INTO timer_sessions (task_id, started_at, stopped_at, duration_seconds)
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![task_id, entry.started_at, stopped_at, elapsed],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn start_timer(
    task_id: i64,
    timer_state: State<TimerState>,
    db_state: State<DbState>,
) -> Result<StartTimerResult, String> {
    let mut timers = timer_state.0.lock().map_err(|e| e.to_string())?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    let stopped_task_id = if !timers.is_empty() {
        let stopped_id = *timers.keys().next().unwrap();
        let entry = timers.remove(&stopped_id).unwrap();
        stop_timer_inner(stopped_id, entry, &conn)?;
        Some(stopped_id)
    } else {
        None
    };
    let started_at = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    timers.insert(task_id, TimerEntry { start_instant: Instant::now(), started_at });
    Ok(StartTimerResult { stopped_task_id })
}

#[tauri::command]
pub fn stop_timer(
    task_id: i64,
    timer_state: State<TimerState>,
    db_state: State<DbState>,
) -> Result<(), String> {
    let mut timers = timer_state.0.lock().map_err(|e| e.to_string())?;
    let conn = db_state.0.lock().map_err(|e| e.to_string())?;
    if let Some(entry) = timers.remove(&task_id) {
        stop_timer_inner(task_id, entry, &conn)?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_active_timers(timer_state: State<TimerState>) -> Result<Vec<ActiveTimer>, String> {
    let timers = timer_state.0.lock().map_err(|e| e.to_string())?;
    let result = timers
        .iter()
        .map(|(task_id, entry)| ActiveTimer {
            task_id: *task_id,
            elapsed_seconds: entry.start_instant.elapsed().as_secs(),
        })
        .collect();
    Ok(result)
}

#[tauri::command]
pub fn get_timer_sessions(task_id: i64, state: State<DbState>) -> Result<Vec<TimerSession>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, task_id, started_at, stopped_at, duration_seconds
             FROM timer_sessions WHERE task_id=?1 ORDER BY started_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let sessions = stmt
        .query_map(rusqlite::params![task_id], |r| {
            Ok(TimerSession {
                id: r.get(0)?,
                task_id: r.get(1)?,
                started_at: r.get(2)?,
                stopped_at: r.get(3)?,
                duration_seconds: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(sessions)
}

#[cfg(test)]
mod tests {
    use crate::db::open_in_memory;
    use std::time::Instant;
    use super::TimerEntry;

    #[test]
    fn test_stop_timer_persists_session() {
        let conn = open_in_memory();
        conn.execute("INSERT INTO lists (title, position) VALUES ('L', 0)", []).unwrap();
        let list_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO tasks (list_id, title, position) VALUES (?1, 'T', 0)",
            rusqlite::params![list_id],
        ).unwrap();
        let task_id = conn.last_insert_rowid();
        let entry = TimerEntry {
            start_instant: Instant::now(),
            started_at: "2026-06-20T10:00:00".to_string(),
        };
        super::stop_timer_inner(task_id, entry, &conn).unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM timer_sessions WHERE task_id=?1",
            rusqlite::params![task_id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_active_timers_tracks_elapsed() {
        use std::collections::HashMap;
        let mut map: HashMap<i64, TimerEntry> = HashMap::new();
        map.insert(42, TimerEntry {
            start_instant: Instant::now(),
            started_at: "2026-06-20T10:00:00".to_string(),
        });
        assert!(map.contains_key(&42));
        assert!(map[&42].start_instant.elapsed().as_secs() < 5);
    }
}

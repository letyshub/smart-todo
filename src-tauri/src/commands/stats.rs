use crate::db::DbState;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize)]
pub struct ProductivityStats {
    pub tasks_completed_week: i64,
    pub total_seconds_week: i64,
    pub on_time_count: i64,
    pub late_count: i64,
}

pub fn get_stats_from_conn(conn: &rusqlite::Connection) -> Result<ProductivityStats, String> {
    let tasks_completed_week: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tasks
         WHERE completed = 1
           AND completed_at >= datetime('now', '-7 days')
           AND parent_task_id IS NULL",
        [],
        |r| r.get(0),
    ).map_err(|e| e.to_string())?;

    let total_seconds_week: i64 = conn.query_row(
        "SELECT COALESCE(SUM(duration_seconds), 0) FROM timer_sessions
         WHERE started_at >= datetime('now', '-7 days')",
        [],
        |r| r.get(0),
    ).map_err(|e| e.to_string())?;

    let (on_time_count, late_count): (i64, i64) = conn.query_row(
        "SELECT
           COALESCE(SUM(CASE WHEN date(completed_at) <= due_date THEN 1 ELSE 0 END), 0),
           COALESCE(SUM(CASE WHEN date(completed_at) > due_date  THEN 1 ELSE 0 END), 0)
         FROM tasks
         WHERE completed = 1
           AND completed_at >= datetime('now', '-7 days')
           AND due_date IS NOT NULL
           AND parent_task_id IS NULL",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).map_err(|e| e.to_string())?;

    Ok(ProductivityStats { tasks_completed_week, total_seconds_week, on_time_count, late_count })
}

#[tauri::command]
pub fn get_productivity_stats(state: State<DbState>) -> Result<ProductivityStats, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    get_stats_from_conn(&conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    fn setup() -> (rusqlite::Connection, i64) {
        let conn = open_in_memory();
        conn.execute("INSERT INTO lists (title, position) VALUES ('Test', 0)", []).unwrap();
        let list_id = conn.last_insert_rowid();
        (conn, list_id)
    }

    #[test]
    fn test_stats_empty_db() {
        let (conn, _) = setup();
        let stats = get_stats_from_conn(&conn).unwrap();
        assert_eq!(stats.tasks_completed_week, 0);
        assert_eq!(stats.total_seconds_week, 0);
        assert_eq!(stats.on_time_count, 0);
        assert_eq!(stats.late_count, 0);
    }

    #[test]
    fn test_counts_completed_tasks_this_week() {
        let (conn, list_id) = setup();
        // Insert a task completed now (within 7 days)
        conn.execute(
            "INSERT INTO tasks (list_id, title, priority, position, completed, completed_at, status)
             VALUES (?1, 'Done task', 'normal', 0, 1, datetime('now'), 'done')",
            rusqlite::params![list_id],
        ).unwrap();
        // Insert a task completed 10 days ago (outside window)
        conn.execute(
            "INSERT INTO tasks (list_id, title, priority, position, completed, completed_at, status)
             VALUES (?1, 'Old task', 'normal', 1, 1, datetime('now', '-10 days'), 'done')",
            rusqlite::params![list_id],
        ).unwrap();
        let stats = get_stats_from_conn(&conn).unwrap();
        assert_eq!(stats.tasks_completed_week, 1);
    }

    #[test]
    fn test_excludes_subtasks_from_count() {
        let (conn, list_id) = setup();
        conn.execute(
            "INSERT INTO tasks (list_id, title, priority, position) VALUES (?1, 'Parent', 'normal', 0)",
            rusqlite::params![list_id],
        ).unwrap();
        let parent_id = conn.last_insert_rowid();
        // Subtask completed this week
        conn.execute(
            "INSERT INTO tasks (list_id, parent_task_id, is_subtask, title, priority, position, completed, completed_at, status)
             VALUES (?1, ?2, 1, 'Sub', 'normal', 0, 1, datetime('now'), 'done')",
            rusqlite::params![list_id, parent_id],
        ).unwrap();
        let stats = get_stats_from_conn(&conn).unwrap();
        assert_eq!(stats.tasks_completed_week, 0);
    }

    #[test]
    fn test_sums_timer_seconds_this_week() {
        let (conn, list_id) = setup();
        conn.execute(
            "INSERT INTO tasks (list_id, title, priority, position) VALUES (?1, 'T', 'normal', 0)",
            rusqlite::params![list_id],
        ).unwrap();
        let task_id = conn.last_insert_rowid();
        // Session this week: 3600 seconds
        conn.execute(
            "INSERT INTO timer_sessions (task_id, started_at, stopped_at, duration_seconds)
             VALUES (?1, datetime('now', '-1 day'), datetime('now'), 3600)",
            rusqlite::params![task_id],
        ).unwrap();
        // Session outside window: 7200 seconds
        conn.execute(
            "INSERT INTO timer_sessions (task_id, started_at, stopped_at, duration_seconds)
             VALUES (?1, datetime('now', '-10 days'), datetime('now', '-10 days', '+2 hours'), 7200)",
            rusqlite::params![task_id],
        ).unwrap();
        let stats = get_stats_from_conn(&conn).unwrap();
        assert_eq!(stats.total_seconds_week, 3600);
    }

    #[test]
    fn test_on_time_vs_late() {
        let (conn, list_id) = setup();
        // On-time: completed today, due today
        conn.execute(
            "INSERT INTO tasks (list_id, title, priority, position, completed, completed_at, due_date, status)
             VALUES (?1, 'On time', 'normal', 0, 1, datetime('now'), date('now'), 'done')",
            rusqlite::params![list_id],
        ).unwrap();
        // Late: completed today, due yesterday
        conn.execute(
            "INSERT INTO tasks (list_id, title, priority, position, completed, completed_at, due_date, status)
             VALUES (?1, 'Late', 'normal', 1, 1, datetime('now'), date('now', '-1 day'), 'done')",
            rusqlite::params![list_id],
        ).unwrap();
        // No due date: excluded from both counts
        conn.execute(
            "INSERT INTO tasks (list_id, title, priority, position, completed, completed_at, status)
             VALUES (?1, 'No date', 'normal', 2, 1, datetime('now'), 'done')",
            rusqlite::params![list_id],
        ).unwrap();
        let stats = get_stats_from_conn(&conn).unwrap();
        assert_eq!(stats.on_time_count, 1);
        assert_eq!(stats.late_count, 1);
    }
}

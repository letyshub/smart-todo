use crate::db::DbState;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct List {
    pub id: i64,
    pub title: String,
    pub color: Option<String>,
    pub position: i64,
    pub created_at: String,
    pub task_total: i64,
    pub task_completed: i64,
}

fn fetch_list_task_counts(conn: &rusqlite::Connection, list_id: i64) -> (i64, i64) {
    conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(CASE WHEN completed != 0 THEN 1 ELSE 0 END), 0)
         FROM tasks WHERE list_id = ?1 AND parent_task_id IS NULL",
        rusqlite::params![list_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .unwrap_or((0, 0))
}

fn row_to_list(row: &rusqlite::Row, conn: &rusqlite::Connection) -> rusqlite::Result<List> {
    let id: i64 = row.get(0)?;
    let (task_total, task_completed) = fetch_list_task_counts(conn, id);
    Ok(List {
        id,
        title: row.get(1)?,
        color: row.get(2)?,
        position: row.get(3)?,
        created_at: row.get(4)?,
        task_total,
        task_completed,
    })
}

pub(crate) fn query_list_by_id(conn: &rusqlite::Connection, id: i64) -> rusqlite::Result<List> {
    conn.query_row(
        "SELECT id, title, color, position, created_at FROM lists WHERE id = ?1",
        rusqlite::params![id],
        |row| row_to_list(row, conn),
    )
}

#[tauri::command]
pub fn get_lists(state: State<DbState>) -> Result<Vec<List>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, title, color, position, created_at FROM lists ORDER BY position")
        .map_err(|e| e.to_string())?;
    let lists = stmt
        .query_map([], |row| row_to_list(row, &conn))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(lists)
}

#[tauri::command]
pub fn create_list(
    state: State<DbState>,
    title: String,
    color: Option<String>,
) -> Result<List, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let position: i64 = conn
        .query_row("SELECT COALESCE(MAX(position), -1) + 1 FROM lists", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO lists (title, color, position) VALUES (?1, ?2, ?3)",
        rusqlite::params![title, color, position],
    )
    .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    query_list_by_id(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_list(
    state: State<DbState>,
    id: i64,
    title: Option<String>,
    color: Option<String>,
    position: Option<i64>,
) -> Result<List, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(t) = title {
        conn.execute("UPDATE lists SET title = ?1 WHERE id = ?2", rusqlite::params![t, id])
            .map_err(|e| e.to_string())?;
    }
    if let Some(c) = color {
        conn.execute("UPDATE lists SET color = ?1 WHERE id = ?2", rusqlite::params![c, id])
            .map_err(|e| e.to_string())?;
    }
    if let Some(p) = position {
        conn.execute("UPDATE lists SET position = ?1 WHERE id = ?2", rusqlite::params![p, id])
            .map_err(|e| e.to_string())?;
    }
    query_list_by_id(&conn, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_list(state: State<DbState>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM lists WHERE id = ?1", rusqlite::params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::db::open_in_memory;

    #[test]
    fn test_create_and_get_list() {
        let conn = open_in_memory();
        conn.execute(
            "INSERT INTO lists (title, color, position) VALUES (?1, ?2, ?3)",
            rusqlite::params!["Work", "#3b82f6", 0],
        ).unwrap();
        let id: i64 = conn.last_insert_rowid();
        let title: String = conn.query_row(
            "SELECT title FROM lists WHERE id = ?1",
            rusqlite::params![id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(title, "Work");
    }

    #[test]
    fn test_delete_list_cascades_tasks() {
        let conn = open_in_memory();
        conn.execute("INSERT INTO lists (title, position) VALUES ('Test', 0)", []).unwrap();
        let list_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO tasks (list_id, title, position) VALUES (?1, 'task', 0)",
            rusqlite::params![list_id],
        ).unwrap();
        conn.execute("DELETE FROM lists WHERE id = ?1", rusqlite::params![list_id]).unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE list_id = ?1",
            rusqlite::params![list_id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_list_includes_root_task_counts() {
        let conn = open_in_memory();
        conn.execute("INSERT INTO lists (title, position) VALUES ('Test', 0)", []).unwrap();
        let list_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO tasks (list_id, title, completed, position) VALUES (?1, 'T1', 1, 0)",
            rusqlite::params![list_id],
        ).unwrap();
        let parent_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO tasks (list_id, title, completed, position) VALUES (?1, 'T2', 0, 1)",
            rusqlite::params![list_id],
        ).unwrap();
        // Subtask should not count as root task
        conn.execute(
            "INSERT INTO tasks (list_id, parent_task_id, is_subtask, title, completed, position) VALUES (?1, ?2, 1, 'Sub', 1, 0)",
            rusqlite::params![list_id, parent_id],
        ).unwrap();

        let list = super::query_list_by_id(&conn, list_id).unwrap();
        assert_eq!(list.task_total, 2);
        assert_eq!(list.task_completed, 1);
    }
}

use crate::db::DbState;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub list_id: i64,
    pub title: String,
    pub description: Option<String>,
    pub priority: String,
    pub due_date: Option<String>,
    pub completed: bool,
    pub completed_at: Option<String>,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
    pub tags: Vec<Tag>,
    pub total_seconds: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardData {
    pub overdue: Vec<Task>,
    pub high_priority: Vec<Task>,
    pub upcoming: Vec<Task>,
}

fn fetch_tags_for_task(conn: &rusqlite::Connection, task_id: i64) -> Vec<Tag> {
    let mut stmt = conn
        .prepare(
            "SELECT t.id, t.name, t.color FROM tags t
             JOIN task_tags tt ON tt.tag_id = t.id
             WHERE tt.task_id = ?1",
        )
        .unwrap();
    stmt.query_map(rusqlite::params![task_id], |r| {
        Ok(Tag { id: r.get(0)?, name: r.get(1)?, color: r.get(2)? })
    })
    .unwrap()
    .filter_map(|r| r.ok())
    .collect()
}

fn fetch_total_seconds(conn: &rusqlite::Connection, task_id: i64) -> i64 {
    conn.query_row(
        "SELECT COALESCE(SUM(duration_seconds), 0) FROM timer_sessions WHERE task_id = ?1",
        rusqlite::params![task_id],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

fn row_to_task(row: &rusqlite::Row, conn: &rusqlite::Connection) -> rusqlite::Result<Task> {
    let id: i64 = row.get(0)?;
    let tags = fetch_tags_for_task(conn, id);
    let total_seconds = fetch_total_seconds(conn, id);
    Ok(Task {
        id,
        list_id: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        priority: row.get(4)?,
        due_date: row.get(5)?,
        completed: row.get::<_, i64>(6)? != 0,
        completed_at: row.get(7)?,
        position: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        tags,
        total_seconds,
    })
}

#[tauri::command]
pub fn get_tasks(state: State<DbState>, list_id: i64) -> Result<Vec<Task>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id,list_id,title,description,priority,due_date,completed,completed_at,
                    position,created_at,updated_at
             FROM tasks WHERE list_id=?1 ORDER BY completed ASC, position ASC",
        )
        .map_err(|e| e.to_string())?;
    let tasks: Vec<Task> = stmt
        .query_map(rusqlite::params![list_id], |row| row_to_task(row, &conn))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(tasks)
}

#[tauri::command]
pub fn create_task(
    state: State<DbState>,
    list_id: i64,
    title: String,
    priority: Option<String>,
    due_date: Option<String>,
    description: Option<String>,
) -> Result<Task, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let position: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM tasks WHERE list_id = ?1",
            rusqlite::params![list_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let priority = priority.unwrap_or_else(|| "normal".to_string());
    conn.execute(
        "INSERT INTO tasks (list_id,title,description,priority,due_date,position)
         VALUES (?1,?2,?3,?4,?5,?6)",
        rusqlite::params![list_id, title, description, priority, due_date, position],
    )
    .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    conn.query_row(
        "SELECT id,list_id,title,description,priority,due_date,completed,completed_at,
                position,created_at,updated_at FROM tasks WHERE id=?1",
        rusqlite::params![id],
        |row| row_to_task(row, &conn),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_task(
    state: State<DbState>,
    id: i64,
    title: Option<String>,
    description: Option<String>,
    priority: Option<String>,
    due_date: Option<String>,
    completed: Option<bool>,
    position: Option<i64>,
) -> Result<Task, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(v) = title {
        conn.execute("UPDATE tasks SET title=?1,updated_at=datetime('now') WHERE id=?2",
            rusqlite::params![v, id]).map_err(|e| e.to_string())?;
    }
    if let Some(v) = description {
        conn.execute("UPDATE tasks SET description=?1,updated_at=datetime('now') WHERE id=?2",
            rusqlite::params![v, id]).map_err(|e| e.to_string())?;
    }
    if let Some(v) = priority {
        conn.execute("UPDATE tasks SET priority=?1,updated_at=datetime('now') WHERE id=?2",
            rusqlite::params![v, id]).map_err(|e| e.to_string())?;
    }
    if let Some(v) = due_date {
        let val: Option<String> = if v.is_empty() { None } else { Some(v) };
        conn.execute("UPDATE tasks SET due_date=?1,updated_at=datetime('now') WHERE id=?2",
            rusqlite::params![val, id]).map_err(|e| e.to_string())?;
    }
    if let Some(v) = completed {
        conn.execute(
            "UPDATE tasks SET completed=?1,completed_at=CASE WHEN ?1=1 THEN datetime('now') ELSE NULL END,
             updated_at=datetime('now') WHERE id=?2",
            rusqlite::params![v as i64, id],
        ).map_err(|e| e.to_string())?;
    }
    if let Some(v) = position {
        conn.execute("UPDATE tasks SET position=?1,updated_at=datetime('now') WHERE id=?2",
            rusqlite::params![v, id]).map_err(|e| e.to_string())?;
    }
    conn.query_row(
        "SELECT id,list_id,title,description,priority,due_date,completed,completed_at,
                position,created_at,updated_at FROM tasks WHERE id=?1",
        rusqlite::params![id],
        |row| row_to_task(row, &conn),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_task(state: State<DbState>, id: i64) -> Result<(), String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM tasks WHERE id=?1", rusqlite::params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn set_task_tags(
    state: State<DbState>,
    task_id: i64,
    tag_names: Vec<String>,
) -> Result<Vec<Tag>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM task_tags WHERE task_id=?1", rusqlite::params![task_id])
        .map_err(|e| e.to_string())?;
    for name in &tag_names {
        conn.execute("INSERT OR IGNORE INTO tags (name) VALUES (?1)", rusqlite::params![name])
            .map_err(|e| e.to_string())?;
        let tag_id: i64 = conn
            .query_row("SELECT id FROM tags WHERE name=?1", rusqlite::params![name], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR IGNORE INTO task_tags (task_id, tag_id) VALUES (?1,?2)",
            rusqlite::params![task_id, tag_id],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(fetch_tags_for_task(&conn, task_id))
}

#[tauri::command]
pub fn get_all_tags(state: State<DbState>) -> Result<Vec<Tag>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT id, name, color FROM tags ORDER BY name")
        .map_err(|e| e.to_string())?;
    let tags = stmt
        .query_map([], |r| Ok(Tag { id: r.get(0)?, name: r.get(1)?, color: r.get(2)? }))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(tags)
}

#[tauri::command]
pub fn get_dashboard_tasks(state: State<DbState>) -> Result<DashboardData, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let upcoming_limit = chrono::Local::now()
        .checked_add_days(chrono::Days::new(7))
        .unwrap()
        .format("%Y-%m-%d")
        .to_string();

    let base = "SELECT id,list_id,title,description,priority,due_date,completed,completed_at,
                        position,created_at,updated_at FROM tasks WHERE completed=0";

    let mut stmt = conn.prepare(
        &format!("{base} AND due_date IS NOT NULL AND due_date <= ?1 ORDER BY due_date ASC, priority DESC")
    ).map_err(|e| e.to_string())?;
    let overdue: Vec<Task> = stmt
        .query_map(rusqlite::params![today], |row| row_to_task(row, &conn))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    let overdue_ids: Vec<i64> = overdue.iter().map(|t| t.id).collect();

    let mut stmt2 = conn.prepare(
        &format!("{base} AND priority='high' ORDER BY due_date ASC")
    ).map_err(|e| e.to_string())?;
    let high_priority: Vec<Task> = stmt2
        .query_map([], |row| row_to_task(row, &conn))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .filter(|t| !overdue_ids.contains(&t.id))
        .collect();

    let mut exclude_ids = overdue_ids.clone();
    exclude_ids.extend(high_priority.iter().map(|t| t.id));

    let mut stmt3 = conn.prepare(
        &format!("{base} AND due_date IS NOT NULL AND due_date > ?1 AND due_date <= ?2 ORDER BY due_date ASC")
    ).map_err(|e| e.to_string())?;
    let upcoming: Vec<Task> = stmt3
        .query_map(rusqlite::params![today, upcoming_limit], |row| row_to_task(row, &conn))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .filter(|t| !exclude_ids.contains(&t.id))
        .collect();

    Ok(DashboardData { overdue, high_priority, upcoming })
}

#[cfg(test)]
mod tests {
    use crate::db::open_in_memory;

    fn setup() -> (rusqlite::Connection, i64) {
        let conn = open_in_memory();
        conn.execute("INSERT INTO lists (title, position) VALUES ('Test', 0)", []).unwrap();
        let list_id = conn.last_insert_rowid();
        (conn, list_id)
    }

    #[test]
    fn test_create_task() {
        let (conn, list_id) = setup();
        conn.execute(
            "INSERT INTO tasks (list_id, title, priority, position) VALUES (?1, ?2, 'normal', 0)",
            rusqlite::params![list_id, "My Task"],
        ).unwrap();
        let title: String = conn.query_row(
            "SELECT title FROM tasks WHERE list_id=?1",
            rusqlite::params![list_id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(title, "My Task");
    }

    #[test]
    fn test_set_task_tags() {
        let (conn, list_id) = setup();
        conn.execute(
            "INSERT INTO tasks (list_id, title, position) VALUES (?1, 'T', 0)",
            rusqlite::params![list_id],
        ).unwrap();
        let task_id = conn.last_insert_rowid();
        conn.execute("INSERT OR IGNORE INTO tags (name) VALUES ('work')", []).unwrap();
        let tag_id: i64 = conn.query_row("SELECT id FROM tags WHERE name='work'", [], |r| r.get(0)).unwrap();
        conn.execute(
            "INSERT INTO task_tags (task_id, tag_id) VALUES (?1, ?2)",
            rusqlite::params![task_id, tag_id],
        ).unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM task_tags WHERE task_id=?1",
            rusqlite::params![task_id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 1);
    }
}

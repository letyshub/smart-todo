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
    pub parent_task_id: Option<i64>,
    pub is_subtask: bool,
    pub status: String,
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

// SELECT column order: 0=id 1=list_id 2=title 3=description 4=priority 5=due_date
//   6=completed 7=completed_at 8=position 9=created_at 10=updated_at
//   11=parent_task_id 12=is_subtask 13=status
const TASK_COLS: &str =
    "id,list_id,title,description,priority,due_date,completed,completed_at,\
     position,created_at,updated_at,parent_task_id,is_subtask,status";

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
        parent_task_id: row.get(11)?,
        is_subtask: row.get::<_, i64>(12)? != 0,
        status: row.get(13)?,
        tags,
        total_seconds,
    })
}

#[tauri::command]
pub fn get_tasks(state: State<DbState>, list_id: i64) -> Result<Vec<Task>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let sql = format!(
        "SELECT {TASK_COLS} FROM tasks WHERE list_id=?1 AND parent_task_id IS NULL \
         ORDER BY completed ASC, position ASC"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
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
            "SELECT COALESCE(MAX(position), -1) + 1 FROM tasks WHERE list_id = ?1 AND parent_task_id IS NULL",
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
    let sql = format!("SELECT {TASK_COLS} FROM tasks WHERE id=?1");
    conn.query_row(&sql, rusqlite::params![id], |row| row_to_task(row, &conn))
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
    status: Option<String>,
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
    if let Some(v) = status {
        // Sync completed with status
        let done = if v == "done" { 1i64 } else { 0i64 };
        conn.execute(
            "UPDATE tasks SET status=?1,completed=?2,\
             completed_at=CASE WHEN ?2=1 THEN datetime('now') ELSE NULL END,\
             updated_at=datetime('now') WHERE id=?3",
            rusqlite::params![v, done, id],
        ).map_err(|e| e.to_string())?;
    } else if let Some(v) = completed {
        // Sync status with completed
        let status = if v { "done" } else { "todo" };
        conn.execute(
            "UPDATE tasks SET completed=?1,status=?2,\
             completed_at=CASE WHEN ?1=1 THEN datetime('now') ELSE NULL END,\
             updated_at=datetime('now') WHERE id=?3",
            rusqlite::params![v as i64, status, id],
        ).map_err(|e| e.to_string())?;
    }
    if let Some(v) = position {
        conn.execute("UPDATE tasks SET position=?1,updated_at=datetime('now') WHERE id=?2",
            rusqlite::params![v, id]).map_err(|e| e.to_string())?;
    }
    let sql = format!("SELECT {TASK_COLS} FROM tasks WHERE id=?1");
    conn.query_row(&sql, rusqlite::params![id], |row| row_to_task(row, &conn))
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

    let base = format!(
        "SELECT {TASK_COLS} FROM tasks WHERE completed=0 AND parent_task_id IS NULL"
    );

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

    let mut stmt3 = conn.prepare(
        &format!("{base} AND due_date IS NOT NULL AND due_date > ?1 AND due_date <= ?2 ORDER BY due_date ASC")
    ).map_err(|e| e.to_string())?;
    let upcoming: Vec<Task> = stmt3
        .query_map(rusqlite::params![today, upcoming_limit], |row| row_to_task(row, &conn))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .filter(|t| !overdue_ids.contains(&t.id))
        .collect();

    Ok(DashboardData { overdue, high_priority, upcoming })
}

#[tauri::command]
pub fn create_subtask(
    state: State<DbState>,
    parent_task_id: i64,
    title: String,
) -> Result<Task, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let list_id: i64 = conn
        .query_row("SELECT list_id FROM tasks WHERE id = ?1", rusqlite::params![parent_task_id], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let position: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM tasks WHERE parent_task_id = ?1 AND is_subtask = 1",
            rusqlite::params![parent_task_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO tasks (list_id, parent_task_id, is_subtask, title, priority, position)
         VALUES (?1, ?2, 1, ?3, 'normal', ?4)",
        rusqlite::params![list_id, parent_task_id, title, position],
    )
    .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    let sql = format!("SELECT {TASK_COLS} FROM tasks WHERE id=?1");
    conn.query_row(&sql, rusqlite::params![id], |row| row_to_task(row, &conn))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_subtasks(state: State<DbState>, task_id: i64) -> Result<Vec<Task>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let sql = format!(
        "SELECT {TASK_COLS} FROM tasks WHERE parent_task_id=?1 AND is_subtask=1 ORDER BY position ASC"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let tasks: Vec<Task> = stmt
        .query_map(rusqlite::params![task_id], |row| row_to_task(row, &conn))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(tasks)
}

#[tauri::command]
pub fn create_child_task(
    state: State<DbState>,
    parent_task_id: i64,
    title: String,
) -> Result<Task, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let list_id: i64 = conn
        .query_row("SELECT list_id FROM tasks WHERE id = ?1", rusqlite::params![parent_task_id], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    let position: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM tasks WHERE parent_task_id = ?1 AND is_subtask = 0",
            rusqlite::params![parent_task_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO tasks (list_id, parent_task_id, is_subtask, title, priority, position)
         VALUES (?1, ?2, 0, ?3, 'normal', ?4)",
        rusqlite::params![list_id, parent_task_id, title, position],
    )
    .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    let sql = format!("SELECT {TASK_COLS} FROM tasks WHERE id=?1");
    conn.query_row(&sql, rusqlite::params![id], |row| row_to_task(row, &conn))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_child_tasks(state: State<DbState>, task_id: i64) -> Result<Vec<Task>, String> {
    let conn = state.0.lock().map_err(|e| e.to_string())?;
    let sql = format!(
        "SELECT {TASK_COLS} FROM tasks WHERE parent_task_id=?1 AND is_subtask=0 ORDER BY completed ASC, position ASC"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let tasks: Vec<Task> = stmt
        .query_map(rusqlite::params![task_id], |row| row_to_task(row, &conn))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();
    Ok(tasks)
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
    fn test_create_subtask() {
        let (conn, list_id) = setup();
        conn.execute(
            "INSERT INTO tasks (list_id, title, priority, position) VALUES (?1, 'Parent', 'normal', 0)",
            rusqlite::params![list_id],
        ).unwrap();
        let parent_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO tasks (list_id, parent_task_id, is_subtask, title, priority, position) VALUES (?1, ?2, 1, 'Sub', 'normal', 0)",
            rusqlite::params![list_id, parent_id],
        ).unwrap();
        let (subtask_parent, is_sub): (i64, i64) = conn.query_row(
            "SELECT parent_task_id, is_subtask FROM tasks WHERE title='Sub'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap();
        assert_eq!(subtask_parent, parent_id);
        assert_eq!(is_sub, 1);
    }

    #[test]
    fn test_create_child_task() {
        let (conn, list_id) = setup();
        conn.execute(
            "INSERT INTO tasks (list_id, title, priority, position) VALUES (?1, 'Parent', 'normal', 0)",
            rusqlite::params![list_id],
        ).unwrap();
        let parent_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO tasks (list_id, parent_task_id, is_subtask, title, priority, position) VALUES (?1, ?2, 0, 'Child', 'normal', 0)",
            rusqlite::params![list_id, parent_id],
        ).unwrap();
        let (child_parent, is_sub): (i64, i64) = conn.query_row(
            "SELECT parent_task_id, is_subtask FROM tasks WHERE title='Child'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap();
        assert_eq!(child_parent, parent_id);
        assert_eq!(is_sub, 0);
    }

    #[test]
    fn test_delete_parent_cascades_subtasks() {
        let (conn, list_id) = setup();
        conn.execute(
            "INSERT INTO tasks (list_id, title, priority, position) VALUES (?1, 'Parent', 'normal', 0)",
            rusqlite::params![list_id],
        ).unwrap();
        let parent_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO tasks (list_id, parent_task_id, title, priority, position) VALUES (?1, ?2, 'Sub', 'normal', 0)",
            rusqlite::params![list_id, parent_id],
        ).unwrap();
        conn.execute("DELETE FROM tasks WHERE id=?1", rusqlite::params![parent_id]).unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE parent_task_id=?1",
            rusqlite::params![parent_id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_get_tasks_excludes_subtasks() {
        let (conn, list_id) = setup();
        conn.execute(
            "INSERT INTO tasks (list_id, title, priority, position) VALUES (?1, 'Parent', 'normal', 0)",
            rusqlite::params![list_id],
        ).unwrap();
        let parent_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO tasks (list_id, parent_task_id, title, priority, position) VALUES (?1, ?2, 'Sub', 'normal', 0)",
            rusqlite::params![list_id, parent_id],
        ).unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tasks WHERE list_id=?1 AND parent_task_id IS NULL",
            rusqlite::params![list_id],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, 1);
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

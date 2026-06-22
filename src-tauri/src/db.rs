use rusqlite::{Connection, Result};
use std::sync::Mutex;

pub struct DbState(pub Mutex<Connection>);

pub fn open(path: &str) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS lists (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            title      TEXT NOT NULL,
            color      TEXT,
            position   INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS tasks (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            list_id      INTEGER NOT NULL REFERENCES lists(id) ON DELETE CASCADE,
            title        TEXT NOT NULL,
            description  TEXT,
            priority     TEXT NOT NULL DEFAULT 'normal' CHECK(priority IN ('normal','high')),
            due_date     TEXT,
            completed    INTEGER NOT NULL DEFAULT 0,
            completed_at TEXT,
            position     INTEGER NOT NULL DEFAULT 0,
            created_at   TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS tags (
            id    INTEGER PRIMARY KEY AUTOINCREMENT,
            name  TEXT NOT NULL UNIQUE,
            color TEXT
        );

        CREATE TABLE IF NOT EXISTS task_tags (
            task_id INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
            tag_id  INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
            PRIMARY KEY (task_id, tag_id)
        );

        CREATE TABLE IF NOT EXISTS timer_sessions (
            id               INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id          INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
            started_at       TEXT NOT NULL,
            stopped_at       TEXT,
            duration_seconds INTEGER
        );

        CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
    ")
}

#[cfg(test)]
pub fn open_in_memory() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    migrate(&conn).unwrap();
    conn
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrations_create_all_tables() {
        let conn = open_in_memory();
        let tables: Vec<String> = {
            let mut stmt = conn.prepare(
                "SELECT name FROM sqlite_master WHERE type='table' ORDER BY name"
            ).unwrap();
            stmt.query_map([], |r| r.get(0))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect()
        };
        assert!(tables.contains(&"lists".to_string()));
        assert!(tables.contains(&"tasks".to_string()));
        assert!(tables.contains(&"tags".to_string()));
        assert!(tables.contains(&"task_tags".to_string()));
        assert!(tables.contains(&"timer_sessions".to_string()));
        assert!(tables.contains(&"settings".to_string()));
    }

    #[test]
    fn test_foreign_keys_enabled() {
        let conn = open_in_memory();
        let fk_enabled: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fk_enabled, 1);
    }
}

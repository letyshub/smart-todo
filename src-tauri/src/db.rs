use rusqlite::{Connection, Result};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct DbState(pub Mutex<Connection>);

pub const DB_FILE: &str = "data.db";
const POINTER_FILE: &str = "data-dir.txt";

/// App-owned config directory. Always local — never on a synced drive, because
/// it holds the pointer telling us which data directory to use.
pub fn config_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("smart-todo")
}

/// Data directory chosen by the user, or None to use the default (config dir).
///
/// The pointer lives in a plain file rather than the `settings` table because
/// reading it from SQLite would require already knowing which database to open.
/// A pointer to a directory that no longer exists (cloud folder not synced yet)
/// is ignored, so we fall back to the local database instead of silently
/// creating an empty one inside a half-synced folder.
pub fn read_data_dir() -> Option<PathBuf> {
    read_data_dir_in(&config_dir())
}

fn read_data_dir_in(config: &Path) -> Option<PathBuf> {
    let raw = std::fs::read_to_string(config.join(POINTER_FILE)).ok()?;
    let dir = PathBuf::from(raw.trim());
    if dir.as_os_str().is_empty() || !dir.is_dir() {
        return None;
    }
    Some(dir)
}

pub fn write_data_dir(dir: &Path) -> std::io::Result<()> {
    write_data_dir_in(&config_dir(), dir)
}

fn write_data_dir_in(config: &Path, dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(config)?;
    std::fs::write(config.join(POINTER_FILE), dir.to_string_lossy().as_bytes())
}

/// Full path to the database file the app should open on startup.
pub fn resolve_db_path() -> PathBuf {
    read_data_dir().unwrap_or_else(config_dir).join(DB_FILE)
}

pub fn open(path: &str) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    migrate(&conn)?;
    Ok(conn)
}

/// Flush the write-ahead log into the main database file so that copying
/// `data.db` alone captures everything. Without this the `-wal` file holds
/// recent writes and the copy comes out stale or empty.
pub fn checkpoint(conn: &Connection) -> Result<()> {
    conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
    Ok(())
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
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            list_id         INTEGER NOT NULL REFERENCES lists(id) ON DELETE CASCADE,
            parent_task_id  INTEGER REFERENCES tasks(id) ON DELETE CASCADE,
            title           TEXT NOT NULL,
            description     TEXT,
            priority        TEXT NOT NULL DEFAULT 'normal' CHECK(priority IN ('normal','high')),
            due_date        TEXT,
            completed       INTEGER NOT NULL DEFAULT 0,
            completed_at    TEXT,
            position        INTEGER NOT NULL DEFAULT 0,
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
            status          TEXT NOT NULL DEFAULT 'todo' CHECK(status IN ('todo','inprogress','done'))
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
    ")?;
    // Idempotent column add for existing databases — ignored if column already exists
    conn.execute(
        "ALTER TABLE tasks ADD COLUMN parent_task_id INTEGER REFERENCES tasks(id) ON DELETE CASCADE",
        [],
    ).ok();
    conn.execute(
        "ALTER TABLE tasks ADD COLUMN is_subtask INTEGER NOT NULL DEFAULT 0",
        [],
    ).ok();
    conn.execute(
        "ALTER TABLE tasks ADD COLUMN status TEXT NOT NULL DEFAULT 'todo'",
        [],
    ).ok();
    Ok(())
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
    fn test_checkpoint_flushes_wal_into_db_file() {
        // Regression: copying data.db without checkpointing first lost every
        // write still sitting in the -wal file, so a "moved" database arrived
        // at the destination empty.
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("data.db");
        let conn = open(src.to_str().unwrap()).unwrap();
        conn.execute("INSERT INTO lists(title) VALUES('Praca')", []).unwrap();

        assert!(src.with_extension("db-wal").exists(), "expected WAL mode");
        checkpoint(&conn).unwrap();

        let dst = dir.path().join("copy.db");
        std::fs::copy(&src, &dst).unwrap();

        let copied = Connection::open(&dst).unwrap();
        let n: i64 = copied
            .query_row("SELECT COUNT(*) FROM lists", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1, "checkpointed copy must carry the rows");
    }

    #[test]
    fn test_data_dir_pointer_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("config");
        let target = tmp.path().join("not-synced-yet");

        assert!(read_data_dir_in(&config).is_none(), "no pointer yet");

        write_data_dir_in(&config, &target).unwrap();
        // A pointer at a directory that does not exist (cloud folder still
        // syncing) must not be honoured, or we would create a second, empty
        // database there and the user would see no tasks.
        assert!(read_data_dir_in(&config).is_none(), "missing dir ignored");

        std::fs::create_dir_all(&target).unwrap();
        assert_eq!(read_data_dir_in(&config).as_deref(), Some(target.as_path()));
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

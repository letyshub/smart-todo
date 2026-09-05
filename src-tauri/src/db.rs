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

/// Directory an older version was told to keep the database in.
///
/// Read only to migrate away from it: hosting the database in a cloud-synced
/// folder is what corrupted it, so the path is now used as a sync folder and
/// the database itself comes home.
fn legacy_data_dir_in(config: &Path) -> Option<PathBuf> {
    let raw = std::fs::read_to_string(config.join(POINTER_FILE)).ok()?;
    let dir = PathBuf::from(raw.trim());
    (!dir.as_os_str().is_empty() && dir.is_dir()).then_some(dir)
}

/// Full path to the database file. Always local: never a network share, never
/// a folder a cloud client writes to.
pub fn resolve_db_path() -> PathBuf {
    config_dir().join(DB_FILE)
}

/// What the app has to do before it can open the database.
#[derive(Debug, PartialEq, Eq)]
pub struct Startup {
    pub db_path: PathBuf,
    /// A directory that used to hold the database and should now be adopted as
    /// the sync folder, with the local database published into it.
    pub adopt_sync_dir: Option<PathBuf>,
}

/// Move a database that an earlier version left in a cloud folder back to local
/// storage, and remember that folder so sync can keep using it.
///
/// The copy in the cloud folder is deliberately left where it is: it costs
/// nothing and is the obvious thing to fall back on if anything here surprises
/// the user.
pub fn plan_startup_in(config: &Path) -> Startup {
    let local = config.join(DB_FILE);
    let Some(legacy) = legacy_data_dir_in(config) else {
        return Startup { db_path: local, adopt_sync_dir: None };
    };
    let legacy_db = legacy.join(DB_FILE);

    if !local.exists() && legacy_db.exists() {
        let _ = std::fs::create_dir_all(config);
        // Recent writes may still be sitting in the cloud copy's -wal file;
        // folding them in first is the difference between migrating the user's
        // data and migrating a stale snapshot of it.
        if let Ok(old) = Connection::open(&legacy_db) {
            let _ = checkpoint(&old);
        }
        if std::fs::copy(&legacy_db, &local).is_ok() {
            // Sidecars belonging to the cloud copy describe a different file and
            // would be applied to ours as if they were ours.
            for ext in ["-wal", "-shm"] {
                let mut sidecar = local.clone().into_os_string();
                sidecar.push(ext);
                let _ = std::fs::remove_file(PathBuf::from(sidecar));
            }
        }
    }

    let already_syncing = crate::sync::read_folder(config).is_some();
    Startup {
        db_path: local,
        adopt_sync_dir: (!already_syncing).then_some(legacy),
    }
}

pub fn plan_startup() -> Startup {
    plan_startup_in(&config_dir())
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
    // Sync tables, uuid columns and the change-capture triggers.
    crate::sync::schema::migrate(conn)?;
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
    fn test_startup_is_local_only_when_nothing_was_configured() {
        let tmp = tempfile::tempdir().unwrap();
        let plan = plan_startup_in(tmp.path());
        assert_eq!(plan.db_path, tmp.path().join(DB_FILE));
        assert_eq!(plan.adopt_sync_dir, None);
    }

    #[test]
    fn test_a_database_left_in_a_cloud_folder_is_brought_home() {
        // The upgrade path for everyone hitting corruption today: the database
        // moves to local storage and the cloud folder becomes the sync folder.
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("config");
        let cloud = tmp.path().join("OneDrive/smart-todo");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::create_dir_all(&cloud).unwrap();

        let cloud_db = cloud.join(DB_FILE);
        let conn = open(cloud_db.to_str().unwrap()).unwrap();
        conn.execute("INSERT INTO lists(title) VALUES('Praca')", []).unwrap();
        checkpoint(&conn).unwrap();
        drop(conn);
        std::fs::write(config.join(POINTER_FILE), cloud.to_string_lossy().as_bytes()).unwrap();

        let plan = plan_startup_in(&config);

        assert_eq!(plan.db_path, config.join(DB_FILE));
        assert_eq!(plan.adopt_sync_dir.as_deref(), Some(cloud.as_path()));
        let local = Connection::open(plan.db_path).unwrap();
        let rows: i64 = local
            .query_row("SELECT COUNT(*) FROM lists", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1, "the migrated database keeps the user's data");
        assert!(cloud_db.exists(), "the old copy stays as a fallback");
    }

    #[test]
    fn test_an_existing_local_database_is_never_overwritten_by_the_cloud_copy() {
        let tmp = tempfile::tempdir().unwrap();
        let config = tmp.path().join("config");
        let cloud = tmp.path().join("cloud");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::create_dir_all(&cloud).unwrap();

        let local = open(config.join(DB_FILE).to_str().unwrap()).unwrap();
        local.execute("INSERT INTO lists(title) VALUES('Local')", []).unwrap();
        checkpoint(&local).unwrap();
        drop(local);

        let cloud_conn = open(cloud.join(DB_FILE).to_str().unwrap()).unwrap();
        cloud_conn.execute("INSERT INTO lists(title) VALUES('Cloud')", []).unwrap();
        checkpoint(&cloud_conn).unwrap();
        drop(cloud_conn);
        std::fs::write(config.join(POINTER_FILE), cloud.to_string_lossy().as_bytes()).unwrap();

        plan_startup_in(&config);

        let conn = Connection::open(config.join(DB_FILE)).unwrap();
        let title: String = conn
            .query_row("SELECT title FROM lists", [], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "Local", "local work wins; the cloud copy is merged in by sync");
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

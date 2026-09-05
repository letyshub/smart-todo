//! Sync-side schema: bookkeeping tables, uuid columns, and the triggers that
//! capture every local change.
//!
//! Capture is done with triggers rather than calls inside each command so that
//! a command added later cannot forget to record what it changed.

use crate::sync::model::{EntitySpec, FieldKind, Identity, ENTITIES};
use rusqlite::{Connection, Result};

/// Key whose presence in `sync_meta` disables the capture triggers. Set while
/// applying remote ops and during migrations, so replaying another device's
/// changes does not echo them back into our own log.
pub const SUSPEND_KEY: &str = "suspend_capture";

const SYNC_TABLES: &str = "
    CREATE TABLE IF NOT EXISTS sync_meta (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );

    -- Rows whose local changes have not reached the op log yet.
    CREATE TABLE IF NOT EXISTS sync_outbox (
        id       INTEGER PRIMARY KEY AUTOINCREMENT,
        entity   TEXT NOT NULL,
        local_id INTEGER,
        uuid     TEXT,
        kind     TEXT NOT NULL
    );

    -- Current revision of every individual field, so two devices editing
    -- different fields of one task merge instead of overwriting each other.
    CREATE TABLE IF NOT EXISTS sync_field_revs (
        entity  TEXT NOT NULL,
        uuid    TEXT NOT NULL,
        field   TEXT NOT NULL,
        lamport INTEGER NOT NULL,
        device  TEXT NOT NULL,
        PRIMARY KEY (entity, uuid, field)
    );

    -- Last value both sides are known to agree on, used to work out which
    -- fields a local edit actually touched.
    CREATE TABLE IF NOT EXISTS sync_shadow (
        entity TEXT NOT NULL,
        uuid   TEXT NOT NULL,
        json   TEXT NOT NULL,
        PRIMARY KEY (entity, uuid)
    );

    -- Segment files already consumed, so an unchanged file is not re-read.
    CREATE TABLE IF NOT EXISTS sync_files (
        device  TEXT NOT NULL,
        file    TEXT NOT NULL,
        size    INTEGER NOT NULL,
        mtime   INTEGER NOT NULL,
        max_seq INTEGER NOT NULL,
        PRIMARY KEY (device, file)
    );

    -- Ops that arrived before the row they point at. Retried on later passes
    -- instead of dropped, so arrival order never costs data.
    CREATE TABLE IF NOT EXISTS sync_deferred (
        device   TEXT NOT NULL,
        seq      INTEGER NOT NULL,
        json     TEXT NOT NULL,
        attempts INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (device, seq)
    );

    -- Two devices that independently created the same tag name end up with one
    -- row; this maps the losing uuid onto the surviving one.
    CREATE TABLE IF NOT EXISTS sync_alias (
        entity      TEXT NOT NULL,
        remote_uuid TEXT NOT NULL,
        local_uuid  TEXT NOT NULL,
        PRIMARY KEY (entity, remote_uuid)
    );

    -- Deletions are remembered so a late upsert cannot resurrect a row.
    CREATE TABLE IF NOT EXISTS sync_tombstones (
        entity  TEXT NOT NULL,
        uuid    TEXT NOT NULL,
        lamport INTEGER NOT NULL,
        device  TEXT NOT NULL,
        PRIMARY KEY (entity, uuid)
    );

    -- Fields edited on two devices independently. Sync picks a winner so it
    -- never blocks; the discarded value stays here for the user to review.
    CREATE TABLE IF NOT EXISTS sync_conflicts (
        id            INTEGER PRIMARY KEY AUTOINCREMENT,
        entity        TEXT NOT NULL,
        uuid          TEXT NOT NULL,
        field         TEXT NOT NULL,
        kept          TEXT,
        kept_rev      TEXT NOT NULL,
        discarded     TEXT,
        discarded_rev TEXT NOT NULL,
        detected_at   TEXT NOT NULL,
        resolved      INTEGER NOT NULL DEFAULT 0
    );

    CREATE INDEX IF NOT EXISTS idx_sync_conflicts_open
        ON sync_conflicts(resolved, id);
";

/// Add the uuid column to a table that predates sync, then fill it in.
fn add_uuid_column(conn: &Connection, spec: &EntitySpec) -> Result<()> {
    // ALTER TABLE ADD COLUMN rejects a non-constant DEFAULT, so the column
    // arrives empty and is backfilled here. Failure means it already exists.
    conn.execute(&format!("ALTER TABLE {} ADD COLUMN uuid TEXT", spec.table), [])
        .ok();
    conn.execute(
        &format!(
            "UPDATE {} SET uuid = lower(hex(randomblob(16))) WHERE uuid IS NULL",
            spec.table
        ),
        [],
    )?;
    conn.execute(
        &format!(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_{}_uuid ON {}(uuid)",
            spec.table, spec.table
        ),
        [],
    )?;
    Ok(())
}

/// SQL expression building a join row's composite uuid from the two rows it links.
fn composite_key(spec: &EntitySpec, row: &str) -> String {
    let mut parts = Vec::new();
    for f in spec.fields {
        let FieldKind::Ref(target) = f.kind else {
            continue;
        };
        let table = crate::sync::model::entity(target)
            .expect("composite entity references a known entity")
            .table;
        parts.push(format!(
            "(SELECT uuid FROM {table} WHERE id = {row}.{})",
            f.column
        ));
    }
    parts.join(" || ':' || ")
}

fn triggers_for(spec: &EntitySpec) -> String {
    let (table, name) = (spec.table, spec.name);
    let guard = format!("WHEN NOT EXISTS (SELECT 1 FROM sync_meta WHERE key = '{SUSPEND_KEY}')");

    let mut sql = String::new();
    for event in ["ai", "au", "ad"] {
        sql.push_str(&format!("DROP TRIGGER IF EXISTS sync_{event}_{table};\n"));
    }

    match spec.identity {
        Identity::Uuid => sql.push_str(&format!(
            "CREATE TRIGGER sync_ai_{table} AFTER INSERT ON {table} {guard}
             BEGIN
                 UPDATE {table} SET uuid = lower(hex(randomblob(16)))
                  WHERE id = NEW.id AND uuid IS NULL;
                 INSERT INTO sync_outbox(entity, local_id, kind)
                 VALUES ('{name}', NEW.id, 'upsert');
             END;
             CREATE TRIGGER sync_au_{table} AFTER UPDATE ON {table} {guard}
             BEGIN
                 INSERT INTO sync_outbox(entity, local_id, kind)
                 VALUES ('{name}', NEW.id, 'upsert');
             END;
             CREATE TRIGGER sync_ad_{table} AFTER DELETE ON {table} {guard}
             BEGIN
                 INSERT INTO sync_outbox(entity, uuid, kind)
                 VALUES ('{name}', OLD.uuid, 'delete');
             END;\n"
        )),
        Identity::Composite => sql.push_str(&format!(
            "CREATE TRIGGER sync_ai_{table} AFTER INSERT ON {table} {guard}
             BEGIN
                 INSERT INTO sync_outbox(entity, uuid, kind)
                 VALUES ('{name}', {new_key}, 'upsert');
             END;
             CREATE TRIGGER sync_ad_{table} AFTER DELETE ON {table} {guard}
             BEGIN
                 INSERT INTO sync_outbox(entity, uuid, kind)
                 VALUES ('{name}', {old_key}, 'delete');
             END;\n",
            new_key = composite_key(spec, "NEW"),
            old_key = composite_key(spec, "OLD"),
        )),
    }
    sql
}

/// Bring a database up to the sync-capable schema. Safe to run on every open.
pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(SYNC_TABLES)?;

    // Backfilling uuids is itself a write; without suspending capture the whole
    // database would land in the outbox looking like fresh user edits.
    suspend(conn, true)?;
    let backfilled = ENTITIES
        .iter()
        .filter(|s| s.identity == Identity::Uuid)
        .try_for_each(|s| add_uuid_column(conn, s));
    suspend(conn, false)?;
    backfilled?;

    for spec in ENTITIES {
        conn.execute_batch(&triggers_for(spec))?;
    }
    Ok(())
}

pub fn suspend(conn: &Connection, on: bool) -> Result<()> {
    if on {
        conn.execute(
            "INSERT OR REPLACE INTO sync_meta(key, value) VALUES (?1, '1')",
            [SUSPEND_KEY],
        )?;
    } else {
        conn.execute("DELETE FROM sync_meta WHERE key = ?1", [SUSPEND_KEY])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::db::open_in_memory;
    use rusqlite::Connection;

    fn outbox(conn: &Connection) -> Vec<(String, String)> {
        let mut stmt = conn
            .prepare("SELECT entity, kind FROM sync_outbox ORDER BY id")
            .unwrap();
        let rows = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        rows
    }

    #[test]
    fn inserting_a_row_mints_a_uuid_and_queues_it() {
        let conn = open_in_memory();
        conn.execute("INSERT INTO lists(title, position) VALUES('Work', 0)", [])
            .unwrap();
        let uuid: Option<String> = conn.query_row("SELECT uuid FROM lists", [], |r| r.get(0)).unwrap();
        assert!(
            uuid.is_some_and(|u| u.len() == 32),
            "the insert trigger should mint a uuid"
        );
        assert!(outbox(&conn).contains(&("list".into(), "upsert".into())));
    }

    #[test]
    fn deleting_a_list_queues_its_cascaded_tasks_too() {
        // The other device has to learn the tasks are gone, not just the list.
        let conn = open_in_memory();
        conn.execute("INSERT INTO lists(title, position) VALUES('Work', 0)", [])
            .unwrap();
        let list_id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO tasks(list_id, title, position) VALUES(?1, 'a', 0)",
            rusqlite::params![list_id],
        )
        .unwrap();
        conn.execute("DELETE FROM sync_outbox", []).unwrap();

        conn.execute("DELETE FROM lists WHERE id = ?1", rusqlite::params![list_id])
            .unwrap();

        let queued = outbox(&conn);
        assert!(queued.contains(&("list".into(), "delete".into())));
        assert!(
            queued.contains(&("task".into(), "delete".into())),
            "cascaded deletes must be captured"
        );
    }

    #[test]
    fn suspending_capture_leaves_the_outbox_untouched() {
        // Applying a remote op must not queue that same change for sending back.
        let conn = open_in_memory();
        super::suspend(&conn, true).unwrap();
        conn.execute("INSERT INTO lists(title, position) VALUES('Work', 0)", [])
            .unwrap();
        super::suspend(&conn, false).unwrap();
        assert!(outbox(&conn).is_empty());
    }

    #[test]
    fn tag_links_are_queued_under_the_pair_of_uuids_they_join() {
        let conn = open_in_memory();
        conn.execute("INSERT INTO lists(title, position) VALUES('W', 0)", [])
            .unwrap();
        conn.execute("INSERT INTO tasks(list_id, title, position) VALUES(1, 't', 0)", [])
            .unwrap();
        conn.execute("INSERT INTO tags(name) VALUES('home')", []).unwrap();
        conn.execute("DELETE FROM sync_outbox", []).unwrap();

        conn.execute("INSERT INTO task_tags(task_id, tag_id) VALUES(1, 1)", [])
            .unwrap();

        let uuid: String = conn
            .query_row("SELECT uuid FROM sync_outbox WHERE entity = 'task_tag'", [], |r| {
                r.get(0)
            })
            .unwrap();
        let (task_uuid, tag_uuid) = uuid.split_once(':').expect("composite uuid");
        assert_eq!(task_uuid.len(), 32);
        assert_eq!(tag_uuid.len(), 32);
    }

    #[test]
    fn migration_is_idempotent() {
        let conn = open_in_memory();
        super::migrate(&conn).unwrap();
        super::migrate(&conn).unwrap();
        let queued: i64 = conn
            .query_row("SELECT COUNT(*) FROM sync_outbox", [], |r| r.get(0))
            .unwrap();
        assert_eq!(queued, 0, "re-running migration must not look like user edits");
    }
}

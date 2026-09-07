//! File-based sync between devices over a shared cloud folder.
//!
//! The database itself stays local on every machine. What travels through
//! OneDrive (or iCloud, Dropbox, anything that syncs a folder) is an
//! append-only log of changes, split so that each device only ever writes its
//! own files. Nothing has two writers, so the cloud provider has nothing to
//! conflict over and no SQLite file is ever open across machines.

pub mod apply;
pub mod capture;
pub mod meta;
pub mod model;
pub mod op;
pub mod row;
pub mod schema;
pub mod store;

use crate::sync::op::{Op, OpKind};
use crate::sync::store::{DeviceMeta, SyncFolder};
use rusqlite::Connection;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// An op is dropped after this many failed passes. Reaching it means the row it
/// points at was never going to arrive — usually because the peer's log was
/// reset — and retrying forever would just grow the table.
const MAX_DEFER_ATTEMPTS: i64 = 50;

/// Rewrite our own log once it holds more than this many ops.
const COMPACT_THRESHOLD: usize = 2_000;

const POINTER_FILE: &str = "sync-dir.txt";

#[derive(Debug)]
pub enum SyncError {
    Db(rusqlite::Error),
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::Db(e) => write!(f, "database error: {e}"),
            SyncError::Io(e) => write!(f, "sync folder error: {e}"),
            SyncError::Json(e) => write!(f, "malformed change log: {e}"),
        }
    }
}

impl std::error::Error for SyncError {}

impl From<rusqlite::Error> for SyncError {
    fn from(e: rusqlite::Error) -> Self {
        SyncError::Db(e)
    }
}
impl From<std::io::Error> for SyncError {
    fn from(e: std::io::Error) -> Self {
        SyncError::Io(e)
    }
}
impl From<serde_json::Error> for SyncError {
    fn from(e: serde_json::Error) -> Self {
        SyncError::Json(e)
    }
}

#[derive(Debug, Default, Clone, Serialize, PartialEq, Eq)]
pub struct SyncReport {
    pub pushed: usize,
    pub applied: usize,
    pub conflicts: usize,
    pub waiting: usize,
}

/// Where the pointer to the sync folder lives. Kept next to the database rather
/// than inside it, because it has to be readable before anything is open.
pub fn pointer_path(config_dir: &Path) -> PathBuf {
    config_dir.join(POINTER_FILE)
}

pub fn read_folder(config_dir: &Path) -> Option<PathBuf> {
    let raw = std::fs::read_to_string(pointer_path(config_dir)).ok()?;
    let dir = PathBuf::from(raw.trim());
    // A cloud folder that has not synced down yet must not be treated as an
    // empty one, or we would publish a fresh log alongside the real history.
    (!dir.as_os_str().is_empty() && dir.is_dir()).then_some(dir)
}

pub fn write_folder(config_dir: &Path, dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(config_dir)?;
    std::fs::write(pointer_path(config_dir), dir.to_string_lossy().as_bytes())
}

pub fn clear_folder(config_dir: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(pointer_path(config_dir)) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

fn defer(conn: &Connection, op: &Op) -> Result<(), SyncError> {
    conn.execute(
        "INSERT INTO sync_deferred(device, seq, json, attempts) VALUES (?1, ?2, ?3, 0)
         ON CONFLICT(device, seq) DO UPDATE SET attempts = sync_deferred.attempts + 1",
        rusqlite::params![op.device, op.seq as i64, serde_json::to_string(op)?],
    )?;
    Ok(())
}

/// Publish everything captured since the last pass.
///
/// The drain and the file append share a transaction: if the folder cannot be
/// written (cloud client holding a lock, drive not mounted), the ops stay in
/// the outbox and go out next time instead of vanishing.
fn push(conn: &Connection, folder: &SyncFolder, device: &str) -> Result<usize, SyncError> {
    let tx = conn.unchecked_transaction()?;
    let ops = capture::drain(conn)?;
    if ops.is_empty() {
        tx.rollback()?;
        return Ok(0);
    }
    folder.append(device, &ops)?;
    tx.commit()?;
    Ok(ops.len())
}

/// Ops from segments that have changed since we last read them.
///
/// A segment is identified by its size and timestamp rather than by how far a
/// sequence number got, because a cloud client is free to deliver segments out
/// of order or replace them wholesale after the peer compacts its log. Ops that
/// come round again are harmless: applying one twice changes nothing.
fn changed_ops(conn: &Connection, folder: &SyncFolder, device: &str) -> Result<Vec<Op>, SyncError> {
    let mut ops = Vec::new();

    for segment in folder.segments(device)? {
        let name = segment.path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let cached: Option<(i64, i64)> = conn
            .query_row(
                "SELECT size, mtime FROM sync_files WHERE device = ?1 AND file = ?2",
                [device, name.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .ok();
        if cached == Some((segment.size as i64, segment.mtime)) {
            continue;
        }

        let found = store::read_segment(&segment.path)?;
        let max_seq = found.iter().map(|o| o.seq).max().unwrap_or(0);
        conn.execute(
            "INSERT INTO sync_files(device, file, size, mtime, max_seq) VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(device, file) DO UPDATE
                SET size = excluded.size, mtime = excluded.mtime, max_seq = excluded.max_seq",
            rusqlite::params![device, name, segment.size as i64, segment.mtime, max_seq as i64],
        )?;
        ops.extend(found);
    }

    // A device writes its own ops in order, so replaying them in that order
    // reproduces the sequence of edits the user actually made.
    ops.sort_by_key(|o| o.seq);
    Ok(ops)
}

fn retry_deferred(conn: &Connection, report: &mut SyncReport) -> Result<(), SyncError> {
    let mut stmt = conn.prepare("SELECT device, seq, json, attempts FROM sync_deferred ORDER BY device, seq")?;
    let rows: Vec<(String, i64, String, i64)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<Result<_, _>>()?;

    for (device, seq, json, attempts) in rows {
        let Ok(op) = serde_json::from_str::<Op>(&json) else {
            conn.execute(
                "DELETE FROM sync_deferred WHERE device = ?1 AND seq = ?2",
                rusqlite::params![device, seq],
            )?;
            continue;
        };
        match apply::apply(conn, &op)? {
            apply::Applied::Deferred(_) if attempts < MAX_DEFER_ATTEMPTS => {
                conn.execute(
                    "UPDATE sync_deferred SET attempts = attempts + 1 WHERE device = ?1 AND seq = ?2",
                    rusqlite::params![device, seq],
                )?;
            }
            outcome => {
                if let apply::Applied::Ok { conflicts } = outcome {
                    report.applied += 1;
                    report.conflicts += conflicts;
                }
                conn.execute(
                    "DELETE FROM sync_deferred WHERE device = ?1 AND seq = ?2",
                    rusqlite::params![device, seq],
                )?;
            }
        }
    }
    Ok(())
}

fn pull(conn: &Connection, folder: &SyncFolder, device: &str, report: &mut SyncReport) -> Result<(), SyncError> {
    for peer in folder.peers(device)? {
        let ops = changed_ops(conn, folder, &peer)?;
        if ops.is_empty() {
            continue;
        }
        let tx = conn.unchecked_transaction()?;
        for op in &ops {
            match apply::apply(conn, op)? {
                apply::Applied::Ok { conflicts } => {
                    report.applied += 1;
                    report.conflicts += conflicts;
                }
                // Held for a later pass; the op itself is safe in sync_deferred.
                apply::Applied::Deferred(_) => defer(conn, op)?,
                apply::Applied::Ignored(_) => {}
            }
        }
        tx.commit()?;
    }
    Ok(())
}

/// Drop ops from our own log that nothing can learn from any more.
///
/// An op survives only if it still sets at least one field that no later op
/// overwrites; a delete makes everything before it for that row redundant.
fn prune(ops: Vec<Op>) -> Vec<Op> {
    let mut deleted: HashSet<(String, String)> = HashSet::new();
    let mut covered: HashMap<(String, String), HashSet<String>> = HashMap::new();
    let mut kept: Vec<Op> = Vec::new();

    for op in ops.into_iter().rev() {
        let key = (op.entity.clone(), op.uuid.clone());
        if deleted.contains(&key) {
            continue;
        }
        match op.kind {
            OpKind::Delete => {
                deleted.insert(key);
                kept.push(op);
            }
            OpKind::Upsert => {
                let seen = covered.entry(key).or_default();
                // Join rows carry no fields; the newest one is all that matters.
                let is_new = if op.fields.is_empty() {
                    seen.insert(String::new())
                } else {
                    op.fields.keys().fold(false, |acc, f| seen.insert(f.clone()) || acc)
                };
                if is_new {
                    kept.push(op);
                }
            }
        }
    }
    kept.reverse();
    kept
}

fn compact(folder: &SyncFolder, device: &str) -> Result<(), SyncError> {
    let mut ops = Vec::new();
    for segment in folder.segments(device)? {
        ops.extend(store::read_segment(&segment.path)?);
    }
    if ops.len() < COMPACT_THRESHOLD {
        return Ok(());
    }
    ops.sort_by_key(|o| o.seq);
    let pruned = prune(ops);
    folder.rewrite(device, &pruned)?;
    Ok(())
}

/// One full pass: publish local changes, take in remote ones, retry whatever
/// was waiting on a row that had not arrived.
pub fn run(conn: &Connection, folder: &SyncFolder) -> Result<SyncReport, SyncError> {
    let device = meta::device_id(conn)?;
    let mut report = SyncReport::default();

    report.pushed = push(conn, folder, &device)?;
    pull(conn, folder, &device, &mut report)?;
    retry_deferred(conn, &mut report)?;
    // Counted from the table rather than tallied along the way: an op deferred
    // during the pull and retried moments later is one op waiting, not two.
    report.waiting = conn.query_row("SELECT COUNT(*) FROM sync_deferred", [], |r| {
        r.get::<_, i64>(0)
    })? as usize;

    folder.write_meta(&DeviceMeta {
        device_id: device.clone(),
        name: meta::device_name(conn)?,
        platform: std::env::consts::OS.to_string(),
        last_seen: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        compaction_epoch: 0,
    })?;
    compact(folder, &device)?;

    Ok(report)
}

/// Start publishing into `dir`, sending the database we already have.
pub fn adopt_folder(conn: &Connection, config_dir: &Path, dir: &Path) -> Result<SyncReport, SyncError> {
    std::fs::create_dir_all(dir)?;
    write_folder(config_dir, dir)?;
    capture::seed_outbox(conn)?;
    run(conn, &SyncFolder::new(dir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    /// Two independent databases sharing one folder, as two machines would.
    struct Pair {
        a: Connection,
        b: Connection,
        folder: SyncFolder,
        _dir: tempfile::TempDir,
    }

    impl Pair {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let folder = SyncFolder::new(dir.path());
            let (a, b) = (open_in_memory(), open_in_memory());
            // Distinct ids, as two real installs would have.
            meta::set(&a, meta::DEVICE_ID, "aaaaaaaa").unwrap();
            meta::set(&b, meta::DEVICE_ID, "bbbbbbbb").unwrap();
            Pair { a, b, folder, _dir: dir }
        }

        fn sync_a(&self) -> SyncReport {
            run(&self.a, &self.folder).unwrap()
        }
        fn sync_b(&self) -> SyncReport {
            run(&self.b, &self.folder).unwrap()
        }
        /// Settle both sides: each needs a pass to send and one to receive.
        fn settle(&self) {
            for _ in 0..2 {
                self.sync_a();
                self.sync_b();
            }
        }
    }

    fn titles(conn: &Connection) -> Vec<String> {
        let mut stmt = conn.prepare("SELECT title FROM tasks ORDER BY title").unwrap();
        let out = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        out
    }

    #[test]
    fn a_task_created_on_one_machine_shows_up_on_the_other() {
        let p = Pair::new();
        p.a.execute("INSERT INTO lists(title, position) VALUES('Work', 0)", []).unwrap();
        p.a.execute("INSERT INTO tasks(list_id, title, position) VALUES(1, 'Buy milk', 0)", [])
            .unwrap();

        p.sync_a();
        p.sync_b();

        assert_eq!(titles(&p.b), vec!["Buy milk".to_string()]);
    }

    #[test]
    fn both_machines_converge_after_editing_at_the_same_time() {
        let p = Pair::new();
        p.a.execute("INSERT INTO lists(title, position) VALUES('Work', 0)", []).unwrap();
        p.a.execute("INSERT INTO tasks(list_id, title, position) VALUES(1, 'Draft', 0)", [])
            .unwrap();
        p.settle();

        // Neither machine has seen the other's change when it makes its own.
        p.a.execute("UPDATE tasks SET title = 'From A' WHERE id = 1", []).unwrap();
        p.b.execute("UPDATE tasks SET title = 'From B' WHERE id = 1", []).unwrap();
        p.settle();

        assert_eq!(titles(&p.a), titles(&p.b), "the two machines must agree");
        assert_eq!(titles(&p.a).len(), 1);
    }

    #[test]
    fn the_machine_whose_edit_was_overridden_is_the_one_told_about_it() {
        // Only one side needs the notice, and it is the side whose value is
        // about to vanish from the screen — that is where restoring it means
        // anything.
        let p = Pair::new();
        p.a.execute("INSERT INTO lists(title, position) VALUES('Work', 0)", []).unwrap();
        p.settle();

        p.a.execute("UPDATE lists SET title = 'Job' WHERE id = 1", []).unwrap();
        p.b.execute("UPDATE lists SET title = 'Office' WHERE id = 1", []).unwrap();
        p.settle();

        let open = |conn: &Connection| -> i64 {
            conn.query_row("SELECT COUNT(*) FROM sync_conflicts WHERE resolved = 0", [], |r| {
                r.get(0)
            })
            .unwrap()
        };
        let title = |conn: &Connection| -> String {
            conn.query_row("SELECT title FROM lists", [], |r| r.get(0)).unwrap()
        };

        assert_eq!(title(&p.a), title(&p.b), "both machines must show the same title");
        let loser = if title(&p.a) == "Job" { &p.b } else { &p.a };
        let winner = if title(&p.a) == "Job" { &p.a } else { &p.b };
        assert_eq!(open(loser), 1, "the overridden edit is reported");
        assert_eq!(open(winner), 0, "nothing changed here, so there is nothing to report");
    }

    #[test]
    fn a_discarded_value_is_kept_so_it_can_be_put_back() {
        let p = Pair::new();
        p.a.execute("INSERT INTO lists(title, position) VALUES('Work', 0)", []).unwrap();
        p.settle();
        p.a.execute("UPDATE lists SET title = 'Job' WHERE id = 1", []).unwrap();
        p.b.execute("UPDATE lists SET title = 'Office' WHERE id = 1", []).unwrap();
        p.settle();

        for conn in [&p.a, &p.b] {
            let rows: Vec<(String, String)> = {
                let mut stmt = conn
                    .prepare("SELECT kept, discarded FROM sync_conflicts")
                    .unwrap();
                stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                    .unwrap()
                    .map(|r| r.unwrap())
                    .collect()
            };
            for (kept, discarded) in rows {
                assert_ne!(kept, discarded);
                assert!(discarded == "\"Job\"" || discarded == "\"Office\"");
            }
        }
    }

    #[test]
    fn edits_to_different_fields_merge_without_a_conflict() {
        let p = Pair::new();
        p.a.execute("INSERT INTO lists(title, position) VALUES('Work', 0)", []).unwrap();
        p.a.execute("INSERT INTO tasks(list_id, title, position) VALUES(1, 'Task', 0)", [])
            .unwrap();
        p.settle();

        p.a.execute("UPDATE tasks SET description = 'notes' WHERE id = 1", []).unwrap();
        p.b.execute("UPDATE tasks SET due_date = '2026-09-10' WHERE id = 1", []).unwrap();
        p.settle();

        for conn in [&p.a, &p.b] {
            let (desc, due): (String, String) = conn
                .query_row("SELECT description, due_date FROM tasks", [], |r| {
                    Ok((r.get(0)?, r.get(1)?))
                })
                .unwrap();
            assert_eq!(desc, "notes");
            assert_eq!(due, "2026-09-10");
            let conflicts: i64 = conn
                .query_row("SELECT COUNT(*) FROM sync_conflicts", [], |r| r.get(0))
                .unwrap();
            assert_eq!(conflicts, 0, "different fields are not a conflict");
        }
    }

    #[test]
    fn a_deletion_travels() {
        let p = Pair::new();
        p.a.execute("INSERT INTO lists(title, position) VALUES('Work', 0)", []).unwrap();
        p.a.execute("INSERT INTO tasks(list_id, title, position) VALUES(1, 'Gone', 0)", [])
            .unwrap();
        p.settle();
        assert_eq!(titles(&p.b).len(), 1);

        p.a.execute("DELETE FROM tasks WHERE id = 1", []).unwrap();
        p.settle();

        assert!(titles(&p.b).is_empty());
    }

    #[test]
    fn tags_survive_the_trip_and_are_not_duplicated() {
        let p = Pair::new();
        p.a.execute("INSERT INTO lists(title, position) VALUES('Work', 0)", []).unwrap();
        p.a.execute("INSERT INTO tasks(list_id, title, position) VALUES(1, 'Tagged', 0)", [])
            .unwrap();
        p.a.execute("INSERT INTO tags(name) VALUES('urgent')", []).unwrap();
        p.a.execute("INSERT INTO task_tags(task_id, tag_id) VALUES(1, 1)", []).unwrap();
        // The other machine invented the same tag on its own.
        p.b.execute("INSERT INTO tags(name) VALUES('urgent')", []).unwrap();
        p.settle();

        for conn in [&p.a, &p.b] {
            let tags: i64 = conn.query_row("SELECT COUNT(*) FROM tags", [], |r| r.get(0)).unwrap();
            assert_eq!(tags, 1, "one tag name, one row");
            let links: i64 = conn
                .query_row("SELECT COUNT(*) FROM task_tags", [], |r| r.get(0))
                .unwrap();
            assert_eq!(links, 1);
        }
    }

    #[test]
    fn a_database_that_predates_sync_is_published_when_the_folder_is_chosen() {
        let dir = tempfile::tempdir().unwrap();
        let config = tempfile::tempdir().unwrap();
        let a = open_in_memory();
        meta::set(&a, meta::DEVICE_ID, "aaaaaaaa").unwrap();
        schema::suspend(&a, true).unwrap();
        a.execute("INSERT INTO lists(title, position, uuid) VALUES('Old', 0, 'l1')", [])
            .unwrap();
        a.execute(
            "INSERT INTO tasks(list_id, title, position, uuid) VALUES(1, 'Legacy', 0, 't1')",
            [],
        )
        .unwrap();
        schema::suspend(&a, false).unwrap();

        adopt_folder(&a, config.path(), dir.path()).unwrap();

        let b = open_in_memory();
        meta::set(&b, meta::DEVICE_ID, "bbbbbbbb").unwrap();
        run(&b, &SyncFolder::new(dir.path())).unwrap();
        assert_eq!(titles(&b), vec!["Legacy".to_string()]);
    }

    #[test]
    fn an_op_that_arrives_before_its_list_is_retried_not_lost() {
        // Segments can reach the other machine out of order, so a task may land
        // before the list it belongs to.
        let p = Pair::new();
        p.a.execute("INSERT INTO lists(title, position) VALUES('Work', 0)", []).unwrap();
        p.a.execute("INSERT INTO tasks(list_id, title, position) VALUES(1, 'Orphan', 0)", [])
            .unwrap();
        let ops = capture::drain(&p.a).unwrap();
        let (list_ops, task_ops): (Vec<Op>, Vec<Op>) =
            ops.into_iter().partition(|o| o.entity == "list");

        // Deliver the task first.
        p.folder.append("aaaaaaaa", &task_ops).unwrap();
        let first = run(&p.b, &p.folder).unwrap();
        assert_eq!(first.waiting, 1);
        assert!(titles(&p.b).is_empty());

        p.folder.append("aaaaaaaa", &list_ops).unwrap();
        run(&p.b, &p.folder).unwrap();
        assert_eq!(titles(&p.b), vec!["Orphan".to_string()]);
    }

    #[test]
    fn pruning_keeps_the_creation_and_the_newest_value_of_each_field() {
        let p = Pair::new();
        p.a.execute("INSERT INTO lists(title, position) VALUES('Work', 0)", []).unwrap();
        p.a.execute("INSERT INTO tasks(list_id, title, position) VALUES(1, 'v1', 0)", [])
            .unwrap();
        let mut ops = capture::drain(&p.a).unwrap();
        p.a.execute("UPDATE tasks SET title = 'v2' WHERE id = 1", []).unwrap();
        ops.extend(capture::drain(&p.a).unwrap());
        p.a.execute("UPDATE tasks SET title = 'v3' WHERE id = 1", []).unwrap();
        ops.extend(capture::drain(&p.a).unwrap());

        let pruned = prune(ops);

        // Replaying the pruned log onto a fresh machine must still work.
        p.folder.append("aaaaaaaa", &pruned).unwrap();
        run(&p.b, &p.folder).unwrap();
        assert_eq!(titles(&p.b), vec!["v3".to_string()]);
        assert!(pruned.len() < 5, "superseded ops should be gone");
    }

    #[test]
    fn pruning_drops_everything_before_a_deletion() {
        let p = Pair::new();
        p.a.execute("INSERT INTO lists(title, position) VALUES('Work', 0)", []).unwrap();
        p.a.execute("INSERT INTO tasks(list_id, title, position) VALUES(1, 'Doomed', 0)", [])
            .unwrap();
        let mut ops = capture::drain(&p.a).unwrap();
        p.a.execute("DELETE FROM tasks WHERE id = 1", []).unwrap();
        ops.extend(capture::drain(&p.a).unwrap());

        let pruned = prune(ops);

        let task_ops: Vec<&Op> = pruned.iter().filter(|o| o.entity == "task").collect();
        assert_eq!(task_ops.len(), 1);
        assert_eq!(task_ops[0].kind, OpKind::Delete);
    }

    #[test]
    fn a_folder_that_has_not_synced_down_yet_is_not_treated_as_configured() {
        let config = tempfile::tempdir().unwrap();
        let missing = config.path().join("OneDrive-not-here");
        write_folder(config.path(), &missing).unwrap();
        assert!(
            read_folder(config.path()).is_none(),
            "publishing into a folder that is not there would fork the history"
        );
    }

    #[test]
    fn syncing_twice_with_no_changes_does_nothing() {
        let p = Pair::new();
        p.a.execute("INSERT INTO lists(title, position) VALUES('Work', 0)", []).unwrap();
        p.settle();

        assert_eq!(p.sync_a(), SyncReport::default());
        assert_eq!(p.sync_b(), SyncReport::default());
    }
}

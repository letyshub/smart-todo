//! Applying another device's ops to the local database.
//!
//! Merging is per field. For each field the op carries the revision it was
//! written on top of (`base`):
//!
//! * `base` matches the revision we hold  -> the writer had seen our value, so
//!   this is a clean successor and simply lands.
//! * `base` does not match               -> the two devices edited that field
//!   without knowing about each other. The higher revision wins so both
//!   machines end up identical, and the losing value is filed as a conflict for
//!   the user to look at.
//!
//! Sync therefore never stops to ask a question and never blocks on a conflict.

use crate::sync::capture::{clear_shadow, field_rev, set_field_rev, set_tombstone, tombstone};
use crate::sync::model::{entity, EntitySpec, FieldKind, Identity};
use crate::sync::op::{Op, OpKind, Rev};
use crate::sync::row::{self, Fields};
use crate::sync::{meta, schema, SyncError};
use rusqlite::Connection;
use serde_json::Value;

#[derive(Debug, PartialEq, Eq)]
pub enum Applied {
    /// Landed, possibly after filing conflicts.
    Ok { conflicts: usize },
    /// A row it references has not arrived yet; retry on a later pass.
    Deferred(&'static str),
    /// Already superseded by something we hold.
    Ignored(&'static str),
}

/// Follow a remote uuid to the local row it was merged into, if any.
fn resolve_alias(conn: &Connection, entity_name: &str, uuid: &str) -> Result<String, SyncError> {
    let mapped: Option<String> = conn
        .query_row(
            "SELECT local_uuid FROM sync_alias WHERE entity = ?1 AND remote_uuid = ?2",
            [entity_name, uuid],
            |r| r.get(0),
        )
        .ok();
    Ok(mapped.unwrap_or_else(|| uuid.to_string()))
}

fn set_alias(conn: &Connection, entity_name: &str, remote: &str, local: &str) -> Result<(), SyncError> {
    conn.execute(
        "INSERT OR REPLACE INTO sync_alias(entity, remote_uuid, local_uuid) VALUES (?1, ?2, ?3)",
        [entity_name, remote, local],
    )?;
    Ok(())
}

/// Translate a field value from op form into the value the column takes.
///
/// Reference fields arrive as uuids; `None` means the referenced row is not
/// here yet and the caller has to retry later.
fn column_value(
    conn: &Connection,
    spec_field_kind: &FieldKind,
    entity_name: &str,
    value: &Value,
) -> Result<Option<Value>, SyncError> {
    match spec_field_kind {
        FieldKind::Scalar => Ok(Some(value.clone())),
        FieldKind::Ref(target) => {
            if value.is_null() {
                return Ok(Some(Value::Null));
            }
            let Some(uuid) = value.as_str() else { return Ok(Some(Value::Null)) };
            let uuid = resolve_alias(conn, target, uuid)?;
            let table = entity(target).expect("known entity").table;
            let _ = entity_name;
            Ok(row::local_id(conn, table, &uuid)?.map(Value::from))
        }
    }
}

fn record_conflict(
    conn: &Connection,
    entity_name: &str,
    uuid: &str,
    field: &str,
    kept: &Value,
    kept_rev: &Rev,
    discarded: &Value,
    discarded_rev: &Rev,
) -> Result<(), SyncError> {
    conn.execute(
        "INSERT INTO sync_conflicts
            (entity, uuid, field, kept, kept_rev, discarded, discarded_rev, detected_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            entity_name,
            uuid,
            field,
            kept.to_string(),
            kept_rev.to_string(),
            discarded.to_string(),
            discarded_rev.to_string(),
            chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        ],
    )?;
    Ok(())
}

fn insert_row(
    conn: &Connection,
    spec: &EntitySpec,
    uuid: &str,
    values: &Fields,
) -> Result<(), SyncError> {
    let mut columns = vec!["uuid".to_string()];
    let mut params: Vec<rusqlite::types::Value> = vec![rusqlite::types::Value::Text(uuid.into())];

    for f in spec.fields {
        match values.get(f.name) {
            Some(v) => {
                columns.push(f.column.to_string());
                params.push(row::to_sql(v));
            }
            // A NOT NULL column the op did not carry would abort the insert.
            // An empty placeholder keeps the row (and the user's other fields)
            // rather than dropping the whole thing.
            None if f.required => {
                columns.push(f.column.to_string());
                params.push(rusqlite::types::Value::Text(String::new()));
            }
            None => {}
        }
    }

    let placeholders = (1..=columns.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(", ");
    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({placeholders})",
        spec.table,
        columns.join(", ")
    );
    conn.execute(&sql, rusqlite::params_from_iter(params))?;
    Ok(())
}

fn update_fields(
    conn: &Connection,
    spec: &EntitySpec,
    uuid: &str,
    values: &Fields,
) -> Result<(), SyncError> {
    for (name, value) in values {
        let Some(f) = spec.field(name) else { continue };
        let sql = format!("UPDATE {} SET {} = ?1 WHERE uuid = ?2", spec.table, f.column);
        conn.execute(&sql, rusqlite::params![row::to_sql(value), uuid])?;
    }
    Ok(())
}

/// Merge a tag that another device created independently into the one we
/// already have with that name, so the user does not end up with two identical
/// tags that cannot be told apart.
fn adopt_existing_tag(
    conn: &Connection,
    spec: &EntitySpec,
    op: &Op,
    uuid: &str,
) -> Result<Option<String>, SyncError> {
    if spec.name != "tag" {
        return Ok(None);
    }
    let Some(name) = op.fields.get("name").and_then(|f| f.v.as_str()) else {
        return Ok(None);
    };
    let existing: Option<String> = conn
        .query_row("SELECT uuid FROM tags WHERE name = ?1", [name], |r| r.get(0))
        .ok();
    match existing {
        Some(local) if local != uuid => {
            set_alias(conn, "tag", uuid, &local)?;
            Ok(Some(local))
        }
        _ => Ok(None),
    }
}

fn apply_delete(conn: &Connection, spec: &EntitySpec, op: &Op, uuid: &str) -> Result<Applied, SyncError> {
    set_tombstone(conn, spec.name, uuid, &op.rev())?;
    match spec.identity {
        Identity::Uuid => {
            conn.execute(
                &format!("DELETE FROM {} WHERE uuid = ?1", spec.table),
                [uuid],
            )?;
        }
        Identity::Composite => {
            let Some((a_uuid, b_uuid)) = row::split_composite(uuid) else {
                return Ok(Applied::Ignored("malformed composite uuid"));
            };
            let (a, b) = (spec.fields[0], spec.fields[1]);
            conn.execute(
                &format!(
                    "DELETE FROM {} WHERE {} = (SELECT id FROM {} WHERE uuid = ?1)
                                       AND {} = (SELECT id FROM {} WHERE uuid = ?2)",
                    spec.table,
                    a.column,
                    row::ref_table(&a.kind),
                    b.column,
                    row::ref_table(&b.kind)
                ),
                [a_uuid, b_uuid],
            )?;
        }
    }
    clear_shadow(conn, spec.name, uuid)?;
    Ok(Applied::Ok { conflicts: 0 })
}

fn apply_composite_upsert(
    conn: &Connection,
    spec: &EntitySpec,
    uuid: &str,
) -> Result<Applied, SyncError> {
    let Some((a_uuid, b_uuid)) = row::split_composite(uuid) else {
        return Ok(Applied::Ignored("malformed composite uuid"));
    };
    let (a, b) = (spec.fields[0], spec.fields[1]);
    let a_uuid = resolve_alias(conn, "task", a_uuid)?;
    let b_uuid = resolve_alias(conn, "tag", b_uuid)?;

    let (Some(a_id), Some(b_id)) = (
        row::local_id(conn, row::ref_table(&a.kind), &a_uuid)?,
        row::local_id(conn, row::ref_table(&b.kind), &b_uuid)?,
    ) else {
        return Ok(Applied::Deferred("linked row not here yet"));
    };

    conn.execute(
        &format!(
            "INSERT OR IGNORE INTO {} ({}, {}) VALUES (?1, ?2)",
            spec.table, a.column, b.column
        ),
        rusqlite::params![a_id, b_id],
    )?;
    conn.execute(
        "DELETE FROM sync_tombstones WHERE entity = ?1 AND uuid = ?2",
        [spec.name, uuid],
    )?;
    conn.execute(
        "INSERT INTO sync_shadow(entity, uuid, json) VALUES (?1, ?2, '{}')
         ON CONFLICT(entity, uuid) DO UPDATE SET json = excluded.json",
        [spec.name, uuid],
    )?;
    Ok(Applied::Ok { conflicts: 0 })
}

fn apply_upsert(conn: &Connection, spec: &EntitySpec, op: &Op, uuid: &str) -> Result<Applied, SyncError> {
    // A delete that happened after this change must not be undone by it.
    if let Some(grave) = tombstone(conn, spec.name, uuid)? {
        if grave.lamport >= op.lamport {
            return Ok(Applied::Ignored("row was deleted more recently"));
        }
    }

    if spec.identity == Identity::Composite {
        return apply_composite_upsert(conn, spec, uuid);
    }

    // Resolve references first: if any target row is still on its way, the
    // whole op waits rather than landing with a dangling link.
    let mut resolved: Fields = Fields::new();
    for (name, field_op) in &op.fields {
        let Some(f) = spec.field(name) else { continue };
        match column_value(conn, &f.kind, spec.name, &field_op.v)? {
            Some(value) => {
                resolved.insert(name.clone(), value);
            }
            None => return Ok(Applied::Deferred("referenced row not here yet")),
        }
    }

    let uuid = match adopt_existing_tag(conn, spec, op, uuid)? {
        Some(local) => local,
        None => uuid.to_string(),
    };
    let uuid = uuid.as_str();

    let incoming = op.rev();
    let existing = row::read(conn, spec, uuid)?;
    let mut conflicts = 0;
    let mut winning: Fields = Fields::new();

    if existing.is_none() {
        insert_row(conn, spec, uuid, &resolved)?;
        for name in resolved.keys() {
            set_field_rev(conn, spec.name, uuid, name, &incoming)?;
        }
    } else {
        let current = existing.expect("checked above");
        for (name, value) in &resolved {
            let ours = field_rev(conn, spec.name, uuid, name)?;
            let base = op.fields.get(name).and_then(|f| f.base.clone());

            let takes_it = match &ours {
                None => true,
                // Already applied. Segments can be delivered again after the
                // peer appends to or rewrites them, and seeing the same op
                // twice must not look like a disagreement.
                Some(ours) if *ours == incoming => false,
                Some(ours) if base.as_ref() == Some(ours) => true, // clean successor
                // Our value is the newer one, so it stands. Nothing is
                // reported here: the machine that wrote the older value is the
                // one whose edit is about to disappear, and it finds that out
                // when our value reaches it. Staying quiet on this side is also
                // what makes re-reading a segment harmless.
                Some(ours) if incoming < *ours => false,
                Some(ours) => {
                    // Independent edits to the same field, and theirs is newer.
                    // Take it so both machines agree, and keep ours on record.
                    conflicts += 1;
                    let mine = current.get(name).cloned().unwrap_or(Value::Null);
                    record_conflict(conn, spec.name, uuid, name, value, &incoming, &mine, ours)?;
                    true
                }
            };

            if takes_it {
                winning.insert(name.clone(), value.clone());
                set_field_rev(conn, spec.name, uuid, name, &incoming)?;
            }
        }
        update_fields(conn, spec, uuid, &winning)?;
    }

    // Refresh the shadow, or the next local drain would read these remote
    // values as fresh local edits and send them straight back.
    if let Some(now) = row::read(conn, spec, uuid)? {
        conn.execute(
            "INSERT INTO sync_shadow(entity, uuid, json) VALUES (?1, ?2, ?3)
             ON CONFLICT(entity, uuid) DO UPDATE SET json = excluded.json",
            rusqlite::params![spec.name, uuid, serde_json::to_string(&now)?],
        )?;
    }
    Ok(Applied::Ok { conflicts })
}

/// Apply one remote op. Capture stays suspended throughout, so nothing applied
/// here is echoed back into our own log.
pub fn apply(conn: &Connection, op: &Op) -> Result<Applied, SyncError> {
    meta::observe(conn, op.lamport)?;
    let Some(spec) = entity(&op.entity) else {
        return Ok(Applied::Ignored("unknown entity"));
    };
    let uuid = resolve_alias(conn, spec.name, &op.uuid)?;

    schema::suspend(conn, true)?;
    let outcome = match op.kind {
        OpKind::Delete => apply_delete(conn, spec, op, &uuid),
        OpKind::Upsert => apply_upsert(conn, spec, op, &uuid),
    };
    schema::suspend(conn, false)?;
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::sync::op::FieldOp;
    use std::collections::BTreeMap;

    fn upsert(seq: u64, device: &str, lamport: u64, entity: &str, uuid: &str) -> Op {
        Op {
            seq,
            device: device.into(),
            lamport,
            ts: "2026-09-04T10:00:00Z".into(),
            entity: entity.into(),
            uuid: uuid.into(),
            kind: OpKind::Upsert,
            fields: BTreeMap::new(),
        }
    }

    fn with(mut op: Op, field: &str, value: Value, base: Option<Rev>) -> Op {
        op.fields.insert(field.into(), FieldOp { v: value, base });
        op
    }

    #[test]
    fn a_remote_list_lands_as_a_real_row() {
        let conn = open_in_memory();
        let op = with(
            upsert(1, "remote", 5, "list", "u1"),
            "title",
            Value::from("Groceries"),
            None,
        );

        assert_eq!(apply(&conn, &op).unwrap(), Applied::Ok { conflicts: 0 });

        let title: String = conn
            .query_row("SELECT title FROM lists WHERE uuid = 'u1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "Groceries");
    }

    #[test]
    fn applying_a_remote_op_does_not_queue_it_for_sending_back() {
        let conn = open_in_memory();
        let op = with(upsert(1, "remote", 5, "list", "u1"), "title", Value::from("X"), None);
        apply(&conn, &op).unwrap();
        assert!(
            crate::sync::capture::drain(&conn).unwrap().is_empty(),
            "remote changes must not echo back into our log"
        );
    }

    #[test]
    fn a_task_whose_list_has_not_arrived_waits_instead_of_failing() {
        let conn = open_in_memory();
        let op = with(
            with(upsert(1, "remote", 5, "task", "t1"), "title", Value::from("Buy milk"), None),
            "list_uuid",
            Value::from("missing-list"),
            None,
        );

        assert_eq!(apply(&conn, &op).unwrap(), Applied::Deferred("referenced row not here yet"));
        let rows: i64 = conn.query_row("SELECT COUNT(*) FROM tasks", [], |r| r.get(0)).unwrap();
        assert_eq!(rows, 0);
    }

    #[test]
    fn edits_to_different_fields_of_one_row_both_survive() {
        // The case that motivates field-level merging: description edited here,
        // due date edited on the other machine.
        let conn = open_in_memory();
        apply(&conn, &with(upsert(1, "r", 1, "list", "l1"), "title", Value::from("L"), None)).unwrap();
        apply(
            &conn,
            &with(
                with(upsert(2, "r", 2, "task", "t1"), "title", Value::from("Task"), None),
                "list_uuid",
                Value::from("l1"),
                None,
            ),
        )
        .unwrap();

        // Local edit to the description.
        conn.execute("UPDATE tasks SET description = 'mine' WHERE uuid = 't1'", []).unwrap();
        crate::sync::capture::drain(&conn).unwrap();

        // Remote edit to the due date, made without seeing ours.
        let remote = with(
            upsert(3, "r", 9, "task", "t1"),
            "due_date",
            Value::from("2026-09-10"),
            None,
        );
        assert_eq!(apply(&conn, &remote).unwrap(), Applied::Ok { conflicts: 0 });

        let (desc, due): (String, String) = conn
            .query_row("SELECT description, due_date FROM tasks WHERE uuid = 't1'", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(desc, "mine");
        assert_eq!(due, "2026-09-10");
    }

    #[test]
    fn the_same_field_edited_on_both_machines_is_recorded_as_a_conflict() {
        let conn = open_in_memory();
        apply(&conn, &with(upsert(1, "r", 1, "list", "l1"), "title", Value::from("L"), None)).unwrap();

        // We rename it locally, on top of revision 1@r.
        conn.execute("UPDATE lists SET title = 'Mine' WHERE uuid = 'l1'", []).unwrap();
        crate::sync::capture::drain(&conn).unwrap();

        // The other machine renames it too, still based on revision 1@r.
        let remote = with(
            upsert(2, "r", 9, "list", "l1"),
            "title",
            Value::from("Theirs"),
            Some(Rev::new(1, "r")),
        );
        assert_eq!(apply(&conn, &remote).unwrap(), Applied::Ok { conflicts: 1 });

        let (field, kept, discarded): (String, String, String) = conn
            .query_row(
                "SELECT field, kept, discarded FROM sync_conflicts",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(field, "title");
        assert_eq!(kept, "\"Theirs\"", "the higher revision wins");
        assert_eq!(discarded, "\"Mine\"", "the losing value is kept for review");
    }

    #[test]
    fn a_clean_successor_is_not_treated_as_a_conflict() {
        // The other machine had already seen our value before editing it.
        let conn = open_in_memory();
        apply(&conn, &with(upsert(1, "r", 1, "list", "l1"), "title", Value::from("A"), None)).unwrap();
        let ours = field_rev(&conn, "list", "l1", "title").unwrap().unwrap();

        let remote = with(upsert(2, "r", 7, "list", "l1"), "title", Value::from("B"), Some(ours));
        assert_eq!(apply(&conn, &remote).unwrap(), Applied::Ok { conflicts: 0 });

        let title: String = conn
            .query_row("SELECT title FROM lists WHERE uuid = 'l1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "B");
    }

    #[test]
    fn an_op_older_than_the_delete_that_followed_it_cannot_resurrect_the_row() {
        let conn = open_in_memory();
        apply(&conn, &with(upsert(1, "r", 1, "list", "l1"), "title", Value::from("A"), None)).unwrap();

        let mut gone = upsert(2, "r", 10, "list", "l1");
        gone.kind = OpKind::Delete;
        apply(&conn, &gone).unwrap();

        // An op written before the delete arrives out of order.
        let stale = with(upsert(3, "r", 5, "list", "l1"), "title", Value::from("Back"), None);
        assert_eq!(
            apply(&conn, &stale).unwrap(),
            Applied::Ignored("row was deleted more recently")
        );
        let rows: i64 = conn.query_row("SELECT COUNT(*) FROM lists", [], |r| r.get(0)).unwrap();
        assert_eq!(rows, 0);
    }

    #[test]
    fn two_devices_that_invented_the_same_tag_end_up_with_one() {
        let conn = open_in_memory();
        conn.execute("INSERT INTO tags(name) VALUES('home')", []).unwrap();
        let local_uuid: String = conn.query_row("SELECT uuid FROM tags", [], |r| r.get(0)).unwrap();

        apply(
            &conn,
            &with(upsert(1, "r", 3, "tag", "remote-uuid"), "name", Value::from("home"), None),
        )
        .unwrap();

        let count: i64 = conn.query_row("SELECT COUNT(*) FROM tags", [], |r| r.get(0)).unwrap();
        assert_eq!(count, 1, "a duplicate tag name is merged, not duplicated");
        assert_eq!(resolve_alias(&conn, "tag", "remote-uuid").unwrap(), local_uuid);
    }
}

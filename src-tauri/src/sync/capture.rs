//! Turning locally captured changes into ops.
//!
//! The triggers only record *that* a row changed. Working out *what* changed
//! happens here, by diffing the row against the shadow copy of the last value
//! we know is shared. Sending only the fields that actually moved is what lets
//! two devices edit different fields of one task without fighting.

use crate::sync::model::{entity, Identity, ENTITIES};
use crate::sync::op::{FieldOp, Op, OpKind, Rev};
use crate::sync::row::{self, Fields};
use crate::sync::{meta, SyncError};
use rusqlite::Connection;
use std::collections::HashMap;

const NEXT_SEQ: &str = "next_seq";

fn next_seq(conn: &Connection) -> Result<u64, SyncError> {
    let seq = meta::get(conn, NEXT_SEQ)?
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(1);
    meta::set(conn, NEXT_SEQ, &(seq + 1).to_string())?;
    Ok(seq)
}

fn shadow(conn: &Connection, entity: &str, uuid: &str) -> Result<Option<Fields>, SyncError> {
    let found: Option<String> = conn
        .query_row(
            "SELECT json FROM sync_shadow WHERE entity = ?1 AND uuid = ?2",
            [entity, uuid],
            |r| r.get(0),
        )
        .ok();
    Ok(found.and_then(|j| serde_json::from_str(&j).ok()))
}

fn set_shadow(conn: &Connection, entity: &str, uuid: &str, fields: &Fields) -> Result<(), SyncError> {
    conn.execute(
        "INSERT INTO sync_shadow(entity, uuid, json) VALUES (?1, ?2, ?3)
         ON CONFLICT(entity, uuid) DO UPDATE SET json = excluded.json",
        rusqlite::params![entity, uuid, serde_json::to_string(fields)?],
    )?;
    Ok(())
}

pub fn clear_shadow(conn: &Connection, entity: &str, uuid: &str) -> Result<(), SyncError> {
    conn.execute(
        "DELETE FROM sync_shadow WHERE entity = ?1 AND uuid = ?2",
        [entity, uuid],
    )?;
    conn.execute(
        "DELETE FROM sync_field_revs WHERE entity = ?1 AND uuid = ?2",
        [entity, uuid],
    )?;
    Ok(())
}

pub fn field_rev(conn: &Connection, entity: &str, uuid: &str, field: &str) -> Result<Option<Rev>, SyncError> {
    let found: Option<(i64, String)> = conn
        .query_row(
            "SELECT lamport, device FROM sync_field_revs
              WHERE entity = ?1 AND uuid = ?2 AND field = ?3",
            [entity, uuid, field],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    Ok(found.map(|(l, d)| Rev::new(l as u64, d)))
}

pub fn set_field_rev(
    conn: &Connection,
    entity: &str,
    uuid: &str,
    field: &str,
    rev: &Rev,
) -> Result<(), SyncError> {
    conn.execute(
        "INSERT INTO sync_field_revs(entity, uuid, field, lamport, device)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(entity, uuid, field)
         DO UPDATE SET lamport = excluded.lamport, device = excluded.device",
        rusqlite::params![entity, uuid, field, rev.lamport as i64, rev.device],
    )?;
    Ok(())
}

pub fn set_tombstone(conn: &Connection, entity: &str, uuid: &str, rev: &Rev) -> Result<(), SyncError> {
    conn.execute(
        "INSERT INTO sync_tombstones(entity, uuid, lamport, device) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(entity, uuid)
         DO UPDATE SET lamport = excluded.lamport, device = excluded.device
          WHERE excluded.lamport > sync_tombstones.lamport",
        rusqlite::params![entity, uuid, rev.lamport as i64, rev.device],
    )?;
    Ok(())
}

pub fn tombstone(conn: &Connection, entity: &str, uuid: &str) -> Result<Option<Rev>, SyncError> {
    let found: Option<(i64, String)> = conn
        .query_row(
            "SELECT lamport, device FROM sync_tombstones WHERE entity = ?1 AND uuid = ?2",
            [entity, uuid],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    Ok(found.map(|(l, d)| Rev::new(l as u64, d)))
}

/// Queue every existing row for publication.
///
/// Used when a sync folder is first chosen: the log starts empty, so the other
/// device needs the whole database, not just what changes from now on.
pub fn seed_outbox(conn: &Connection) -> Result<(), SyncError> {
    for spec in ENTITIES {
        match spec.identity {
            Identity::Uuid => {
                conn.execute(
                    &format!(
                        "INSERT INTO sync_outbox(entity, local_id, kind)
                         SELECT '{}', id, 'upsert' FROM {}",
                        spec.name, spec.table
                    ),
                    [],
                )?;
            }
            Identity::Composite => {
                let (a, b) = (spec.fields[0], spec.fields[1]);
                conn.execute(
                    &format!(
                        "INSERT INTO sync_outbox(entity, uuid, kind)
                         SELECT '{}',
                                (SELECT uuid FROM {} WHERE id = t.{}) || ':' ||
                                (SELECT uuid FROM {} WHERE id = t.{}),
                                'upsert'
                           FROM {} t",
                        spec.name,
                        row::ref_table(&a.kind),
                        a.column,
                        row::ref_table(&b.kind),
                        b.column,
                        spec.table
                    ),
                    [],
                )?;
            }
        }
    }
    Ok(())
}

/// The queued changes, deduplicated to one entry per row, in the order the
/// rows were first touched.
fn pending(conn: &Connection) -> Result<(Vec<(String, String)>, HashMap<(String, String), String>, i64), SyncError> {
    let mut stmt = conn.prepare(
        "SELECT id, entity, local_id, uuid, kind FROM sync_outbox ORDER BY id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<i64>>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, String>(4)?,
        ))
    })?;

    let mut order: Vec<(String, String)> = Vec::new();
    let mut kinds: HashMap<(String, String), String> = HashMap::new();
    let mut high_water = 0i64;

    for row in rows {
        let (id, entity_name, local_id, uuid, kind) = row?;
        high_water = id;
        let Some(spec) = entity(&entity_name) else { continue };
        // Upserts are queued by row id; resolve it now, while the row is still
        // there. A row that has since been deleted is covered by its own
        // delete entry further down the queue.
        let uuid = match uuid {
            Some(u) => Some(u),
            None => match local_id {
                Some(id) => conn
                    .query_row(
                        &format!("SELECT uuid FROM {} WHERE id = ?1", spec.table),
                        [id],
                        |r| r.get::<_, Option<String>>(0),
                    )
                    .ok()
                    .flatten(),
                None => None,
            },
        };
        let Some(uuid) = uuid else { continue };
        let key = (entity_name, uuid);
        if !kinds.contains_key(&key) {
            order.push(key.clone());
        }
        kinds.insert(key, kind);
    }
    Ok((order, kinds, high_water))
}

/// Drain the outbox into ops, updating the shadow and field revisions as it goes.
pub fn drain(conn: &Connection) -> Result<Vec<Op>, SyncError> {
    let device = meta::device_id(conn)?;
    let (order, kinds, high_water) = pending(conn)?;
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let mut ops = Vec::new();

    for key in order {
        let (entity_name, uuid) = &key;
        let Some(spec) = entity(entity_name) else { continue };
        let is_delete = kinds.get(&key).map(|k| k == "delete").unwrap_or(false);

        if is_delete {
            let lamport = meta::tick(conn)?;
            let rev = Rev::new(lamport, device.clone());
            set_tombstone(conn, entity_name, uuid, &rev)?;
            clear_shadow(conn, entity_name, uuid)?;
            ops.push(Op {
                seq: next_seq(conn)?,
                device: device.clone(),
                lamport,
                ts: now.clone(),
                entity: entity_name.clone(),
                uuid: uuid.clone(),
                kind: OpKind::Delete,
                fields: Default::default(),
            });
            continue;
        }

        let Some(current) = row::read(conn, spec, uuid)? else { continue };
        let previous = shadow(conn, entity_name, uuid)?;

        // A join row carries no state, so the only news is that it exists.
        if spec.identity == Identity::Composite {
            if previous.is_some() {
                continue;
            }
            let lamport = meta::tick(conn)?;
            conn.execute(
                "DELETE FROM sync_tombstones WHERE entity = ?1 AND uuid = ?2",
                [entity_name.as_str(), uuid.as_str()],
            )?;
            set_shadow(conn, entity_name, uuid, &current)?;
            ops.push(Op {
                seq: next_seq(conn)?,
                device: device.clone(),
                lamport,
                ts: now.clone(),
                entity: entity_name.clone(),
                uuid: uuid.clone(),
                kind: OpKind::Upsert,
                fields: Default::default(),
            });
            continue;
        }

        let changed: Vec<&str> = spec
            .fields
            .iter()
            .map(|f| f.name)
            .filter(|name| match &previous {
                Some(prev) => prev.get(*name) != current.get(*name),
                None => true,
            })
            .collect();
        if changed.is_empty() {
            continue;
        }

        let lamport = meta::tick(conn)?;
        let rev = Rev::new(lamport, device.clone());
        let mut fields = std::collections::BTreeMap::new();
        for name in changed {
            let base = field_rev(conn, entity_name, uuid, name)?;
            fields.insert(
                name.to_string(),
                FieldOp { v: current.get(name).cloned().unwrap_or_default(), base },
            );
            set_field_rev(conn, entity_name, uuid, name, &rev)?;
        }
        set_shadow(conn, entity_name, uuid, &current)?;
        ops.push(Op {
            seq: next_seq(conn)?,
            device: device.clone(),
            lamport,
            ts: now.clone(),
            entity: entity_name.clone(),
            uuid: uuid.clone(),
            kind: OpKind::Upsert,
            fields,
        });
    }

    conn.execute("DELETE FROM sync_outbox WHERE id <= ?1", [high_water])?;
    Ok(ops)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    #[test]
    fn a_new_list_produces_one_op_carrying_every_field() {
        let conn = open_in_memory();
        conn.execute("INSERT INTO lists(title, position) VALUES('Work', 0)", [])
            .unwrap();

        let ops = drain(&conn).unwrap();

        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].kind, OpKind::Upsert);
        assert_eq!(ops[0].fields["title"].v, serde_json::Value::from("Work"));
        assert!(ops[0].fields["title"].base.is_none(), "nothing preceded the creation");
    }

    #[test]
    fn editing_one_field_sends_only_that_field() {
        // The whole point: a title edit here must not overwrite a description
        // edit made on the other machine at the same time.
        let conn = open_in_memory();
        conn.execute("INSERT INTO lists(title, position) VALUES('Work', 0)", [])
            .unwrap();
        drain(&conn).unwrap();

        conn.execute("UPDATE lists SET title = 'Job' WHERE id = 1", []).unwrap();
        let ops = drain(&conn).unwrap();

        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].fields.keys().collect::<Vec<_>>(), vec!["title"]);
        assert_eq!(
            ops[0].fields["title"].base.as_ref().map(|r| r.lamport),
            Some(1),
            "the edit must name the revision it replaces"
        );
    }

    #[test]
    fn a_write_that_changes_nothing_produces_no_op() {
        let conn = open_in_memory();
        conn.execute("INSERT INTO lists(title, position) VALUES('Work', 0)", [])
            .unwrap();
        drain(&conn).unwrap();

        conn.execute("UPDATE lists SET title = 'Work' WHERE id = 1", []).unwrap();

        assert!(drain(&conn).unwrap().is_empty());
    }

    #[test]
    fn several_edits_between_syncs_collapse_into_one_op() {
        let conn = open_in_memory();
        conn.execute("INSERT INTO lists(title, position) VALUES('Work', 0)", [])
            .unwrap();
        drain(&conn).unwrap();

        conn.execute("UPDATE lists SET title = 'A' WHERE id = 1", []).unwrap();
        conn.execute("UPDATE lists SET title = 'B' WHERE id = 1", []).unwrap();
        conn.execute("UPDATE lists SET color = '#fff' WHERE id = 1", []).unwrap();
        let ops = drain(&conn).unwrap();

        assert_eq!(ops.len(), 1, "one row, one op");
        assert_eq!(ops[0].fields["title"].v, serde_json::Value::from("B"));
        assert!(ops[0].fields.contains_key("color"));
    }

    #[test]
    fn a_row_created_and_deleted_before_syncing_leaves_only_the_delete() {
        let conn = open_in_memory();
        conn.execute("INSERT INTO lists(title, position) VALUES('Work', 0)", [])
            .unwrap();
        conn.execute("DELETE FROM lists WHERE id = 1", []).unwrap();

        let ops = drain(&conn).unwrap();

        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].kind, OpKind::Delete);
    }

    #[test]
    fn seeding_publishes_a_database_that_predates_sync() {
        let conn = open_in_memory();
        crate::sync::schema::suspend(&conn, true).unwrap();
        conn.execute("INSERT INTO lists(title, position, uuid) VALUES('Old', 0, 'u1')", [])
            .unwrap();
        crate::sync::schema::suspend(&conn, false).unwrap();
        assert!(drain(&conn).unwrap().is_empty(), "nothing was captured");

        seed_outbox(&conn).unwrap();
        let ops = drain(&conn).unwrap();

        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].uuid, "u1");
    }

    #[test]
    fn ops_get_increasing_sequence_numbers() {
        // Readers use seq as a cursor, so it must never repeat or go backwards.
        let conn = open_in_memory();
        conn.execute("INSERT INTO lists(title, position) VALUES('A', 0)", []).unwrap();
        conn.execute("INSERT INTO lists(title, position) VALUES('B', 1)", []).unwrap();
        let first = drain(&conn).unwrap();
        conn.execute("INSERT INTO lists(title, position) VALUES('C', 2)", []).unwrap();
        let second = drain(&conn).unwrap();

        let seqs: Vec<u64> = first.iter().chain(second.iter()).map(|o| o.seq).collect();
        assert_eq!(seqs, vec![1, 2, 3]);
    }
}

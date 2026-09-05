//! Reading and writing a synced row as a field map.
//!
//! Ops speak in uuids, the database speaks in row ids. Everything that
//! translates between the two lives here so the merge logic never has to think
//! about local ids.

use crate::sync::model::{EntitySpec, FieldKind, Identity};
use crate::sync::SyncError;
use rusqlite::types::Value as SqlValue;
use rusqlite::Connection;
use serde_json::Value;
use std::collections::BTreeMap;

pub type Fields = BTreeMap<String, Value>;

pub fn to_json(value: SqlValue) -> Value {
    match value {
        SqlValue::Null => Value::Null,
        SqlValue::Integer(i) => Value::from(i),
        SqlValue::Real(f) => Value::from(f),
        SqlValue::Text(s) => Value::from(s),
        SqlValue::Blob(_) => Value::Null,
    }
}

pub fn to_sql(value: &Value) -> SqlValue {
    match value {
        Value::Null => SqlValue::Null,
        Value::Bool(b) => SqlValue::Integer(*b as i64),
        Value::Number(n) => n
            .as_i64()
            .map(SqlValue::Integer)
            .or_else(|| n.as_f64().map(SqlValue::Real))
            .unwrap_or(SqlValue::Null),
        Value::String(s) => SqlValue::Text(s.clone()),
        other => SqlValue::Text(other.to_string()),
    }
}

/// SELECT list that yields uuids in place of row ids for reference columns.
fn select_list(spec: &EntitySpec) -> String {
    spec.fields
        .iter()
        .map(|f| match f.kind {
            FieldKind::Scalar => format!("t.{}", f.column),
            FieldKind::Ref(target) => {
                let table = crate::sync::model::entity(target).expect("known entity").table;
                format!("(SELECT uuid FROM {table} WHERE id = t.{})", f.column)
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Current values of a row, keyed by op field name, or None if it is gone.
pub fn read(conn: &Connection, spec: &EntitySpec, uuid: &str) -> Result<Option<Fields>, SyncError> {
    if spec.identity == Identity::Composite {
        // A join row has no state beyond the pair it links, which the uuid
        // already carries.
        return Ok(exists(conn, spec, uuid)?.then(Fields::new));
    }
    let sql = format!(
        "SELECT {} FROM {} t WHERE t.uuid = ?1",
        select_list(spec),
        spec.table
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([uuid])?;
    let Some(row) = rows.next()? else {
        return Ok(None);
    };
    let mut fields = Fields::new();
    for (i, f) in spec.fields.iter().enumerate() {
        fields.insert(f.name.to_string(), to_json(row.get::<_, SqlValue>(i)?));
    }
    Ok(Some(fields))
}

/// The two uuids a composite row links.
pub fn split_composite(uuid: &str) -> Option<(&str, &str)> {
    uuid.split_once(':')
}

pub fn exists(conn: &Connection, spec: &EntitySpec, uuid: &str) -> Result<bool, SyncError> {
    match spec.identity {
        Identity::Uuid => {
            let sql = format!("SELECT 1 FROM {} WHERE uuid = ?1", spec.table);
            Ok(conn.query_row(&sql, [uuid], |_| Ok(())).is_ok())
        }
        Identity::Composite => {
            let Some((a_uuid, b_uuid)) = split_composite(uuid) else {
                return Ok(false);
            };
            let (a, b) = (spec.fields[0], spec.fields[1]);
            let sql = format!(
                "SELECT 1 FROM {} WHERE {} = (SELECT id FROM {} WHERE uuid = ?1)
                              AND {} = (SELECT id FROM {} WHERE uuid = ?2)",
                spec.table,
                a.column,
                ref_table(&a.kind),
                b.column,
                ref_table(&b.kind),
            );
            Ok(conn.query_row(&sql, [a_uuid, b_uuid], |_| Ok(())).is_ok())
        }
    }
}

pub fn ref_table(kind: &FieldKind) -> &'static str {
    match kind {
        FieldKind::Ref(target) => crate::sync::model::entity(target).expect("known entity").table,
        FieldKind::Scalar => unreachable!("not a reference field"),
    }
}

/// Local row id for a uuid, or None if that row has not arrived yet.
pub fn local_id(conn: &Connection, table: &str, uuid: &str) -> Result<Option<i64>, SyncError> {
    let sql = format!("SELECT id FROM {table} WHERE uuid = ?1");
    match conn.query_row(&sql, [uuid], |r| r.get::<_, i64>(0)) {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::sync::model::entity;

    #[test]
    fn a_task_reads_back_with_its_list_as_a_uuid_not_an_id() {
        // Row ids mean nothing on the other machine, so they must never leave.
        let conn = open_in_memory();
        conn.execute("INSERT INTO lists(title, position) VALUES('Work', 0)", [])
            .unwrap();
        conn.execute(
            "INSERT INTO tasks(list_id, title, position) VALUES(1, 'Buy milk', 0)",
            [],
        )
        .unwrap();

        let list_uuid: String = conn.query_row("SELECT uuid FROM lists", [], |r| r.get(0)).unwrap();
        let task_uuid: String = conn.query_row("SELECT uuid FROM tasks", [], |r| r.get(0)).unwrap();

        let fields = read(&conn, entity("task").unwrap(), &task_uuid).unwrap().unwrap();
        assert_eq!(fields["title"], Value::from("Buy milk"));
        assert_eq!(fields["list_uuid"], Value::from(list_uuid));
        assert_eq!(fields["parent_task_uuid"], Value::Null);
    }

    #[test]
    fn a_missing_row_reads_as_none() {
        let conn = open_in_memory();
        assert!(read(&conn, entity("list").unwrap(), "nope").unwrap().is_none());
    }

    #[test]
    fn composite_rows_are_found_through_the_uuid_pair() {
        let conn = open_in_memory();
        conn.execute("INSERT INTO lists(title, position) VALUES('W', 0)", []).unwrap();
        conn.execute("INSERT INTO tasks(list_id, title, position) VALUES(1, 't', 0)", [])
            .unwrap();
        conn.execute("INSERT INTO tags(name) VALUES('home')", []).unwrap();
        conn.execute("INSERT INTO task_tags(task_id, tag_id) VALUES(1, 1)", []).unwrap();

        let task_uuid: String = conn.query_row("SELECT uuid FROM tasks", [], |r| r.get(0)).unwrap();
        let tag_uuid: String = conn.query_row("SELECT uuid FROM tags", [], |r| r.get(0)).unwrap();
        let spec = entity("task_tag").unwrap();

        assert!(exists(&conn, spec, &format!("{task_uuid}:{tag_uuid}")).unwrap());
        assert!(!exists(&conn, spec, &format!("{tag_uuid}:{task_uuid}")).unwrap());
    }
}

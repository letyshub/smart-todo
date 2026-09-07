use crate::db::{self, DbState};
use crate::sync::{self, model, row, store::SyncFolder, SyncReport};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;

/// The sync folder currently in use, or None when sync is off.
pub struct SyncState(pub Mutex<Option<PathBuf>>);

#[derive(Debug, Serialize, Deserialize)]
pub struct Peer {
    pub device_id: String,
    pub name: String,
    pub platform: String,
    pub last_seen: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncStatus {
    pub folder: Option<String>,
    pub device_name: String,
    pub peers: Vec<Peer>,
    pub open_conflicts: i64,
    /// Ops still waiting for a row they depend on.
    pub waiting: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Conflict {
    pub id: i64,
    pub entity: String,
    pub uuid: String,
    pub field: String,
    /// Human-readable name of the row the conflict is on, e.g. the task title.
    pub subject: String,
    pub kept: String,
    pub discarded: String,
    pub detected_at: String,
}

fn folder_of(state: &State<SyncState>) -> Result<Option<SyncFolder>, String> {
    let guard = state.0.lock().map_err(|e| e.to_string())?;
    Ok(guard.as_ref().map(SyncFolder::new))
}

#[tauri::command]
pub fn get_sync_status(db: State<DbState>, state: State<SyncState>) -> Result<SyncStatus, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let folder = folder_of(&state)?;

    let peers = match &folder {
        Some(f) => {
            let own = sync::meta::device_id(&conn).map_err(|e| e.to_string())?;
            f.peers(&own)
                .map_err(|e| e.to_string())?
                .into_iter()
                .filter_map(|id| f.read_meta(&id))
                .map(|m| Peer {
                    device_id: m.device_id,
                    name: m.name,
                    platform: m.platform,
                    last_seen: m.last_seen,
                })
                .collect()
        }
        None => Vec::new(),
    };

    Ok(SyncStatus {
        folder: folder.map(|f| f.root().to_string_lossy().to_string()),
        device_name: sync::meta::device_name(&conn).map_err(|e| e.to_string())?,
        peers,
        open_conflicts: conn
            .query_row("SELECT COUNT(*) FROM sync_conflicts WHERE resolved = 0", [], |r| r.get(0))
            .map_err(|e| e.to_string())?,
        waiting: conn
            .query_row("SELECT COUNT(*) FROM sync_deferred", [], |r| r.get(0))
            .map_err(|e| e.to_string())?,
    })
}

/// Point sync at a folder, publishing whatever this machine already has.
#[tauri::command]
pub fn set_sync_folder(
    db: State<DbState>,
    state: State<SyncState>,
    path: String,
) -> Result<SyncReport, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let dir = PathBuf::from(&path);
    let report = sync::adopt_folder(&conn, &db::config_dir(), &dir).map_err(|e| e.to_string())?;
    *state.0.lock().map_err(|e| e.to_string())? = Some(dir);
    Ok(report)
}

#[tauri::command]
pub fn disable_sync(state: State<SyncState>) -> Result<(), String> {
    sync::clear_folder(&db::config_dir()).map_err(|e| e.to_string())?;
    *state.0.lock().map_err(|e| e.to_string())? = None;
    Ok(())
}

#[tauri::command]
pub fn sync_now(db: State<DbState>, state: State<SyncState>) -> Result<SyncReport, String> {
    let Some(folder) = folder_of(&state)? else {
        return Ok(SyncReport::default());
    };
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    sync::run(&conn, &folder).map_err(|e| e.to_string())
}

/// A label for the row a conflict sits on, so the user is not shown a uuid.
fn subject_of(conn: &rusqlite::Connection, entity: &str, uuid: &str) -> String {
    let column = match entity {
        "tag" => "name",
        "list" | "task" => "title",
        _ => return entity.to_string(),
    };
    let table = model::entity(entity).map(|e| e.table).unwrap_or_default();
    conn.query_row(
        &format!("SELECT {column} FROM {table} WHERE uuid = ?1"),
        [uuid],
        |r| r.get::<_, String>(0),
    )
    .unwrap_or_else(|_| entity.to_string())
}

#[tauri::command]
pub fn get_conflicts(db: State<DbState>) -> Result<Vec<Conflict>, String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, entity, uuid, field, kept, discarded, detected_at
               FROM sync_conflicts WHERE resolved = 0 ORDER BY id DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows: Vec<(i64, String, String, String, Option<String>, Option<String>, String)> = stmt
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;

    Ok(rows
        .into_iter()
        .map(|(id, entity, uuid, field, kept, discarded, detected_at)| Conflict {
            subject: subject_of(&conn, &entity, &uuid),
            id,
            entity,
            uuid,
            field,
            kept: kept.unwrap_or_default(),
            discarded: discarded.unwrap_or_default(),
            detected_at,
        })
        .collect())
}

/// Settle a conflict.
///
/// Restoring the discarded value is done as an ordinary edit, so it is captured
/// and travels to the other machine like any other change the user makes.
#[tauri::command]
pub fn resolve_conflict(
    db: State<DbState>,
    id: i64,
    restore_discarded: bool,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;

    if restore_discarded {
        let (entity, uuid, field, discarded): (String, String, String, Option<String>) = conn
            .query_row(
                "SELECT entity, uuid, field, discarded FROM sync_conflicts WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .map_err(|e| e.to_string())?;

        let spec = model::entity(&entity).ok_or("unknown entity")?;
        let f = spec.field(&field).ok_or("unknown field")?;
        let value: serde_json::Value =
            serde_json::from_str(discarded.as_deref().unwrap_or("null")).map_err(|e| e.to_string())?;

        let stored = match f.kind {
            model::FieldKind::Scalar => row::to_sql(&value),
            model::FieldKind::Ref(target) => {
                let table = model::entity(target).ok_or("unknown entity")?.table;
                match value.as_str() {
                    Some(ref_uuid) => row::local_id(&conn, table, ref_uuid)
                        .map_err(|e| e.to_string())?
                        .map(rusqlite::types::Value::Integer)
                        .ok_or("the referenced row is not on this device")?,
                    None => rusqlite::types::Value::Null,
                }
            }
        };
        conn.execute(
            &format!("UPDATE {} SET {} = ?1 WHERE uuid = ?2", spec.table, f.column),
            rusqlite::params![stored, uuid],
        )
        .map_err(|e| e.to_string())?;
    }

    conn.execute("UPDATE sync_conflicts SET resolved = 1 WHERE id = ?1", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn set_device_name(db: State<DbState>, name: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|e| e.to_string())?;
    sync::meta::set(&conn, sync::meta::DEVICE_NAME, &name).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use crate::db::open_in_memory;

    #[test]
    fn a_conflict_names_the_row_it_is_about_rather_than_its_uuid() {
        let conn = open_in_memory();
        conn.execute("INSERT INTO lists(title, position) VALUES('Groceries', 0)", [])
            .unwrap();
        let uuid: String = conn.query_row("SELECT uuid FROM lists", [], |r| r.get(0)).unwrap();

        assert_eq!(super::subject_of(&conn, "list", &uuid), "Groceries");
    }

    #[test]
    fn an_unknown_row_falls_back_to_the_entity_name() {
        let conn = open_in_memory();
        assert_eq!(super::subject_of(&conn, "task", "gone"), "task");
    }
}

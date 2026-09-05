//! Device identity and the Lamport clock, both persisted in `sync_meta`.

use rusqlite::{Connection, Result};

pub const DEVICE_ID: &str = "device_id";
pub const DEVICE_NAME: &str = "device_name";
pub const LAMPORT: &str = "lamport";

pub fn get(conn: &Connection, key: &str) -> Result<Option<String>> {
    conn.query_row("SELECT value FROM sync_meta WHERE key = ?1", [key], |r| r.get(0))
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })
}

pub fn set(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO sync_meta(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [key, value],
    )?;
    Ok(())
}

/// Best-effort machine name, used only to label this device in the UI.
fn hostname() -> String {
    for var in ["COMPUTERNAME", "HOSTNAME", "HOST"] {
        if let Ok(name) = std::env::var(var) {
            if !name.trim().is_empty() {
                return name;
            }
        }
    }
    std::env::consts::OS.to_string()
}

/// This device's stable id, minted on first use.
///
/// It is the name of our subdirectory in the sync folder, and no other device
/// ever writes there — that is what keeps the cloud provider from having
/// anything to conflict over.
pub fn device_id(conn: &Connection) -> Result<String> {
    if let Some(id) = get(conn, DEVICE_ID)? {
        return Ok(id);
    }
    let id: String = conn.query_row("SELECT lower(hex(randomblob(8)))", [], |r| r.get(0))?;
    set(conn, DEVICE_ID, &id)?;
    Ok(id)
}

pub fn device_name(conn: &Connection) -> Result<String> {
    if let Some(name) = get(conn, DEVICE_NAME)? {
        return Ok(name);
    }
    let name = hostname();
    set(conn, DEVICE_NAME, &name)?;
    Ok(name)
}

fn lamport(conn: &Connection) -> Result<u64> {
    Ok(get(conn, LAMPORT)?.and_then(|v| v.parse().ok()).unwrap_or(0))
}

/// Advance the clock for a local write and return the new value.
pub fn tick(conn: &Connection) -> Result<u64> {
    let next = lamport(conn)? + 1;
    set(conn, LAMPORT, &next.to_string())?;
    Ok(next)
}

/// Fold a revision seen from another device into our clock, so anything we
/// write afterwards sorts above what we have already seen.
pub fn observe(conn: &Connection, remote: u64) -> Result<()> {
    if remote > lamport(conn)? {
        set(conn, LAMPORT, &remote.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    #[test]
    fn device_id_is_minted_once_and_then_reused() {
        let conn = open_in_memory();
        let first = device_id(&conn).unwrap();
        assert_eq!(first.len(), 16);
        assert_eq!(device_id(&conn).unwrap(), first);
    }

    #[test]
    fn clock_advances_on_every_local_write() {
        let conn = open_in_memory();
        assert_eq!(tick(&conn).unwrap(), 1);
        assert_eq!(tick(&conn).unwrap(), 2);
    }

    #[test]
    fn a_remote_revision_pushes_the_clock_forward_but_never_back() {
        // Our next write has to sort above anything we have already applied,
        // otherwise it would silently lose to a change the user made earlier.
        let conn = open_in_memory();
        tick(&conn).unwrap();
        observe(&conn, 50).unwrap();
        assert_eq!(tick(&conn).unwrap(), 51);
        observe(&conn, 3).unwrap();
        assert_eq!(tick(&conn).unwrap(), 52);
    }
}

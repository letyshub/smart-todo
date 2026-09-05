//! The sync folder on disk.
//!
//! Layout:
//!
//! ```text
//! <sync folder>/
//!   devices/
//!     <device id>/
//!       meta.json
//!       ops-000001.jsonl
//!       ops-000002.jsonl
//! ```
//!
//! Every device writes only inside its own `devices/<id>/` subtree and reads
//! everyone else's. Because each file has exactly one writer, the cloud
//! provider never has two versions of a file to reconcile, which is what makes
//! this safe on OneDrive where hosting the database itself is not.

use crate::sync::op::Op;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Roll over to a new segment past this size, so a busy device does not force
/// the cloud client to re-upload one ever-growing file on every change.
const SEGMENT_MAX_BYTES: u64 = 1024 * 1024;

const DEVICES_DIR: &str = "devices";
const META_FILE: &str = "meta.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceMeta {
    pub device_id: String,
    pub name: String,
    pub platform: String,
    pub last_seen: String,
    /// Bumped when the device rewrites its own log; readers use it only as a
    /// hint that previously seen files may have been replaced.
    #[serde(default)]
    pub compaction_epoch: u64,
}

/// One segment file as found on disk.
pub struct Segment {
    pub path: PathBuf,
    pub size: u64,
    pub mtime: i64,
}

#[derive(Debug)]
pub struct SyncFolder {
    root: PathBuf,
}

fn mtime_of(meta: &fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl SyncFolder {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        SyncFolder { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn device_dir(&self, device: &str) -> PathBuf {
        self.root.join(DEVICES_DIR).join(device)
    }

    /// Ids of every device that has ever written here, excluding our own.
    pub fn peers(&self, own_device: &str) -> std::io::Result<Vec<String>> {
        let devices = self.root.join(DEVICES_DIR);
        if !devices.is_dir() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in fs::read_dir(devices)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name != own_device {
                out.push(name);
            }
        }
        out.sort();
        Ok(out)
    }

    /// A device's segments, oldest first.
    pub fn segments(&self, device: &str) -> std::io::Result<Vec<Segment>> {
        let dir = self.device_dir(device);
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            let is_segment = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("ops-") && n.ends_with(".jsonl"));
            if !is_segment {
                continue;
            }
            let meta = entry.metadata()?;
            out.push(Segment { path, size: meta.len(), mtime: mtime_of(&meta) });
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
    }

    fn segment_path(&self, device: &str, index: u64) -> PathBuf {
        self.device_dir(device).join(format!("ops-{index:06}.jsonl"))
    }

    /// The segment new ops should go into, rolling over when the current one
    /// has grown past the size limit.
    fn current_segment(&self, device: &str) -> std::io::Result<PathBuf> {
        let segments = self.segments(device)?;
        match segments.last() {
            Some(last) if last.size < SEGMENT_MAX_BYTES => Ok(last.path.clone()),
            Some(last) => {
                let index = last
                    .path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .and_then(|s| s.strip_prefix("ops-"))
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);
                Ok(self.segment_path(device, index + 1))
            }
            None => Ok(self.segment_path(device, 1)),
        }
    }

    /// Append ops to our own log, one JSON object per line.
    ///
    /// The trailing newline is what marks a line as complete; a reader that
    /// finds a partially uploaded tail stops before it rather than parsing
    /// half an op.
    pub fn append(&self, device: &str, ops: &[Op]) -> std::io::Result<()> {
        if ops.is_empty() {
            return Ok(());
        }
        fs::create_dir_all(self.device_dir(device))?;
        let path = self.current_segment(device)?;
        let file = OpenOptions::new().create(true).append(true).open(&path)?;
        let mut out = BufWriter::new(file);
        for op in ops {
            let line = serde_json::to_string(op)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
            out.write_all(line.as_bytes())?;
            out.write_all(b"\n")?;
        }
        out.flush()?;
        // Durability matters more than speed here: a half-written op that the
        // cloud client uploads before the rest of the file lands is exactly the
        // case the reader has to survive.
        out.into_inner()?.sync_all()?;
        Ok(())
    }

    pub fn write_meta(&self, meta: &DeviceMeta) -> std::io::Result<()> {
        let dir = self.device_dir(&meta.device_id);
        fs::create_dir_all(&dir)?;
        let json = serde_json::to_string_pretty(meta)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let tmp = dir.join("meta.json.tmp");
        fs::write(&tmp, json)?;
        // Replace atomically so a reader never sees a truncated meta file.
        fs::rename(tmp, dir.join(META_FILE))
    }

    pub fn read_meta(&self, device: &str) -> Option<DeviceMeta> {
        let raw = fs::read_to_string(self.device_dir(device).join(META_FILE)).ok()?;
        serde_json::from_str(&raw).ok()
    }

    /// Replace a device's whole log with `ops`, written to fresh segments.
    ///
    /// Only ever called on our own log. Old segments are removed after the new
    /// ones are in place, so a reader interrupted mid-compaction sees either
    /// the old set or both, never a gap.
    pub fn rewrite(&self, device: &str, ops: &[Op]) -> std::io::Result<()> {
        let dir = self.device_dir(device);
        fs::create_dir_all(&dir)?;
        let old: Vec<PathBuf> = self.segments(device)?.into_iter().map(|s| s.path).collect();

        let mut index = 1_000_000; // fresh, sorts after anything written so far
        let mut written = Vec::new();
        let mut chunk: Vec<&Op> = Vec::new();
        let mut bytes = 0usize;
        let flush = |chunk: &mut Vec<&Op>, index: &mut u64, written: &mut Vec<PathBuf>| -> std::io::Result<()> {
            if chunk.is_empty() {
                return Ok(());
            }
            let path = self.segment_path(device, *index);
            let mut file = BufWriter::new(File::create(&path)?);
            for op in chunk.iter() {
                let line = serde_json::to_string(op)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                file.write_all(line.as_bytes())?;
                file.write_all(b"\n")?;
            }
            file.flush()?;
            file.into_inner()?.sync_all()?;
            written.push(path);
            *index += 1;
            chunk.clear();
            Ok(())
        };

        for op in ops {
            bytes += serde_json::to_string(op).map(|s| s.len() + 1).unwrap_or(0);
            chunk.push(op);
            if bytes as u64 >= SEGMENT_MAX_BYTES {
                flush(&mut chunk, &mut index, &mut written)?;
                bytes = 0;
            }
        }
        flush(&mut chunk, &mut index, &mut written)?;

        for path in old {
            let _ = fs::remove_file(path);
        }
        Ok(())
    }
}

/// Ops in one segment file, stopping at the first line that is not complete.
///
/// Returns the ops it could read; a truncated or still-uploading tail is simply
/// left for the next pass.
pub fn read_segment(path: &Path) -> std::io::Result<Vec<Op>> {
    let raw = fs::read_to_string(path)?;
    let mut ops = Vec::new();
    for line in raw.split_inclusive('\n') {
        if !line.ends_with('\n') {
            break; // incomplete final line: the writer or the sync client is mid-flight
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<Op>(line) {
            Ok(op) => ops.push(op),
            Err(_) => break, // corrupt line: stop rather than skip, and retry later
        }
    }
    Ok(ops)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::op::OpKind;
    use std::collections::BTreeMap;

    fn op(seq: u64, device: &str) -> Op {
        Op {
            seq,
            device: device.into(),
            lamport: seq,
            ts: "2026-09-04T10:00:00Z".into(),
            entity: "list".into(),
            uuid: format!("u{seq}"),
            kind: OpKind::Upsert,
            fields: BTreeMap::new(),
        }
    }

    #[test]
    fn appended_ops_read_back_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let folder = SyncFolder::new(dir.path());
        folder.append("dev1", &[op(1, "dev1"), op(2, "dev1")]).unwrap();
        folder.append("dev1", &[op(3, "dev1")]).unwrap();

        let segments = folder.segments("dev1").unwrap();
        assert_eq!(segments.len(), 1, "small logs stay in one segment");
        let ops = read_segment(&segments[0].path).unwrap();
        assert_eq!(ops.iter().map(|o| o.seq).collect::<Vec<_>>(), vec![1, 2, 3]);
    }

    #[test]
    fn a_half_written_final_line_is_ignored_not_parsed() {
        // OneDrive can publish a file while the last op is still being written.
        // Reading that must yield the complete ops and nothing else.
        let dir = tempfile::tempdir().unwrap();
        let folder = SyncFolder::new(dir.path());
        folder.append("dev1", &[op(1, "dev1")]).unwrap();
        let path = folder.segments("dev1").unwrap()[0].path.clone();
        let mut raw = fs::read_to_string(&path).unwrap();
        raw.push_str("{\"seq\":2,\"device\":\"dev1\",\"lam");
        fs::write(&path, raw).unwrap();

        let ops = read_segment(&path).unwrap();
        assert_eq!(ops.len(), 1);
        assert_eq!(ops[0].seq, 1);
    }

    #[test]
    fn peers_exclude_our_own_device() {
        let dir = tempfile::tempdir().unwrap();
        let folder = SyncFolder::new(dir.path());
        folder.append("mine", &[op(1, "mine")]).unwrap();
        folder.append("theirs", &[op(1, "theirs")]).unwrap();
        assert_eq!(folder.peers("mine").unwrap(), vec!["theirs".to_string()]);
    }

    #[test]
    fn rewriting_replaces_every_old_segment() {
        let dir = tempfile::tempdir().unwrap();
        let folder = SyncFolder::new(dir.path());
        folder.append("dev1", &[op(1, "dev1"), op(2, "dev1")]).unwrap();

        folder.rewrite("dev1", &[op(2, "dev1")]).unwrap();

        let segments = folder.segments("dev1").unwrap();
        assert_eq!(segments.len(), 1);
        let ops = read_segment(&segments[0].path).unwrap();
        assert_eq!(ops.iter().map(|o| o.seq).collect::<Vec<_>>(), vec![2]);
    }

    #[test]
    fn meta_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let folder = SyncFolder::new(dir.path());
        let meta = DeviceMeta {
            device_id: "dev1".into(),
            name: "Laptop".into(),
            platform: "windows".into(),
            last_seen: "2026-09-04T10:00:00Z".into(),
            compaction_epoch: 2,
        };
        folder.write_meta(&meta).unwrap();
        let back = folder.read_meta("dev1").unwrap();
        assert_eq!(back.name, "Laptop");
        assert_eq!(back.compaction_epoch, 2);
    }
}

//! The unit of replication: one change to one row, as written to a device's log.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

/// A revision stamp: a Lamport counter plus the device that produced it.
///
/// The device breaks ties, so two devices comparing the same pair of revisions
/// always pick the same winner and therefore converge on the same row.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Rev {
    pub lamport: u64,
    pub device: String,
}

impl Rev {
    pub fn new(lamport: u64, device: impl Into<String>) -> Self {
        Rev { lamport, device: device.into() }
    }
}

impl fmt::Display for Rev {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.lamport, self.device)
    }
}

impl FromStr for Rev {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (l, d) = s.split_once('@').ok_or_else(|| format!("bad rev: {s}"))?;
        Ok(Rev { lamport: l.parse().map_err(|_| format!("bad rev: {s}"))?, device: d.to_string() })
    }
}

impl Serialize for Rev {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Rev {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OpKind {
    Upsert,
    Delete,
}

/// One field's new value, plus the revision it was written on top of.
///
/// `base` is what makes conflict detection possible: if it matches the
/// revision we already hold for that field, the writer had seen our value and
/// the change is a clean successor. If it does not, the two edits were made
/// independently and the user needs to know.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldOp {
    pub v: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<Rev>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Op {
    /// Monotonic within the writing device's log; readers track it as a cursor.
    pub seq: u64,
    pub device: String,
    pub lamport: u64,
    pub ts: String,
    pub entity: String,
    pub uuid: String,
    pub kind: OpKind,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, FieldOp>,
}

impl Op {
    pub fn rev(&self) -> Rev {
        Rev::new(self.lamport, self.device.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rev_ordering_breaks_ties_by_device() {
        // Same Lamport counter on both machines: the ordering must still be
        // total and identical on each, or they would settle on different rows.
        let a = Rev::new(7, "aaa");
        let b = Rev::new(7, "bbb");
        assert!(a < b);
        assert!(Rev::new(8, "aaa") > b);
    }

    #[test]
    fn rev_survives_a_json_round_trip() {
        let rev = Rev::new(42, "d00d");
        let json = serde_json::to_string(&rev).unwrap();
        assert_eq!(json, "\"42@d00d\"");
        assert_eq!(serde_json::from_str::<Rev>(&json).unwrap(), rev);
    }

    #[test]
    fn op_round_trips_as_a_single_json_line() {
        let mut fields = BTreeMap::new();
        fields.insert(
            "title".to_string(),
            FieldOp { v: Value::from("Buy milk"), base: Some(Rev::new(3, "aaa")) },
        );
        let op = Op {
            seq: 12,
            device: "bbb".into(),
            lamport: 9,
            ts: "2026-09-04T10:00:00Z".into(),
            entity: "task".into(),
            uuid: "u1".into(),
            kind: OpKind::Upsert,
            fields,
        };
        let line = serde_json::to_string(&op).unwrap();
        assert!(!line.contains('\n'), "an op must occupy exactly one line");
        let back: Op = serde_json::from_str(&line).unwrap();
        assert_eq!(back.rev(), Rev::new(9, "bbb"));
        assert_eq!(back.fields["title"].v, Value::from("Buy milk"));
    }

    #[test]
    fn delete_ops_carry_no_fields() {
        let op = Op {
            seq: 1,
            device: "a".into(),
            lamport: 1,
            ts: "t".into(),
            entity: "task".into(),
            uuid: "u".into(),
            kind: OpKind::Delete,
            fields: BTreeMap::new(),
        };
        let line = serde_json::to_string(&op).unwrap();
        assert!(!line.contains("fields"));
    }
}

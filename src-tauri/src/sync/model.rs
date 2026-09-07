//! Description of every table that takes part in sync.
//!
//! Sync logic is written once and driven by these specs rather than repeated
//! per table, so adding a synced column means adding one line here.

/// How a column's value travels between devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    /// Value is carried verbatim.
    Scalar,
    /// Column holds a local row id; the op carries the referenced row's uuid
    /// instead, because row ids are only meaningful on the device that minted
    /// them. The payload is the name of the referenced entity.
    Ref(&'static str),
}

#[derive(Debug, Clone, Copy)]
pub struct FieldSpec {
    /// Name used inside ops. Ref fields are named `<column without _id>_uuid`.
    pub name: &'static str,
    pub column: &'static str,
    pub kind: FieldKind,
    /// Column is NOT NULL, so a row cannot be created without it. A required
    /// reference that has not arrived yet forces the op to be retried later.
    pub required: bool,
}

/// How a row is identified across devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Identity {
    /// Row carries its own `uuid` column.
    Uuid,
    /// Join row with no identity of its own; its uuid is `<a_uuid>:<b_uuid>`
    /// built from the two rows it links.
    Composite,
}

#[derive(Debug, Clone, Copy)]
pub struct EntitySpec {
    pub name: &'static str,
    pub table: &'static str,
    pub identity: Identity,
    pub fields: &'static [FieldSpec],
}

impl EntitySpec {
    pub fn field(&self, name: &str) -> Option<&FieldSpec> {
        self.fields.iter().find(|f| f.name == name)
    }
}

const fn scalar(name: &'static str) -> FieldSpec {
    FieldSpec { name, column: name, kind: FieldKind::Scalar, required: false }
}

const fn required_scalar(name: &'static str) -> FieldSpec {
    FieldSpec { name, column: name, kind: FieldKind::Scalar, required: true }
}

const fn reference(name: &'static str, column: &'static str, entity: &'static str) -> FieldSpec {
    FieldSpec { name, column, kind: FieldKind::Ref(entity), required: false }
}

const fn required_reference(
    name: &'static str,
    column: &'static str,
    entity: &'static str,
) -> FieldSpec {
    FieldSpec { name, column, kind: FieldKind::Ref(entity), required: true }
}

/// Entities in dependency order: a row's referents always appear before it, so
/// a single forward pass applies most batches without deferring anything.
pub const ENTITIES: &[EntitySpec] = &[
    EntitySpec {
        name: "list",
        table: "lists",
        identity: Identity::Uuid,
        fields: &[
            required_scalar("title"),
            scalar("color"),
            scalar("position"),
            scalar("created_at"),
        ],
    },
    EntitySpec {
        name: "tag",
        table: "tags",
        identity: Identity::Uuid,
        fields: &[required_scalar("name"), scalar("color")],
    },
    EntitySpec {
        name: "task",
        table: "tasks",
        identity: Identity::Uuid,
        fields: &[
            required_reference("list_uuid", "list_id", "list"),
            reference("parent_task_uuid", "parent_task_id", "task"),
            required_scalar("title"),
            scalar("description"),
            scalar("priority"),
            scalar("due_date"),
            scalar("completed"),
            scalar("completed_at"),
            scalar("position"),
            scalar("created_at"),
            scalar("updated_at"),
            scalar("status"),
            scalar("is_subtask"),
        ],
    },
    EntitySpec {
        name: "task_tag",
        table: "task_tags",
        identity: Identity::Composite,
        fields: &[
            required_reference("task_uuid", "task_id", "task"),
            required_reference("tag_uuid", "tag_id", "tag"),
        ],
    },
    EntitySpec {
        name: "timer_session",
        table: "timer_sessions",
        identity: Identity::Uuid,
        fields: &[
            required_reference("task_uuid", "task_id", "task"),
            required_scalar("started_at"),
            scalar("stopped_at"),
            scalar("duration_seconds"),
        ],
    },
];

pub fn entity(name: &str) -> Option<&'static EntitySpec> {
    ENTITIES.iter().find(|e| e.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn referents_are_declared_before_the_entities_that_point_at_them() {
        // Apply order relies on this: a task op must be applicable as soon as
        // the list op that precedes it in the same batch has landed.
        for (i, spec) in ENTITIES.iter().enumerate() {
            for f in spec.fields {
                if let FieldKind::Ref(target) = f.kind {
                    let at = ENTITIES.iter().position(|e| e.name == target).unwrap();
                    assert!(
                        at <= i,
                        "{}.{} points at {}, declared later",
                        spec.name, f.name, target
                    );
                }
            }
        }
    }

    #[test]
    fn every_ref_field_is_named_for_the_uuid_it_carries() {
        for spec in ENTITIES {
            for f in spec.fields {
                if matches!(f.kind, FieldKind::Ref(_)) {
                    assert!(f.name.ends_with("_uuid"), "{}.{}", spec.name, f.name);
                    assert!(f.column.ends_with("_id"), "{}.{}", spec.name, f.column);
                }
            }
        }
    }
}

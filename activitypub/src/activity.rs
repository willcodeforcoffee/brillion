//! AS2 activity types. Inbound activities are parsed loosely (`object`
//! as raw JSON, since its shape depends on `type`); outbound activities
//! we send ourselves are built with the concrete constructors below.
//!
//! TODO(phase 4): `Like`/`Announce` land with replies/likes/boosts.

use crate::AS2_CONTEXT;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// An activity as received in an inbox POST. `object` is left as raw
/// JSON because its shape is polymorphic: a bare actor URL string for a
/// `Follow`, an embedded activity for `Undo`, etc. — callers match on
/// `kind` first, then interpret `object` accordingly.
#[derive(Debug, Clone, Deserialize)]
pub struct IncomingActivity {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub actor: String,
    pub object: Value,
}

/// Builds an outbound `Accept` wrapping the `Follow` activity it accepts
/// (SPEC.md §3.3: local actors auto-accept every follow in phase 2).
#[derive(Debug, Clone, Serialize)]
pub struct Accept {
    #[serde(rename = "@context")]
    pub context: String,
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub actor: String,
    pub object: Value,
}

impl Accept {
    pub fn new(id: String, actor: &str, followed_activity: &IncomingActivity) -> Self {
        Self {
            context: AS2_CONTEXT.to_string(),
            id,
            kind: "Accept".to_string(),
            actor: actor.to_string(),
            object: serde_json::json!({
                "id": followed_activity.id,
                "type": followed_activity.kind,
                "actor": followed_activity.actor,
                "object": followed_activity.object,
            }),
        }
    }
}

/// Builds an outbound `Create` wrapping `object` (an embedded AS2
/// object, e.g. an `Article` for a newly published post) — SPEC.md
/// §3.4. `object` should *not* carry its own `@context` — nested
/// contexts are technically legal JSON-LD but unnecessary noise here,
/// so callers strip it before passing the value in (see
/// `backend::delivery`).
#[derive(Debug, Clone, Serialize)]
pub struct Create {
    #[serde(rename = "@context")]
    pub context: String,
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub actor: String,
    pub published: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub object: Value,
}

impl Create {
    pub fn new(
        id: String,
        actor: &str,
        published: String,
        to: Vec<String>,
        cc: Vec<String>,
        object: Value,
    ) -> Self {
        Self {
            context: AS2_CONTEXT.to_string(),
            id,
            kind: "Create".to_string(),
            actor: actor.to_string(),
            published,
            to,
            cc,
            object,
        }
    }
}

/// Builds an outbound `Update` — same shape as `Create`, sent when an
/// already-published post is edited (SPEC.md §3.4). A distinct type
/// (rather than reusing `Create`) because that's what the AS2 vocabulary
/// calls for and what remote servers key their handling on.
#[derive(Debug, Clone, Serialize)]
pub struct Update {
    #[serde(rename = "@context")]
    pub context: String,
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub actor: String,
    pub published: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub object: Value,
}

impl Update {
    pub fn new(
        id: String,
        actor: &str,
        published: String,
        to: Vec<String>,
        cc: Vec<String>,
        object: Value,
    ) -> Self {
        Self {
            context: AS2_CONTEXT.to_string(),
            id,
            kind: "Update".to_string(),
            actor: actor.to_string(),
            published,
            to,
            cc,
            object,
        }
    }
}

/// Builds an outbound `Delete` wrapping a `Tombstone` (SPEC.md §3.4/§3.5),
/// sent when a published post is deleted.
#[derive(Debug, Clone, Serialize)]
pub struct Delete {
    #[serde(rename = "@context")]
    pub context: String,
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub actor: String,
    pub published: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub object: Value,
}

impl Delete {
    pub fn new(
        id: String,
        actor: &str,
        published: String,
        to: Vec<String>,
        cc: Vec<String>,
        object: Value,
    ) -> Self {
        Self {
            context: AS2_CONTEXT.to_string(),
            id,
            kind: "Delete".to_string(),
            actor: actor.to_string(),
            published,
            to,
            cc,
            object,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_follow_with_a_string_object() {
        let json = serde_json::json!({
            "id": "https://remote.example/activities/1",
            "type": "Follow",
            "actor": "https://remote.example/users/bob",
            "object": "https://brillion.example.com/users/alice",
        });
        let activity: IncomingActivity = serde_json::from_value(json).unwrap();
        assert_eq!(activity.kind, "Follow");
        assert_eq!(activity.object, "https://brillion.example.com/users/alice");
    }

    #[test]
    fn builds_an_accept_wrapping_the_follow() {
        let follow = IncomingActivity {
            id: "https://remote.example/activities/1".to_string(),
            kind: "Follow".to_string(),
            actor: "https://remote.example/users/bob".to_string(),
            object: Value::String("https://brillion.example.com/users/alice".to_string()),
        };
        let accept = Accept::new(
            "https://brillion.example.com/activities/accept/1".to_string(),
            "https://brillion.example.com/users/alice",
            &follow,
        );

        let json = serde_json::to_value(&accept).unwrap();
        assert_eq!(json["type"], "Accept");
        assert_eq!(json["object"]["type"], "Follow");
        assert_eq!(json["object"]["actor"], "https://remote.example/users/bob");
    }

    #[test]
    fn builds_a_create_wrapping_an_object() {
        let create = Create::new(
            "https://brillion.example.com/users/alice/hello#create".to_string(),
            "https://brillion.example.com/users/alice",
            "2026-01-01T00:00:00Z".to_string(),
            vec![crate::AS2_PUBLIC.to_string()],
            vec!["https://brillion.example.com/users/alice/followers".to_string()],
            serde_json::json!({"type": "Article", "id": "https://brillion.example.com/users/alice/hello"}),
        );

        let json = serde_json::to_value(&create).unwrap();
        assert_eq!(json["type"], "Create");
        assert_eq!(json["to"][0], crate::AS2_PUBLIC);
        assert_eq!(json["object"]["type"], "Article");
    }

    #[test]
    fn builds_a_delete_wrapping_a_tombstone() {
        let delete = Delete::new(
            "https://brillion.example.com/users/alice/hello#delete-1".to_string(),
            "https://brillion.example.com/users/alice",
            "2026-01-02T00:00:00Z".to_string(),
            vec![crate::AS2_PUBLIC.to_string()],
            vec![],
            serde_json::json!({"type": "Tombstone", "id": "https://brillion.example.com/users/alice/hello"}),
        );

        let json = serde_json::to_value(&delete).unwrap();
        assert_eq!(json["type"], "Delete");
        assert_eq!(json["object"]["type"], "Tombstone");
    }
}

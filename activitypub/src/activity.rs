//! AS2 activity types. Inbound activities are parsed loosely (`object`
//! as raw JSON, since its shape depends on `type`); outbound activities
//! we send ourselves are built with the concrete constructors below.
//!
//! TODO(phase 3/4): `Create`/`Update`/`Delete`/`Like`/`Announce` land
//! with posts/replies/outbound federation.

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
}

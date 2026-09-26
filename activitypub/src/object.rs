//! AS2 object types: `Person` (actor documents, §3.2) and the
//! `OrderedCollection` shape used for followers/following/outbox (§3.3).
//!
//! TODO(phase 3): `Article`/`Note`/`Tombstone` land with posts/replies.

use crate::AS2_CONTEXT;
use serde::{Deserialize, Serialize};

/// The `https://w3id.org/security/v1` context, needed alongside the AS2
/// context whenever a document includes a `publicKey` (every local
/// actor document does).
pub const SECURITY_CONTEXT: &str = "https://w3id.org/security/v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Person {
    #[serde(rename = "@context")]
    pub context: Vec<String>,
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(rename = "preferredUsername")]
    pub preferred_username: String,
    pub name: String,
    pub summary: String,
    pub inbox: String,
    pub outbox: String,
    pub followers: String,
    pub following: String,
    #[serde(rename = "publicKey")]
    pub public_key: PublicKey,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<Image>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<Image>,
}

impl Person {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        urls: &crate::urls::ActorUrls,
        preferred_username: &str,
        display_name: &str,
        bio: &str,
        public_key_pem: &str,
        avatar_url: Option<&str>,
        header_url: Option<&str>,
    ) -> Self {
        Self {
            context: vec![AS2_CONTEXT.to_string(), SECURITY_CONTEXT.to_string()],
            id: urls.id.clone(),
            kind: "Person".to_string(),
            preferred_username: preferred_username.to_string(),
            name: display_name.to_string(),
            summary: bio.to_string(),
            inbox: urls.inbox.clone(),
            outbox: urls.outbox.clone(),
            followers: urls.followers.clone(),
            following: urls.following.clone(),
            public_key: PublicKey {
                id: format!("{}#main-key", urls.id),
                owner: urls.id.clone(),
                public_key_pem: public_key_pem.to_string(),
            },
            icon: avatar_url.map(Image::new),
            image: header_url.map(Image::new),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicKey {
    pub id: String,
    pub owner: String,
    #[serde(rename = "publicKeyPem")]
    pub public_key_pem: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
}

impl Image {
    pub fn new(url: &str) -> Self {
        Self {
            kind: "Image".to_string(),
            url: url.to_string(),
        }
    }
}

/// A non-paginated `OrderedCollection` — fine for followers/following at
/// the scale a single-instance blog will see; outbox pagination lands
/// with post volume in phase 3.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderedCollection {
    #[serde(rename = "@context")]
    pub context: String,
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(rename = "totalItems")]
    pub total_items: usize,
    #[serde(rename = "orderedItems")]
    pub ordered_items: Vec<String>,
}

impl OrderedCollection {
    pub fn new(id: &str, items: Vec<String>) -> Self {
        Self {
            context: AS2_CONTEXT.to_string(),
            id: id.to_string(),
            kind: "OrderedCollection".to_string(),
            total_items: items.len(),
            ordered_items: items,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::urls::ActorUrls;

    #[test]
    fn person_document_has_expected_shape() {
        let urls = ActorUrls::new("https://brillion.example.com", "alice");
        let person = Person::new(
            &urls,
            "alice",
            "Alice",
            "hello",
            "-----BEGIN PUBLIC KEY-----",
            None,
            None,
        );

        let json = serde_json::to_value(&person).unwrap();
        assert_eq!(json["type"], "Person");
        assert_eq!(json["id"], "https://brillion.example.com/users/alice");
        assert_eq!(
            json["publicKey"]["id"],
            "https://brillion.example.com/users/alice#main-key"
        );
        assert!(json.get("icon").is_none());
    }

    #[test]
    fn ordered_collection_reports_total_items() {
        let collection = OrderedCollection::new(
            "https://brillion.example.com/users/alice/followers",
            vec!["https://remote.example/users/bob".to_string()],
        );
        assert_eq!(collection.total_items, 1);
        assert_eq!(collection.kind, "OrderedCollection");
    }
}

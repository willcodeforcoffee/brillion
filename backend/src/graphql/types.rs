use crate::db;
use async_graphql::{Enum, InputObject, SimpleObject, ID};
use chrono::{DateTime, Utc};

#[derive(SimpleObject, Clone)]
pub struct ActorGql {
    pub id: ID,
    pub preferred_username: String,
    pub domain: String,
    pub display_name: String,
    pub bio: String,
    pub avatar_url: Option<String>,
    pub header_url: Option<String>,
}

impl From<db::actors::Actor> for ActorGql {
    fn from(actor: db::actors::Actor) -> Self {
        Self {
            id: ID(actor.id.to_string()),
            preferred_username: actor.preferred_username,
            domain: actor.domain,
            display_name: actor.display_name,
            bio: actor.bio,
            avatar_url: actor.avatar_url,
            header_url: actor.header_url,
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct MediaAttachmentGql {
    pub id: ID,
    pub url: String,
    pub media_type: String,
    pub alt_text: Option<String>,
}

impl From<db::media_attachments::MediaAttachment> for MediaAttachmentGql {
    fn from(media: db::media_attachments::MediaAttachment) -> Self {
        Self {
            id: ID(media.id.to_string()),
            url: media.url,
            media_type: media.media_type,
            alt_text: media.alt_text,
        }
    }
}

#[derive(SimpleObject, Clone)]
pub struct PostGql {
    pub id: ID,
    pub slug: String,
    pub title: String,
    pub summary: Option<String>,
    pub body_markdown: String,
    pub body_html: String,
    pub status: String,
    pub published_at: Option<DateTime<Utc>>,
    pub author: ActorGql,
    pub images: Vec<MediaAttachmentGql>,
}

#[derive(SimpleObject)]
pub struct PageInfo {
    pub has_next_page: bool,
    pub end_cursor: Option<String>,
}

#[derive(SimpleObject)]
pub struct PostEdge {
    pub cursor: String,
    pub node: PostGql,
}

#[derive(SimpleObject)]
pub struct PostConnection {
    pub edges: Vec<PostEdge>,
    pub page_info: PageInfo,
}

#[derive(InputObject)]
pub struct CreatePostInput {
    pub title: String,
    pub summary: Option<String>,
    pub body_markdown: String,
}

#[derive(InputObject)]
pub struct UpdatePostInput {
    pub title: String,
    pub summary: Option<String>,
    pub body_markdown: String,
}

#[derive(SimpleObject)]
pub struct ImageUploadTarget {
    /// Where the browser `PUT`s the file bytes directly (SPEC.md §5.2).
    pub upload_url: String,
    /// Pass this to `attachImage`/`updateActorImage` once the `PUT`
    /// completes.
    pub media_id: ID,
    pub public_url: String,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum ActorImageKind {
    Avatar,
    Header,
}

#[derive(SimpleObject)]
pub struct UserGql {
    pub username: String,
    pub email: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

impl From<db::users::UserSummary> for UserGql {
    fn from(user: db::users::UserSummary) -> Self {
        Self {
            username: user.username,
            email: user.email,
            role: user.role.to_string(),
            created_at: user.created_at,
        }
    }
}

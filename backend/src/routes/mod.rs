pub mod actor;
pub mod collections;
pub mod feeds;
pub mod graphql;
pub mod inbox;
pub mod nodeinfo;
pub mod oauth;
pub mod pages;
pub mod post;
pub mod webfinger;

use axum::response::{IntoResponse, Response};
use axum::Json;

/// A JSON response with the AS2 media type instead of `Json`'s default
/// `application/json`.
pub fn as2_json<T: serde::Serialize>(value: T) -> Response {
    ([("Content-Type", "application/activity+json")], Json(value)).into_response()
}

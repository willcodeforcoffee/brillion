//! ActivityStreams 2.0 / ActivityPub types and helpers for Brillion.
//!
//! Kept as a separate crate from `backend` so the AS2 data model and
//! HTTP Signatures logic have no dependency on Axum or any particular
//! HTTP framework. See `SPEC.md` §3 for the protocol surface this crate
//! will grow into across phases 2-3.

pub mod activity;
pub mod object;
pub mod signature;
pub mod urls;

/// The ActivityStreams 2.0 JSON-LD context URL, used verbatim (no
/// general-purpose JSON-LD expansion — see SPEC.md §3.6).
pub const AS2_CONTEXT: &str = "https://www.w3.org/ns/activitystreams";

/// The magic "public" addressee — an activity with this in `to` is
/// visible to anyone, per the AS2/ActivityPub convention (SPEC.md §3.4).
pub const AS2_PUBLIC: &str = "https://www.w3.org/ns/activitystreams#Public";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as2_context_is_the_standard_url() {
        assert_eq!(AS2_CONTEXT, "https://www.w3.org/ns/activitystreams");
    }
}

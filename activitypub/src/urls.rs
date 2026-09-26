//! URL conventions for local actors, matching the routes in SPEC.md
//! §3.2 (actor documents) and §3.3 (inbox/outbox/followers/following).

pub struct ActorUrls {
    /// The actor's AS2 `id` — also its profile page URL (§3.2, §4.1).
    pub id: String,
    pub inbox: String,
    pub outbox: String,
    pub followers: String,
    pub following: String,
}

impl ActorUrls {
    pub fn new(base_url: &str, preferred_username: &str) -> Self {
        let base = base_url.trim_end_matches('/');
        let id = format!("{base}/users/{preferred_username}");
        Self {
            inbox: format!("{id}/inbox"),
            outbox: format!("{id}/outbox"),
            followers: format!("{id}/followers"),
            following: format!("{id}/following"),
            id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_urls_under_the_actor_id() {
        let urls = ActorUrls::new("https://brillion.example.com/", "alice");
        assert_eq!(urls.id, "https://brillion.example.com/users/alice");
        assert_eq!(urls.inbox, "https://brillion.example.com/users/alice/inbox");
        assert_eq!(
            urls.followers,
            "https://brillion.example.com/users/alice/followers"
        );
    }
}

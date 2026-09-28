//! RustFS (S3-compatible) media storage — SPEC.md §5. Presigned PUT
//! URLs so image bytes go straight from the browser to RustFS, never
//! through `backend` (§5.2).

use rusty_s3::actions::{CreateBucket, PutObject, S3Action};
use rusty_s3::{Bucket, Credentials, UrlStyle};
use std::time::Duration;

#[derive(Clone)]
pub struct MediaStore {
    /// Compose-internal address (`http://rustfs:9000`) — used only for
    /// backend-to-RustFS admin calls (`ensure_bucket`), which never go
    /// through the reverse proxy.
    internal_bucket: Bucket,
    /// The blog's own public origin (e.g. `https://blog.example.com`,
    /// no path suffix) — used to sign presigned URLs, per §5.1: a
    /// signature is only valid for the exact host+path the signer used,
    /// so a URL meant to be hit by a browser through the proxy has to
    /// be signed against the proxy's address, not RustFS's internal one.
    public_bucket: Bucket,
    credentials: Credentials,
    public_origin: String,
}

impl MediaStore {
    pub fn new(
        internal_endpoint: &str,
        public_origin: &str,
        bucket_name: String,
        access_key: impl Into<String>,
        secret_key: impl Into<String>,
    ) -> anyhow::Result<Self> {
        let internal_url: url::Url = internal_endpoint.parse()?;
        let internal_bucket = Bucket::new(
            internal_url,
            UrlStyle::Path,
            bucket_name.clone(),
            "us-east-1",
        )
        .map_err(|e| anyhow::anyhow!("invalid RustFS internal endpoint/bucket: {e}"))?;
        let public_url: url::Url = public_origin.parse()?;
        let public_bucket = Bucket::new(public_url, UrlStyle::Path, bucket_name, "us-east-1")
            .map_err(|e| anyhow::anyhow!("invalid public origin/bucket: {e}"))?;
        let credentials = Credentials::new(access_key, secret_key);
        Ok(Self {
            internal_bucket,
            public_bucket,
            credentials,
            public_origin: public_origin.trim_end_matches('/').to_string(),
        })
    }

    /// Idempotently creates the configured bucket (SPEC.md §5.1) —
    /// best-effort: an "already exists" response is treated the same
    /// as success. Talks to RustFS directly (Compose-internal address),
    /// never through the proxy.
    ///
    /// NOTE: requests the `public-read` canned ACL (SPEC.md §12 #7
    /// decided public-read over presigned GETs), but this was verified
    /// to be a no-op against the actual `rustfs/rustfs:latest` image —
    /// it appears to ignore canned ACLs entirely (likely running with
    /// object ownership enforced, the modern S3 default that disables
    /// ACLs in favor of bucket policies). Confirmed via a real
    /// create+upload+anonymous-GET round trip: the object came back
    /// 403 regardless of this header, on both the bucket and the
    /// object. A real fix needs either a `PutBucketPolicy` call (not
    /// exposed by the `rusty-s3` crate; would need hand-rolled SigV4
    /// via `rusty_s3::signing::sign` for the `?policy` subresource) or
    /// falling back to presigned GETs after all. Left in as harmless
    /// and standards-correct in case a future RustFS version or a
    /// non-default config honors it.
    pub async fn ensure_bucket(&self, http: &reqwest::Client) -> anyhow::Result<()> {
        let mut action = CreateBucket::new(&self.internal_bucket, &self.credentials);
        action.headers_mut().insert("x-amz-acl", "public-read");
        let url = action.sign(Duration::from_secs(30));

        let response = http
            .put(url)
            .header("x-amz-acl", "public-read")
            .send()
            .await?;
        let status = response.status();
        if status.is_success() || status.as_u16() == 409 {
            Ok(())
        } else {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("failed to ensure RustFS bucket exists: {status} {body}")
        }
    }

    /// Returns a `(presigned PUT URL, public URL)` pair for uploading
    /// `object_key` with the given `content_type` — the browser PUTs
    /// directly to the first (through the reverse proxy's `/media/*`,
    /// §5.3), then the second is what gets stored on the post/actor.
    ///
    /// Signs against `public_bucket` (the proxy's own address) rather
    /// than RustFS's internal one — required for the signature to
    /// validate once the request actually reaches RustFS. The reverse
    /// proxy's `/media/*` route strips that prefix before forwarding to
    /// RustFS (`handle_path`, not `handle`) — so it's added back onto
    /// the signed URL's path here, *after* signing, without touching
    /// the signature itself (the signature only covers the pre-prefix
    /// path, which is exactly what RustFS sees once the proxy strips it
    /// back off) — and `deploy/Caddyfile` adds the CORS headers RustFS
    /// itself doesn't send.
    ///
    /// Verified correct via direct HTTP requests (curl and a Python
    /// script driving the real OAuth/GraphQL flow): the signature
    /// validates, the CORS headers are present, and repeated identical
    /// requests succeed consistently. **Not yet cleanly reproduced from
    /// a real end-user browser**: testing through this session's
    /// browser-automation tool showed intermittent `503`s on the same
    /// URLs that succeed every time over curl/Python run immediately
    /// afterward — i.e. the one variable that correlates with failure
    /// is "request came from that automated browser tab," not anything
    /// about the URL, signature, or CORS headers themselves, which is
    /// why this reads as a quirk of that tooling rather than a server-
    /// side bug — but it hasn't been confirmed clean in an ordinary
    /// browser outside that harness. If uploads still fail for you here
    /// with a genuine CORS error (not just a 503), that's new
    /// information this note doesn't already cover.
    pub fn presign_upload(&self, object_key: &str, content_type: &str) -> (String, String) {
        let mut action = PutObject::new(&self.public_bucket, Some(&self.credentials), object_key);
        action.headers_mut().insert("content-type", content_type);
        let mut upload_url = action.sign(Duration::from_secs(5 * 60));
        let bucket_relative_path = upload_url.path().to_string();
        upload_url.set_path(&format!("/media{bucket_relative_path}"));

        let public_url = format!("{}/media{bucket_relative_path}", self.public_origin);
        (upload_url.to_string(), public_url)
    }
}

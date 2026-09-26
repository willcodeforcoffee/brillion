//! HTTP Signatures: signing outbound federated requests and verifying
//! inbound ones against a remote actor's public key.
//!
//! Implements the widely-deployed `draft-cavage-http-signatures` scheme
//! (RSA-SHA256), the same one Mastodon/Pleroma/etc. use for S2S —
//! *not* the newer RFC 9421, which isn't yet common across the
//! Fediverse. See SPEC.md §3.4/§3.6.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use rsa::pkcs1v15::Pkcs1v15Sign;
use rsa::pkcs8::{
    DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey, LineEnding,
};
use rsa::{RsaPrivateKey, RsaPublicKey};
use sha2::{Digest, Sha256};

/// A local actor's RSA keypair, PEM-encoded for storage.
pub struct Keypair {
    pub public_key_pem: String,
    pub private_key_pem: String,
}

/// Generates a fresh 2048-bit RSA keypair for a new local actor.
pub fn generate_keypair() -> anyhow::Result<Keypair> {
    let mut rng = rand::rngs::OsRng;
    let private_key = RsaPrivateKey::new(&mut rng, 2048)?;
    let public_key = RsaPublicKey::from(&private_key);

    Ok(Keypair {
        private_key_pem: private_key.to_pkcs8_pem(LineEnding::LF)?.to_string(),
        public_key_pem: public_key.to_public_key_pem(LineEnding::LF)?,
    })
}

/// The `Digest` header value for a request body (`SHA-256=<base64>`).
pub fn compute_digest(body: &[u8]) -> String {
    format!("SHA-256={}", BASE64.encode(Sha256::digest(body)))
}

/// Verifies an inbound `Digest` header against the actual request body.
pub fn verify_digest(digest_header: &str, body: &[u8]) -> anyhow::Result<()> {
    anyhow::ensure!(
        digest_header == compute_digest(body),
        "Digest header does not match request body"
    );
    Ok(())
}

/// Builds a `Signature` header value, signing `(request-target)` plus
/// the given headers (in the order given) with the actor's private key.
///
/// `headers` should be the literal header name/value pairs that will be
/// sent on the wire (e.g. `("host", "example.com")`, `("date", ..)`,
/// `("digest", ..)`) — every one of them gets covered by the signature.
pub fn sign(
    private_key_pem: &str,
    key_id: &str,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
) -> anyhow::Result<String> {
    let mut signed_names = vec!["(request-target)".to_string()];
    signed_names.extend(headers.iter().map(|(name, _)| name.to_lowercase()));

    let signing_string = build_signing_string(method, path, headers, &signed_names)?;
    let hashed = Sha256::digest(signing_string.as_bytes());

    let private_key = RsaPrivateKey::from_pkcs8_pem(private_key_pem)?;
    let signature = private_key.sign(Pkcs1v15Sign::new::<Sha256>(), &hashed)?;

    Ok(format!(
        r#"keyId="{key_id}",algorithm="rsa-sha256",headers="{}",signature="{}""#,
        signed_names.join(" "),
        BASE64.encode(signature)
    ))
}

/// Verifies an inbound `Signature` header against the sender's public
/// key, reconstructing the same signing string from the *actual*
/// request's `method`/`path`/`headers` and the header list the sender
/// says it signed (from the `headers=` field).
pub fn verify(
    public_key_pem: &str,
    signature_header: &str,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
) -> anyhow::Result<()> {
    let parsed = ParsedSignature::parse(signature_header)?;
    let signed_names: Vec<String> = parsed.headers.split(' ').map(str::to_lowercase).collect();
    let signing_string = build_signing_string(method, path, headers, &signed_names)?;
    let hashed = Sha256::digest(signing_string.as_bytes());

    let public_key = RsaPublicKey::from_public_key_pem(public_key_pem)?;
    let signature_bytes = BASE64
        .decode(&parsed.signature)
        .map_err(|e| anyhow::anyhow!("invalid base64 in Signature header: {e}"))?;

    public_key
        .verify(Pkcs1v15Sign::new::<Sha256>(), &hashed, &signature_bytes)
        .map_err(|e| anyhow::anyhow!("signature verification failed: {e}"))
}

/// Reads just the `keyId` out of a `Signature` header, before the
/// caller has necessarily resolved/fetched that actor's public key.
pub fn extract_key_id(signature_header: &str) -> anyhow::Result<String> {
    Ok(ParsedSignature::parse(signature_header)?.key_id)
}

fn build_signing_string(
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    signed_names: &[String],
) -> anyhow::Result<String> {
    let mut lines = Vec::with_capacity(signed_names.len());
    for name in signed_names {
        if name == "(request-target)" {
            lines.push(format!(
                "(request-target): {} {path}",
                method.to_lowercase()
            ));
        } else {
            let value = headers
                .iter()
                .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
                .map(|(_, value)| *value)
                .ok_or_else(|| anyhow::anyhow!("missing header required by signature: {name}"))?;
            lines.push(format!("{name}: {value}"));
        }
    }
    Ok(lines.join("\n"))
}

struct ParsedSignature {
    key_id: String,
    headers: String,
    signature: String,
}

impl ParsedSignature {
    fn parse(header: &str) -> anyhow::Result<Self> {
        let mut key_id = None;
        let mut headers = None;
        let mut signature = None;

        for field in header.split(',') {
            let (name, value) = field
                .trim()
                .split_once('=')
                .ok_or_else(|| anyhow::anyhow!("malformed Signature header field: {field}"))?;
            let value = value.trim_matches('"');
            match name {
                "keyId" => key_id = Some(value.to_string()),
                "headers" => headers = Some(value.to_string()),
                "signature" => signature = Some(value.to_string()),
                _ => {}
            }
        }

        Ok(Self {
            key_id: key_id.ok_or_else(|| anyhow::anyhow!("Signature header missing keyId"))?,
            // Per the spec, headers defaults to "(created)" for other
            // schemes, but for draft-cavage/rsa-sha256 in practice every
            // implementation we federate with always sends `headers=`.
            headers: headers.ok_or_else(|| anyhow::anyhow!("Signature header missing headers"))?,
            signature: signature
                .ok_or_else(|| anyhow::anyhow!("Signature header missing signature"))?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_a_pem_encoded_keypair() {
        let keypair = generate_keypair().unwrap();
        assert!(keypair
            .private_key_pem
            .starts_with("-----BEGIN PRIVATE KEY-----"));
        assert!(keypair
            .public_key_pem
            .starts_with("-----BEGIN PUBLIC KEY-----"));
    }

    #[test]
    fn digest_round_trips() {
        let body = br#"{"type":"Follow"}"#;
        let digest = compute_digest(body);
        assert!(digest.starts_with("SHA-256="));
        assert!(verify_digest(&digest, body).is_ok());
        assert!(verify_digest(&digest, b"tampered body").is_err());
    }

    #[test]
    fn sign_and_verify_round_trip() {
        let keypair = generate_keypair().unwrap();
        let key_id = "https://example.com/users/alice#main-key";
        let body = br#"{"type":"Follow"}"#;
        let digest = compute_digest(body);
        let headers = [
            ("host", "brillion.example.com"),
            ("date", "Fri, 26 Sep 2026 12:00:00 GMT"),
            ("digest", digest.as_str()),
        ];

        let signature_header = sign(
            &keypair.private_key_pem,
            key_id,
            "post",
            "/users/bob/inbox",
            &headers,
        )
        .unwrap();

        assert_eq!(extract_key_id(&signature_header).unwrap(), key_id);
        verify(
            &keypair.public_key_pem,
            &signature_header,
            "POST",
            "/users/bob/inbox",
            &headers,
        )
        .unwrap();
    }

    #[test]
    fn verify_rejects_tampered_request() {
        let keypair = generate_keypair().unwrap();
        let headers = [("host", "brillion.example.com"), ("date", "irrelevant")];
        let signature_header = sign(
            &keypair.private_key_pem,
            "https://example.com/users/alice#main-key",
            "post",
            "/users/bob/inbox",
            &headers,
        )
        .unwrap();

        // A different path than what was actually signed must fail.
        let tampered_path = "/users/eve/inbox";
        assert!(verify(
            &keypair.public_key_pem,
            &signature_header,
            "POST",
            tampered_path,
            &headers,
        )
        .is_err());
    }

    #[test]
    fn verify_rejects_wrong_key() {
        let signer = generate_keypair().unwrap();
        let other = generate_keypair().unwrap();
        let headers = [("host", "brillion.example.com")];
        let signature_header = sign(
            &signer.private_key_pem,
            "https://example.com/users/alice#main-key",
            "post",
            "/users/bob/inbox",
            &headers,
        )
        .unwrap();

        assert!(verify(
            &other.public_key_pem,
            &signature_header,
            "POST",
            "/users/bob/inbox",
            &headers,
        )
        .is_err());
    }
}

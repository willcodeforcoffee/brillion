use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as BASE64;
use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("failed to hash password: {e}"))?;
    Ok(hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> anyhow::Result<bool> {
    let parsed =
        PasswordHash::new(hash).map_err(|e| anyhow::anyhow!("invalid password hash: {e}"))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// A random, URL-safe, high-entropy token — for OAuth client
/// ids/secrets, authorization codes, and access/refresh tokens. Not for
/// user passwords (see `hash_password` for those).
pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    BASE64.encode(bytes)
}

/// Deterministic hash for looking up a high-entropy token by equality
/// (unlike Argon2id, which salts randomly and so can't be looked up by
/// value) — safe here because these tokens are random, not
/// user-chosen, so a fast hash isn't a brute-force risk.
pub fn hash_token(token: &str) -> String {
    BASE64.encode(Sha256::digest(token.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashed_password_verifies_against_the_original() {
        let hash = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password("correct horse battery staple", &hash).unwrap());
        assert!(!verify_password("wrong password", &hash).unwrap());
    }

    #[test]
    fn tokens_are_unique_and_hash_deterministically() {
        let a = generate_token();
        let b = generate_token();
        assert_ne!(a, b);
        assert_eq!(hash_token(&a), hash_token(&a));
        assert_ne!(hash_token(&a), hash_token(&b));
    }
}

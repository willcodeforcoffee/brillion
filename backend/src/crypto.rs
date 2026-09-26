use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHasher, SaltString};
use argon2::Argon2;

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("failed to hash password: {e}"))?;
    Ok(hash.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use argon2::password_hash::{PasswordHash, PasswordVerifier};

    #[test]
    fn hashed_password_verifies_against_the_original() {
        let hash = hash_password("correct horse battery staple").unwrap();
        let parsed = PasswordHash::new(&hash).unwrap();
        assert!(Argon2::default()
            .verify_password(b"correct horse battery staple", &parsed)
            .is_ok());
        assert!(Argon2::default()
            .verify_password(b"wrong password", &parsed)
            .is_err());
    }
}

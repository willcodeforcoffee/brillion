//! HTTP Signatures: signing outbound federated requests and verifying
//! inbound ones against a remote actor's public key.
//!
//! TODO(phase 2/3): implement request signing/verification per
//! SPEC.md §3.4/§3.6 — `Signature` header construction/parsing and
//! `Digest: SHA-256=...` verification. Keypair generation (below) is
//! implemented now since it's needed at actor-creation time (§8 CLI).

use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};
use rsa::{RsaPrivateKey, RsaPublicKey};

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
}

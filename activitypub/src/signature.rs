//! HTTP Signatures: signing outbound federated requests and verifying
//! inbound ones against a remote actor's public key.
//!
//! TODO(phase 2/3): implement per SPEC.md §3.4/§3.6 — `Signature` header
//! construction/parsing, `Digest: SHA-256=...` verification, and RSA
//! keypair generation for local actors.

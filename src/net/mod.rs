//! Networking primitives for the JULIAN Protocol (JROC-NET).
//!
//! These modules are gated behind the `net` Cargo feature and provide the
//! functionality required by the `julian net` CLI commands: JSON schemas for
//! anchors and envelopes, signing helpers, and the libp2p swarm orchestration
//! that powers the public testnet mode.

#![cfg(feature = "net")]

/// Anchor checkpoint helpers for fast sync.
pub mod checkpoint;
/// Governance policy implementations for membership rotation.
pub mod governance;
/// Identity admission policy helpers.
pub mod policy;
/// Machine-readable schema types shared across the network CLI and swarm.
pub mod schema;
/// Deterministic key derivation and ed25519 signing helpers.
pub mod sign;
/// Libp2p orchestration layer and networking runtime.
pub mod swarm;

pub use checkpoint::{
    anchor_hasher, latest_log_cutoff, load_latest_checkpoint, write_checkpoint, AnchorCheckpoint,
    CheckpointError, CheckpointSignature,
};
pub use governance::{
    GovernanceUpdate, MembershipPolicy, MultisigPolicy, PolicyUpdateError, StaticPolicy,
};
pub use policy::{IdentityPolicy, PolicyError};
pub use schema::{AnchorEnvelope, AnchorJson};
pub use sign::{
    decode_public_key_base64, decode_signature_base64, encode_public_key_base64,
    encode_signature_base64, load_encrypted_identity, load_or_derive_keypair, sign_payload,
    verify_signature, verify_signature_base64, Ed25519KeySource, KeyError, KeyMaterial,
};
pub use swarm::{run_network, NetConfig, NetworkError};

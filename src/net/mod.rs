//! Networking primitives for the JULIAN Protocol (MFENX Power-House Network).
//!
//! These modules are gated behind the `net` Cargo feature and provide the
//! functionality required by the `julian net` CLI commands: JSON schemas for
//! anchors and envelopes, signing helpers, and the libp2p swarm orchestration
//! that powers the public testnet mode.

/// Availability attestations and quorum helpers.
pub mod attestation;
/// Erasure coding helpers and commitments.
pub mod availability;
/// Data-availability blob schema and envelope types.
pub mod blob;
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
/// Durable stake/balance store for fee enforcement and slashing.
pub mod stake_registry;
/// Libp2p orchestration layer and networking runtime.
pub mod swarm;

pub use attestation::{aggregate_attestations, Attestation, AttestationQuorum};
pub use availability::{encode_shares, share_proof, verify_sample, ShareCommitment};
pub use blob::{BlobCodecError, BlobEnvelope, BlobJson, SCHEMA_BLOB, TOPIC_BLOBS};
pub use checkpoint::{
    anchor_hasher, latest_log_cutoff, load_latest_checkpoint, write_checkpoint, AnchorCheckpoint,
    CheckpointError, CheckpointSignature,
};
pub use governance::{
    GovernanceUpdate, MembershipPolicy, MultisigPolicy, PolicyUpdateError, StakePolicy,
    StaticPolicy,
};
pub use policy::{IdentityPolicy, PolicyError};
pub use schema::{AnchorEnvelope, AnchorJson};
pub use sign::{
    decode_public_key_base64, decode_signature_base64, encode_public_key_base64,
    encode_signature_base64, load_encrypted_identity, load_or_derive_keypair, sign_payload,
    verify_signature, verify_signature_base64, Ed25519KeySource, KeyError, KeyMaterial,
};
pub use stake_registry::StakeRegistry;
pub use swarm::{run_network, NamespaceRule, NetConfig, NetworkError};

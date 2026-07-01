#![deny(missing_docs)]

//! Deterministic verification, portable proof provenance, and optional quorum
//! networking.
//!
//! Power House combines seven interoperable layers:
//!
//! - [`identity`] provides immutable computational identities over `.pha` and
//!   Rootprint.
//! - [`memory`] packages core artifacts, Rootprint lineage, replay state,
//!   semantic bindings, witnesses, and challenge vectors into portable
//!   proof-memory capsules.
//! - [`observatory`] binds optional human-readable visualization sidecars to
//!   verified Rootprint replay state.
//! - [`provenance`] defines Power House Archive (`.pha`) and Rootprint v1.
//! - [`sumcheck`] implements dense, streaming, constant, and seeded-affine
//!   sum-check workflows.
//! - [`sfcs`] defines opt-in draft computational-fractal primitives when the
//!   `sfcs` feature is enabled.
//! - [`sparse_sumcheck`] implements stable seeded and commitment-bound sparse
//!   certificate formats.
//! - [`julian`] records proof transcripts, anchors them, and reconciles quorum
//!   state.
//! - [`net`] adds signed libp2p transport, data availability, governance, and
//!   quorum-finalized native RPC when the `net` feature is enabled.
//!
//! # Power House Archive
//!
//! A [`provenance::PhaArtifact`] binds core proof data and provenance to a
//! deterministic `phx_fingerprint`. Optional external proof attachments are
//! transported with the artifact but do not alter its Power House core
//! identity.
//!
//! ```
//! use power_house::provenance::PhaArtifact;
//! use serde_json::json;
//!
//! let artifact = PhaArtifact::new(
//!     json!({"producer": "example"}),
//!     "power-house/example/v1",
//!     json!({"claim": 7}),
//!     json!({"accepted": true}),
//! )?;
//! artifact.verify()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Rootprint
//!
//! [`provenance::Rootprint`] is a deterministic directed acyclic graph of
//! `.pha` artifacts. The [`prove_with_rootprint!`] macro is the recommended
//! construction interface.
//!
//! ```
//! use power_house::{prove_with_rootprint, provenance::PhaArtifact};
//! use serde_json::json;
//!
//! let artifact = PhaArtifact::new(
//!     json!({"source": "rootprint-example"}),
//!     "power-house/example/v1",
//!     json!({"claim": 11}),
//!     json!({"accepted": true}),
//! )?;
//! let graph = prove_with_rootprint!(label: "main", artifact: artifact)?;
//! graph.verify()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Structured sum-check
//!
//! Closed-form and seeded proof constructors operate on compact algebraic
//! descriptions without allocating the expanded Boolean hypercube.
//!
//! ```
//! use power_house::{Field, GeneralSumProof};
//!
//! let field = Field::new(1_000_000_007);
//! let proof = GeneralSumProof::prove_seeded_affine(
//!     4096,
//!     &field,
//!     b"public reproducible workload",
//! );
//! assert!(proof.verify_seeded_affine(
//!     &field,
//!     b"public reproducible workload",
//! ));
//! ```
//!
//! # Feature flags
//!
//! - `default`: proof, provenance, transcript, sparse-certificate, and memory
//!   APIs.
//! - `memory`: portable proof-memory capsules without network access.
//! - `memory-net`: memory workflows that may compose with network features.
//! - `net`: networking, migration commands, data availability, governance,
//!   staking, and native JSON-RPC.
//!
//! # Specifications and guides
//!
//! The repository contains the normative `.pha` and Rootprint specifications,
//! cross-language conformance vectors, provenance and sparse security models,
//! verification guide, and operational runbooks. See the
//! [documentation index](https://github.com/JROChub/power_house/blob/main/docs/README.md).

pub mod consensus;
mod data;
pub mod economics;
mod field;
pub mod identity;
mod io;
pub mod julian;
mod log_parser;
pub mod memory;
mod merkle;
mod multilinear;
pub mod observatory;
mod prng;
pub mod provenance;
pub mod rollup;
#[cfg(feature = "sfcs")]
pub mod sfcs;
pub mod sparse_sumcheck;
mod streaming;
pub mod sumcheck;
mod transcript;

/// CLI command helpers for migration and deterministic artifacts.
#[cfg(feature = "net")]
pub mod commands;
#[cfg(feature = "net")]
pub mod net;

pub use consensus::consensus;
pub use data::{
    compute_digest as transcript_digest, digest_from_hex as transcript_digest_from_hex,
    digest_to_hex as transcript_digest_to_hex, parse_record as parse_transcript_record,
    verify_record_lines as verify_transcript_lines, write_record as write_transcript_record,
    TranscriptDigest,
};
pub use field::Field;
pub use identity::{Identity, IdentityError, IdentityState};
pub use io::write_text_series;
pub use julian::{
    compute_fold_digest, julian_genesis_anchor, julian_genesis_hash, reconcile_anchors,
    reconcile_anchors_with_quorum, AnchorMetadata, AnchorVote, EntryAnchor, LedgerAnchor, Proof,
    ProofKind, ProofLedger, Statement, JULIAN_GENESIS_STATEMENT,
};
pub use log_parser::{parse_log_file, read_fold_digest_hint, LogRecordMetadata, ParsedLogFile};
pub use memory::{
    ChallengeSuite, ChallengeVector, MemoryCapsule, MemoryCapsuleBuilder, MemoryCapsuleReport,
    MemoryChallengeReport, MemoryError, MemoryReplayReport, MemoryVerificationPolicy,
    MemoryVerificationReport, RejectionTrace, WitnessReceipt,
};
pub use merkle::{
    build_proof as build_merkle_proof, merkle_root, verify_proof as verify_merkle_proof,
    MerkleProof, MerkleProofNode,
};
pub use multilinear::MultilinearPolynomial;
pub use observatory::{ObservatoryError, ObservatorySidecar};
pub use prng::SimplePrng;
#[cfg(feature = "sfcs")]
pub use sfcs::compiler::{
    compile_llvm_ir_source, compile_public_rust_source, compile_wasm_stack_source,
    SfcsCompiledLlvmIr, SfcsCompiledPublicRust, SfcsCompiledWasmStack, SfcsCompilerError,
};
#[cfg(feature = "sfcs-zk")]
pub use sfcs::compiler::{compile_private_add_source, SfcsCompiledPrivateAdd};
#[cfg(feature = "sfcs")]
pub use sfcs::constraints::{
    verify_vm_constraint_embedding, SfcsVmConstraintError, SfcsVmConstraintProof,
    SfcsVmMemoryConstraintEvent, SfcsVmTransitionCommitment, SFCS_VM_CONSTRAINT_PROTOCOL_V1_DRAFT,
};
#[cfg(feature = "sfcs")]
pub use sfcs::vm::{
    verify_vm_execution_embedding, SfcsVmError, SfcsVmExecutionEmbeddingReport,
    SfcsVmExecutionTrace, SfcsVmInputs, SfcsVmMemoryAccess, SfcsVmMemoryRange, SfcsVmProgram,
    SfcsVmPublicOutputs, SfcsVmTraceStep,
};
#[cfg(feature = "sfcs-zk")]
pub use sfcs::zk::{
    encode_rv32_add, verify_private_add_embedding, verify_private_add_program, SfcsZkError,
    SfcsZkPrivateAddProof, SfcsZkPrivateAddStatement, SfcsZkPrivateAddWitness,
    SFCS_ZK_PRIVATE_ADD_PROTOCOL_V1_DRAFT,
};
#[cfg(feature = "sfcs")]
pub use sfcs::{
    SfcsDiscoveryReport, SfcsEmbeddingReport, SfcsError, SfcsExecutionEmbeddingReport,
    SfcsExecutionTrace, SfcsFastPathCertificate, SfcsFastPathWorkload, SfcsGraph, SfcsNode, SfcsOp,
    SfcsRegionKind, SfcsRewriteKind, SfcsRewriteOperation, SfcsStructureRegion, SfcsSynthesisPlan,
    SfcsTraceStep, SovereignFastPath,
};
pub use sparse_sumcheck::{
    CommittedSparsePolynomial, CommittedSparseProof, SeededSparseProof, SeededSparseSpec,
    SparseMonomial, SparseProofError, SparseVerificationReport,
};
pub use streaming::StreamingPolynomial;
pub use sumcheck::{ChainedSumProof, GeneralSumClaim, GeneralSumProof, ProofStats, SumClaim};
pub use transcript::Transcript;

/// Verify that a `.pha` artifact contains a deterministic SFCS graph embedding.
///
/// This is the root-level API for callers that want the SFCS draft bridge while
/// keeping the `.pha` core fingerprint and Rootprint rules unchanged.
#[cfg(feature = "sfcs")]
pub fn verify_sfcs_pha_embedding(
    artifact: &provenance::pha::PhaArtifact,
) -> Result<SfcsEmbeddingReport, SfcsError> {
    sfcs::verify_pha_embedding(artifact)
}

/// Verify that a `.pha` artifact contains a deterministic SFCS execution trace.
///
/// This checks graph parsing, replayed execution, synthesis-plan replay,
/// output binding, and the SFCS embedding invariant while leaving Rootprint v1
/// and `.pha` core fingerprint rules unchanged.
#[cfg(feature = "sfcs")]
pub fn verify_sfcs_execution_embedding(
    artifact: &provenance::pha::PhaArtifact,
) -> Result<SfcsExecutionEmbeddingReport, SfcsError> {
    sfcs::verify_execution_embedding(artifact)
}

/// Verify that a `.pha` artifact contains a deterministic SFCS VM execution.
///
/// This checks the RV32I program image, public inputs, full instruction trace,
/// memory/register digests, and public output binding without changing
/// Rootprint v1 or `.pha` core fingerprint rules.
#[cfg(feature = "sfcs")]
pub fn verify_sfcs_vm_execution_embedding(
    artifact: &provenance::pha::PhaArtifact,
) -> Result<SfcsVmExecutionEmbeddingReport, SfcsVmError> {
    sfcs::vm::verify_vm_execution_embedding(artifact)
}

/// Verify that a `.pha` artifact contains a public SFCS VM constraint proof.
///
/// This recomputes the supported RV32I execution and verifies transition,
/// memory consistency, range-check coverage, trace digest, and execution
/// fractal binding. It is a transparent public proof layer, not a
/// zero-knowledge proof.
#[cfg(feature = "sfcs")]
pub fn verify_sfcs_vm_constraint_embedding(
    artifact: &provenance::pha::PhaArtifact,
) -> Result<SfcsVmConstraintProof, SfcsVmConstraintError> {
    sfcs::constraints::verify_vm_constraint_embedding(artifact)
}

/// Verify that a `.pha` artifact contains the SFCS ZK private-add proof profile.
#[cfg(feature = "sfcs-zk")]
pub fn verify_sfcs_zk_private_add_embedding(
    artifact: &provenance::pha::PhaArtifact,
) -> Result<SfcsZkPrivateAddProof, SfcsZkError> {
    sfcs::zk::verify_private_add_embedding(artifact)
}

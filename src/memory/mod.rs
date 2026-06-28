//! Portable proof-memory capsules.
//!
//! Memory Capsules package Power House core artifacts, Rootprint lineage,
//! replay expectations, optional non-core semantic bindings, witnesses,
//! challenge vectors, and reproduction receipts into one offline-verifiable
//! object. Semantic data remains outside `.pha` fingerprints and Rootprint
//! branch identity.

mod canonical;
mod capsule;
mod challenge;
mod errors;
mod policy;
mod report;

pub use canonical::{digest_bytes, digest_json, validate_sha256};
pub use capsule::{
    semantic_packet_digest, CapsuleHeader, CoreLayer, CoreProofDescriptor, CoreVerificationPolicy,
    EquivalenceClaim, LineageLayer, MemoryBranch, MemoryCapsule, MemoryCapsuleBuilder,
    ProducerInfo, ReplayExpected, ReplayLayer, ReplayPlan, ReplayResourceBounds,
    ReproductionReceipt, SemanticLayer, SemanticPacketBinding, SemanticPolicy, WitnessReceipt,
    MEMORY_CAPSULE_SCHEMA_V1,
};
pub use challenge::{ChallengeExpectation, ChallengeSuite, ChallengeVector};
pub use errors::{MemoryError, RejectionTrace};
pub use policy::MemoryVerificationPolicy;
pub use report::{
    ChallengeResult, MemoryCapsuleReport, MemoryChallengeReport, MemoryReplayReport,
    MemoryVerificationReport, SoundnessReport, VerificationTimings, WitnessValidity,
};

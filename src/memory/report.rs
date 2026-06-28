//! Verification, replay, and challenge reports for Memory Capsules.

use super::errors::RejectionTrace;

/// Timing fields emitted by verifiers.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VerificationTimings {
    /// Total wall-clock verification time in milliseconds.
    pub total_ms: u64,
}

/// Soundness and scope statement attached to verification reports.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SoundnessReport {
    /// Verification profile name.
    pub profile: String,
    /// Public domain size statement.
    pub domain_size_description: String,
    /// Verification mode.
    pub verification_mode: String,
    /// Whether an expanded evaluation table was allocated.
    pub expanded_table_allocated: bool,
    /// Whether public inputs are present.
    pub public_inputs: bool,
    /// Whether a hidden witness is claimed.
    pub hidden_witness: bool,
    /// Whether a succinct VM proof is claimed.
    pub succinct_vm_proof: bool,
    /// Classical soundness statement.
    pub classical_soundness_bits: String,
    /// Scope notes.
    pub notes: Vec<String>,
}

impl Default for SoundnessReport {
    fn default() -> Self {
        Self {
            profile: "portable_proof_memory".to_string(),
            domain_size_description: "profile-specific compact verification".to_string(),
            verification_mode: "deterministic conformance replay".to_string(),
            expanded_table_allocated: false,
            public_inputs: true,
            hidden_witness: false,
            succinct_vm_proof: false,
            classical_soundness_bits: "profile-specific".to_string(),
            notes: vec![
                "This verifies the specified deterministic artifact, lineage, and bindings."
                    .to_string(),
                "Semantic packets explain verified state but do not change proof identity."
                    .to_string(),
            ],
        }
    }
}

/// Per-witness verification result.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WitnessValidity {
    /// Witness identifier.
    pub witness_id: String,
    /// Whether the receipt matched capsule state.
    pub valid: bool,
    /// Verification detail.
    pub detail: String,
}

/// Report returned by Memory Capsule verification.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MemoryVerificationReport {
    /// Capsule digest.
    pub capsule_digest: String,
    /// Core layer validity.
    pub core_valid: bool,
    /// Rootprint validity.
    pub rootprint_valid: bool,
    /// Replay validity.
    pub replay_valid: bool,
    /// Sidecar validity when present.
    pub sidecar_valid: Option<bool>,
    /// Semantic packet binding validity when present.
    pub semantic_valid: Option<bool>,
    /// Witness receipt results.
    pub witness_validity: Vec<WitnessValidity>,
    /// Rejection trace when verification failed.
    pub rejection_trace: Option<RejectionTrace>,
    /// Soundness and scope statement.
    pub soundness_report: Option<SoundnessReport>,
    /// Timing data.
    pub timings: VerificationTimings,
}

/// Deterministic replay report.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MemoryReplayReport {
    /// Capsule digest.
    pub capsule_digest: String,
    /// Replayed Rootprint fingerprint.
    pub replay_fingerprint: String,
    /// Number of replayed branches.
    pub branch_count: usize,
    /// Whether replay matched the expected state.
    pub replay_valid: bool,
    /// Tool version used for replay.
    pub power_house_version: String,
    /// Network access requirement.
    pub network_required: bool,
}

/// One challenge vector result.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChallengeResult {
    /// Challenge vector identifier.
    pub id: String,
    /// Whether the expected rejection occurred.
    pub passed: bool,
    /// Expected rejection layer.
    pub expected_layer: String,
    /// Actual rejection layer.
    pub actual_layer: Option<String>,
    /// Expected rejection code.
    pub expected_code: String,
    /// Actual rejection code.
    pub actual_code: Option<String>,
    /// Whether the core layer remained valid before failure.
    pub core_valid_before_failure: bool,
    /// Human-readable detail.
    pub detail: String,
}

/// Challenge suite report.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MemoryChallengeReport {
    /// Total vectors run.
    pub total: usize,
    /// Number of expected rejections observed.
    pub expected_rejections: usize,
    /// Number of mismatches.
    pub mismatches: usize,
    /// Per-vector results.
    pub results: Vec<ChallengeResult>,
}

/// Compatibility alias for high-level memory reports.
pub type MemoryCapsuleReport = MemoryVerificationReport;

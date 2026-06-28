//! Verification policy for Memory Capsules.

/// Verification policy controlling Memory Capsule admission.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MemoryVerificationPolicy {
    /// Require a valid Rootprint graph.
    pub require_rootprint: bool,
    /// Require replay fingerprint verification.
    pub require_replay: bool,
    /// Require a valid sidecar when a semantic layer is present.
    pub require_sidecar_if_present: bool,
    /// Permit network access during verification.
    pub allow_network: bool,
    /// Reject unknown critical extensions.
    pub fail_on_unknown_critical: bool,
    /// Maximum input bytes accepted by strict parsers.
    pub max_bytes: u64,
    /// Advisory memory ceiling in MiB.
    pub max_memory_mb: u64,
    /// Optional maximum round count for profiles that expose rounds.
    pub max_rounds: Option<u64>,
}

impl MemoryVerificationPolicy {
    /// Returns the strict offline policy.
    pub fn strict() -> Self {
        Self {
            require_rootprint: true,
            require_replay: true,
            require_sidecar_if_present: true,
            allow_network: false,
            fail_on_unknown_critical: true,
            max_bytes: 64 * 1024 * 1024,
            max_memory_mb: 512,
            max_rounds: Some(1_000_000),
        }
    }

    /// Returns a permissive local inspection policy.
    pub fn inspect() -> Self {
        Self {
            require_rootprint: true,
            require_replay: false,
            require_sidecar_if_present: false,
            allow_network: false,
            fail_on_unknown_critical: true,
            max_bytes: 128 * 1024 * 1024,
            max_memory_mb: 1024,
            max_rounds: None,
        }
    }
}

impl Default for MemoryVerificationPolicy {
    fn default() -> Self {
        Self::strict()
    }
}

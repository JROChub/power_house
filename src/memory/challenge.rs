//! Challenge vectors for adversarial Memory Capsule verification.

/// Expected failure for a challenge vector.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ChallengeExpectation {
    /// Expected validity of the core layer before rejection.
    pub core_valid: bool,
    /// Expected validity of the sidecar layer.
    pub sidecar_valid: Option<bool>,
    /// Expected validity of the semantic layer.
    pub semantic_valid: Option<bool>,
    /// Expected rejection layer.
    pub rejection_layer: String,
    /// Expected rejection code.
    pub rejection_code: String,
    /// Expected message substring.
    pub reason_contains: String,
}

/// One deterministic mutation vector.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ChallengeVector {
    /// Stable vector identifier.
    pub id: String,
    /// Human-readable target layer or field.
    pub target: String,
    /// Mutation kind.
    pub mutation: String,
    /// JSON pointer used by replace/remove mutations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Replacement value for replace mutations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    /// Expected failure.
    pub expected: ChallengeExpectation,
}

impl ChallengeVector {
    /// Creates a JSON pointer replacement vector.
    pub fn replace(
        id: impl Into<String>,
        path: impl Into<String>,
        value: serde_json::Value,
        layer: impl Into<String>,
        code: impl Into<String>,
        core_valid: bool,
    ) -> Self {
        let layer = layer.into();
        let code = code.into();
        Self {
            id: id.into(),
            target: "json-pointer".to_string(),
            mutation: "replace".to_string(),
            path: Some(path.into()),
            value: Some(value),
            expected: ChallengeExpectation {
                core_valid,
                sidecar_valid: None,
                semantic_valid: None,
                rejection_layer: layer.clone(),
                rejection_code: code.clone(),
                reason_contains: code,
            },
        }
    }
}

/// A set of challenge vectors.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ChallengeSuite {
    /// Mutation vectors.
    pub mutations: Vec<ChallengeVector>,
}

impl ChallengeSuite {
    /// Creates an empty suite.
    pub fn new() -> Self {
        Self {
            mutations: Vec::new(),
        }
    }

    /// Adds a mutation vector.
    pub fn with_vector(mut self, vector: ChallengeVector) -> Self {
        self.mutations.push(vector);
        self
    }

    /// Standard capsule challenge set covering core, lineage, replay, and semantic boundaries.
    pub fn standard() -> Self {
        use serde_json::json;

        let vectors = vec![
            ChallengeVector::replace(
                "mut_capsule_digest_001",
                "/header/capsule_digest",
                json!("sha256:0000000000000000000000000000000000000000000000000000000000000000"),
                "capsule",
                "CAPSULE_DIGEST_MISMATCH",
                false,
            ),
            ChallengeVector::replace(
                "mut_schema_001",
                "/header/schema",
                json!("power-house/memory-capsule/v0"),
                "header",
                "UNSUPPORTED_SCHEMA",
                false,
            ),
            ChallengeVector::replace(
                "mut_core_digest_001",
                "/core/core_digest",
                json!("sha256:1111111111111111111111111111111111111111111111111111111111111111"),
                "core",
                "CORE_DIGEST_MISMATCH",
                false,
            ),
            ChallengeVector::replace(
                "mut_core_fingerprint_001",
                "/core/pha/phx_fingerprint",
                json!("sha256:2222222222222222222222222222222222222222222222222222222222222222"),
                "core",
                "PHA_CORE_INVALID",
                false,
            ),
            ChallengeVector::replace(
                "mut_rootprint_root_001",
                "/lineage/rootprint/root_branch",
                json!("sha256:3333333333333333333333333333333333333333333333333333333333333333"),
                "rootprint",
                "ROOTPRINT_INVALID",
                true,
            ),
            ChallengeVector::replace(
                "mut_replay_fingerprint_001",
                "/replay/replay/expected/replay_fingerprint",
                json!("sha256:4444444444444444444444444444444444444444444444444444444444444444"),
                "replay",
                "REPLAY_FINGERPRINT_MISMATCH",
                true,
            ),
            ChallengeVector::replace(
                "mut_sidecar_digest_001",
                "/semantics/sidecar/sidecar_sha256",
                json!("sha256:5555555555555555555555555555555555555555555555555555555555555555"),
                "sidecar",
                "SIDECAR_INVALID",
                true,
            ),
            ChallengeVector::replace(
                "mut_semantic_digest_001",
                "/semantics/packets/0/packet_digest",
                json!("sha256:6666666666666666666666666666666666666666666666666666666666666666"),
                "semantic",
                "PACKET_DIGEST_MISMATCH",
                true,
            ),
            ChallengeVector::replace(
                "mut_semantic_branch_001",
                "/semantics/packets/0/bound_branch_id",
                json!("sha256:7777777777777777777777777777777777777777777777777777777777777777"),
                "semantic",
                "SEMANTIC_BRANCH_BINDING_MISMATCH",
                true,
            ),
            ChallengeVector::replace(
                "mut_semantic_replay_001",
                "/semantics/packets/0/bound_replay_fingerprint",
                json!("sha256:8888888888888888888888888888888888888888888888888888888888888888"),
                "semantic",
                "SEMANTIC_REPLAY_BINDING_MISMATCH",
                true,
            ),
        ];

        Self { mutations: vectors }
    }
}

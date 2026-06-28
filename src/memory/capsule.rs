//! Power House Memory Capsule v1.

use super::{
    canonical::{canonical_bytes, digest_json, parse_strict_value, validate_sha256},
    challenge::ChallengeSuite,
    errors::{MemoryError, RejectionTrace},
    policy::MemoryVerificationPolicy,
    report::{
        ChallengeResult, MemoryChallengeReport, MemoryReplayReport, MemoryVerificationReport,
        SoundnessReport, VerificationTimings, WitnessValidity,
    },
};
use crate::{
    observatory::ObservatorySidecar,
    provenance::{PhaArtifact, Rootprint},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs, path::Path, time::Instant};

/// Memory Capsule v1 schema identifier.
pub const MEMORY_CAPSULE_SCHEMA_V1: &str = "power-house/memory-capsule/v1";

const CAPSULE_DOMAIN: &[u8] = b"PHM-CAPSULE-v1\0";
const CORE_DOMAIN: &[u8] = b"PHM-CORE-v1\0";
const SEMANTIC_PACKET_DOMAIN: &[u8] = b"PHM-SEMANTIC-PACKET-v1\0";

/// Producer metadata for a Memory Capsule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProducerInfo {
    /// Producer name.
    pub name: String,
    /// Producing tool.
    pub tool: String,
    /// Power House version.
    pub power_house_version: String,
    /// slbit version or declared semantic producer version.
    pub slbit_version: Option<String>,
    /// Rust compiler identifier when known.
    pub rustc: Option<String>,
    /// Producing platform.
    pub platform: Option<String>,
}

/// Memory Capsule header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapsuleHeader {
    /// Capsule schema.
    pub schema: String,
    /// Stable capsule identifier.
    pub capsule_id: String,
    /// Domain-separated capsule digest.
    pub capsule_digest: Option<String>,
    /// Metadata timestamp in Unix milliseconds.
    pub created_at_unix_ms: u64,
    /// Producer metadata.
    pub producer: ProducerInfo,
    /// Critical extension identifiers.
    pub critical_extensions: Vec<String>,
    /// Noncritical extension identifiers.
    pub noncritical_extensions: Vec<String>,
}

/// Core proof descriptor carried by a Memory Capsule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreProofDescriptor {
    /// Proof kind.
    pub kind: String,
    /// Proof schema.
    pub schema: String,
    /// Proof digest.
    pub digest: String,
    /// Optional blob reference.
    pub bytes_ref: Option<String>,
    /// Public statement.
    pub public_statement: String,
    /// Verification profile.
    pub verification_profile: String,
}

/// Core verification policy recorded inside the capsule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreVerificationPolicy {
    /// Require Rootprint.
    pub require_rootprint: bool,
    /// Require replay.
    pub require_replay: bool,
    /// Allow explicitly marked external attachments.
    pub allow_external_attachments: bool,
    /// Reject unknown critical extensions.
    pub fail_on_unknown_critical: bool,
}

impl Default for CoreVerificationPolicy {
    fn default() -> Self {
        Self {
            require_rootprint: true,
            require_replay: true,
            allow_external_attachments: true,
            fail_on_unknown_critical: true,
        }
    }
}

/// Core Power House layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoreLayer {
    /// Primary Power House Archive artifact.
    pub pha: PhaArtifact,
    /// Proof descriptors.
    pub proofs: Vec<CoreProofDescriptor>,
    /// Domain-separated digest of core fields.
    pub core_digest: String,
    /// Core verification policy.
    pub core_verification_policy: CoreVerificationPolicy,
}

/// Branch summary carried for human and cross-language inspection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryBranch {
    /// Branch identifier.
    pub branch_id: String,
    /// Branch label.
    pub label: String,
    /// Parent branch identifiers.
    pub parent_ids: Vec<String>,
    /// Bound artifact digest.
    pub artifact_digest: String,
    /// Replay state fingerprint after this branch.
    pub state_fingerprint: String,
    /// Operation label.
    pub operation: String,
}

/// Equivalence claim between two branches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquivalenceClaim {
    /// Left branch selector.
    pub left_branch: String,
    /// Right branch selector.
    pub right_branch: String,
    /// Equivalence result.
    pub result: String,
    /// Optional proof reference.
    pub proof_ref: Option<String>,
}

/// Rootprint lineage layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineageLayer {
    /// Embedded Rootprint graph.
    pub rootprint: Rootprint,
    /// Branch summaries.
    pub branches: Vec<MemoryBranch>,
    /// Explicit equivalence claims.
    pub equivalence: Vec<EquivalenceClaim>,
}

/// Expected replay state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayExpected {
    /// Core validity expectation.
    pub core_valid: bool,
    /// Rootprint validity expectation.
    pub rootprint_valid: bool,
    /// Expected replay fingerprint.
    pub replay_fingerprint: String,
    /// Sidecar validity expectation.
    pub sidecar_valid: Option<bool>,
}

/// Replay resource bounds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayResourceBounds {
    /// Maximum memory in MiB.
    pub max_memory_mb: u64,
    /// Maximum disk in MiB.
    pub max_disk_mb: u64,
    /// Reference wall-clock ceiling in seconds.
    pub max_wall_seconds_reference: u64,
}

/// Replay command plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayPlan {
    /// Replay engine.
    pub engine: String,
    /// Engine version.
    pub version: String,
    /// Offline replay commands.
    pub commands: Vec<String>,
    /// Expected replay state.
    pub expected: ReplayExpected,
    /// Resource bounds.
    pub resource_bounds: ReplayResourceBounds,
    /// Whether replay needs network access.
    pub network_required: bool,
}

/// Replay layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayLayer {
    /// Replay plan.
    pub replay: ReplayPlan,
}

/// A semantic packet bound to verified replay state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticPacketBinding {
    /// Packet schema.
    pub packet_schema: String,
    /// Packet identifier.
    pub packet_id: String,
    /// Transport digest of the packet JSON with its digest field blanked.
    pub packet_digest: String,
    /// Bound Rootprint branch.
    pub bound_branch_id: String,
    /// Bound replay fingerprint.
    pub bound_replay_fingerprint: String,
    /// Semantic role.
    pub role: String,
    /// Opaque semantic packet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packet: Option<Value>,
}

/// Semantic verification policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticPolicy {
    /// Whether semantic changes affect core identity.
    pub semantic_changes_affect_core: bool,
    /// Whether natural language text is non-authoritative.
    pub llm_text_is_non_authoritative: bool,
    /// Require packet digests.
    pub require_packet_digest: bool,
    /// Require branch bindings.
    pub require_branch_binding: bool,
}

impl Default for SemanticPolicy {
    fn default() -> Self {
        Self {
            semantic_changes_affect_core: false,
            llm_text_is_non_authoritative: true,
            require_packet_digest: true,
            require_branch_binding: true,
        }
    }
}

/// Non-core semantic layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticLayer {
    /// Sidecar schema.
    pub sidecar_schema: String,
    /// Optional embedded sidecar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sidecar: Option<ObservatorySidecar>,
    /// Bound semantic packets.
    pub packets: Vec<SemanticPacketBinding>,
    /// Semantic policy.
    pub semantic_policy: SemanticPolicy,
}

/// Witness receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WitnessReceipt {
    /// Witness identifier.
    pub witness_id: String,
    /// Witness kind.
    pub kind: String,
    /// Witness public key or reference.
    pub public_key: String,
    /// Observed capsule digest.
    pub observed_capsule_digest: String,
    /// Observed core digest.
    pub observed_core_digest: String,
    /// Observed replay fingerprint.
    pub observed_replay_fingerprint: String,
    /// Observation timestamp in Unix milliseconds.
    pub timestamp_unix_ms: u64,
    /// Witness signature or signature reference.
    pub signature: String,
}

/// Reproduction receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproductionReceipt {
    /// Receipt identifier.
    pub receipt_id: String,
    /// Tool name.
    pub tool: String,
    /// Tool version.
    pub version: String,
    /// Report digest.
    pub report_digest: String,
}

/// Portable proof-memory object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryCapsule {
    /// Header layer.
    pub header: CapsuleHeader,
    /// Core layer.
    pub core: CoreLayer,
    /// Lineage layer.
    pub lineage: LineageLayer,
    /// Replay layer.
    pub replay: ReplayLayer,
    /// Optional semantic layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantics: Option<SemanticLayer>,
    /// Witness receipts.
    pub witnesses: Vec<WitnessReceipt>,
    /// Optional challenge suite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub challenge: Option<ChallengeSuite>,
    /// Reproduction receipts.
    pub receipts: Vec<ReproductionReceipt>,
}

impl MemoryCapsule {
    /// Reads and strictly parses a Memory Capsule from bytes.
    pub fn from_slice(
        bytes: &[u8],
        policy: &MemoryVerificationPolicy,
    ) -> Result<Self, MemoryError> {
        if bytes.len() as u64 > policy.max_bytes {
            return Err(MemoryError::rejected(
                RejectionTrace::new(
                    "capsule",
                    "RESOURCE_LIMIT_EXCEEDED",
                    "capsule exceeds configured byte limit",
                )
                .at("/"),
            ));
        }
        let text = std::str::from_utf8(bytes)
            .map_err(|error| MemoryError::Canonical(format!("capsule is not UTF-8: {error}")))?;
        let value = parse_strict_value(text)?;
        serde_json::from_value(value).map_err(MemoryError::Json)
    }

    /// Reads a Memory Capsule from a path.
    pub fn from_path(
        path: impl AsRef<Path>,
        policy: &MemoryVerificationPolicy,
    ) -> Result<Self, MemoryError> {
        let bytes = fs::read(path)?;
        Self::from_slice(&bytes, policy)
    }

    /// Writes canonical compact JSON to a path.
    pub fn write_canonical(&self, path: impl AsRef<Path>) -> Result<(), MemoryError> {
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }
        let mut bytes = canonical_bytes(self)?;
        bytes.push(b'\n');
        fs::write(path, bytes)?;
        Ok(())
    }

    /// Calculates the capsule digest with `header.capsule_digest` excluded.
    pub fn calculate_capsule_digest(&self) -> Result<String, MemoryError> {
        let mut projection = self.clone();
        projection.header.capsule_digest = None;
        digest_json(CAPSULE_DOMAIN, &projection)
    }

    /// Calculates the core digest.
    pub fn calculate_core_digest(&self) -> Result<String, MemoryError> {
        calculate_core_digest(&self.core)
    }

    /// Verifies the capsule and returns a structured report.
    pub fn verify(
        &self,
        policy: MemoryVerificationPolicy,
    ) -> Result<MemoryVerificationReport, MemoryError> {
        let started = Instant::now();
        let mut rootprint_valid = false;
        let mut sidecar_valid = None;
        let mut semantic_valid = None;

        if self.header.schema != MEMORY_CAPSULE_SCHEMA_V1 {
            return Err(rejected(
                "header",
                "UNSUPPORTED_SCHEMA",
                "unsupported Memory Capsule schema",
                "/header/schema",
                false,
                false,
            ));
        }
        if policy.fail_on_unknown_critical && !self.header.critical_extensions.is_empty() {
            return Err(rejected(
                "header",
                "UNKNOWN_CRITICAL_EXTENSION",
                "unknown critical extension",
                "/header/critical_extensions",
                false,
                false,
            ));
        }

        let capsule_digest = self.calculate_capsule_digest()?;
        if let Some(stored) = &self.header.capsule_digest {
            validate_sha256(stored)?;
            if stored != &capsule_digest {
                return Err(rejected_with_values(
                    "capsule",
                    "CAPSULE_DIGEST_MISMATCH",
                    "capsule digest mismatch",
                    "/header/capsule_digest",
                    &capsule_digest,
                    stored,
                    (false, false),
                ));
            }
        }

        self.core.pha.verify().map_err(|error| {
            MemoryError::rejected(
                RejectionTrace::new("core", "PHA_CORE_INVALID", error.to_string())
                    .at("/core/pha")
                    .boundary(false, false),
            )
        })?;
        let expected_core_digest = self.calculate_core_digest()?;
        validate_sha256(&self.core.core_digest)?;
        if expected_core_digest != self.core.core_digest {
            return Err(rejected_with_values(
                "core",
                "CORE_DIGEST_MISMATCH",
                "core digest mismatch",
                "/core/core_digest",
                &expected_core_digest,
                &self.core.core_digest,
                (false, false),
            ));
        }
        for (index, proof) in self.core.proofs.iter().enumerate() {
            validate_sha256(&proof.digest).map_err(|_| {
                MemoryError::rejected(
                    RejectionTrace::new("core", "INVALID_PROOF_DIGEST", "proof digest malformed")
                        .at(format!("/core/proofs/{index}/digest"))
                        .boundary(false, false),
                )
            })?;
        }
        let core_valid = true;

        if policy.require_rootprint {
            self.lineage.rootprint.verify().map_err(|error| {
                MemoryError::rejected(
                    RejectionTrace::new("rootprint", "ROOTPRINT_INVALID", error.to_string())
                        .at("/lineage/rootprint")
                        .boundary(core_valid, false),
                )
            })?;
            if !self
                .lineage
                .rootprint
                .branches
                .values()
                .any(|branch| branch.artifact.phx_fingerprint == self.core.pha.phx_fingerprint)
            {
                return Err(rejected(
                    "rootprint",
                    "CORE_ARTIFACT_NOT_IN_ROOTPRINT",
                    "core PHA fingerprint is not present in Rootprint lineage",
                    "/lineage/rootprint/branches",
                    core_valid,
                    false,
                ));
            }
            rootprint_valid = true;
        }

        let replay_state = self.lineage.rootprint.replay().map_err(|error| {
            MemoryError::rejected(
                RejectionTrace::new("replay", "REPLAY_FAILED", error.to_string())
                    .at("/lineage/rootprint")
                    .boundary(core_valid, rootprint_valid),
            )
        })?;
        if policy.require_replay
            && replay_state.state_fingerprint != self.replay.replay.expected.replay_fingerprint
        {
            return Err(rejected_with_values(
                "replay",
                "REPLAY_FINGERPRINT_MISMATCH",
                "replay fingerprint mismatch",
                "/replay/replay/expected/replay_fingerprint",
                &replay_state.state_fingerprint,
                &self.replay.replay.expected.replay_fingerprint,
                (core_valid, rootprint_valid),
            ));
        }
        let replay_valid =
            replay_state.state_fingerprint == self.replay.replay.expected.replay_fingerprint;

        if let Some(semantics) = &self.semantics {
            if let Some(sidecar) = &semantics.sidecar {
                sidecar.verify(&self.lineage.rootprint).map_err(|error| {
                    MemoryError::rejected(
                        RejectionTrace::new("sidecar", "SIDECAR_INVALID", error.to_string())
                            .at("/semantics/sidecar")
                            .boundary(core_valid, rootprint_valid),
                    )
                })?;
                sidecar_valid = Some(true);
            } else if policy.require_sidecar_if_present {
                return Err(rejected(
                    "sidecar",
                    "SIDECAR_REQUIRED",
                    "semantic layer present without required sidecar",
                    "/semantics/sidecar",
                    core_valid,
                    rootprint_valid,
                ));
            }
            verify_semantics(semantics, self, &replay_state.state_fingerprint)?;
            semantic_valid = Some(true);
        }

        let witness_validity =
            verify_witnesses(self, &capsule_digest, &replay_state.state_fingerprint)?;
        Ok(MemoryVerificationReport {
            capsule_digest,
            core_valid,
            rootprint_valid,
            replay_valid,
            sidecar_valid,
            semantic_valid,
            witness_validity,
            rejection_trace: None,
            soundness_report: Some(SoundnessReport::default()),
            timings: VerificationTimings {
                total_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
            },
        })
    }

    /// Replays the capsule Rootprint state.
    pub fn replay(&self) -> Result<MemoryReplayReport, MemoryError> {
        let state = self
            .lineage
            .rootprint
            .replay()
            .map_err(MemoryError::Rootprint)?;
        let capsule_digest = self.calculate_capsule_digest()?;
        Ok(MemoryReplayReport {
            capsule_digest,
            replay_valid: state.state_fingerprint == self.replay.replay.expected.replay_fingerprint,
            replay_fingerprint: state.state_fingerprint,
            branch_count: state.branches.len(),
            power_house_version: env!("CARGO_PKG_VERSION").to_string(),
            network_required: self.replay.replay.network_required,
        })
    }

    /// Runs all challenge vectors against mutated copies of this capsule.
    pub fn challenge_all(
        &self,
        policy: MemoryVerificationPolicy,
    ) -> Result<MemoryChallengeReport, MemoryError> {
        let Some(suite) = &self.challenge else {
            return Ok(MemoryChallengeReport {
                total: 0,
                expected_rejections: 0,
                mismatches: 0,
                results: Vec::new(),
            });
        };
        let mut results = Vec::new();
        for vector in &suite.mutations {
            let result = run_challenge_vector(self, vector, policy.clone())?;
            results.push(result);
        }
        let expected_rejections = results.iter().filter(|result| result.passed).count();
        let mismatches = results.len().saturating_sub(expected_rejections);
        Ok(MemoryChallengeReport {
            total: results.len(),
            expected_rejections,
            mismatches,
            results,
        })
    }
}

/// Fluent builder for Memory Capsules.
#[derive(Debug, Clone)]
pub struct MemoryCapsuleBuilder {
    capsule_id: String,
    created_at_unix_ms: u64,
    producer: ProducerInfo,
    pha: Option<PhaArtifact>,
    rootprint: Option<Rootprint>,
    sidecar: Option<ObservatorySidecar>,
    semantic_packets: Vec<SemanticPacketBinding>,
    challenge: Option<ChallengeSuite>,
}

impl MemoryCapsuleBuilder {
    /// Creates a builder.
    pub fn new(capsule_id: impl Into<String>) -> Self {
        let mut capsule_id = capsule_id.into();
        if !capsule_id.starts_with("phm_") {
            capsule_id = format!("phm_{capsule_id}");
        }
        Self {
            capsule_id,
            created_at_unix_ms: 0,
            producer: ProducerInfo {
                name: "mfenx".to_string(),
                tool: "julian".to_string(),
                power_house_version: env!("CARGO_PKG_VERSION").to_string(),
                slbit_version: None,
                rustc: None,
                platform: Some(format!(
                    "{}-{}",
                    std::env::consts::OS,
                    std::env::consts::ARCH
                )),
            },
            pha: None,
            rootprint: None,
            sidecar: None,
            semantic_packets: Vec::new(),
            challenge: None,
        }
    }

    /// Sets producer metadata.
    pub fn producer(mut self, name: impl Into<String>, version: impl Into<String>) -> Self {
        self.producer.name = name.into();
        self.producer.power_house_version = version.into();
        self
    }

    /// Sets the slbit version declaration.
    pub fn slbit_version(mut self, version: impl Into<String>) -> Self {
        self.producer.slbit_version = Some(version.into());
        self
    }

    /// Sets the metadata timestamp.
    pub fn created_at_unix_ms(mut self, timestamp: u64) -> Self {
        self.created_at_unix_ms = timestamp;
        self
    }

    /// Adds the core `.pha` artifact.
    pub fn with_pha(mut self, artifact: PhaArtifact) -> Self {
        self.pha = Some(artifact);
        self
    }

    /// Adds the Rootprint graph.
    pub fn with_rootprint(mut self, graph: Rootprint) -> Self {
        self.rootprint = Some(graph);
        self
    }

    /// Marks replay as required.
    pub fn with_replay_required(self) -> Self {
        self
    }

    /// Adds an Observatory sidecar.
    pub fn with_sidecar(mut self, sidecar: ObservatorySidecar) -> Self {
        self.sidecar = Some(sidecar);
        self
    }

    /// Adds an opaque semantic packet bound to a branch and replay fingerprint.
    pub fn with_semantic_packet(
        mut self,
        packet_schema: impl Into<String>,
        packet_id: impl Into<String>,
        bound_branch_id: impl Into<String>,
        bound_replay_fingerprint: impl Into<String>,
        role: impl Into<String>,
        packet: Value,
    ) -> Result<Self, MemoryError> {
        let packet_digest = semantic_packet_digest(&packet)?;
        self.semantic_packets.push(SemanticPacketBinding {
            packet_schema: packet_schema.into(),
            packet_id: packet_id.into(),
            packet_digest,
            bound_branch_id: bound_branch_id.into(),
            bound_replay_fingerprint: bound_replay_fingerprint.into(),
            role: role.into(),
            packet: Some(packet),
        });
        Ok(self)
    }

    /// Adds a challenge suite.
    pub fn with_challenge_suite(mut self, suite: ChallengeSuite) -> Self {
        self.challenge = Some(suite);
        self
    }

    /// Builds, digests, and verifies the capsule.
    pub fn build(self) -> Result<MemoryCapsule, MemoryError> {
        let pha = self.pha.ok_or_else(|| {
            MemoryError::Core("Memory Capsule requires a .pha artifact".to_string())
        })?;
        let rootprint = self.rootprint.ok_or_else(|| {
            MemoryError::Core("Memory Capsule requires Rootprint lineage".to_string())
        })?;
        pha.verify()
            .map_err(|error| MemoryError::Core(error.to_string()))?;
        rootprint.verify().map_err(MemoryError::Rootprint)?;
        let replay_state = rootprint.replay().map_err(MemoryError::Rootprint)?;
        let mut core = CoreLayer {
            pha,
            proofs: Vec::new(),
            core_digest: String::new(),
            core_verification_policy: CoreVerificationPolicy::default(),
        };
        core.core_digest = calculate_core_digest(&core)?;
        let branches = rootprint
            .branches
            .values()
            .map(|branch| MemoryBranch {
                branch_id: branch.id.clone(),
                label: branch.label.clone(),
                parent_ids: branch.parents.clone(),
                artifact_digest: branch.artifact.phx_fingerprint.clone(),
                state_fingerprint: replay_state.state_fingerprint.clone(),
                operation: if branch.parents.is_empty() {
                    "create".to_string()
                } else if branch.parents.len() == 1 {
                    "fork".to_string()
                } else {
                    "merge".to_string()
                },
            })
            .collect();
        let semantics = if self.sidecar.is_some() || !self.semantic_packets.is_empty() {
            Some(SemanticLayer {
                sidecar_schema: "power-house/observatory-sidecar/v1".to_string(),
                sidecar: self.sidecar,
                packets: self.semantic_packets,
                semantic_policy: SemanticPolicy::default(),
            })
        } else {
            None
        };
        let mut capsule = MemoryCapsule {
            header: CapsuleHeader {
                schema: MEMORY_CAPSULE_SCHEMA_V1.to_string(),
                capsule_id: self.capsule_id,
                capsule_digest: None,
                created_at_unix_ms: self.created_at_unix_ms,
                producer: self.producer,
                critical_extensions: Vec::new(),
                noncritical_extensions: Vec::new(),
            },
            core,
            lineage: LineageLayer {
                rootprint,
                branches,
                equivalence: Vec::new(),
            },
            replay: ReplayLayer {
                replay: ReplayPlan {
                    engine: "power_house".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    commands: vec![
                        "julian memory verify capsule.phm".to_string(),
                        "julian memory replay capsule.phm".to_string(),
                        "julian memory challenge capsule.phm --all".to_string(),
                    ],
                    expected: ReplayExpected {
                        core_valid: true,
                        rootprint_valid: true,
                        replay_fingerprint: replay_state.state_fingerprint,
                        sidecar_valid: semantics.as_ref().map(|_| true),
                    },
                    resource_bounds: ReplayResourceBounds {
                        max_memory_mb: 512,
                        max_disk_mb: 1024,
                        max_wall_seconds_reference: 600,
                    },
                    network_required: false,
                },
            },
            semantics,
            witnesses: Vec::new(),
            challenge: self.challenge,
            receipts: Vec::new(),
        };
        capsule.header.capsule_digest = Some(capsule.calculate_capsule_digest()?);
        capsule.verify(MemoryVerificationPolicy::strict())?;
        Ok(capsule)
    }
}

/// Calculates the digest of a core layer.
pub fn calculate_core_digest(core: &CoreLayer) -> Result<String, MemoryError> {
    let projection = serde_json::json!({
        "core_verification_policy": &core.core_verification_policy,
        "pha": &core.pha,
        "proofs": &core.proofs,
    });
    digest_json(CORE_DOMAIN, &projection)
}

/// Calculates a transport digest for opaque semantic packet JSON.
pub fn semantic_packet_digest(packet: &Value) -> Result<String, MemoryError> {
    let mut projection = packet.clone();
    if let Some(object) = projection.as_object_mut() {
        if object.contains_key("packet_digest") {
            object.insert("packet_digest".to_string(), Value::String(String::new()));
        }
        if let Some(digests) = object.get_mut("digests").and_then(Value::as_object_mut) {
            if digests.contains_key("packet") {
                digests.insert("packet".to_string(), Value::String(String::new()));
            }
            if digests.contains_key("packet_digest") {
                digests.insert("packet_digest".to_string(), Value::String(String::new()));
            }
        }
    }
    digest_json(SEMANTIC_PACKET_DOMAIN, &projection)
}

fn verify_semantics(
    semantics: &SemanticLayer,
    capsule: &MemoryCapsule,
    replay_fingerprint: &str,
) -> Result<(), MemoryError> {
    if semantics.semantic_policy.semantic_changes_affect_core {
        return Err(rejected(
            "semantic",
            "SEMANTIC_CORE_ESCALATION_FORBIDDEN",
            "semantic policy attempted to affect core identity",
            "/semantics/semantic_policy/semantic_changes_affect_core",
            true,
            true,
        ));
    }
    for (index, packet) in semantics.packets.iter().enumerate() {
        validate_sha256(&packet.packet_digest)?;
        validate_sha256(&packet.bound_replay_fingerprint)?;
        if !capsule
            .lineage
            .rootprint
            .branches
            .contains_key(&packet.bound_branch_id)
        {
            return Err(rejected(
                "semantic",
                "SEMANTIC_BRANCH_BINDING_MISMATCH",
                "semantic packet is bound to an unknown branch",
                format!("/semantics/packets/{index}/bound_branch_id"),
                true,
                true,
            ));
        }
        if packet.bound_replay_fingerprint != replay_fingerprint {
            return Err(rejected_with_values(
                "semantic",
                "SEMANTIC_REPLAY_BINDING_MISMATCH",
                "semantic packet replay binding does not match Rootprint replay",
                format!("/semantics/packets/{index}/bound_replay_fingerprint"),
                replay_fingerprint,
                &packet.bound_replay_fingerprint,
                (true, true),
            ));
        }
        if let Some(value) = &packet.packet {
            let expected = semantic_packet_digest(value)?;
            if expected != packet.packet_digest {
                return Err(rejected_with_values(
                    "semantic",
                    "PACKET_DIGEST_MISMATCH",
                    "semantic packet digest mismatch",
                    format!("/semantics/packets/{index}/packet_digest"),
                    &expected,
                    &packet.packet_digest,
                    (true, true),
                ));
            }
        }
    }
    Ok(())
}

fn verify_witnesses(
    capsule: &MemoryCapsule,
    capsule_digest: &str,
    replay_fingerprint: &str,
) -> Result<Vec<WitnessValidity>, MemoryError> {
    let mut results = Vec::new();
    for witness in &capsule.witnesses {
        validate_sha256(&witness.observed_capsule_digest)?;
        validate_sha256(&witness.observed_core_digest)?;
        validate_sha256(&witness.observed_replay_fingerprint)?;
        let valid = witness.observed_capsule_digest == capsule_digest
            && witness.observed_core_digest == capsule.core.core_digest
            && witness.observed_replay_fingerprint == replay_fingerprint
            && !witness.public_key.trim().is_empty()
            && !witness.signature.trim().is_empty();
        results.push(WitnessValidity {
            witness_id: witness.witness_id.clone(),
            valid,
            detail: if valid {
                "receipt matches capsule state".to_string()
            } else {
                "receipt does not match capsule state".to_string()
            },
        });
    }
    Ok(results)
}

fn run_challenge_vector(
    capsule: &MemoryCapsule,
    vector: &super::challenge::ChallengeVector,
    policy: MemoryVerificationPolicy,
) -> Result<ChallengeResult, MemoryError> {
    let mut value = serde_json::to_value(capsule)?;
    match vector.mutation.as_str() {
        "replace" => {
            let path = vector
                .path
                .as_ref()
                .ok_or_else(|| MemoryError::UnsupportedMutation(vector.id.clone()))?;
            let replacement = vector
                .value
                .clone()
                .ok_or_else(|| MemoryError::UnsupportedMutation(vector.id.clone()))?;
            let slot = value
                .pointer_mut(path)
                .ok_or_else(|| MemoryError::UnsupportedMutation(path.clone()))?;
            *slot = replacement;
        }
        other => return Err(MemoryError::UnsupportedMutation(other.to_string())),
    }
    let mut mutated: MemoryCapsule = serde_json::from_value(value)?;
    if vector.path.as_deref() != Some("/header/capsule_digest") {
        mutated.header.capsule_digest = Some(mutated.calculate_capsule_digest()?);
    }
    let verification = mutated.verify(policy);
    let (passed, actual_layer, actual_code, core_valid_before_failure, detail) = match verification
    {
        Ok(_) => (
            false,
            None,
            None,
            true,
            "mutation unexpectedly verified".to_string(),
        ),
        Err(MemoryError::Rejected(trace)) => {
            let layer_ok = trace.layer == vector.expected.rejection_layer;
            let code_ok = trace.code == vector.expected.rejection_code;
            let reason_ok = trace.message.contains(&vector.expected.reason_contains)
                || trace.code.contains(&vector.expected.reason_contains);
            (
                layer_ok && code_ok && reason_ok,
                Some(trace.layer.clone()),
                Some(trace.code.clone()),
                trace.core_valid_before_failure,
                trace.message,
            )
        }
        Err(error) => (
            false,
            Some("internal".to_string()),
            Some("INTERNAL_ERROR".to_string()),
            false,
            error.to_string(),
        ),
    };
    Ok(ChallengeResult {
        id: vector.id.clone(),
        passed,
        expected_layer: vector.expected.rejection_layer.clone(),
        actual_layer,
        expected_code: vector.expected.rejection_code.clone(),
        actual_code,
        core_valid_before_failure,
        detail,
    })
}

fn rejected(
    layer: impl Into<String>,
    code: impl Into<String>,
    message: impl Into<String>,
    pointer: impl Into<String>,
    core_valid: bool,
    rootprint_valid: bool,
) -> MemoryError {
    MemoryError::rejected(
        RejectionTrace::new(layer, code, message)
            .at(pointer)
            .boundary(core_valid, rootprint_valid),
    )
}

fn rejected_with_values(
    layer: impl Into<String>,
    code: impl Into<String>,
    message: impl Into<String>,
    pointer: impl Into<String>,
    expected: impl Into<String>,
    actual: impl Into<String>,
    boundary: (bool, bool),
) -> MemoryError {
    MemoryError::rejected(
        RejectionTrace::new(layer, code, message)
            .at(pointer)
            .values(expected, actual)
            .boundary(boundary.0, boundary.1),
    )
}

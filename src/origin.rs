//! Verified creation and deterministic creative-capacity issuance.
//!
//! [`Origin::manifest`] collapses source lowering, execution, synthesis,
//! `.pha` construction, replay verification, Rootprint anchoring, and identity
//! verification into one all-or-nothing transition. The returned [`Origin`]
//! is therefore usable only after every layer has accepted the same state.
//!
//! [`Origin::derive`] turns that verified state into a live source of further
//! identity-bound creations. Each successful derivation consumes deterministic
//! [`CreativeCapacity`] units; failed derivations leave the origin unchanged.
//! Capacity units are an explicit software resource budget, not physical
//! energy, currency, or a claim of supernatural behavior.

use crate::identity::{Identity, IdentityError};
use crate::provenance::{PhaArtifact, Rootprint, RootprintId};
use crate::sfcs::{verify_execution_embedding, SfcsError, SfcsExecutionEmbeddingReport, SfcsGraph};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

/// Schema identifier for verified Origin receipts.
pub const ORIGIN_RECEIPT_SCHEMA_V1_DRAFT: &str = "power-house/origin-receipt/v1-draft";

const CAPACITY_DOMAIN: &[u8] = b"power-house:origin:v1-draft:creative-capacity\0";
const RECEIPT_DOMAIN: &[u8] = b"power-house:origin:v1-draft:creation-receipt\0";
const SHA256_PREFIX: &str = "sha256:";
const DENSE_BOUNDARY_MULTIPLIER: u64 = 4;

/// A declarative request for one verified SFCS creation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OriginSpec {
    label: String,
    source: String,
    inputs: BTreeMap<String, i64>,
}

impl OriginSpec {
    /// Creates a specification with no inputs.
    pub fn new(label: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            source: source.into(),
            inputs: BTreeMap::new(),
        }
    }

    /// Adds or replaces one deterministic public input.
    pub fn with_input(mut self, name: impl Into<String>, value: i64) -> Self {
        self.inputs.insert(name.into(), value);
        self
    }

    /// Replaces all deterministic public inputs.
    pub fn with_inputs(mut self, inputs: BTreeMap<String, i64>) -> Self {
        self.inputs = inputs;
        self
    }

    /// Returns the Rootprint and provenance label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the native SFCS source text.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Returns the deterministic public inputs.
    pub fn inputs(&self) -> &BTreeMap<String, i64> {
        &self.inputs
    }

    fn validate(&self) -> Result<(), OriginError> {
        if self.label.trim().is_empty() {
            return Err(OriginError::InvalidSpec(
                "origin label must not be empty".to_string(),
            ));
        }
        if self.source.trim().is_empty() {
            return Err(OriginError::InvalidSpec(
                "origin source must not be empty".to_string(),
            ));
        }
        Ok(())
    }
}

/// Resource limits applied to every creation in one [`Origin`] lineage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OriginPolicy {
    max_nodes: usize,
    max_trace_steps: usize,
    max_synthesis_operations: usize,
    creative_units: u64,
}

impl OriginPolicy {
    /// Creates a deterministic resource policy.
    pub fn new(
        max_nodes: usize,
        max_trace_steps: usize,
        max_synthesis_operations: usize,
        creative_units: u64,
    ) -> Self {
        Self {
            max_nodes,
            max_trace_steps,
            max_synthesis_operations,
            creative_units,
        }
    }

    /// Returns the maximum admitted SFCS node count per creation.
    pub fn max_nodes(&self) -> usize {
        self.max_nodes
    }

    /// Returns the maximum admitted execution trace length per creation.
    pub fn max_trace_steps(&self) -> usize {
        self.max_trace_steps
    }

    /// Returns the maximum admitted synthesis operation count per creation.
    pub fn max_synthesis_operations(&self) -> usize {
        self.max_synthesis_operations
    }

    /// Returns the total creative-capacity units issued to the lineage.
    pub fn creative_units(&self) -> u64 {
        self.creative_units
    }

    fn authorize(
        &self,
        report: &SfcsExecutionEmbeddingReport,
        cost: &CreationCost,
    ) -> Result<(), OriginError> {
        for (resource, actual, limit) in [
            ("nodes", report.node_count, self.max_nodes),
            ("trace_steps", report.trace_steps, self.max_trace_steps),
            (
                "synthesis_operations",
                report.synthesis_operations,
                self.max_synthesis_operations,
            ),
        ] {
            if actual > limit {
                return Err(OriginError::PolicyLimitExceeded {
                    resource,
                    actual: actual as u64,
                    limit: limit as u64,
                });
            }
        }
        if cost.total_units > self.creative_units {
            return Err(OriginError::CapacityExhausted {
                required: cost.total_units,
                remaining: self.creative_units,
            });
        }
        Ok(())
    }
}

impl Default for OriginPolicy {
    fn default() -> Self {
        Self::new(4_096, 4_096, 4_096, 1_000_000)
    }
}

/// Deterministic software-resource cost for one verified creation.
///
/// One unit is charged for each graph node, trace step, and synthesis
/// operation. Each dense-boundary node adds four units. These accounting units
/// are intentionally simple, reproducible, and independent of wall-clock time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreationCost {
    /// Units charged for graph nodes.
    pub node_units: u64,
    /// Units charged for deterministic trace steps.
    pub trace_units: u64,
    /// Units charged for synthesis operations.
    pub synthesis_units: u64,
    /// Additional units charged for dense-boundary nodes.
    pub dense_boundary_units: u64,
    /// Total units consumed by the creation.
    pub total_units: u64,
}

impl CreationCost {
    /// Calculates deterministic cost from a verified SFCS execution report.
    pub fn from_report(report: &SfcsExecutionEmbeddingReport) -> Result<Self, OriginError> {
        let node_units = report.node_count as u64;
        let trace_units = report.trace_steps as u64;
        let synthesis_units = report.synthesis_operations as u64;
        let dense_boundary_units = (report.dense_nodes as u64)
            .checked_mul(DENSE_BOUNDARY_MULTIPLIER)
            .ok_or(OriginError::ArithmeticOverflow)?;
        let total_units = node_units
            .checked_add(trace_units)
            .and_then(|total| total.checked_add(synthesis_units))
            .and_then(|total| total.checked_add(dense_boundary_units))
            .ok_or(OriginError::ArithmeticOverflow)?;
        Ok(Self {
            node_units,
            trace_units,
            synthesis_units,
            dense_boundary_units,
            total_units,
        })
    }
}

/// A fully evaluated and verified creation estimate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreationEstimate {
    /// The same verifier report that a successful creation will bind.
    pub report: SfcsExecutionEmbeddingReport,
    /// Public outputs produced by deterministic replay.
    pub outputs: BTreeMap<String, i64>,
    /// Creative-capacity units required by the creation.
    pub cost: CreationCost,
}

/// A linear, identity-bound software capability budget.
///
/// The type is intentionally not `Clone` or `Deserialize`. Safe Rust callers
/// cannot duplicate or restore it from an untrusted record. It is an in-process
/// capability and does not replace signatures or authorization across a trust
/// boundary.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct CreativeCapacity {
    issued_units: u64,
    spent_units: u64,
    remaining_units: u64,
    bound_artifact_phx_fingerprint: String,
    bound_rootprint_id: RootprintId,
    grant_digest: String,
}

impl CreativeCapacity {
    fn issue(
        identity: &Identity,
        issued_units: u64,
        initial_cost: u64,
    ) -> Result<Self, OriginError> {
        if initial_cost > issued_units {
            return Err(OriginError::CapacityExhausted {
                required: initial_cost,
                remaining: issued_units,
            });
        }
        let mut capacity = Self {
            issued_units,
            spent_units: initial_cost,
            remaining_units: issued_units - initial_cost,
            bound_artifact_phx_fingerprint: identity.pha().phx_fingerprint.clone(),
            bound_rootprint_id: identity.rootprint_id().clone(),
            grant_digest: String::new(),
        };
        capacity.grant_digest = capacity.calculate_digest()?;
        Ok(capacity)
    }

    fn next_bound(&self, identity: &Identity, debit: u64) -> Result<Self, OriginError> {
        if debit > self.remaining_units {
            return Err(OriginError::CapacityExhausted {
                required: debit,
                remaining: self.remaining_units,
            });
        }
        let spent_units = self
            .spent_units
            .checked_add(debit)
            .ok_or(OriginError::ArithmeticOverflow)?;
        let mut capacity = Self {
            issued_units: self.issued_units,
            spent_units,
            remaining_units: self.remaining_units - debit,
            bound_artifact_phx_fingerprint: identity.pha().phx_fingerprint.clone(),
            bound_rootprint_id: identity.rootprint_id().clone(),
            grant_digest: String::new(),
        };
        capacity.grant_digest = capacity.calculate_digest()?;
        Ok(capacity)
    }

    /// Returns the units originally issued to this lineage.
    pub fn issued_units(&self) -> u64 {
        self.issued_units
    }

    /// Returns the units consumed by verified creations.
    pub fn spent_units(&self) -> u64 {
        self.spent_units
    }

    /// Returns the units still available for verified derivations.
    pub fn remaining_units(&self) -> u64 {
        self.remaining_units
    }

    /// Returns the current bound Power House core fingerprint.
    pub fn bound_artifact_phx_fingerprint(&self) -> &str {
        &self.bound_artifact_phx_fingerprint
    }

    /// Returns the current bound Rootprint identity.
    pub fn bound_rootprint_id(&self) -> &RootprintId {
        &self.bound_rootprint_id
    }

    /// Returns the deterministic grant digest.
    pub fn grant_digest(&self) -> &str {
        &self.grant_digest
    }

    fn verify_binding(&self, identity: &Identity) -> Result<(), OriginError> {
        if self.bound_artifact_phx_fingerprint != identity.pha().phx_fingerprint {
            return Err(OriginError::Invariant(
                "creative capacity is bound to a different artifact".to_string(),
            ));
        }
        if &self.bound_rootprint_id != identity.rootprint_id() {
            return Err(OriginError::Invariant(
                "creative capacity is bound to a different Rootprint identity".to_string(),
            ));
        }
        let recomposed = self
            .spent_units
            .checked_add(self.remaining_units)
            .ok_or(OriginError::ArithmeticOverflow)?;
        if recomposed != self.issued_units {
            return Err(OriginError::Invariant(
                "creative capacity accounting does not recompose".to_string(),
            ));
        }
        let expected = self.calculate_digest()?;
        if expected != self.grant_digest {
            return Err(OriginError::Invariant(
                "creative capacity digest does not match its binding".to_string(),
            ));
        }
        Ok(())
    }

    fn calculate_digest(&self) -> Result<String, OriginError> {
        digest_json(
            CAPACITY_DOMAIN,
            &serde_json::json!({
                "issued_units": self.issued_units,
                "spent_units": self.spent_units,
                "remaining_units": self.remaining_units,
                "bound_artifact_phx_fingerprint": self.bound_artifact_phx_fingerprint,
                "bound_rootprint_id": self.bound_rootprint_id,
            }),
        )
    }
}

/// Portable evidence emitted by one successful verified creation.
///
/// A receipt is evidence, not authority. [`Origin::verify`] replays the bound
/// artifact, identity, lineage, capacity, outputs, and receipt together.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreationReceipt {
    /// Receipt schema identifier.
    pub schema: String,
    /// Producer-declared creation label.
    pub label: String,
    /// Parent identity for a derivation, or `None` for the initial origin.
    pub parent_rootprint_id: Option<RootprintId>,
    /// New identity created by this transition.
    pub rootprint_id: RootprintId,
    /// Sequence number of the new Rootprint branch.
    pub generation: u64,
    /// Power House core fingerprint of the verified execution artifact.
    pub artifact_phx_fingerprint: String,
    /// Digest of the native SFCS graph.
    pub graph_digest: String,
    /// Digest of deterministic execution replay.
    pub trace_digest: String,
    /// Digest of deterministic synthesis replay.
    pub synthesis_digest: String,
    /// Digest binding graph identity to synthesis identity.
    pub embedding_invariant_digest: String,
    /// Digest of the public outputs.
    pub output_digest: String,
    /// Rootprint replay-state fingerprint after the transition.
    pub lineage_fingerprint: String,
    /// Public outputs produced by the creation.
    pub outputs: BTreeMap<String, i64>,
    /// Deterministic creative-capacity cost.
    pub cost: CreationCost,
    /// Creative-capacity units remaining after the transition.
    pub remaining_creative_units: u64,
    /// Domain-separated digest of every preceding receipt field.
    pub receipt_digest: String,
}

impl CreationReceipt {
    fn new(
        label: String,
        parent_rootprint_id: Option<RootprintId>,
        identity: &Identity,
        rootprint: &Rootprint,
        report: &SfcsExecutionEmbeddingReport,
        outputs: BTreeMap<String, i64>,
        cost: CreationCost,
        remaining_creative_units: u64,
    ) -> Result<Self, OriginError> {
        let state = identity.replay(rootprint).map_err(OriginError::Identity)?;
        let generation = state
            .graph
            .branches
            .iter()
            .find(|branch| branch.id == *identity.rootprint_id())
            .map(|branch| branch.sequence)
            .ok_or_else(|| {
                OriginError::Invariant(
                    "verified identity is absent from its replay state".to_string(),
                )
            })?;
        let mut receipt = Self {
            schema: ORIGIN_RECEIPT_SCHEMA_V1_DRAFT.to_string(),
            label,
            parent_rootprint_id,
            rootprint_id: identity.rootprint_id().clone(),
            generation,
            artifact_phx_fingerprint: identity.pha().phx_fingerprint.clone(),
            graph_digest: report.graph_digest.clone(),
            trace_digest: report.trace_digest.clone(),
            synthesis_digest: report.synthesis_digest.clone(),
            embedding_invariant_digest: report.embedding_invariant_digest.clone(),
            output_digest: report.output_digest.clone(),
            lineage_fingerprint: state.graph.state_fingerprint,
            outputs,
            cost,
            remaining_creative_units,
            receipt_digest: String::new(),
        };
        receipt.receipt_digest = receipt.calculate_digest()?;
        receipt.verify()?;
        Ok(receipt)
    }

    /// Verifies the receipt schema and deterministic digest.
    pub fn verify(&self) -> Result<(), OriginError> {
        if self.schema != ORIGIN_RECEIPT_SCHEMA_V1_DRAFT {
            return Err(OriginError::Invariant(format!(
                "unsupported Origin receipt schema {}",
                self.schema
            )));
        }
        validate_sha256(&self.artifact_phx_fingerprint)?;
        validate_sha256(&self.graph_digest)?;
        validate_sha256(&self.trace_digest)?;
        validate_sha256(&self.synthesis_digest)?;
        validate_sha256(&self.embedding_invariant_digest)?;
        validate_sha256(&self.output_digest)?;
        validate_sha256(&self.lineage_fingerprint)?;
        validate_sha256(&self.receipt_digest)?;
        let expected = self.calculate_digest()?;
        if expected != self.receipt_digest {
            return Err(OriginError::Invariant(
                "Origin receipt digest does not match its fields".to_string(),
            ));
        }
        Ok(())
    }

    fn calculate_digest(&self) -> Result<String, OriginError> {
        digest_json(
            RECEIPT_DOMAIN,
            &serde_json::json!({
                "schema": self.schema,
                "label": self.label,
                "parent_rootprint_id": self.parent_rootprint_id,
                "rootprint_id": self.rootprint_id,
                "generation": self.generation,
                "artifact_phx_fingerprint": self.artifact_phx_fingerprint,
                "graph_digest": self.graph_digest,
                "trace_digest": self.trace_digest,
                "synthesis_digest": self.synthesis_digest,
                "embedding_invariant_digest": self.embedding_invariant_digest,
                "output_digest": self.output_digest,
                "lineage_fingerprint": self.lineage_fingerprint,
                "outputs": self.outputs,
                "cost": self.cost,
                "remaining_creative_units": self.remaining_creative_units,
            }),
        )
    }
}

/// A verified, identity-bound source of further deterministic creations.
///
/// All fields are private. Construction is possible only through
/// [`Origin::manifest`], and state evolution is possible only through
/// [`Origin::derive`]. Both methods prepare and verify a candidate completely
/// before committing it.
pub struct Origin {
    policy: OriginPolicy,
    identity: Identity,
    rootprint: Rootprint,
    graph: SfcsGraph,
    report: SfcsExecutionEmbeddingReport,
    outputs: BTreeMap<String, i64>,
    capacity: CreativeCapacity,
    receipt: CreationReceipt,
}

impl Origin {
    /// Fully evaluates and verifies a specification without issuing capacity.
    pub fn estimate(spec: &OriginSpec) -> Result<CreationEstimate, OriginError> {
        let prepared = PreparedCreation::prepare(spec)?;
        Ok(CreationEstimate {
            report: prepared.report,
            outputs: prepared.outputs,
            cost: prepared.cost,
        })
    }

    /// Materializes a specification as a verified Origin in one transition.
    ///
    /// No partially built graph, artifact, identity, or capability is returned
    /// on failure.
    pub fn manifest(spec: OriginSpec, policy: OriginPolicy) -> Result<Self, OriginError> {
        let prepared = PreparedCreation::prepare(&spec)?;
        policy.authorize(&prepared.report, &prepared.cost)?;
        let (identity, rootprint) = Identity::create(spec.label.clone(), prepared.artifact)
            .map_err(OriginError::Identity)?;
        let report = verify_execution_embedding(identity.pha()).map_err(OriginError::Sfcs)?;
        if report != prepared.report {
            return Err(OriginError::Invariant(
                "identity binding changed the verified SFCS report".to_string(),
            ));
        }
        let capacity =
            CreativeCapacity::issue(&identity, policy.creative_units, prepared.cost.total_units)?;
        let receipt = CreationReceipt::new(
            spec.label,
            None,
            &identity,
            &rootprint,
            &report,
            prepared.outputs.clone(),
            prepared.cost,
            capacity.remaining_units,
        )?;
        let origin = Self {
            policy,
            identity,
            rootprint,
            graph: prepared.graph,
            report,
            outputs: prepared.outputs,
            capacity,
            receipt,
        };
        origin.verify()?;
        Ok(origin)
    }

    /// Creates a verified child identity and atomically advances this Origin.
    ///
    /// The current state is unchanged if parsing, execution, synthesis,
    /// artifact verification, policy checks, capacity checks, Rootprint
    /// mutation, identity verification, or final invariant replay fails.
    pub fn derive(&mut self, spec: OriginSpec) -> Result<CreationReceipt, OriginError> {
        let prepared = PreparedCreation::prepare(&spec)?;
        self.policy.authorize(&prepared.report, &prepared.cost)?;
        if prepared.cost.total_units > self.capacity.remaining_units {
            return Err(OriginError::CapacityExhausted {
                required: prepared.cost.total_units,
                remaining: self.capacity.remaining_units,
            });
        }

        let parent_rootprint_id = self.identity.rootprint_id().clone();
        let mut rootprint = self.rootprint.clone();
        let identity = self
            .identity
            .fork(&mut rootprint, spec.label.clone(), prepared.artifact)
            .map_err(OriginError::Identity)?;
        let report = verify_execution_embedding(identity.pha()).map_err(OriginError::Sfcs)?;
        if report != prepared.report {
            return Err(OriginError::Invariant(
                "derived identity changed the verified SFCS report".to_string(),
            ));
        }
        let capacity = self
            .capacity
            .next_bound(&identity, prepared.cost.total_units)?;
        let receipt = CreationReceipt::new(
            spec.label,
            Some(parent_rootprint_id),
            &identity,
            &rootprint,
            &report,
            prepared.outputs.clone(),
            prepared.cost,
            capacity.remaining_units,
        )?;
        let candidate = Self {
            policy: self.policy.clone(),
            identity,
            rootprint,
            graph: prepared.graph,
            report,
            outputs: prepared.outputs,
            capacity,
            receipt: receipt.clone(),
        };
        candidate.verify()?;
        *self = candidate;
        Ok(receipt)
    }

    /// Replays every current Origin invariant.
    pub fn verify(&self) -> Result<(), OriginError> {
        self.identity
            .verify(&self.rootprint)
            .map_err(OriginError::Identity)?;
        let report = verify_execution_embedding(self.identity.pha()).map_err(OriginError::Sfcs)?;
        if report != self.report {
            return Err(OriginError::Invariant(
                "stored Origin report does not match artifact replay".to_string(),
            ));
        }
        self.graph.verify().map_err(OriginError::Sfcs)?;
        if self.graph.fractal_digest().map_err(OriginError::Sfcs)? != self.report.graph_digest {
            return Err(OriginError::Invariant(
                "stored Origin graph does not match artifact identity".to_string(),
            ));
        }
        let inputs = self
            .identity
            .pha()
            .embedded_proof
            .public_inputs
            .get("inputs")
            .ok_or_else(|| OriginError::Invariant("execution inputs are missing".to_string()))?;
        let inputs = serde_json::from_value::<BTreeMap<String, i64>>(inputs.clone())
            .map_err(OriginError::Serialization)?;
        let trace = self
            .graph
            .execution_trace(&inputs)
            .map_err(OriginError::Sfcs)?;
        if trace.outputs != self.outputs || trace.output_digest != self.report.output_digest {
            return Err(OriginError::Invariant(
                "stored Origin outputs do not match deterministic replay".to_string(),
            ));
        }
        let cost = CreationCost::from_report(&self.report)?;
        self.policy.authorize(&self.report, &cost)?;
        if cost != self.receipt.cost {
            return Err(OriginError::Invariant(
                "stored Origin cost does not match verified report".to_string(),
            ));
        }
        self.capacity.verify_binding(&self.identity)?;
        self.receipt.verify()?;
        let state = self
            .identity
            .replay(&self.rootprint)
            .map_err(OriginError::Identity)?;
        let expected_generation = state
            .graph
            .branches
            .iter()
            .find(|branch| branch.id == *self.identity.rootprint_id())
            .map(|branch| branch.sequence)
            .ok_or_else(|| {
                OriginError::Invariant(
                    "current identity is absent from Rootprint replay".to_string(),
                )
            })?;
        if self.receipt.rootprint_id != *self.identity.rootprint_id()
            || self.receipt.generation != expected_generation
            || self.receipt.artifact_phx_fingerprint != self.identity.pha().phx_fingerprint
            || self.receipt.graph_digest != self.report.graph_digest
            || self.receipt.trace_digest != self.report.trace_digest
            || self.receipt.synthesis_digest != self.report.synthesis_digest
            || self.receipt.embedding_invariant_digest != self.report.embedding_invariant_digest
            || self.receipt.output_digest != self.report.output_digest
            || self.receipt.lineage_fingerprint != state.graph.state_fingerprint
            || self.receipt.outputs != self.outputs
            || self.receipt.remaining_creative_units != self.capacity.remaining_units
        {
            return Err(OriginError::Invariant(
                "Origin receipt does not describe the current verified state".to_string(),
            ));
        }
        Ok(())
    }

    /// Returns the immutable current identity.
    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    /// Returns the complete current Rootprint lineage.
    pub fn rootprint(&self) -> &Rootprint {
        &self.rootprint
    }

    /// Returns the current native SFCS graph.
    pub fn graph(&self) -> &SfcsGraph {
        &self.graph
    }

    /// Returns the current verified SFCS execution report.
    pub fn report(&self) -> &SfcsExecutionEmbeddingReport {
        &self.report
    }

    /// Returns the current public outputs.
    pub fn outputs(&self) -> &BTreeMap<String, i64> {
        &self.outputs
    }

    /// Returns the linear creative-capacity capability.
    pub fn capacity(&self) -> &CreativeCapacity {
        &self.capacity
    }

    /// Returns the current portable creation receipt.
    pub fn receipt(&self) -> &CreationReceipt {
        &self.receipt
    }

    /// Returns the immutable lineage policy.
    pub fn policy(&self) -> &OriginPolicy {
        &self.policy
    }
}

struct PreparedCreation {
    graph: SfcsGraph,
    artifact: PhaArtifact,
    report: SfcsExecutionEmbeddingReport,
    outputs: BTreeMap<String, i64>,
    cost: CreationCost,
}

impl PreparedCreation {
    fn prepare(spec: &OriginSpec) -> Result<Self, OriginError> {
        spec.validate()?;
        let graph = SfcsGraph::from_source(&spec.source).map_err(OriginError::Sfcs)?;
        let trace = graph
            .execution_trace(&spec.inputs)
            .map_err(OriginError::Sfcs)?;
        let artifact = graph
            .to_execution_pha_artifact(spec.label.clone(), &spec.inputs)
            .map_err(OriginError::Sfcs)?;
        let report = verify_execution_embedding(&artifact).map_err(OriginError::Sfcs)?;
        if report.graph_digest != trace.graph_digest
            || report.trace_digest != trace.trace_digest
            || report.output_digest != trace.output_digest
        {
            return Err(OriginError::Invariant(
                "prepared execution does not match verifier replay".to_string(),
            ));
        }
        let cost = CreationCost::from_report(&report)?;
        Ok(Self {
            graph,
            artifact,
            report,
            outputs: trace.outputs,
            cost,
        })
    }
}

/// Errors returned by verified Origin operations.
#[derive(Debug)]
pub enum OriginError {
    /// The declarative specification is empty or malformed before SFCS parsing.
    InvalidSpec(String),
    /// SFCS construction, execution, synthesis, or embedding verification failed.
    Sfcs(SfcsError),
    /// Power House identity or Rootprint verification failed.
    Identity(IdentityError),
    /// A deterministic resource limit was exceeded.
    PolicyLimitExceeded {
        /// Resource name.
        resource: &'static str,
        /// Observed resource count.
        actual: u64,
        /// Admitted resource count.
        limit: u64,
    },
    /// The requested creation requires more capacity than remains.
    CapacityExhausted {
        /// Units required by the candidate.
        required: u64,
        /// Units currently available.
        remaining: u64,
    },
    /// Deterministic resource arithmetic overflowed.
    ArithmeticOverflow,
    /// JSON serialization or decoding failed.
    Serialization(serde_json::Error),
    /// A cross-layer Origin invariant was violated.
    Invariant(String),
}

impl fmt::Display for OriginError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSpec(message) => {
                write!(formatter, "invalid Origin specification: {message}")
            }
            Self::Sfcs(error) => write!(formatter, "Origin SFCS verification failed: {error}"),
            Self::Identity(error) => {
                write!(formatter, "Origin identity verification failed: {error}")
            }
            Self::PolicyLimitExceeded {
                resource,
                actual,
                limit,
            } => write!(
                formatter,
                "Origin policy limit exceeded for {resource}: {actual} > {limit}"
            ),
            Self::CapacityExhausted {
                required,
                remaining,
            } => write!(
                formatter,
                "creative capacity exhausted: required {required}, remaining {remaining}"
            ),
            Self::ArithmeticOverflow => {
                formatter.write_str("Origin resource arithmetic overflowed")
            }
            Self::Serialization(error) => {
                write!(formatter, "Origin serialization failed: {error}")
            }
            Self::Invariant(message) => write!(formatter, "Origin invariant failed: {message}"),
        }
    }
}

impl Error for OriginError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Sfcs(error) => Some(error),
            Self::Identity(error) => Some(error),
            Self::Serialization(error) => Some(error),
            _ => None,
        }
    }
}

fn digest_json<T: Serialize>(domain: &[u8], value: &T) -> Result<String, OriginError> {
    let encoded = serde_json::to_vec(value).map_err(OriginError::Serialization)?;
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(encoded);
    Ok(format!("{SHA256_PREFIX}{}", hex::encode(hasher.finalize())))
}

fn validate_sha256(value: &str) -> Result<(), OriginError> {
    let Some(hex_value) = value.strip_prefix(SHA256_PREFIX) else {
        return Err(OriginError::Invariant(format!(
            "digest is missing {SHA256_PREFIX} prefix"
        )));
    };
    if hex_value.len() != 64 || !hex_value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(OriginError::Invariant(
            "digest is not a 32-byte hexadecimal SHA-256 value".to_string(),
        ));
    }
    Ok(())
}

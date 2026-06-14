//! Deterministic Power House provenance branching.
//!
//! Rootprint is a directed acyclic graph of `.pha` artifacts. Branch identity,
//! navigation, forking, merging, equivalence, and graph verification use only
//! Power House core fingerprints. External proof attachments remain optional
//! transport data and never participate in these operations.

use super::{PhaArtifact, PhaError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::str::FromStr;

/// Schema identifier for Rootprint v1 documents.
pub const ROOTPRINT_SCHEMA_V1: &str = "power-house/rootprint/v1";

const BRANCH_ID_DOMAIN: &[u8] = b"power-house:rootprint:v1:branch-id\0";
const REPLAY_STATE_DOMAIN: &[u8] = b"power-house:rootprint:v1:replay-state\0";
const SHA256_PREFIX: &str = "sha256:";

/// A validated deterministic Rootprint branch identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RootprintId(String);

impl RootprintId {
    /// Parses and validates a Rootprint identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, RootprintError> {
        let value = value.into();
        validate_rootprint_id(&value)?;
        Ok(Self(value))
    }

    /// Returns the encoded identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RootprintId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for RootprintId {
    type Err = RootprintError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl AsRef<str> for RootprintId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// One deterministic branch reconstructed by Rootprint replay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootprintReplayBranch {
    /// Deterministic branch identifier.
    pub id: RootprintId,
    /// Human-readable branch label.
    pub label: String,
    /// Deterministic parent-before-child sequence.
    pub sequence: u64,
    /// Sorted parent identifiers.
    pub parents: Vec<RootprintId>,
    /// Power House core artifact fingerprint.
    pub artifact_phx_fingerprint: String,
}

/// Canonical logical state reconstructed from a valid Rootprint graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootprintState {
    /// Root branch identifier.
    pub root_branch: RootprintId,
    /// Branches in deterministic replay order.
    pub branches: Vec<RootprintReplayBranch>,
    /// Branches with no children, sorted by identifier.
    pub tips: Vec<RootprintId>,
    /// Domain-separated digest of the replayed logical state.
    pub state_fingerprint: String,
}

/// A branch in a Rootprint provenance graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RootprintBranch {
    /// Deterministic branch identifier.
    pub id: String,
    /// Human-readable branch label.
    pub label: String,
    /// Monotonic graph sequence used to prove parent-before-child ordering.
    pub sequence: u64,
    /// Zero parents for the root, one for forks, and two for merges.
    pub parents: Vec<String>,
    /// Power House artifact carried by this branch.
    pub artifact: PhaArtifact,
}

impl RootprintBranch {
    /// Calculates this branch's deterministic core identifier.
    pub fn calculate_id(&self) -> Result<String, RootprintError> {
        calculate_branch_id(&self.label, &self.parents, &self.artifact)
    }
}

/// A deterministic Power House provenance graph.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rootprint {
    /// Rootprint schema identifier.
    pub schema: String,
    /// Identifier of the graph root branch.
    pub root_branch: String,
    /// Branches keyed by deterministic identifier.
    pub branches: BTreeMap<String, RootprintBranch>,
}

impl Rootprint {
    /// Creates a Rootprint graph from one verified Power House artifact.
    pub fn new(label: impl Into<String>, artifact: PhaArtifact) -> Result<Self, RootprintError> {
        artifact.verify().map_err(RootprintError::Pha)?;
        let label = normalized_label(label.into())?;
        let id = calculate_branch_id(&label, &[], &artifact)?;
        let branch = RootprintBranch {
            id: id.clone(),
            label,
            sequence: 0,
            parents: Vec::new(),
            artifact,
        };
        let mut branches = BTreeMap::new();
        branches.insert(id.clone(), branch);
        Ok(Self {
            schema: ROOTPRINT_SCHEMA_V1.to_string(),
            root_branch: id,
            branches,
        })
    }

    /// Resolves an exact ID, unique ID prefix, or unique branch label.
    pub fn navigate(&self, selector: &str) -> Result<&RootprintBranch, RootprintError> {
        if let Some(branch) = self.branches.get(selector) {
            return Ok(branch);
        }
        let mut matches = self
            .branches
            .values()
            .filter(|branch| branch.id.starts_with(selector) || branch.label == selector);
        let Some(first) = matches.next() else {
            return Err(RootprintError::BranchNotFound(selector.to_string()));
        };
        if matches.next().is_some() {
            return Err(RootprintError::AmbiguousSelector(selector.to_string()));
        }
        Ok(first)
    }

    /// Creates a one-parent branch and returns its deterministic ID.
    pub fn fork(
        &mut self,
        parent: &str,
        label: impl Into<String>,
        artifact: PhaArtifact,
    ) -> Result<String, RootprintError> {
        artifact.verify().map_err(RootprintError::Pha)?;
        let parent = self.navigate(parent)?.clone();
        let label = normalized_label(label.into())?;
        let parents = vec![parent.id];
        let id = calculate_branch_id(&label, &parents, &artifact)?;
        self.insert_branch(RootprintBranch {
            id: id.clone(),
            label,
            sequence: parent.sequence.saturating_add(1),
            parents,
            artifact,
        })?;
        Ok(id)
    }

    /// Creates a two-parent merge branch and returns its deterministic ID.
    pub fn merge(
        &mut self,
        left: &str,
        right: &str,
        label: impl Into<String>,
        artifact: PhaArtifact,
    ) -> Result<String, RootprintError> {
        artifact.verify().map_err(RootprintError::Pha)?;
        let left = self.navigate(left)?.clone();
        let right = self.navigate(right)?.clone();
        if left.id == right.id {
            return Err(RootprintError::DuplicateMergeParent(left.id));
        }
        let label = normalized_label(label.into())?;
        let mut parents = vec![left.id, right.id];
        parents.sort();
        let id = calculate_branch_id(&label, &parents, &artifact)?;
        self.insert_branch(RootprintBranch {
            id: id.clone(),
            label,
            sequence: left.sequence.max(right.sequence).saturating_add(1),
            parents,
            artifact,
        })?;
        Ok(id)
    }

    /// Returns whether two branches carry the same Power House core identity.
    ///
    /// External proof attachments are ignored.
    pub fn equivalent(&self, left: &str, right: &str) -> Result<bool, RootprintError> {
        let left = self.navigate(left)?;
        let right = self.navigate(right)?;
        Ok(left.artifact.phx_fingerprint == right.artifact.phx_fingerprint)
    }

    /// Reconstructs the graph's canonical logical state.
    pub fn replay(&self) -> Result<RootprintState, RootprintError> {
        replay(self)
    }

    /// Returns whether another graph reconstructs the same logical state.
    pub fn graph_equivalent(&self, other: &Self) -> Result<bool, RootprintError> {
        try_equivalent(self, other)
    }

    /// Verifies the complete Rootprint graph using Power House core data only.
    pub fn verify(&self) -> Result<(), RootprintError> {
        if self.schema != ROOTPRINT_SCHEMA_V1 {
            return Err(RootprintError::UnsupportedSchema(self.schema.clone()));
        }
        let root = self
            .branches
            .get(&self.root_branch)
            .ok_or_else(|| RootprintError::BranchNotFound(self.root_branch.clone()))?;
        if root.sequence != 0 || !root.parents.is_empty() {
            return Err(RootprintError::InvalidGraph(
                "root branch must have sequence 0 and no parents".to_string(),
            ));
        }

        for (key, branch) in &self.branches {
            if key != &branch.id {
                return Err(RootprintError::InvalidGraph(format!(
                    "branch map key {key} does not match branch id {}",
                    branch.id
                )));
            }
            branch.artifact.verify().map_err(RootprintError::Pha)?;
            let expected = branch.calculate_id()?;
            if expected != branch.id {
                return Err(RootprintError::BranchIdMismatch {
                    expected,
                    found: branch.id.clone(),
                });
            }
            if branch.parents.len() > 2 {
                return Err(RootprintError::InvalidGraph(format!(
                    "branch {} has more than two parents",
                    branch.id
                )));
            }
            if branch.id != self.root_branch && branch.parents.is_empty() {
                return Err(RootprintError::InvalidGraph(format!(
                    "non-root branch {} has no parent",
                    branch.id
                )));
            }
            if branch.parents.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(RootprintError::InvalidGraph(format!(
                    "branch {} parents must be sorted and unique",
                    branch.id
                )));
            }
            for parent_id in &branch.parents {
                let parent = self
                    .branches
                    .get(parent_id)
                    .ok_or_else(|| RootprintError::BranchNotFound(parent_id.clone()))?;
                if parent.sequence >= branch.sequence {
                    return Err(RootprintError::InvalidGraph(format!(
                        "branch {} does not follow parent {}",
                        branch.id, parent_id
                    )));
                }
            }
        }

        let mut children: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for branch in self.branches.values() {
            for parent in &branch.parents {
                children
                    .entry(parent.as_str())
                    .or_default()
                    .push(branch.id.as_str());
            }
        }

        let mut reachable = BTreeSet::new();
        let mut stack = vec![self.root_branch.clone()];
        while let Some(branch_id) = stack.pop() {
            if !reachable.insert(branch_id.clone()) {
                continue;
            }
            if let Some(branch_children) = children.get(branch_id.as_str()) {
                stack.extend(branch_children.iter().map(|child| (*child).to_string()));
            }
        }
        if reachable.len() != self.branches.len() {
            return Err(RootprintError::InvalidGraph(
                "graph contains a branch unreachable from the root".to_string(),
            ));
        }
        Ok(())
    }

    /// Explicitly verifies attachment integrity on every branch.
    ///
    /// This operation is separate from Rootprint core verification.
    pub fn verify_external_proof_attachments(&self) -> Result<(), RootprintError> {
        self.verify()?;
        for branch in self.branches.values() {
            branch
                .artifact
                .verify_external_proof_attachments()
                .map_err(RootprintError::Pha)?;
        }
        Ok(())
    }

    fn insert_branch(&mut self, branch: RootprintBranch) -> Result<(), RootprintError> {
        if self.branches.contains_key(&branch.id) {
            return Err(RootprintError::DuplicateBranch(branch.id));
        }
        self.branches.insert(branch.id.clone(), branch);
        Ok(())
    }
}

/// Deterministically merges two valid Rootprint graphs with the same root.
///
/// Duplicate branches must have identical core identity. Differences limited
/// to optional transport data are resolved by selecting the lexicographically
/// smaller canonical JSON encoding, making the operation commutative.
pub fn merge(left: Rootprint, right: Rootprint) -> Result<Rootprint, RootprintError> {
    left.verify()?;
    right.verify()?;
    if left.root_branch != right.root_branch {
        return Err(RootprintError::IncompatibleRoots {
            left: left.root_branch,
            right: right.root_branch,
        });
    }

    let mut merged = left;
    for (id, candidate) in right.branches {
        match merged.branches.get(&id) {
            None => {
                merged.branches.insert(id, candidate);
            }
            Some(existing) => {
                if branch_core_projection(existing) != branch_core_projection(&candidate) {
                    return Err(RootprintError::BranchConflict(id));
                }
                let existing_bytes =
                    serde_json::to_vec(existing).map_err(RootprintError::Serialization)?;
                let candidate_bytes =
                    serde_json::to_vec(&candidate).map_err(RootprintError::Serialization)?;
                if candidate_bytes < existing_bytes {
                    merged.branches.insert(id, candidate);
                }
            }
        }
    }
    merged.verify()?;
    Ok(merged)
}

/// Reconstructs canonical logical state from a valid Rootprint graph.
pub fn replay(graph: &Rootprint) -> Result<RootprintState, RootprintError> {
    graph.verify()?;
    let mut ordered: Vec<&RootprintBranch> = graph.branches.values().collect();
    ordered.sort_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then_with(|| left.id.cmp(&right.id))
    });
    let mut canonical_sequences = BTreeMap::new();
    for branch in &ordered {
        let sequence = branch
            .parents
            .iter()
            .filter_map(|parent| canonical_sequences.get(parent))
            .copied()
            .max()
            .unwrap_or(0_u64)
            .saturating_add(u64::from(!branch.parents.is_empty()));
        canonical_sequences.insert(branch.id.clone(), sequence);
    }
    ordered.sort_by(|left, right| {
        canonical_sequences[&left.id]
            .cmp(&canonical_sequences[&right.id])
            .then_with(|| left.id.cmp(&right.id))
    });

    let branches = ordered
        .into_iter()
        .map(|branch| {
            Ok(RootprintReplayBranch {
                id: RootprintId::new(branch.id.clone())?,
                label: branch.label.clone(),
                sequence: canonical_sequences[&branch.id],
                parents: branch
                    .parents
                    .iter()
                    .cloned()
                    .map(RootprintId::new)
                    .collect::<Result<Vec<_>, _>>()?,
                artifact_phx_fingerprint: branch.artifact.phx_fingerprint.clone(),
            })
        })
        .collect::<Result<Vec<_>, RootprintError>>()?;

    let parent_ids: BTreeSet<&str> = graph
        .branches
        .values()
        .flat_map(|branch| branch.parents.iter().map(String::as_str))
        .collect();
    let tips = graph
        .branches
        .keys()
        .filter(|id| !parent_ids.contains(id.as_str()))
        .cloned()
        .map(RootprintId::new)
        .collect::<Result<Vec<_>, _>>()?;
    let root_branch = RootprintId::new(graph.root_branch.clone())?;
    let projection = serde_json::json!({
        "branches": &branches,
        "root_branch": &root_branch,
        "tips": &tips,
    });
    let encoded = serde_json::to_vec(&projection).map_err(RootprintError::Serialization)?;
    let mut hasher = Sha256::new();
    hasher.update(REPLAY_STATE_DOMAIN);
    hasher.update(encoded);

    Ok(RootprintState {
        root_branch,
        branches,
        tips,
        state_fingerprint: format!("{SHA256_PREFIX}{}", hex::encode(hasher.finalize())),
    })
}

/// Checks graph equivalence, returning validation errors to the caller.
pub fn try_equivalent(left: &Rootprint, right: &Rootprint) -> Result<bool, RootprintError> {
    Ok(replay(left)? == replay(right)?)
}

/// Returns whether two valid graphs reconstruct the same logical state.
///
/// Invalid graphs are never equivalent.
pub fn equivalent(left: &Rootprint, right: &Rootprint) -> bool {
    try_equivalent(left, right).unwrap_or(false)
}

/// Errors returned by Rootprint operations.
#[derive(Debug)]
pub enum RootprintError {
    /// A `.pha` artifact failed core or explicit attachment verification.
    Pha(PhaError),
    /// The Rootprint schema is unsupported.
    UnsupportedSchema(String),
    /// A branch selector matched nothing.
    BranchNotFound(String),
    /// A branch selector matched multiple branches.
    AmbiguousSelector(String),
    /// A branch with the same deterministic identity already exists.
    DuplicateBranch(String),
    /// A merge specified the same parent twice.
    DuplicateMergeParent(String),
    /// A branch label is invalid.
    InvalidLabel(String),
    /// A Rootprint identifier is malformed.
    InvalidId(String),
    /// The stored branch ID does not match core branch data.
    BranchIdMismatch {
        /// Recalculated deterministic ID.
        expected: String,
        /// Stored branch ID.
        found: String,
    },
    /// The graph violates Rootprint invariants.
    InvalidGraph(String),
    /// Whole-graph merge requires both inputs to share the same root.
    IncompatibleRoots {
        /// Left graph root.
        left: String,
        /// Right graph root.
        right: String,
    },
    /// Two branches with the same ID disagree on core graph data.
    BranchConflict(String),
    /// Rootprint serialization failed.
    Serialization(serde_json::Error),
}

impl fmt::Display for RootprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pha(error) => write!(formatter, "PHA verification failed: {error}"),
            Self::UnsupportedSchema(schema) => {
                write!(formatter, "unsupported Rootprint schema: {schema}")
            }
            Self::BranchNotFound(selector) => write!(formatter, "branch not found: {selector}"),
            Self::AmbiguousSelector(selector) => {
                write!(formatter, "ambiguous branch selector: {selector}")
            }
            Self::DuplicateBranch(id) => write!(formatter, "branch already exists: {id}"),
            Self::DuplicateMergeParent(id) => {
                write!(formatter, "merge parents resolve to the same branch: {id}")
            }
            Self::InvalidLabel(label) => write!(formatter, "invalid branch label: {label:?}"),
            Self::InvalidId(id) => write!(formatter, "invalid Rootprint identifier: {id}"),
            Self::BranchIdMismatch { expected, found } => write!(
                formatter,
                "Rootprint branch ID mismatch: expected {expected}, found {found}"
            ),
            Self::InvalidGraph(message) => write!(formatter, "invalid Rootprint graph: {message}"),
            Self::IncompatibleRoots { left, right } => {
                write!(formatter, "cannot merge Rootprint roots {left} and {right}")
            }
            Self::BranchConflict(id) => {
                write!(formatter, "conflicting Rootprint branch data for {id}")
            }
            Self::Serialization(error) => {
                write!(formatter, "Rootprint serialization failed: {error}")
            }
        }
    }
}

impl Error for RootprintError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Pha(error) => Some(error),
            Self::Serialization(error) => Some(error),
            _ => None,
        }
    }
}

fn normalized_label(label: String) -> Result<String, RootprintError> {
    let trimmed = label.trim();
    if trimmed.is_empty() || trimmed.len() > 128 || trimmed.chars().any(char::is_control) {
        return Err(RootprintError::InvalidLabel(label));
    }
    Ok(trimmed.to_string())
}

fn validate_rootprint_id(value: &str) -> Result<(), RootprintError> {
    let Some(hex_digest) = value.strip_prefix(SHA256_PREFIX) else {
        return Err(RootprintError::InvalidId(value.to_string()));
    };
    if hex_digest.len() != 64
        || !hex_digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(RootprintError::InvalidId(value.to_string()));
    }
    Ok(())
}

fn branch_core_projection(branch: &RootprintBranch) -> Value {
    serde_json::json!({
        "artifact_phx_fingerprint": branch.artifact.phx_fingerprint,
        "id": branch.id,
        "label": branch.label,
        "parents": branch.parents,
    })
}

fn calculate_branch_id(
    label: &str,
    parents: &[String],
    artifact: &PhaArtifact,
) -> Result<String, RootprintError> {
    artifact.verify().map_err(RootprintError::Pha)?;
    let encoded = serde_json::to_vec(&serde_json::json!({
        "artifact_phx_fingerprint": artifact.phx_fingerprint,
        "label": label,
        "parents": parents,
    }))
    .map_err(RootprintError::Serialization)?;
    let mut hasher = Sha256::new();
    hasher.update(BRANCH_ID_DOMAIN);
    hasher.update(encoded);
    Ok(format!("{SHA256_PREFIX}{}", hex::encode(hasher.finalize())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provenance::ExternalProofAttachment;
    use serde_json::json;

    fn artifact(value: u64) -> PhaArtifact {
        PhaArtifact::new(
            json!({"source": "rootprint-test"}),
            "power-house/test/v1",
            json!({"value": value}),
            json!({"accepted": true}),
        )
        .unwrap()
    }

    fn attachment() -> ExternalProofAttachment {
        ExternalProofAttachment::new("epa", "external/test/v1", json!({"proof": "x"})).unwrap()
    }

    #[test]
    fn branching_and_merging_are_core_only() {
        let root_artifact = artifact(1);
        let mut graph = Rootprint::new("main", root_artifact.clone()).unwrap();

        let mut with_epa = root_artifact.clone();
        with_epa.embedded_proof.external_proof_attachments = Some(vec![attachment()]);
        let left = graph.fork("main", "left", with_epa.clone()).unwrap();

        with_epa
            .embedded_proof
            .external_proof_attachments
            .as_mut()
            .unwrap()[0]
            .payload = json!({"proof": "mutated"});
        let right = graph.fork("main", "right", with_epa).unwrap();
        let merged = graph.merge(&left, &right, "merged", artifact(2)).unwrap();

        assert!(graph.verify().is_ok());
        assert!(graph.equivalent(&left, &right).unwrap());
        assert_eq!(graph.navigate("merged").unwrap().id, merged);
        assert!(graph.verify_external_proof_attachments().is_err());
    }

    #[test]
    fn attachment_presence_does_not_change_branch_id() {
        let base = artifact(1);
        let graph_without = Rootprint::new("main", base.clone()).unwrap();
        let mut attached = base;
        attached.embedded_proof.external_proof_attachments = Some(vec![attachment()]);
        let graph_with = Rootprint::new("main", attached).unwrap();
        assert_eq!(graph_without.root_branch, graph_with.root_branch);
    }

    #[test]
    fn core_mutation_breaks_graph_verification() {
        let mut graph = Rootprint::new("main", artifact(1)).unwrap();
        graph
            .branches
            .get_mut(&graph.root_branch)
            .unwrap()
            .artifact
            .embedded_proof
            .proof = json!({"accepted": false});
        assert!(graph.verify().is_err());
    }

    #[test]
    fn replay_and_whole_graph_merge_are_deterministic() {
        let root = artifact(1);
        let mut left = Rootprint::new("main", root.clone()).unwrap();
        left.fork("main", "left", artifact(2)).unwrap();
        let mut right = Rootprint::new("main", root).unwrap();
        right.fork("main", "right", artifact(3)).unwrap();

        let left_first = merge(left.clone(), right.clone()).unwrap();
        let right_first = merge(right, left).unwrap();
        assert_eq!(left_first.replay().unwrap(), right_first.replay().unwrap());
        assert!(equivalent(&left_first, &right_first));
        assert_eq!(left_first.replay().unwrap(), left_first.replay().unwrap());
    }

    #[test]
    fn cycles_are_rejected_and_replay_canonicalizes_sequence() {
        let mut graph = Rootprint::new("main", artifact(1)).unwrap();
        let child = graph.fork("main", "child", artifact(2)).unwrap();
        let root = graph.root_branch.clone();
        graph.branches.get_mut(&root).unwrap().parents = vec![child];
        assert!(graph.verify().is_err());

        let mut elevated = Rootprint::new("main", artifact(1)).unwrap();
        let child = elevated.fork("main", "child", artifact(2)).unwrap();
        let canonical = elevated.replay().unwrap();
        elevated.branches.get_mut(&child).unwrap().sequence = 9;
        assert!(elevated.verify().is_ok());
        assert_eq!(elevated.replay().unwrap(), canonical);
    }
}

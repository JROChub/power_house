//! Immutable computational identities backed by `.pha` and Rootprint.

use crate::provenance::{
    PhaArtifact, PhaError, Rootprint, RootprintError, RootprintId, RootprintState,
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

/// An immutable identity view over one `.pha` artifact and Rootprint node.
///
/// Fields are private so an identity cannot be altered in place. Fork and
/// merge operations always return a new identity while preserving the source
/// identities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Identity {
    pha: PhaArtifact,
    rootprint_id: RootprintId,
}

impl Identity {
    /// Constructs an identity envelope.
    ///
    /// Use [`Self::verify`] before trusting envelopes received from external
    /// storage or transport.
    pub fn new(pha: PhaArtifact, rootprint_id: RootprintId) -> Self {
        Self { pha, rootprint_id }
    }

    /// Creates an identity and initializes its Rootprint graph.
    pub fn create(
        label: impl Into<String>,
        mut pha: PhaArtifact,
    ) -> Result<(Self, Rootprint), IdentityError> {
        pha.identity_root = None;
        pha.verify().map_err(IdentityError::Pha)?;
        let mut graph = Rootprint::new(label, pha).map_err(IdentityError::Rootprint)?;
        let rootprint_id =
            RootprintId::new(graph.root_branch.clone()).map_err(IdentityError::Rootprint)?;
        let identity = bind_branch(&mut graph, rootprint_id)?;
        identity.verify(&graph)?;
        Ok((identity, graph))
    }

    /// Forks this identity into a new Rootprint node.
    pub fn fork(
        &self,
        graph: &mut Rootprint,
        label: impl Into<String>,
        mut pha: PhaArtifact,
    ) -> Result<Self, IdentityError> {
        self.verify(graph)?;
        pha.identity_root = None;
        let branch_id = graph
            .fork(self.rootprint_id.as_str(), label, pha)
            .map_err(IdentityError::Rootprint)?;
        let identity = bind_branch(
            graph,
            RootprintId::new(branch_id).map_err(IdentityError::Rootprint)?,
        )?;
        identity.verify(graph)?;
        Ok(identity)
    }

    /// Merges two identities into a deterministic reconciliation node.
    pub fn merge(
        left: &Self,
        right: &Self,
        graph: &mut Rootprint,
        label: impl Into<String>,
        mut pha: PhaArtifact,
    ) -> Result<Self, IdentityError> {
        left.verify(graph)?;
        right.verify(graph)?;
        pha.identity_root = None;
        let branch_id = graph
            .merge(
                left.rootprint_id.as_str(),
                right.rootprint_id.as_str(),
                label,
                pha,
            )
            .map_err(IdentityError::Rootprint)?;
        let identity = bind_branch(
            graph,
            RootprintId::new(branch_id).map_err(IdentityError::Rootprint)?,
        )?;
        identity.verify(graph)?;
        Ok(identity)
    }

    /// Verifies the artifact, graph, node resolution, and identity binding.
    pub fn verify(&self, graph: &Rootprint) -> Result<(), IdentityError> {
        self.pha.verify().map_err(IdentityError::Pha)?;
        graph.verify().map_err(IdentityError::Rootprint)?;
        let bound_root = self
            .pha
            .identity_root
            .as_ref()
            .ok_or(IdentityError::MissingIdentityRoot)?;
        if bound_root != &self.rootprint_id {
            return Err(IdentityError::IdentityRootMismatch {
                expected: self.rootprint_id.clone(),
                found: bound_root.clone(),
            });
        }

        let branch = graph
            .navigate(self.rootprint_id.as_str())
            .map_err(IdentityError::Rootprint)?;
        if branch.id != self.rootprint_id.as_str() {
            return Err(IdentityError::UnresolvedIdentityRoot(
                self.rootprint_id.clone(),
            ));
        }
        if branch.artifact.identity_root.as_ref() != Some(&self.rootprint_id) {
            return Err(IdentityError::GraphBindingMismatch(
                self.rootprint_id.clone(),
            ));
        }
        if branch.artifact.phx_fingerprint != self.pha.phx_fingerprint {
            return Err(IdentityError::ArtifactMismatch {
                rootprint_id: self.rootprint_id.clone(),
            });
        }
        Ok(())
    }

    /// Deterministically replays the graph and resolves this identity.
    pub fn replay(&self, graph: &Rootprint) -> Result<IdentityState, IdentityError> {
        self.verify(graph)?;
        let graph_state = graph.replay().map_err(IdentityError::Rootprint)?;
        if !graph_state
            .branches
            .iter()
            .any(|branch| branch.id == self.rootprint_id)
        {
            return Err(IdentityError::UnresolvedIdentityRoot(
                self.rootprint_id.clone(),
            ));
        }
        Ok(IdentityState {
            rootprint_id: self.rootprint_id.clone(),
            artifact_phx_fingerprint: self.pha.phx_fingerprint.clone(),
            graph: graph_state,
        })
    }

    /// Checks whether two identities resolve to equivalent core artifacts.
    pub fn equivalent(&self, other: &Self, graph: &Rootprint) -> Result<bool, IdentityError> {
        self.verify(graph)?;
        other.verify(graph)?;
        graph
            .equivalent(self.rootprint_id.as_str(), other.rootprint_id.as_str())
            .map_err(IdentityError::Rootprint)
    }

    /// Returns the immutable `.pha` artifact.
    pub fn pha(&self) -> &PhaArtifact {
        &self.pha
    }

    /// Returns the stable Rootprint node identifier.
    pub fn rootprint_id(&self) -> &RootprintId {
        &self.rootprint_id
    }

    /// Serializes the identity with deterministic JSON object ordering.
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, IdentityError> {
        serde_json::to_vec(self).map_err(IdentityError::Serialization)
    }

    /// Consumes the identity and returns its components.
    pub fn into_parts(self) -> (PhaArtifact, RootprintId) {
        (self.pha, self.rootprint_id)
    }
}

/// Deterministic replay outcome for one identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityState {
    /// Replayed identity node.
    pub rootprint_id: RootprintId,
    /// Core artifact fingerprint at that node.
    pub artifact_phx_fingerprint: String,
    /// Canonical Rootprint replay state.
    pub graph: RootprintState,
}

/// Errors returned by identity operations.
#[derive(Debug)]
pub enum IdentityError {
    /// `.pha` verification failed.
    Pha(PhaError),
    /// Rootprint verification or mutation failed.
    Rootprint(RootprintError),
    /// The identity artifact has no graph binding.
    MissingIdentityRoot,
    /// The artifact binding disagrees with the identity envelope.
    IdentityRootMismatch {
        /// Root expected by the identity envelope.
        expected: RootprintId,
        /// Root stored by the artifact.
        found: RootprintId,
    },
    /// The identity node cannot be resolved in the graph.
    UnresolvedIdentityRoot(RootprintId),
    /// The graph's artifact does not bind back to the resolved node.
    GraphBindingMismatch(RootprintId),
    /// The identity artifact does not match the graph node's core identity.
    ArtifactMismatch {
        /// Rootprint node containing the conflicting artifact.
        rootprint_id: RootprintId,
    },
    /// Deterministic JSON serialization failed.
    Serialization(serde_json::Error),
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pha(error) => write!(formatter, "identity PHA verification failed: {error}"),
            Self::Rootprint(error) => {
                write!(formatter, "identity Rootprint operation failed: {error}")
            }
            Self::MissingIdentityRoot => formatter.write_str("identity_root is missing"),
            Self::IdentityRootMismatch { expected, found } => {
                write!(
                    formatter,
                    "identity_root mismatch: expected {expected}, found {found}"
                )
            }
            Self::UnresolvedIdentityRoot(id) => {
                write!(formatter, "identity_root cannot be resolved: {id}")
            }
            Self::GraphBindingMismatch(id) => {
                write!(
                    formatter,
                    "Rootprint node does not bind back to identity {id}"
                )
            }
            Self::ArtifactMismatch { rootprint_id } => {
                write!(
                    formatter,
                    "identity artifact does not match Rootprint node {rootprint_id}"
                )
            }
            Self::Serialization(error) => {
                write!(formatter, "identity serialization failed: {error}")
            }
        }
    }
}

impl Error for IdentityError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Pha(error) => Some(error),
            Self::Rootprint(error) => Some(error),
            Self::Serialization(error) => Some(error),
            _ => None,
        }
    }
}

fn bind_branch(
    graph: &mut Rootprint,
    rootprint_id: RootprintId,
) -> Result<Identity, IdentityError> {
    let branch = graph
        .branches
        .get_mut(rootprint_id.as_str())
        .ok_or_else(|| IdentityError::UnresolvedIdentityRoot(rootprint_id.clone()))?;
    branch.artifact.identity_root = Some(rootprint_id.clone());
    Ok(Identity::new(branch.artifact.clone(), rootprint_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn artifact(value: u64) -> PhaArtifact {
        PhaArtifact::new(
            json!({"source": "identity-test"}),
            "power-house/identity-test/v1",
            json!({"value": value}),
            json!({"accepted": true}),
        )
        .unwrap()
    }

    #[test]
    fn create_fork_merge_verify_and_replay_are_deterministic() {
        let (root, mut graph) = Identity::create("main", artifact(1)).unwrap();
        let left = root.fork(&mut graph, "left", artifact(2)).unwrap();
        let right = root.fork(&mut graph, "right", artifact(2)).unwrap();
        assert!(left.equivalent(&right, &graph).unwrap());

        let merged = Identity::merge(&left, &right, &mut graph, "merged", artifact(3)).unwrap();
        let first = merged.replay(&graph).unwrap();
        let second = merged.replay(&graph).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            merged.to_canonical_json().unwrap(),
            merged.to_canonical_json().unwrap()
        );
    }

    #[test]
    fn invalid_or_mutated_bindings_are_rejected() {
        let (identity, graph) = Identity::create("main", artifact(1)).unwrap();
        let mut value = serde_json::to_value(&identity).unwrap();
        value["pha"]["identity_root"] =
            serde_json::Value::String(format!("sha256:{}", "0".repeat(64)));
        let mutated: Identity = serde_json::from_value(value).unwrap();
        assert!(matches!(
            mutated.verify(&graph),
            Err(IdentityError::IdentityRootMismatch { .. })
        ));
    }
}

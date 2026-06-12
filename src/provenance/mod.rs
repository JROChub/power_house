//! Portable provenance artifacts and deterministic identity.

pub mod pha;
pub mod rootprint;

pub use pha::{EmbeddedProof, ExternalProofAttachment, PhaArtifact, PhaError, PHA_SCHEMA_V1};
pub use rootprint::{Rootprint, RootprintBranch, RootprintError, ROOTPRINT_SCHEMA_V1};

/// Creates or extends a Rootprint graph with a verified Power House artifact.
///
/// This is the recommended library interface for provenance-aware proof
/// creation. Every form returns `Result<_, RootprintError>`.
///
/// ```
/// use power_house::{prove_with_rootprint, provenance::PhaArtifact};
/// use serde_json::json;
///
/// let artifact = PhaArtifact::new(
///     json!({"source": "example"}),
///     "power-house/example/v1",
///     json!({"claim": 7}),
///     json!({"valid": true}),
/// )?;
/// let graph = prove_with_rootprint!(label: "main", artifact: artifact)?;
/// graph.verify()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[macro_export]
macro_rules! prove_with_rootprint {
    (label: $label:expr, artifact: $artifact:expr $(,)?) => {
        $crate::provenance::Rootprint::new($label, $artifact)
    };
    (
        rootprint: $rootprint:expr,
        fork: $parent:expr,
        label: $label:expr,
        artifact: $artifact:expr $(,)?
    ) => {
        $rootprint.fork($parent, $label, $artifact)
    };
    (
        rootprint: $rootprint:expr,
        merge: [$left:expr, $right:expr],
        label: $label:expr,
        artifact: $artifact:expr $(,)?
    ) => {
        $rootprint.merge($left, $right, $label, $artifact)
    };
}

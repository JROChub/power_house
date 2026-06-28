//! Error and rejection types for Power House Memory Capsules.

use crate::{observatory::ObservatoryError, provenance::RootprintError};
use std::error::Error;
use std::fmt;

/// A precise rejection trace preserving the layer where verification failed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RejectionTrace {
    /// Stable rejection status.
    pub status: String,
    /// Verification layer that rejected the capsule.
    pub layer: String,
    /// Stable machine-readable rejection code.
    pub code: String,
    /// Human-readable rejection message.
    pub message: String,
    /// JSON pointer to the rejected field when available.
    pub json_pointer: Option<String>,
    /// Expected value when available.
    pub expected: Option<String>,
    /// Actual value when available.
    pub actual: Option<String>,
    /// Whether core verification had passed before this failure.
    pub core_valid_before_failure: bool,
    /// Whether Rootprint verification had passed before this failure.
    pub rootprint_valid_before_failure: bool,
    /// Whether semantic data can alter core identity.
    pub semantic_can_affect_core: bool,
}

impl RejectionTrace {
    /// Creates a rejection trace.
    pub fn new(
        layer: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            status: "rejected".to_string(),
            layer: layer.into(),
            code: code.into(),
            message: message.into(),
            json_pointer: None,
            expected: None,
            actual: None,
            core_valid_before_failure: false,
            rootprint_valid_before_failure: false,
            semantic_can_affect_core: false,
        }
    }

    /// Sets the JSON pointer for the rejected value.
    pub fn at(mut self, pointer: impl Into<String>) -> Self {
        self.json_pointer = Some(pointer.into());
        self
    }

    /// Sets expected and actual values.
    pub fn values(mut self, expected: impl Into<String>, actual: impl Into<String>) -> Self {
        self.expected = Some(expected.into());
        self.actual = Some(actual.into());
        self
    }

    /// Records layer state before the failure.
    pub fn boundary(mut self, core_valid: bool, rootprint_valid: bool) -> Self {
        self.core_valid_before_failure = core_valid;
        self.rootprint_valid_before_failure = rootprint_valid;
        self
    }
}

/// Errors returned by Memory Capsule operations.
#[derive(Debug)]
pub enum MemoryError {
    /// File I/O failed.
    Io(std::io::Error),
    /// JSON serialization or decoding failed.
    Json(serde_json::Error),
    /// Strict canonical JSON validation failed.
    Canonical(String),
    /// A digest string was malformed.
    InvalidDigest(String),
    /// Power House core PHA verification failed.
    Core(String),
    /// Rootprint verification failed.
    Rootprint(RootprintError),
    /// Observatory sidecar verification failed.
    Observatory(ObservatoryError),
    /// Verification rejected the capsule with a structured trace.
    Rejected(Box<RejectionTrace>),
    /// Challenge execution did not match the expected failure.
    ChallengeMismatch(String),
    /// A requested mutation is unsupported.
    UnsupportedMutation(String),
}

impl MemoryError {
    /// Creates a structured rejection error without making every error variant large.
    pub(crate) fn rejected(trace: RejectionTrace) -> Self {
        Self::Rejected(Box::new(trace))
    }
}

impl fmt::Display for MemoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "memory capsule I/O failed: {error}"),
            Self::Json(error) => write!(formatter, "memory capsule JSON failed: {error}"),
            Self::Canonical(message) => write!(formatter, "canonical JSON rejected: {message}"),
            Self::InvalidDigest(digest) => write!(formatter, "invalid SHA-256 digest: {digest}"),
            Self::Core(message) => write!(formatter, "core verification failed: {message}"),
            Self::Rootprint(error) => write!(formatter, "Rootprint verification failed: {error}"),
            Self::Observatory(error) => {
                write!(
                    formatter,
                    "Observatory sidecar verification failed: {error}"
                )
            }
            Self::Rejected(trace) => write!(
                formatter,
                "{} rejected with {}: {}",
                trace.layer, trace.code, trace.message
            ),
            Self::ChallengeMismatch(message) => write!(formatter, "challenge mismatch: {message}"),
            Self::UnsupportedMutation(mutation) => {
                write!(formatter, "unsupported memory mutation: {mutation}")
            }
        }
    }
}

impl Error for MemoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::Rootprint(error) => Some(error),
            Self::Observatory(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for MemoryError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for MemoryError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

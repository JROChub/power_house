//! Whole-program zero-knowledge receipts for SFCS.
//!
//! This module uses a RISC Zero receipt as the authoritative proof that one
//! guest image produced one public journal from hidden input. The receipt is
//! then bound to a deterministic SFCS program graph. The deterministic
//! statement is committed by `.pha` core identity while the guest program binary and
//! receipt are carried as a mandatory, explicitly verified external
//! attachment. Development-mode receipts are rejected explicitly.

use super::{digest_json, SfcsError, SfcsGraph, SfcsNode, SfcsOp};
use crate::memory::{MemoryCapsule, MemoryVerificationPolicy};
use crate::provenance::{ExternalProofAttachment, PhaArtifact, PhaError};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use bincode::Options;
use object::{Object, ObjectSection};
use risc0_binfmt::ProgramBinary;
use risc0_zkvm::sha::Digestible;
use risc0_zkvm::{
    compute_image_id, default_prover, ExecutorEnv, InnerReceipt, Receipt, VerifierContext,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fmt;

/// Stable `.pha` protocol for whole-program SFCS private-VM receipts.
pub const SFCS_RISC0_PRIVATE_VM_PROTOCOL_V1: &str = "power-house/sfcs-risc0-private-vm/v1";

const PROOF_DOMAIN: &[u8] = b"power-house:sfcs-risc0-private-vm:v1:proof\0";
const STATEMENT_DOMAIN: &[u8] = b"power-house:sfcs-risc0-private-vm:v1:statement\0";
const PROGRAM_DIGEST_DOMAIN: &[u8] = b"power-house:sfcs-risc0-private-vm:v1:program\0";
const JOURNAL_DIGEST_DOMAIN: &[u8] = b"power-house:sfcs-risc0-private-vm:v1:journal\0";
const RECEIPT_DIGEST_DOMAIN: &[u8] = b"power-house:sfcs-risc0-private-vm:v1:receipt\0";
const MAX_PROGRAM_BYTES: usize = 64 * 1024 * 1024;
const MAX_RECEIPT_BYTES: u64 = 512 * 1024 * 1024;
const MAX_JOURNAL_BYTES: usize = 16 * 1024 * 1024;
const MAX_EMBEDDED_BASE64_BYTES: usize = (MAX_PROGRAM_BYTES * 4 / 3) + 8;
const TEXT_CHUNK_BYTES: usize = 4 * 1024;
const MAX_TEXT_CHUNKS: usize = 8 * 1024;
const RECEIPT_ATTACHMENT_ID: &str = "sfcs-risc0-private-vm-receipt-v1";

/// Public statement authenticated by a whole-program private-VM receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsRisc0PrivateVmStatement {
    /// Protocol identifier.
    pub schema: String,
    /// RISC Zero guest image identifier.
    pub image_id: String,
    /// Domain-separated digest of the complete guest program binary.
    pub program_digest: String,
    /// Deterministic SFCS graph digest projected from the guest image.
    pub graph_digest: String,
    /// Public guest journal bytes.
    pub journal_base64: String,
    /// Domain-separated digest of the public journal.
    pub journal_digest: String,
    /// RISC Zero digest of the successful execution claim.
    pub receipt_claim_digest: String,
}

/// A real whole-program zero-knowledge receipt and its SFCS binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsRisc0PrivateVmProof {
    /// Public statement.
    pub statement: SfcsRisc0PrivateVmStatement,
    /// Bincode-encoded RISC Zero receipt.
    pub receipt_base64: String,
    /// Domain-separated digest of the encoded receipt.
    pub receipt_digest: String,
    /// Domain-separated digest of the complete proof body.
    pub proof_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SfcsRisc0Embedding {
    program_binary_base64: String,
    proof: SfcsRisc0PrivateVmProof,
}

impl SfcsRisc0PrivateVmProof {
    /// Proves execution of an arbitrary RISC Zero guest program binary.
    ///
    /// `private_input` is written to the guest input channel and is never
    /// included in the returned proof or `.pha` artifact. The guest controls
    /// which result bytes become public by writing to its journal.
    pub fn prove(program_binary: &[u8], private_input: &[u8]) -> Result<Self, SfcsRisc0Error> {
        let binding = ProgramBinding::new(program_binary)?;
        let environment = ExecutorEnv::builder()
            .write_slice(private_input)
            .build()
            .map_err(|error| SfcsRisc0Error::Prover(error.to_string()))?;
        let receipt = default_prover()
            .prove(environment, program_binary)
            .map_err(|error| SfcsRisc0Error::Prover(error.to_string()))?
            .receipt;
        reject_fake_receipt(&receipt)?;
        receipt
            .verify(binding.image_id)
            .map_err(|error| SfcsRisc0Error::Verification(error.to_string()))?;
        Self::from_verified_receipt(binding, receipt)
    }

    /// Constructs the portable proof from an independently produced receipt.
    ///
    /// This path verifies the receipt before any Power House object is
    /// returned and is suitable for receipts produced by local, remote, CPU,
    /// CUDA, or Metal provers.
    pub fn from_receipt(program_binary: &[u8], receipt: Receipt) -> Result<Self, SfcsRisc0Error> {
        let binding = ProgramBinding::new(program_binary)?;
        reject_fake_receipt(&receipt)?;
        receipt
            .verify(binding.image_id)
            .map_err(|error| SfcsRisc0Error::Verification(error.to_string()))?;
        Self::from_verified_receipt(binding, receipt)
    }

    fn from_verified_receipt(
        binding: ProgramBinding,
        receipt: Receipt,
    ) -> Result<Self, SfcsRisc0Error> {
        let receipt_bytes = receipt_options()
            .serialize(&receipt)
            .map_err(|error| SfcsRisc0Error::Serialization(error.to_string()))?;
        if receipt_bytes.len() as u64 > MAX_RECEIPT_BYTES {
            return Err(SfcsRisc0Error::LimitExceeded(format!(
                "receipt exceeds {MAX_RECEIPT_BYTES} bytes"
            )));
        }
        let journal = receipt.journal.bytes.clone();
        if journal.len() > MAX_JOURNAL_BYTES {
            return Err(SfcsRisc0Error::LimitExceeded(format!(
                "journal exceeds {MAX_JOURNAL_BYTES} bytes"
            )));
        }
        let receipt_claim_digest = format!(
            "risc0:{}",
            receipt
                .claim()
                .map_err(|error| SfcsRisc0Error::Verification(error.to_string()))?
                .digest()
        );
        let statement = SfcsRisc0PrivateVmStatement {
            schema: SFCS_RISC0_PRIVATE_VM_PROTOCOL_V1.to_string(),
            image_id: binding.image_id_text,
            program_digest: binding.program_digest,
            graph_digest: binding.graph_digest,
            journal_base64: BASE64.encode(&journal),
            journal_digest: sha256_domain(JOURNAL_DIGEST_DOMAIN, &journal),
            receipt_claim_digest,
        };
        let mut proof = Self {
            statement,
            receipt_base64: BASE64.encode(&receipt_bytes),
            receipt_digest: sha256_domain(RECEIPT_DIGEST_DOMAIN, &receipt_bytes),
            proof_digest: String::new(),
        };
        proof.proof_digest = digest_json(PROOF_DOMAIN, &proof.preimage())?;
        Ok(proof)
    }

    /// Verifies the real receipt, guest image, public journal, and SFCS graph.
    pub fn verify(&self, program_binary: &[u8]) -> Result<(), SfcsRisc0Error> {
        if self.statement.schema != SFCS_RISC0_PRIVATE_VM_PROTOCOL_V1 {
            return Err(SfcsRisc0Error::UnsupportedSchema(
                self.statement.schema.clone(),
            ));
        }
        let binding = ProgramBinding::new(program_binary)?;
        if self.statement.image_id != binding.image_id_text {
            return Err(SfcsRisc0Error::InvalidProof(
                "guest image ID does not match statement".to_string(),
            ));
        }
        if self.statement.program_digest != binding.program_digest {
            return Err(SfcsRisc0Error::InvalidProof(
                "guest program digest does not match statement".to_string(),
            ));
        }
        if self.statement.graph_digest != binding.graph_digest {
            return Err(SfcsRisc0Error::InvalidProof(
                "SFCS program graph does not match guest program binary".to_string(),
            ));
        }
        let receipt_bytes = decode_receipt(&self.receipt_base64)?;
        if self.receipt_digest != sha256_domain(RECEIPT_DIGEST_DOMAIN, &receipt_bytes) {
            return Err(SfcsRisc0Error::InvalidProof(
                "receipt digest does not match receipt bytes".to_string(),
            ));
        }
        let receipt: Receipt = receipt_options()
            .deserialize(&receipt_bytes)
            .map_err(|error| SfcsRisc0Error::Serialization(error.to_string()))?;
        reject_fake_receipt(&receipt)?;
        receipt
            .verify_with_context(&VerifierContext::default(), binding.image_id)
            .map_err(|error| SfcsRisc0Error::Verification(error.to_string()))?;
        let receipt_claim_digest = format!(
            "risc0:{}",
            receipt
                .claim()
                .map_err(|error| SfcsRisc0Error::Verification(error.to_string()))?
                .digest()
        );
        if self.statement.receipt_claim_digest != receipt_claim_digest {
            return Err(SfcsRisc0Error::InvalidProof(
                "receipt claim does not match public statement".to_string(),
            ));
        }
        let journal = decode_base64("journal", &self.statement.journal_base64, MAX_JOURNAL_BYTES)?;
        if receipt.journal.bytes != journal {
            return Err(SfcsRisc0Error::InvalidProof(
                "receipt journal does not match public statement".to_string(),
            ));
        }
        if self.statement.journal_digest != sha256_domain(JOURNAL_DIGEST_DOMAIN, &journal) {
            return Err(SfcsRisc0Error::InvalidProof(
                "journal digest does not match public journal".to_string(),
            ));
        }
        let expected_proof_digest = digest_json(PROOF_DOMAIN, &self.preimage())?;
        if self.proof_digest != expected_proof_digest {
            return Err(SfcsRisc0Error::InvalidProof(
                "proof digest does not match proof body".to_string(),
            ));
        }
        Ok(())
    }

    /// Returns the deterministic SFCS graph projected from the guest image.
    pub fn program_graph(program_binary: &[u8]) -> Result<SfcsGraph, SfcsRisc0Error> {
        ProgramBinding::new(program_binary).map(|binding| binding.graph)
    }

    /// Creates a deterministic `.pha` core statement with a verified receipt
    /// attachment for Rootprint and Memory Capsule workflows.
    ///
    /// Receipt bytes are intentionally excluded from `phx_fingerprint`.
    /// Different valid zero-knowledge receipts for the same guest and public
    /// journal therefore retain one Power House identity.
    pub fn to_pha_artifact(
        &self,
        label: impl Into<String>,
        program_binary: &[u8],
    ) -> Result<PhaArtifact, SfcsRisc0Error> {
        self.verify(program_binary)?;
        let statement_digest = digest_json(STATEMENT_DOMAIN, &self.statement)?;
        let mut artifact = PhaArtifact::new(
            serde_json::json!({
                "producer": "power_house_sfcs_risc0",
                "label": label.into(),
                "profile": SFCS_RISC0_PRIVATE_VM_PROTOCOL_V1,
                "image_id": self.statement.image_id,
                "program_digest": self.statement.program_digest,
                "graph_digest": self.statement.graph_digest,
                "journal_digest": self.statement.journal_digest,
                "receipt_claim_digest": self.statement.receipt_claim_digest,
            }),
            SFCS_RISC0_PRIVATE_VM_PROTOCOL_V1,
            serde_json::to_value(&self.statement)?,
            serde_json::json!({
                "statement_digest": statement_digest,
                "attachment_id": RECEIPT_ATTACHMENT_ID,
            }),
        )
        .map_err(SfcsRisc0Error::Pha)?;
        let mut attachment = ExternalProofAttachment::new(
            RECEIPT_ATTACHMENT_ID,
            SFCS_RISC0_PRIVATE_VM_PROTOCOL_V1,
            serde_json::to_value(SfcsRisc0Embedding {
                program_binary_base64: BASE64.encode(program_binary),
                proof: self.clone(),
            })?,
        )
        .map_err(SfcsRisc0Error::Pha)?;
        attachment.verifier_hint =
            Some("power_house::verify_sfcs_risc0_private_vm_embedding".to_string());
        artifact.embedded_proof.external_proof_attachments = Some(vec![attachment]);
        Ok(artifact)
    }

    fn preimage(&self) -> serde_json::Value {
        serde_json::json!({
            "statement": self.statement,
            "receipt_base64": self.receipt_base64,
            "receipt_digest": self.receipt_digest,
        })
    }
}

/// Verifies a `.pha` artifact carrying a whole-program private-VM receipt.
pub fn verify_sfcs_risc0_private_vm_embedding(
    artifact: &PhaArtifact,
) -> Result<SfcsRisc0PrivateVmProof, SfcsRisc0Error> {
    artifact.verify().map_err(SfcsRisc0Error::Pha)?;
    if artifact.embedded_proof.protocol != SFCS_RISC0_PRIVATE_VM_PROTOCOL_V1 {
        return Err(SfcsRisc0Error::InvalidEmbedding(
            "embedded proof protocol is not SFCS RISC0 private VM".to_string(),
        ));
    }
    artifact
        .verify_external_proof_attachments()
        .map_err(SfcsRisc0Error::Pha)?;
    let statement: SfcsRisc0PrivateVmStatement =
        serde_json::from_value(artifact.embedded_proof.public_inputs.clone())?;
    if statement.schema != SFCS_RISC0_PRIVATE_VM_PROTOCOL_V1 {
        return Err(SfcsRisc0Error::UnsupportedSchema(statement.schema));
    }
    let expected_statement_digest = digest_json(STATEMENT_DOMAIN, &statement)?;
    let statement_digest = artifact.embedded_proof.proof["statement_digest"]
        .as_str()
        .ok_or_else(|| {
            SfcsRisc0Error::InvalidEmbedding(
                "core statement digest is missing or not a string".to_string(),
            )
        })?;
    if statement_digest != expected_statement_digest {
        return Err(SfcsRisc0Error::InvalidEmbedding(
            "core statement digest does not match public inputs".to_string(),
        ));
    }
    if artifact.embedded_proof.proof["attachment_id"].as_str() != Some(RECEIPT_ATTACHMENT_ID) {
        return Err(SfcsRisc0Error::InvalidEmbedding(
            "core receipt attachment identifier is invalid".to_string(),
        ));
    }
    let attachments = artifact
        .embedded_proof
        .external_proof_attachments
        .as_ref()
        .ok_or_else(|| {
            SfcsRisc0Error::InvalidEmbedding(
                "required private-VM receipt attachment is absent".to_string(),
            )
        })?;
    let matching: Vec<_> = attachments
        .iter()
        .filter(|attachment| attachment.id == RECEIPT_ATTACHMENT_ID)
        .collect();
    if matching.len() != 1 {
        return Err(SfcsRisc0Error::InvalidEmbedding(
            "expected exactly one private-VM receipt attachment".to_string(),
        ));
    }
    let attachment = matching[0];
    if attachment.proof_system != SFCS_RISC0_PRIVATE_VM_PROTOCOL_V1 {
        return Err(SfcsRisc0Error::InvalidEmbedding(
            "private-VM receipt attachment uses the wrong proof system".to_string(),
        ));
    }
    let embedding: SfcsRisc0Embedding = serde_json::from_value(attachment.payload.clone())?;
    if embedding.program_binary_base64.len() > MAX_EMBEDDED_BASE64_BYTES {
        return Err(SfcsRisc0Error::LimitExceeded(
            "embedded guest program binary exceeds the supported size".to_string(),
        ));
    }
    let program_binary = decode_base64(
        "guest program binary",
        &embedding.program_binary_base64,
        MAX_PROGRAM_BYTES,
    )?;
    embedding.proof.verify(&program_binary)?;
    if embedding.proof.statement != statement {
        return Err(SfcsRisc0Error::InvalidEmbedding(
            "receipt statement does not match deterministic core statement".to_string(),
        ));
    }
    for (field, expected) in [
        ("image_id", embedding.proof.statement.image_id.as_str()),
        (
            "program_digest",
            embedding.proof.statement.program_digest.as_str(),
        ),
        (
            "graph_digest",
            embedding.proof.statement.graph_digest.as_str(),
        ),
        (
            "journal_digest",
            embedding.proof.statement.journal_digest.as_str(),
        ),
        (
            "receipt_claim_digest",
            embedding.proof.statement.receipt_claim_digest.as_str(),
        ),
    ] {
        require_bound_provenance_string(artifact, field, expected)?;
    }
    Ok(embedding.proof)
}

/// Verifies a Memory Capsule and its mandatory whole-program receipt.
///
/// Generic capsule verification preserves Power House v1 behavior and only
/// checks the transport integrity of core data. This protocol-specific entry
/// point additionally executes RISC Zero receipt verification.
pub fn verify_sfcs_risc0_private_vm_capsule(
    capsule: &MemoryCapsule,
    policy: MemoryVerificationPolicy,
) -> Result<SfcsRisc0PrivateVmProof, SfcsRisc0Error> {
    capsule
        .verify(policy)
        .map_err(|error| SfcsRisc0Error::Capsule(error.to_string()))?;
    verify_sfcs_risc0_private_vm_embedding(&capsule.core.pha)
}

struct ProgramBinding {
    image_id: risc0_zkvm::sha::Digest,
    image_id_text: String,
    program_digest: String,
    graph_digest: String,
    graph: SfcsGraph,
}

impl ProgramBinding {
    fn new(program_binary: &[u8]) -> Result<Self, SfcsRisc0Error> {
        if program_binary.is_empty() {
            return Err(SfcsRisc0Error::InvalidProgram(
                "guest program binary is empty".to_string(),
            ));
        }
        if program_binary.len() > MAX_PROGRAM_BYTES {
            return Err(SfcsRisc0Error::LimitExceeded(format!(
                "guest program binary exceeds {MAX_PROGRAM_BYTES} bytes"
            )));
        }
        let image_id = compute_image_id(program_binary)
            .map_err(|error| SfcsRisc0Error::InvalidProgram(error.to_string()))?;
        let image_id_text = format!("risc0:{image_id}");
        let program_digest = sha256_domain(PROGRAM_DIGEST_DOMAIN, program_binary);
        let graph = graph_from_program(program_binary, &image_id_text, &program_digest)?;
        let graph_digest = graph.fractal_digest()?;
        Ok(Self {
            image_id,
            image_id_text,
            program_digest,
            graph_digest,
            graph,
        })
    }
}

fn graph_from_program(
    program_binary: &[u8],
    image_id: &str,
    program_digest: &str,
) -> Result<SfcsGraph, SfcsRisc0Error> {
    let binary = ProgramBinary::decode(program_binary)
        .map_err(|error| SfcsRisc0Error::InvalidProgram(error.to_string()))?;
    let mut graph = SfcsGraph::new(Vec::new());
    let root = SfcsNode::new("guest_image", SfcsOp::Input, Vec::new())
        .with_label("RISC Zero guest image")
        .with_metadata("image_id", image_id)
        .with_metadata("program_digest", program_digest)
        .with_metadata("abi_kind", format!("{:?}", binary.header.abi_kind))
        .with_metadata("abi_version", binary.header.abi_version.to_string());
    graph.insert_node(root)?;
    let mut previous = "guest_image".to_string();
    let mut total_chunks = 0_usize;
    for (component, elf) in [("user", binary.user_elf), ("kernel", binary.kernel_elf)] {
        let file = object::File::parse(elf)
            .map_err(|error| SfcsRisc0Error::InvalidProgram(error.to_string()))?;
        let text = file.section_by_name(".text").ok_or_else(|| {
            SfcsRisc0Error::InvalidProgram(format!("{component} ELF has no .text section"))
        })?;
        let text_bytes = text
            .uncompressed_data()
            .map_err(|error| SfcsRisc0Error::InvalidProgram(error.to_string()))?;
        if text_bytes.is_empty() {
            return Err(SfcsRisc0Error::InvalidProgram(format!(
                "{component} ELF .text section is empty"
            )));
        }
        for (index, chunk) in text_bytes.chunks(TEXT_CHUNK_BYTES).enumerate() {
            total_chunks = total_chunks.checked_add(1).ok_or_else(|| {
                SfcsRisc0Error::LimitExceeded("executable chunk count overflow".to_string())
            })?;
            if total_chunks > MAX_TEXT_CHUNKS {
                return Err(SfcsRisc0Error::LimitExceeded(format!(
                    "executable text exceeds {MAX_TEXT_CHUNKS} chunks"
                )));
            }
            let id = format!("{component}_text_{index:08x}");
            let node = SfcsNode::new(&id, SfcsOp::DenseStep, vec![previous])
                .with_label(format!("{component} text chunk {index}"))
                .with_metadata("component", component)
                .with_metadata("text_address", text.address().to_string())
                .with_metadata("offset", (index * TEXT_CHUNK_BYTES).to_string())
                .with_metadata("bytes", chunk.len().to_string())
                .with_metadata("chunk_digest", sha256_domain(PROGRAM_DIGEST_DOMAIN, chunk));
            graph.insert_node(node)?;
            previous = id;
        }
    }
    graph.outputs = vec![previous];
    graph.verify()?;
    Ok(graph)
}

fn reject_fake_receipt(receipt: &Receipt) -> Result<(), SfcsRisc0Error> {
    if matches!(&receipt.inner, InnerReceipt::Fake(_)) {
        return Err(SfcsRisc0Error::FakeReceipt);
    }
    Ok(())
}

fn receipt_options() -> impl Options {
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .with_limit(MAX_RECEIPT_BYTES)
        .reject_trailing_bytes()
}

fn decode_receipt(encoded: &str) -> Result<Vec<u8>, SfcsRisc0Error> {
    if encoded.len() as u64 > (MAX_RECEIPT_BYTES.saturating_mul(4) / 3) + 8 {
        return Err(SfcsRisc0Error::LimitExceeded(
            "encoded receipt exceeds the supported size".to_string(),
        ));
    }
    decode_base64("receipt", encoded, MAX_RECEIPT_BYTES as usize)
}

fn decode_base64(
    label: &str,
    encoded: &str,
    max_decoded_bytes: usize,
) -> Result<Vec<u8>, SfcsRisc0Error> {
    let decoded = BASE64.decode(encoded).map_err(|error| {
        SfcsRisc0Error::InvalidProof(format!("invalid {label} base64: {error}"))
    })?;
    if decoded.len() > max_decoded_bytes {
        return Err(SfcsRisc0Error::LimitExceeded(format!(
            "{label} exceeds {max_decoded_bytes} bytes"
        )));
    }
    if BASE64.encode(&decoded) != encoded {
        return Err(SfcsRisc0Error::InvalidProof(format!(
            "{label} base64 is not canonical"
        )));
    }
    Ok(decoded)
}

fn require_bound_provenance_string(
    artifact: &PhaArtifact,
    field: &str,
    expected: &str,
) -> Result<(), SfcsRisc0Error> {
    let provenance = artifact.provenance[field].as_str().ok_or_else(|| {
        SfcsRisc0Error::InvalidEmbedding(format!("provenance {field} is missing or not a string"))
    })?;
    if provenance != expected {
        return Err(SfcsRisc0Error::InvalidEmbedding(format!(
            "{field} does not match proof"
        )));
    }
    Ok(())
}

fn sha256_domain(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

/// Errors returned by the whole-program SFCS private-VM backend.
#[derive(Debug)]
pub enum SfcsRisc0Error {
    /// Unsupported proof schema.
    UnsupportedSchema(String),
    /// Guest program is not a supported RISC Zero program binary.
    InvalidProgram(String),
    /// Proof construction failed.
    Prover(String),
    /// Cryptographic receipt verification failed.
    Verification(String),
    /// Development-mode receipts are never accepted.
    FakeReceipt,
    /// Proof body or public binding is inconsistent.
    InvalidProof(String),
    /// `.pha` binding is inconsistent.
    InvalidEmbedding(String),
    /// A bounded parser limit was exceeded.
    LimitExceeded(String),
    /// Receipt serialization failed.
    Serialization(String),
    /// SFCS graph operation failed.
    Sfcs(SfcsError),
    /// `.pha` operation failed.
    Pha(PhaError),
    /// Memory Capsule verification failed.
    Capsule(String),
    /// JSON serialization failed.
    Json(serde_json::Error),
}

impl fmt::Display for SfcsRisc0Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchema(schema) => write!(formatter, "unsupported schema: {schema}"),
            Self::InvalidProgram(message) => write!(formatter, "invalid guest program: {message}"),
            Self::Prover(message) => write!(formatter, "RISC Zero prover failed: {message}"),
            Self::Verification(message) => {
                write!(
                    formatter,
                    "RISC Zero receipt verification failed: {message}"
                )
            }
            Self::FakeReceipt => write!(formatter, "development-mode receipts are not accepted"),
            Self::InvalidProof(message) => write!(formatter, "invalid private-VM proof: {message}"),
            Self::InvalidEmbedding(message) => {
                write!(formatter, "invalid private-VM embedding: {message}")
            }
            Self::LimitExceeded(message) => {
                write!(formatter, "private-VM limit exceeded: {message}")
            }
            Self::Serialization(message) => {
                write!(formatter, "receipt serialization failed: {message}")
            }
            Self::Sfcs(error) => write!(formatter, "SFCS error: {error}"),
            Self::Pha(error) => write!(formatter, "PHA error: {error}"),
            Self::Capsule(message) => write!(formatter, "Memory Capsule error: {message}"),
            Self::Json(error) => write!(formatter, "JSON error: {error}"),
        }
    }
}

impl Error for SfcsRisc0Error {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Sfcs(error) => Some(error),
            Self::Pha(error) => Some(error),
            Self::Json(error) => Some(error),
            _ => None,
        }
    }
}

impl From<SfcsError> for SfcsRisc0Error {
    fn from(error: SfcsError) -> Self {
        Self::Sfcs(error)
    }
}

impl From<PhaError> for SfcsRisc0Error {
    fn from(error: PhaError) -> Self {
        Self::Pha(error)
    }
}

impl From<serde_json::Error> for SfcsRisc0Error {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

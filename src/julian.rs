//! The design philosophy underlying `power_house` is pedagogical, yet mathematically rigorous.
//! Each module encapsulates a discrete concept in modern computational complexity theory,
//! illustrating how modest abstractions compose into a cohesive proof infrastructure.
//!
//! This crate aspires to bridge gaps between theoretical exposition and practical engineering,
//! serving both as a didactic resource and a foundation for future cryptographic research.
//! Blueprint for the JULIAN Protocol – a proof-transparent ledger that
//! anchors folding transcripts into deterministic consensus states.
//!
//! ## Ledger anchor definition
//!
//! * **Transcript hashes**: For every accepted proof we derive a
//!   deterministic Fiat–Shamir trace `(challenges, round_sums,
//!   final_evaluation)` and hash it to a domain-separated BLAKE2b-256
//!   digest. These hashes live in [`LedgerEntry::hashes`] and form
//!   append-only commitments to the verification trace.
//! * **Chain validity**: A [`LedgerAnchor`] is the ordered list of
//!   [`EntryAnchor { statement, hashes }`].  A ledger state is valid iff
//!   every anchor entry matches a locally recomputed digest.  The helper
//!   [`reconcile_anchors`] enforces this condition.
//! * **Finality predicate**: `Final(anchors, quorum)` holds when at least
//!   *quorum* anchors agree on every statement/hash pair.  In code this is
//!   [`reconcile_anchors_with_quorum`].  Once the predicate returns `Ok(())`
//!   the JULIAN ledger state is final.
//!
//! ## Multi-node reconciliation protocol
//!
//! 1. Each node records ASCII transcript logs (see `examples/verify_logs`) and
//!    recomputes its digest vector.
//! 2. Nodes exchange [`LedgerAnchor`] structures (`Vec<EntryAnchor>`).
//! 3. Nodes run [`reconcile_anchors_with_quorum`] with their desired quorum.
//!    Success implies consensus finality; failure pinpoints divergent anchors.
//! 4. When divergence occurs, fetch the offending log file and run
//!    `verify_logs` to diagnose tampering.
//!
//! The pipeline is intentionally compact: aside from the core Rust standard
//! library, it leans on BLAKE2b-256 to guarantee transcript integrity and
//! deterministic anchor reconciliation.

use crate::{
    merkle_root, transcript_digest, write_text_series, write_transcript_record, ChainedSumProof,
    Field, GeneralSumProof, MultilinearPolynomial, StreamingPolynomial, SumClaim, TranscriptDigest,
};
use blake2::digest::{consts::U32, Digest};
use std::{collections::HashMap, path::PathBuf};

const ANCHOR_DOMAIN: &[u8] = b"MFENX_ANCHOR";

/// Represents a statement to be proved.  In a full system this would
/// encapsulate the input and the specification of the language `L`.
#[derive(Debug, Clone)]
pub struct Statement {
    /// A human-readable description of the claim.
    pub description: String,
}

/// Different proof payloads that the ledger understands.
#[derive(Debug, Clone)]
pub enum ProofKind {
    /// The original pedagogical demo sum-check claim.
    Demo(SumClaim),
    /// A generalized multilinear sum-check proof and its defining polynomial.
    General {
        /// Polynomial evaluated over the Boolean hypercube.
        polynomial: MultilinearPolynomial,
        /// Proof attesting to the polynomial's sum.
        proof: GeneralSumProof,
    },
    /// A generalized sum-check proof accompanied by a streaming evaluator.
    StreamingGeneral {
        /// Streaming polynomial evaluator used for verification.
        polynomial: StreamingPolynomial,
        /// Proof attesting to the polynomial's sum.
        proof: GeneralSumProof,
    },
    /// A chain of proofs, each committing to the previous final evaluation.
    Chain {
        /// Polynomials that participate in the chained proof.
        polynomials: Vec<MultilinearPolynomial>,
        /// Chained sum-check proof object.
        proof: ChainedSumProof,
    },
    /// The JULIAN protocol genesis anchor.
    Genesis,
}

/// Represents a proof object submitted by a prover.
#[derive(Debug, Clone)]
pub struct Proof {
    /// Proof payload understood by the ledger.
    pub kind: ProofKind,
    /// Additional proof data (not implemented).
    pub data: Vec<u8>,
}

/// A ledger entry recording a statement, its proof and the outcome of
/// verification.
#[derive(Debug, Clone)]
pub struct LedgerEntry {
    /// The statement being proved.
    pub statement: Statement,
    /// The proof submitted.
    pub proof: Proof,
    /// Whether the proof was verified successfully.
    pub accepted: bool,
    /// Fiat–Shamir challenges logged for each proof component.
    pub transcripts: Vec<Vec<u64>>,
    /// Running sums recorded during verification.
    pub round_sums: Vec<Vec<u64>>,
    /// Final evaluations observed during verification.
    pub final_values: Vec<u64>,
    /// Files written to disk for replayable transcripts.
    pub log_paths: Vec<PathBuf>,
    /// Optional error captured while attempting to persist transcripts.
    pub log_error: Option<String>,
    /// Deterministic transcript hashes retained in-memory.
    pub hashes: Vec<TranscriptDigest>,
    /// Merkle root over the transcript hashes.
    pub merkle_root: TranscriptDigest,
}

/// A simple proof ledger that stores entries.  In a real system, this
/// would be replicated across validators and incorporate consensus and
/// finality logic.
#[derive(Debug)]
pub struct ProofLedger {
    entries: Vec<LedgerEntry>,
    log_dir: Option<PathBuf>,
    log_counter: usize,
}

/// Anchor representing the hashed transcripts for a ledger entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntryAnchor {
    /// Statement associated with the entry.
    pub statement: String,
    /// Hashes of each transcript record.
    pub hashes: Vec<TranscriptDigest>,
    /// Merkle root for the transcript hashes.
    pub merkle_root: TranscriptDigest,
}

/// Additional metadata associated with a ledger anchor.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct AnchorMetadata {
    /// Challenge derivation mode recorded in transcript logs (e.g., `mod`).
    pub challenge_mode: Option<String>,
    /// Optional fold digest aggregated from transcript hashes.
    pub fold_digest: Option<TranscriptDigest>,
    /// Crate version that produced this anchor.
    pub crate_version: Option<String>,
}

/// Anchor aggregation for an entire ledger.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LedgerAnchor {
    /// Ordered entry anchors mirroring the ledger submissions.
    pub entries: Vec<EntryAnchor>,
    /// Supplementary metadata emitted alongside transcript hashes.
    pub metadata: AnchorMetadata,
}

/// Statement string used for the JULIAN genesis anchor.
pub const JULIAN_GENESIS_STATEMENT: &str = "JULIAN::GENESIS";

/// Pinned digest associated with the JULIAN genesis transcript.
pub const JULIAN_GENESIS_DIGEST: TranscriptDigest = [
    0xcd, 0xcc, 0x8f, 0x36, 0xbf, 0x3d, 0x51, 0x1f, 0x04, 0xdf, 0x86, 0xc6, 0x3b, 0xcf, 0x58,
    0x0d, 0xae, 0xe7, 0x3a, 0xa6, 0x7c, 0x0c, 0xf9, 0x14, 0x48, 0x3a, 0x05, 0xc2, 0xd2, 0x89,
    0x58, 0x4a,
];

/// Returns the digest associated with the JULIAN genesis transcript.
pub fn julian_genesis_hash() -> TranscriptDigest {
    JULIAN_GENESIS_DIGEST
}

/// Returns the canonical JULIAN genesis anchor.
pub fn julian_genesis_anchor() -> LedgerAnchor {
    let hashes = vec![julian_genesis_hash()];
    let merkle = merkle_root(&hashes);
    LedgerAnchor {
        entries: vec![EntryAnchor {
            statement: JULIAN_GENESIS_STATEMENT.to_string(),
            hashes: hashes.clone(),
            merkle_root: merkle,
        }],
        metadata: AnchorMetadata {
            crate_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            fold_digest: Some(fold_digest_from_hashes(&hashes)),
            ..AnchorMetadata::default()
        },
    }
}

impl ProofLedger {
    /// Creates an empty ledger.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            log_dir: None,
            log_counter: 0,
        }
    }

    /// Enables on-disk logging and resets the internal log counter.
    pub fn enable_logging<P: Into<PathBuf>>(&mut self, log_dir: P) {
        self.log_dir = Some(log_dir.into());
        self.log_counter = 0;
    }

    /// Submits a statement and proof to the ledger.  The verifier inspects
    /// demo proofs, generalized multilinear proofs, or chained proofs and logs
    /// the deterministic transcripts for future audit.
    pub fn submit(&mut self, statement: Statement, proof: Proof) {
        if !matches!(proof.kind, ProofKind::Genesis) {
            self.ensure_genesis();
        }

        let mut transcripts = Vec::new();
        let mut round_sums = Vec::new();
        let mut final_values = Vec::new();
        let mut hashes = Vec::new();
        let log_paths = Vec::new();
        let log_error = None;

        let accepted = match &proof.kind {
            ProofKind::Demo(claim) => claim.verify_demo(),
            ProofKind::General { polynomial, proof } => {
                let field = Field::new(proof.claim.p);
                match proof.verify_with_trace(polynomial, &field) {
                    Some(trace) => {
                        transcripts.push(trace.challenges.clone());
                        round_sums.push(trace.round_sums.clone());
                        final_values.push(trace.final_evaluation);
                        hashes.push(transcript_digest(
                            &trace.challenges,
                            &trace.round_sums,
                            trace.final_evaluation,
                        ));
                        true
                    }
                    None => false,
                }
            }
            ProofKind::StreamingGeneral { polynomial, proof } => {
                let field = Field::new(proof.claim.p);
                if polynomial.modulus() != proof.claim.p {
                    false
                } else {
                    match proof.verify_streaming_with_trace(polynomial, &field) {
                        Some(trace) => {
                            transcripts.push(trace.challenges.clone());
                            round_sums.push(trace.round_sums.clone());
                            final_values.push(trace.final_evaluation);
                            hashes.push(transcript_digest(
                                &trace.challenges,
                                &trace.round_sums,
                                trace.final_evaluation,
                            ));
                            true
                        }
                        None => false,
                    }
                }
            }
            ProofKind::Chain {
                polynomials,
                proof: chain,
            } => {
                let modulus = chain
                    .links()
                    .first()
                    .map(|link| link.proof.claim.p)
                    .unwrap_or(0);
                if modulus < 3 || modulus % 2 == 0 {
                    false
                } else {
                    let field = Field::new(modulus);
                    match chain.verify_with_traces(polynomials, &field) {
                        Some(traces) => {
                            for trace in traces {
                                transcripts.push(trace.challenges.clone());
                                round_sums.push(trace.round_sums.clone());
                                final_values.push(trace.final_evaluation);
                                hashes.push(transcript_digest(
                                    &trace.challenges,
                                    &trace.round_sums,
                                    trace.final_evaluation,
                                ));
                            }
                            true
                        }
                        None => false,
                    }
                }
            }
            ProofKind::Genesis => true,
        };

        let mut entry = if matches!(proof.kind, ProofKind::Genesis) {
            let hashes = vec![julian_genesis_hash()];
            let merkle = merkle_root(&hashes);
            LedgerEntry {
                statement,
                proof,
                accepted,
                transcripts: vec![Vec::new()],
                round_sums: vec![Vec::new()],
                final_values: vec![0],
                log_paths: Vec::new(),
                log_error: None,
                hashes,
                merkle_root: merkle,
            }
        } else {
            let merkle = merkle_root(&hashes);
            LedgerEntry {
                statement,
                proof,
                accepted,
                transcripts,
                round_sums,
                final_values,
                log_paths,
                log_error,
                hashes,
                merkle_root: merkle,
            }
        };

        if entry.accepted && !matches!(entry.proof.kind, ProofKind::Genesis) {
            if let Some(dir) = &self.log_dir {
                for idx in 0..entry.transcripts.len() {
                    let mut lines = Vec::new();
                    if let Err(err) = write_transcript_record(
                        |line| {
                            lines.push(line.to_string());
                            Ok(())
                        },
                        &entry.transcripts[idx],
                        &entry.round_sums[idx],
                        entry.final_values[idx],
                    ) {
                        entry.log_error = Some(err.to_string());
                        break;
                    }
                    lines.insert(0, format!("statement:{}", entry.statement.description));
                    match write_text_series(dir, "ledger", self.log_counter, &lines) {
                        Ok(path) => {
                            entry.log_paths.push(path);
                            self.log_counter += 1;
                        }
                        Err(err) => {
                            entry.log_error = Some(err.to_string());
                            break;
                        }
                    }
                }
            }
        }

        self.entries.push(entry);
    }

    /// Returns a read-only view of the current ledger entries.
    pub fn entries(&self) -> &[LedgerEntry] {
        &self.entries
    }

    /// Returns the current ledger anchor containing transcript hashes per entry.
    pub fn anchor(&self) -> LedgerAnchor {
        let entries: Vec<EntryAnchor> = self
            .entries
            .iter()
            .map(|entry| EntryAnchor {
                statement: entry.statement.description.clone(),
                hashes: entry.hashes.clone(),
                merkle_root: entry.merkle_root,
            })
            .collect();
        let fold_digest = fold_digest_from_entries(&entries);
        LedgerAnchor {
            entries,
            metadata: AnchorMetadata {
                challenge_mode: None,
                fold_digest: Some(fold_digest),
                crate_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            },
        }
    }

    /// Ensures the JULIAN genesis anchor is present at the head of the ledger.
    pub fn ensure_genesis(&mut self) {
        let needs_genesis = self
            .entries
            .first()
            .is_none_or(|entry| entry.statement.description != JULIAN_GENESIS_STATEMENT);
        if needs_genesis {
            let genesis_entry = LedgerEntry {
                statement: Statement {
                    description: JULIAN_GENESIS_STATEMENT.to_string(),
                },
                proof: Proof {
                    kind: ProofKind::Genesis,
                    data: Vec::new(),
                },
                accepted: true,
                transcripts: vec![Vec::new()],
                round_sums: vec![Vec::new()],
                final_values: vec![0],
                log_paths: Vec::new(),
                log_error: None,
                hashes: vec![julian_genesis_hash()],
                merkle_root: merkle_root(&[julian_genesis_hash()]),
            };
            self.entries.insert(0, genesis_entry);
        }
    }
}

impl Default for ProofLedger {
    fn default() -> Self {
        Self::new()
    }
}

fn mix_u64(hasher: &mut Blake2b256, value: u64) {
    hasher.update(value.to_be_bytes());
}

fn mix_bytes(hasher: &mut Blake2b256, bytes: &[u8]) {
    mix_u64(hasher, bytes.len() as u64);
    hasher.update(bytes);
}

fn mix_hash_list(hasher: &mut Blake2b256, hashes: &[TranscriptDigest]) {
    mix_u64(hasher, hashes.len() as u64);
    for digest in hashes {
        hasher.update(digest);
    }
}

fn fold_digest_from_hashes(hashes: &[TranscriptDigest]) -> TranscriptDigest {
    let mut hasher = Blake2b256::new();
    hasher.update(ANCHOR_DOMAIN);
    for digest in hashes {
        hasher.update(digest);
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&hasher.finalize());
    out
}

fn fold_digest_from_entries(entries: &[EntryAnchor]) -> TranscriptDigest {
    let mut hasher = Blake2b256::new();
    hasher.update(ANCHOR_DOMAIN);
    for entry in entries {
        for digest in &entry.hashes {
            hasher.update(digest);
        }
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&hasher.finalize());
    out
}

/// Computes the fold digest for a ledger anchor.
pub fn compute_fold_digest(anchor: &LedgerAnchor) -> TranscriptDigest {
    fold_digest_from_entries(&anchor.entries)
}

/// Computes a digest for the anchor contents (entries + statements + hashes).
pub fn anchor_digest(anchor: &LedgerAnchor) -> [u8; 32] {
    let mut hasher = Blake2b256::new();
    hasher.update(ANCHOR_DOMAIN);
    mix_u64(&mut hasher, anchor.entries.len() as u64);
    for entry in &anchor.entries {
        mix_bytes(&mut hasher, entry.statement.as_bytes());
        mix_hash_list(&mut hasher, &entry.hashes);
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&hasher.finalize());
    out
}

/// Anchor vote supplied to the quorum reconciliation function.
pub struct AnchorVote<'a> {
    /// Ledger anchor produced by the peer.
    pub anchor: &'a LedgerAnchor,
    /// Public key bytes identifying the peer.
    pub public_key: &'a [u8],
}

/// Ensures that a collection of ledger anchors agree on every transcript hash.
pub fn reconcile_anchors(anchors: &[LedgerAnchor]) -> Result<(), String> {
    if anchors.is_empty() {
        return Ok(());
    }
    let reference = &anchors[0];
    for (idx, anchor) in anchors.iter().enumerate().skip(1) {
        if anchor.entries.len() != reference.entries.len() {
            return Err(format!(
                "anchor {} entry count {} mismatch reference {}",
                idx,
                anchor.entries.len(),
                reference.entries.len()
            ));
        }
        for (entry_idx, (left, right)) in reference.entries.iter().zip(&anchor.entries).enumerate()
        {
            if left.statement != right.statement {
                return Err(format!(
                    "anchor {} entry {} statement mismatch",
                    idx, entry_idx
                ));
            }
            if left.hashes != right.hashes {
                return Err(format!("anchor {} entry {} hash mismatch", idx, entry_idx));
            }
        }
    }
    Ok(())
}

/// Ensures that at least `quorum` unique identities agree on every transcript hash.
pub fn reconcile_anchors_with_quorum(
    votes: &[AnchorVote<'_>],
    quorum: usize,
) -> Result<(), String> {
    if votes.is_empty() {
        return Ok(());
    }
    if quorum == 0 || quorum > votes.len() {
        return Err("invalid quorum".to_string());
    }
    let mut groups: HashMap<[u8; 32], HashMap<Vec<u8>, LedgerAnchor>> = HashMap::new();
    for vote in votes {
        if vote.public_key.is_empty() {
            return Err("vote missing public key bytes".to_string());
        }
        let digest = anchor_digest(vote.anchor);
        let entry = groups.entry(digest).or_default();
        entry
            .entry(vote.public_key.to_vec())
            .or_insert_with(|| vote.anchor.clone());
    }
    let mut best: Option<(HashMap<Vec<u8>, LedgerAnchor>, usize)> = None;
    for identity_map in groups.into_values() {
        let count = identity_map.len();
        if best
            .as_ref()
            .map(|(_, best_len)| count > *best_len)
            .unwrap_or(true)
        {
            best = Some((identity_map, count));
        }
    }
    if let Some((identity_map, count)) = best {
        if count >= quorum {
            let anchors: Vec<LedgerAnchor> = identity_map.into_values().collect();
            return reconcile_anchors(&anchors);
        }
    }
    Err("no anchor reached required quorum".to_string())
}

type Blake2b256 = blake2::Blake2b<U32>;

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_poly(field: &Field) -> MultilinearPolynomial {
        let mut evals = Vec::with_capacity(4);
        for x1 in 0..=1u64 {
            for x0 in 0..=1u64 {
                let mut val = field.add(x0, field.mul(3, x1));
                val = field.add(val, field.mul(x0, x1));
                evals.push(val);
            }
        }
        MultilinearPolynomial::from_evaluations(2, evals)
    }

    #[test]
    fn test_ledger_accepts_general_proof() {
        let field = Field::new(101);
        let poly = sample_poly(&field);
        let proof = GeneralSumProof::prove(&poly, &field);
        let statement = Statement {
            description: "Sum-check for 2-var polynomial".to_string(),
        };
        let mut ledger = ProofLedger::new();
        let submission = Proof {
            kind: ProofKind::General {
                polynomial: poly.clone(),
                proof: proof.clone(),
            },
            data: Vec::new(),
        };
        ledger.submit(statement, submission);
        let entries = ledger.entries();
        assert_eq!(entries.len(), 2);
        let proof_entry = &entries[1];
        assert!(proof_entry.accepted);
        assert_eq!(proof_entry.transcripts.len(), 1);
        assert_eq!(proof_entry.round_sums.len(), 1);
        assert_eq!(proof_entry.final_values.len(), 1);
        assert_eq!(proof_entry.transcripts[0], proof.challenges);
        assert!(proof_entry.log_paths.is_empty());
        assert!(proof_entry.log_error.is_none());
        assert_eq!(proof_entry.hashes.len(), 1);
    }

    #[test]
    fn test_ledger_ensures_genesis() {
        let mut ledger = ProofLedger::new();
        ledger.ensure_genesis();
        assert_eq!(ledger.entries.len(), 1);
        assert_eq!(
            ledger.entries[0].statement.description,
            JULIAN_GENESIS_STATEMENT
        );
    }

    #[test]
    fn test_ledger_accepts_streaming_proof() {
        let field = Field::new(101);
        let poly = sample_poly(&field);
        let evals = poly.evaluations().to_vec();
        let streaming =
            StreamingPolynomial::new(poly.num_vars(), field.modulus(), move |idx| evals[idx]);
        let proof = GeneralSumProof::prove_streaming_poly(&streaming, &field);
        let statement = Statement {
            description: "Streaming sum-check".to_string(),
        };
        let mut ledger = ProofLedger::new();
        ledger.submit(
            statement,
            Proof {
                kind: ProofKind::StreamingGeneral {
                    polynomial: streaming.clone(),
                    proof: proof.clone(),
                },
                data: Vec::new(),
            },
        );
        let entries = ledger.entries();
        assert_eq!(entries.len(), 2);
        assert!(entries[1].accepted);
        assert_eq!(entries[1].hashes.len(), 1);
    }

    #[test]
    fn test_ledger_rejects_tampered_chain() {
        let field = Field::new(149);
        let poly_a = sample_poly(&field);
        let proof_a = GeneralSumProof::prove(&poly_a, &field);
        let poly_b = {
            let constant = proof_a.final_evaluation;
            let points = 1usize << 3;
            let inv_points = field.inv(points as u64 % field.modulus());
            let c = field.mul(constant, inv_points);
            MultilinearPolynomial::from_evaluations(3, vec![c; points])
        };
        let polynomials = vec![poly_a.clone(), poly_b.clone()];
        let mut chain = ChainedSumProof::prove(&polynomials, &field);
        if let Some(link) = chain.links_mut().get_mut(1) {
            link.parent_final = Some(field.add(link.parent_final.unwrap(), 1));
        }
        let mut ledger = ProofLedger::new();
        let statement = Statement {
            description: "Tampered chained proof".to_string(),
        };
        let submission = Proof {
            kind: ProofKind::Chain {
                polynomials: polynomials.clone(),
                proof: chain,
            },
            data: Vec::new(),
        };
        ledger.submit(statement, submission);
        let entries = ledger.entries();
        assert_eq!(entries.len(), 2);
        let entry = &entries[1];
        assert!(!entry.accepted);
        assert!(entry.transcripts.is_empty());
        assert!(entry.log_paths.is_empty());
        assert!(entry.log_error.is_none());
        assert!(entry.hashes.is_empty());
    }

    #[test]
    fn test_ledger_writes_logs() {
        let field = Field::new(109);
        let poly = sample_poly(&field);
        let proof = GeneralSumProof::prove(&poly, &field);
        let mut ledger = ProofLedger::new();
        let base = std::env::temp_dir().join("power_house_ledger_logs");
        if base.exists() {
            std::fs::remove_dir_all(&base).unwrap();
        }
        ledger.enable_logging(&base);
        let statement = Statement {
            description: "Logged proof".into(),
        };
        ledger.submit(
            statement,
            Proof {
                kind: ProofKind::General {
                    polynomial: poly,
                    proof: proof.clone(),
                },
                data: Vec::new(),
            },
        );
        let entries = ledger.entries();
        assert_eq!(entries.len(), 2);
        let entry = &entries[1];
        assert!(entry.accepted);
        assert!(!entry.log_paths.is_empty());
        assert!(entry.log_error.is_none());
        for path in &entry.log_paths {
            assert!(path.exists());
            let contents = std::fs::read_to_string(path).unwrap();
            assert!(contents.lines().any(|line| line.starts_with("statement:")));
            assert!(contents.lines().any(|line| line.starts_with("hash:")));
        }
        assert!(!entry.hashes.is_empty());
        std::fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn test_anchor_reconciliation_ok() {
        let field = Field::new(101);
        let poly = sample_poly(&field);
        let proof = GeneralSumProof::prove(&poly, &field);
        let mut ledger_a = ProofLedger::new();
        let mut ledger_b = ProofLedger::new();
        let statement = Statement {
            description: "Shared proof".into(),
        };
        let submission = Proof {
            kind: ProofKind::General {
                polynomial: poly.clone(),
                proof: proof.clone(),
            },
            data: Vec::new(),
        };
        ledger_a.submit(statement.clone(), submission.clone());
        ledger_b.submit(statement, submission);
        let anchor_a = ledger_a.anchor();
        let anchor_b = ledger_b.anchor();
        assert!(reconcile_anchors(&[anchor_a, anchor_b]).is_ok());
    }

    #[test]
    fn test_anchor_reconciliation_fails_on_mismatch() {
        let field = Field::new(101);
        let poly = sample_poly(&field);
        let proof = GeneralSumProof::prove(&poly, &field);
        let mut ledger_a = ProofLedger::new();
        let mut ledger_b = ProofLedger::new();
        let statement = Statement {
            description: "Divergent proof".into(),
        };
        let submission = Proof {
            kind: ProofKind::General {
                polynomial: poly.clone(),
                proof: proof.clone(),
            },
            data: Vec::new(),
        };
        ledger_a.submit(statement.clone(), submission.clone());
        ledger_b.submit(statement, submission);
        // Tamper hashes in ledger B to simulate divergence.
        if let Some(entry) = ledger_b.entries.get_mut(0) {
            if let Some(hash) = entry.hashes.get_mut(0) {
                hash[0] ^= 0x01;
            }
        }
        let anchor_a = ledger_a.anchor();
        let anchor_b = ledger_b.anchor();
        assert!(reconcile_anchors(&[anchor_a, anchor_b]).is_err());
    }

    #[test]
    fn test_reconcile_with_quorum() {
        let field = Field::new(101);
        let poly = sample_poly(&field);
        let proof = GeneralSumProof::prove(&poly, &field);
        let mut ledger_a = ProofLedger::new();
        let mut ledger_b = ProofLedger::new();
        let mut ledger_c = ProofLedger::new();
        let statement = Statement {
            description: "Quorum proof".into(),
        };
        let submission = Proof {
            kind: ProofKind::General {
                polynomial: poly.clone(),
                proof: proof.clone(),
            },
            data: Vec::new(),
        };
        ledger_a.submit(statement.clone(), submission.clone());
        ledger_b.submit(statement.clone(), submission.clone());
        ledger_c.submit(statement, submission);
        let anchors = [ledger_a.anchor(), ledger_b.anchor(), ledger_c.anchor()];
        let votes = [
            AnchorVote {
                anchor: &anchors[0],
                public_key: b"A",
            },
            AnchorVote {
                anchor: &anchors[1],
                public_key: b"B",
            },
            AnchorVote {
                anchor: &anchors[2],
                public_key: b"C",
            },
        ];
        assert!(reconcile_anchors_with_quorum(&votes, 2).is_ok());
    }

    #[test]
    fn test_reconcile_with_quorum_failure() {
        let field = Field::new(101);
        let poly = sample_poly(&field);
        let proof = GeneralSumProof::prove(&poly, &field);
        let mut ledger_a = ProofLedger::new();
        let mut ledger_b = ProofLedger::new();
        let statement = Statement {
            description: "Divergent quorum".into(),
        };
        let submission = Proof {
            kind: ProofKind::General {
                polynomial: poly.clone(),
                proof: proof.clone(),
            },
            data: Vec::new(),
        };
        ledger_a.submit(statement.clone(), submission.clone());
        ledger_b.submit(statement, submission);
        // Tamper ledger B hash
        if let Some(entry) = ledger_b.entries.get_mut(0) {
            if let Some(hash) = entry.hashes.get_mut(0) {
                hash[0] ^= 0x2A;
            }
        }
        let anchors = [ledger_a.anchor(), ledger_b.anchor()];
        let votes = [
            AnchorVote {
                anchor: &anchors[0],
                public_key: b"A",
            },
            AnchorVote {
                anchor: &anchors[1],
                public_key: b"B",
            },
        ];
        assert!(reconcile_anchors_with_quorum(&votes, 2).is_err());
    }

    #[test]
    fn test_reconcile_rejects_duplicate_keys() {
        let field = Field::new(101);
        let poly = sample_poly(&field);
        let proof = GeneralSumProof::prove(&poly, &field);
        let mut ledger_a = ProofLedger::new();
        let mut ledger_b = ProofLedger::new();
        let statement = Statement {
            description: "Duplicate key check".into(),
        };
        let submission = Proof {
            kind: ProofKind::General {
                polynomial: poly.clone(),
                proof: proof.clone(),
            },
            data: Vec::new(),
        };
        ledger_a.submit(statement.clone(), submission.clone());
        ledger_b.submit(statement, submission);
        let anchors = [ledger_a.anchor(), ledger_b.anchor()];
        let votes = [
            AnchorVote {
                anchor: &anchors[0],
                public_key: b"SAME",
            },
            AnchorVote {
                anchor: &anchors[1],
                public_key: b"SAME",
            },
        ];
        assert!(reconcile_anchors_with_quorum(&votes, 2).is_err());
    }
}

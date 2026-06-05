//! Event-driven sum-check certificates for seeded and externally committed
//! sparse polynomials.
//!
//! The polynomial family is a sum of square-free monomials:
//!
//! ```text
//! f(x_0, ..., x_{n-1}) = sum_t c_t * product_{j in S_t} x_j
//! ```
//!
//! A public seed or canonical external polynomial file defines the coefficients
//! and supports `S_t`. The prover and verifier process only the declared
//! variables and nonzero term incidences. They never allocate the `2^n`
//! Boolean hypercube.

use crate::{Field, TranscriptDigest};
use blake2::digest::{consts::U32, Digest};
use std::fmt;

type Blake2b256 = blake2::Blake2b<U32>;

const POLYNOMIAL_DOMAIN: &[u8] = b"power_house:v1:seeded-sparse-polynomial";
const COMMITTED_POLYNOMIAL_DOMAIN: &[u8] = b"power_house:v1:committed-sparse-polynomial";
const TRANSCRIPT_DOMAIN: &[u8] = b"power_house:v1:sparse-sumcheck-transcript";
const CHALLENGE_DOMAIN: &[u8] = b"power_house:v1:sparse-sumcheck-challenge";
const RESPONSE_DOMAIN: &[u8] = b"power_house:v1:sparse-sumcheck-response";
// PHSPv1 was published before the project-wide MFENX PRNG domain migration.
// Keep its derivation domain fixed so existing certificates remain reproducible.
const SPARSE_PRNG_DOMAIN: &[u8] = b"JROC_PRNG";
const CERTIFICATE_MAGIC: &[u8; 8] = b"PHSPv1\0\0";
const POLYNOMIAL_MAGIC: &[u8; 8] = b"PHSMv1\0\0";
const COMMITTED_CERTIFICATE_MAGIC: &[u8; 8] = b"PHCPv1\0\0";

/// Public description of a deterministic sparse multilinear polynomial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeededSparseSpec {
    num_vars: usize,
    num_terms: usize,
    max_degree: usize,
    seed: Vec<u8>,
}

impl SeededSparseSpec {
    /// Creates a seeded sparse polynomial specification.
    ///
    /// # Panics
    ///
    /// Panics if any size is zero or `max_degree > num_vars`.
    pub fn new(
        num_vars: usize,
        num_terms: usize,
        max_degree: usize,
        seed: impl Into<Vec<u8>>,
    ) -> Self {
        assert!(num_vars > 0, "num_vars must be positive");
        assert!(num_terms > 0, "num_terms must be positive");
        assert!(max_degree > 0, "max_degree must be positive");
        assert!(max_degree <= num_vars, "max_degree cannot exceed num_vars");
        Self {
            num_vars,
            num_terms,
            max_degree,
            seed: seed.into(),
        }
    }

    /// Number of variables in the polynomial.
    pub fn num_vars(&self) -> usize {
        self.num_vars
    }

    /// Number of nonzero monomials.
    pub fn num_terms(&self) -> usize {
        self.num_terms
    }

    /// Maximum monomial degree.
    pub fn max_degree(&self) -> usize {
        self.max_degree
    }

    /// Public seed defining the polynomial.
    pub fn seed(&self) -> &[u8] {
        &self.seed
    }
}

/// A square-free monomial in an externally supplied sparse polynomial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SparseMonomial {
    coefficient: u64,
    variables: Vec<usize>,
}

impl SparseMonomial {
    /// Creates a monomial with a nonzero coefficient and sorted unique support.
    pub fn new(coefficient: u64, mut variables: Vec<usize>) -> Result<Self, SparseProofError> {
        if coefficient == 0 {
            return Err(SparseProofError::InvalidPolynomial(
                "monomial coefficient must be nonzero",
            ));
        }
        if variables.is_empty() {
            return Err(SparseProofError::InvalidPolynomial(
                "monomial support must be nonempty",
            ));
        }
        variables.sort_unstable();
        if variables.windows(2).any(|window| window[0] == window[1]) {
            return Err(SparseProofError::InvalidPolynomial(
                "monomial variables must be unique",
            ));
        }
        Ok(Self {
            coefficient,
            variables,
        })
    }

    /// Raw coefficient before field reduction.
    pub fn coefficient(&self) -> u64 {
        self.coefficient
    }

    /// Sorted variable indices in the monomial support.
    pub fn variables(&self) -> &[usize] {
        &self.variables
    }
}

/// Canonical externally supplied sparse multilinear polynomial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedSparsePolynomial {
    num_vars: usize,
    terms: Vec<SparseMonomial>,
}

impl CommittedSparsePolynomial {
    /// Creates and validates an externally supplied sparse polynomial.
    pub fn new(num_vars: usize, terms: Vec<SparseMonomial>) -> Result<Self, SparseProofError> {
        if num_vars == 0 {
            return Err(SparseProofError::InvalidPolynomial(
                "num_vars must be positive",
            ));
        }
        if terms.is_empty() {
            return Err(SparseProofError::InvalidPolynomial(
                "polynomial must contain at least one term",
            ));
        }
        if terms
            .iter()
            .flat_map(|term| term.variables.iter())
            .any(|&variable| variable >= num_vars)
        {
            return Err(SparseProofError::InvalidPolynomial(
                "monomial variable is outside the declared domain",
            ));
        }
        Ok(Self { num_vars, terms })
    }

    /// Number of variables in the Boolean domain.
    pub fn num_vars(&self) -> usize {
        self.num_vars
    }

    /// Canonical sparse monomials.
    pub fn terms(&self) -> &[SparseMonomial] {
        &self.terms
    }

    /// Number of nonzero terms.
    pub fn num_terms(&self) -> usize {
        self.terms.len()
    }

    /// Maximum monomial degree.
    pub fn max_degree(&self) -> usize {
        self.terms
            .iter()
            .map(|term| term.variables.len())
            .max()
            .unwrap_or(0)
    }

    /// Domain-separated BLAKE2b-256 commitment to the canonical polynomial.
    pub fn commitment(&self) -> TranscriptDigest {
        digest_committed_polynomial(self)
    }

    /// Encodes the polynomial into the canonical `PHSMv1` binary format.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(POLYNOMIAL_MAGIC);
        push_u64(&mut out, usize_word(self.num_vars));
        push_u64(&mut out, usize_word(self.terms.len()));
        for term in &self.terms {
            push_u64(&mut out, term.coefficient);
            push_u64(&mut out, usize_word(term.variables.len()));
            for &variable in &term.variables {
                push_u64(&mut out, usize_word(variable));
            }
        }
        out
    }

    /// Decodes and validates a canonical `PHSMv1` polynomial.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SparseProofError> {
        let mut reader = CertificateReader::new(bytes);
        if reader.take(8)? != POLYNOMIAL_MAGIC {
            return Err(SparseProofError::InvalidEncoding("bad polynomial magic"));
        }
        let num_vars = reader.usize()?;
        let num_terms = reader.usize()?;
        if num_terms > reader.remaining() / 24 {
            return Err(SparseProofError::InvalidEncoding(
                "polynomial term count exceeds input size",
            ));
        }
        let mut terms = Vec::with_capacity(num_terms);
        for _ in 0..num_terms {
            let coefficient = reader.u64()?;
            let degree = reader.usize()?;
            let mut variables = Vec::with_capacity(degree);
            for _ in 0..degree {
                variables.push(reader.usize()?);
            }
            if variables.windows(2).any(|window| window[0] >= window[1]) {
                return Err(SparseProofError::InvalidEncoding(
                    "polynomial variables are not strictly increasing",
                ));
            }
            terms.push(SparseMonomial::new(coefficient, variables)?);
        }
        if !reader.is_empty() {
            return Err(SparseProofError::InvalidEncoding(
                "trailing polynomial bytes",
            ));
        }
        Self::new(num_vars, terms)
    }
}

/// Verification result for a sparse certificate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SparseVerificationReport {
    /// Number of sum-check rounds replayed.
    pub rounds_verified: usize,
    /// Number of nonzero variable incidences in the sparse polynomial.
    pub term_incidences: usize,
    /// Final evaluation after all verifier challenges are fixed.
    pub final_evaluation: u64,
    /// Digest of the public polynomial derived from the seed.
    pub polynomial_digest: TranscriptDigest,
    /// Final hash-chain state of the Fiat-Shamir transcript.
    pub transcript_digest: TranscriptDigest,
}

/// Stable, self-contained seeded sparse sum-check certificate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeededSparseProof {
    /// Field modulus.
    pub p: u64,
    /// Public polynomial description.
    pub spec: SeededSparseSpec,
    /// Claimed Boolean-hypercube sum.
    pub claimed_sum: u64,
    /// Linear round polynomials `g_i(z) = a_i*z + b_i`.
    pub rounds: Vec<(u64, u64)>,
    /// Final polynomial evaluation.
    pub final_evaluation: u64,
    /// Digest of the expanded sparse polynomial.
    pub polynomial_digest: TranscriptDigest,
    /// Final transcript hash-chain state.
    pub transcript_digest: TranscriptDigest,
}

/// Sum-check certificate bound to a separately supplied sparse polynomial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedSparseProof {
    /// Field modulus.
    pub p: u64,
    /// Number of variables in the committed polynomial.
    pub num_vars: usize,
    /// Number of terms in the committed polynomial.
    pub num_terms: usize,
    /// Maximum monomial degree.
    pub max_degree: usize,
    /// BLAKE2b-256 commitment to the canonical `PHSMv1` bytes.
    pub polynomial_commitment: TranscriptDigest,
    /// Claimed Boolean-hypercube sum.
    pub claimed_sum: u64,
    /// Linear round polynomials `g_i(z) = a_i*z + b_i`.
    pub rounds: Vec<(u64, u64)>,
    /// Final polynomial evaluation.
    pub final_evaluation: u64,
    /// Final transcript hash-chain state.
    pub transcript_digest: TranscriptDigest,
}

/// Errors returned while decoding or verifying sparse certificates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SparseProofError {
    /// Certificate bytes are malformed.
    InvalidEncoding(&'static str),
    /// External polynomial data is invalid.
    InvalidPolynomial(&'static str),
    /// Certificate field modulus does not match the verifier field.
    FieldMismatch,
    /// Polynomial digest does not match the public seeded specification.
    PolynomialDigestMismatch,
    /// Claimed sum is inconsistent with the seeded polynomial.
    ClaimedSumMismatch,
    /// Round count does not match the number of variables.
    RoundCountMismatch,
    /// A round polynomial is inconsistent with the seeded polynomial.
    RoundMismatch(usize),
    /// Final evaluation is inconsistent with the folded polynomial.
    FinalEvaluationMismatch,
    /// Transcript digest does not match the replayed transcript.
    TranscriptDigestMismatch,
}

impl fmt::Display for SparseProofError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEncoding(reason) => write!(formatter, "invalid certificate: {reason}"),
            Self::InvalidPolynomial(reason) => write!(formatter, "invalid polynomial: {reason}"),
            Self::FieldMismatch => write!(formatter, "field modulus mismatch"),
            Self::PolynomialDigestMismatch => write!(formatter, "polynomial digest mismatch"),
            Self::ClaimedSumMismatch => write!(formatter, "claimed sum mismatch"),
            Self::RoundCountMismatch => write!(formatter, "round count mismatch"),
            Self::RoundMismatch(round) => write!(formatter, "round {round} mismatch"),
            Self::FinalEvaluationMismatch => write!(formatter, "final evaluation mismatch"),
            Self::TranscriptDigestMismatch => write!(formatter, "transcript digest mismatch"),
        }
    }
}

impl std::error::Error for SparseProofError {}

type SparseTerm = SparseMonomial;

#[derive(Debug, Clone, Copy)]
struct TermState {
    contribution: u64,
    next_round: usize,
}

#[derive(Debug)]
struct SparseExecution {
    claimed_sum: u64,
    rounds: Vec<(u64, u64)>,
    final_evaluation: u64,
    polynomial_digest: TranscriptDigest,
    transcript_digest: TranscriptDigest,
    term_incidences: usize,
}

#[derive(Debug, Clone)]
struct SparsePrng {
    seed: [u8; 32],
    counter: u64,
    buffer: [u8; 32],
    offset: usize,
}

impl SparsePrng {
    fn from_seed_bytes(seed: [u8; 32]) -> Self {
        Self {
            seed,
            counter: 0,
            buffer: [0u8; 32],
            offset: 32,
        }
    }

    fn refill(&mut self) {
        let mut hasher = Blake2b256::new();
        hasher.update(SPARSE_PRNG_DOMAIN);
        hasher.update(self.seed);
        hasher.update(self.counter.to_be_bytes());
        self.buffer.copy_from_slice(&hasher.finalize());
        self.counter = self.counter.wrapping_add(1);
        self.offset = 0;
    }

    fn next_u64(&mut self) -> u64 {
        if self.offset >= self.buffer.len() {
            self.refill();
        }
        let mut chunk = [0u8; 8];
        chunk.copy_from_slice(&self.buffer[self.offset..self.offset + 8]);
        self.offset += 8;
        u64::from_be_bytes(chunk)
    }

    fn gen_mod(&mut self, modulus: u64) -> u64 {
        self.next_u64() % modulus
    }
}

#[derive(Debug, Clone)]
struct HashChainTranscript {
    state: TranscriptDigest,
    counter: u64,
}

impl HashChainTranscript {
    fn new(
        p: u64,
        num_vars: usize,
        num_terms: usize,
        max_degree: usize,
        claimed_sum: u64,
        polynomial_digest: &TranscriptDigest,
    ) -> Self {
        let mut hasher = Blake2b256::new();
        absorb_bytes(&mut hasher, TRANSCRIPT_DOMAIN);
        hasher.update(p.to_be_bytes());
        hasher.update(usize_word(num_vars).to_be_bytes());
        hasher.update(usize_word(num_terms).to_be_bytes());
        hasher.update(usize_word(max_degree).to_be_bytes());
        hasher.update(claimed_sum.to_be_bytes());
        hasher.update(polynomial_digest);
        Self {
            state: finish_hash(hasher),
            counter: 0,
        }
    }

    fn round(&mut self, field: &Field, a: u64, b: u64) -> u64 {
        let mut challenge_hasher = Blake2b256::new();
        absorb_bytes(&mut challenge_hasher, CHALLENGE_DOMAIN);
        challenge_hasher.update(self.state);
        challenge_hasher.update(a.to_be_bytes());
        challenge_hasher.update(b.to_be_bytes());
        challenge_hasher.update(self.counter.to_be_bytes());
        let challenge_digest = finish_hash(challenge_hasher);
        let challenge = digest_mod_field(&challenge_digest, field);

        let mut response_hasher = Blake2b256::new();
        absorb_bytes(&mut response_hasher, RESPONSE_DOMAIN);
        response_hasher.update(challenge_digest);
        response_hasher.update(challenge.to_be_bytes());
        self.state = finish_hash(response_hasher);
        self.counter = self.counter.wrapping_add(1);
        challenge
    }
}

impl SeededSparseProof {
    /// Produces an event-driven proof without materializing the Boolean domain.
    pub fn prove(spec: SeededSparseSpec, field: &Field) -> Self {
        let execution = execute_sparse(&spec, field, None)
            .expect("internally generated sparse proof must be consistent");
        Self {
            p: field.modulus(),
            spec,
            claimed_sum: execution.claimed_sum,
            rounds: execution.rounds,
            final_evaluation: execution.final_evaluation,
            polynomial_digest: execution.polynomial_digest,
            transcript_digest: execution.transcript_digest,
        }
    }

    /// Replays and verifies every certificate round.
    pub fn verify(&self, field: &Field) -> Result<SparseVerificationReport, SparseProofError> {
        if self.p != field.modulus() {
            return Err(SparseProofError::FieldMismatch);
        }
        if self.rounds.len() != self.spec.num_vars {
            return Err(SparseProofError::RoundCountMismatch);
        }
        let execution = execute_sparse(&self.spec, field, Some(&self.rounds))?;
        if execution.polynomial_digest != self.polynomial_digest {
            return Err(SparseProofError::PolynomialDigestMismatch);
        }
        if execution.claimed_sum != self.claimed_sum {
            return Err(SparseProofError::ClaimedSumMismatch);
        }
        if execution.final_evaluation != self.final_evaluation {
            return Err(SparseProofError::FinalEvaluationMismatch);
        }
        if execution.transcript_digest != self.transcript_digest {
            return Err(SparseProofError::TranscriptDigestMismatch);
        }
        Ok(SparseVerificationReport {
            rounds_verified: self.rounds.len(),
            term_incidences: execution.term_incidences,
            final_evaluation: execution.final_evaluation,
            polynomial_digest: execution.polynomial_digest,
            transcript_digest: execution.transcript_digest,
        })
    }

    /// Encodes the certificate into a stable big-endian binary format.
    pub fn to_bytes(&self) -> Vec<u8> {
        let seed_len = usize_word(self.spec.seed.len());
        let round_count = usize_word(self.rounds.len());
        let mut out = Vec::with_capacity(128 + self.spec.seed.len() + self.rounds.len() * 16);
        out.extend_from_slice(CERTIFICATE_MAGIC);
        push_u64(&mut out, self.p);
        push_u64(&mut out, usize_word(self.spec.num_vars));
        push_u64(&mut out, usize_word(self.spec.num_terms));
        push_u64(&mut out, usize_word(self.spec.max_degree));
        push_u64(&mut out, seed_len);
        out.extend_from_slice(&self.spec.seed);
        push_u64(&mut out, self.claimed_sum);
        out.extend_from_slice(&self.polynomial_digest);
        push_u64(&mut out, round_count);
        for &(a, b) in &self.rounds {
            push_u64(&mut out, a);
            push_u64(&mut out, b);
        }
        push_u64(&mut out, self.final_evaluation);
        out.extend_from_slice(&self.transcript_digest);
        out
    }

    /// Decodes a stable binary certificate.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SparseProofError> {
        let mut reader = CertificateReader::new(bytes);
        if reader.take(8)? != CERTIFICATE_MAGIC {
            return Err(SparseProofError::InvalidEncoding("bad magic"));
        }
        let p = reader.u64()?;
        let num_vars = reader.usize()?;
        let num_terms = reader.usize()?;
        let max_degree = reader.usize()?;
        if num_vars == 0 || num_terms == 0 || max_degree == 0 || max_degree > num_vars {
            return Err(SparseProofError::InvalidEncoding(
                "invalid sparse specification",
            ));
        }
        let seed_len = reader.usize()?;
        if seed_len > reader.remaining() {
            return Err(SparseProofError::InvalidEncoding(
                "seed length exceeds input size",
            ));
        }
        let seed = reader.take(seed_len)?.to_vec();
        let claimed_sum = reader.u64()?;
        let polynomial_digest = reader.digest()?;
        let round_count = reader.usize()?;
        if round_count != num_vars {
            return Err(SparseProofError::RoundCountMismatch);
        }
        if round_count > reader.remaining().saturating_sub(40) / 16 {
            return Err(SparseProofError::InvalidEncoding(
                "round count exceeds input size",
            ));
        }
        let mut rounds = Vec::with_capacity(round_count);
        for _ in 0..round_count {
            rounds.push((reader.u64()?, reader.u64()?));
        }
        let final_evaluation = reader.u64()?;
        let transcript_digest = reader.digest()?;
        if !reader.is_empty() {
            return Err(SparseProofError::InvalidEncoding("trailing bytes"));
        }
        Ok(Self {
            p,
            spec: SeededSparseSpec::new(num_vars, num_terms, max_degree, seed),
            claimed_sum,
            rounds,
            final_evaluation,
            polynomial_digest,
            transcript_digest,
        })
    }
}

impl CommittedSparseProof {
    /// Produces a certificate bound to a separately supplied polynomial.
    pub fn prove(
        polynomial: &CommittedSparsePolynomial,
        field: &Field,
    ) -> Result<Self, SparseProofError> {
        let execution = execute_committed(polynomial, field, None)?;
        Ok(Self {
            p: field.modulus(),
            num_vars: polynomial.num_vars(),
            num_terms: polynomial.num_terms(),
            max_degree: polynomial.max_degree(),
            polynomial_commitment: execution.polynomial_digest,
            claimed_sum: execution.claimed_sum,
            rounds: execution.rounds,
            final_evaluation: execution.final_evaluation,
            transcript_digest: execution.transcript_digest,
        })
    }

    /// Verifies the certificate against separately supplied polynomial bytes.
    pub fn verify(
        &self,
        polynomial: &CommittedSparsePolynomial,
        field: &Field,
    ) -> Result<SparseVerificationReport, SparseProofError> {
        if self.p != field.modulus() {
            return Err(SparseProofError::FieldMismatch);
        }
        if self.num_vars != polynomial.num_vars()
            || self.num_terms != polynomial.num_terms()
            || self.max_degree != polynomial.max_degree()
        {
            return Err(SparseProofError::PolynomialDigestMismatch);
        }
        if self.rounds.len() != self.num_vars {
            return Err(SparseProofError::RoundCountMismatch);
        }
        if polynomial.commitment() != self.polynomial_commitment {
            return Err(SparseProofError::PolynomialDigestMismatch);
        }

        let execution = execute_committed(polynomial, field, Some(&self.rounds))?;
        if execution.claimed_sum != self.claimed_sum {
            return Err(SparseProofError::ClaimedSumMismatch);
        }
        if execution.final_evaluation != self.final_evaluation {
            return Err(SparseProofError::FinalEvaluationMismatch);
        }
        if execution.transcript_digest != self.transcript_digest {
            return Err(SparseProofError::TranscriptDigestMismatch);
        }
        Ok(SparseVerificationReport {
            rounds_verified: self.rounds.len(),
            term_incidences: execution.term_incidences,
            final_evaluation: execution.final_evaluation,
            polynomial_digest: execution.polynomial_digest,
            transcript_digest: execution.transcript_digest,
        })
    }

    /// Encodes the commitment-bound certificate as `PHCPv1`.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(136 + self.rounds.len() * 16);
        out.extend_from_slice(COMMITTED_CERTIFICATE_MAGIC);
        push_u64(&mut out, self.p);
        push_u64(&mut out, usize_word(self.num_vars));
        push_u64(&mut out, usize_word(self.num_terms));
        push_u64(&mut out, usize_word(self.max_degree));
        out.extend_from_slice(&self.polynomial_commitment);
        push_u64(&mut out, self.claimed_sum);
        push_u64(&mut out, usize_word(self.rounds.len()));
        for &(a, b) in &self.rounds {
            push_u64(&mut out, a);
            push_u64(&mut out, b);
        }
        push_u64(&mut out, self.final_evaluation);
        out.extend_from_slice(&self.transcript_digest);
        out
    }

    /// Decodes a `PHCPv1` commitment-bound certificate.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SparseProofError> {
        let mut reader = CertificateReader::new(bytes);
        if reader.take(8)? != COMMITTED_CERTIFICATE_MAGIC {
            return Err(SparseProofError::InvalidEncoding(
                "bad committed certificate magic",
            ));
        }
        let p = reader.u64()?;
        let num_vars = reader.usize()?;
        let num_terms = reader.usize()?;
        let max_degree = reader.usize()?;
        if num_vars == 0 || num_terms == 0 || max_degree == 0 || max_degree > num_vars {
            return Err(SparseProofError::InvalidEncoding(
                "invalid committed certificate metadata",
            ));
        }
        let polynomial_commitment = reader.digest()?;
        let claimed_sum = reader.u64()?;
        let round_count = reader.usize()?;
        if round_count != num_vars {
            return Err(SparseProofError::RoundCountMismatch);
        }
        if round_count > reader.remaining().saturating_sub(40) / 16 {
            return Err(SparseProofError::InvalidEncoding(
                "committed round count exceeds input size",
            ));
        }
        let mut rounds = Vec::with_capacity(round_count);
        for _ in 0..round_count {
            rounds.push((reader.u64()?, reader.u64()?));
        }
        let final_evaluation = reader.u64()?;
        let transcript_digest = reader.digest()?;
        if !reader.is_empty() {
            return Err(SparseProofError::InvalidEncoding(
                "trailing committed certificate bytes",
            ));
        }
        Ok(Self {
            p,
            num_vars,
            num_terms,
            max_degree,
            polynomial_commitment,
            claimed_sum,
            rounds,
            final_evaluation,
            transcript_digest,
        })
    }
}

fn execute_sparse(
    spec: &SeededSparseSpec,
    field: &Field,
    supplied_rounds: Option<&[(u64, u64)]>,
) -> Result<SparseExecution, SparseProofError> {
    let terms = derive_terms(spec, field);
    let polynomial_digest = digest_terms(spec, &terms);
    execute_terms(
        spec.num_vars,
        spec.max_degree,
        &terms,
        polynomial_digest,
        field,
        supplied_rounds,
    )
}

fn execute_committed(
    polynomial: &CommittedSparsePolynomial,
    field: &Field,
    supplied_rounds: Option<&[(u64, u64)]>,
) -> Result<SparseExecution, SparseProofError> {
    execute_terms(
        polynomial.num_vars,
        polynomial.max_degree(),
        &polynomial.terms,
        polynomial.commitment(),
        field,
        supplied_rounds,
    )
}

fn execute_terms(
    num_vars: usize,
    max_degree: usize,
    terms: &[SparseTerm],
    polynomial_digest: TranscriptDigest,
    field: &Field,
    supplied_rounds: Option<&[(u64, u64)]>,
) -> Result<SparseExecution, SparseProofError> {
    let term_incidences = terms.iter().map(|term| term.variables.len()).sum();
    let mut events = Vec::with_capacity(term_incidences);
    let mut states = Vec::with_capacity(terms.len());
    let mut claimed_sum = 0u64;

    for (term_id, term) in terms.iter().enumerate() {
        let coefficient = term.coefficient % field.modulus();
        if coefficient == 0 {
            return Err(SparseProofError::InvalidPolynomial(
                "monomial coefficient is zero in the selected field",
            ));
        }
        let exponent = num_vars - term.variables.len();
        let contribution = field.mul(coefficient, field.pow(2, usize_word(exponent)));
        claimed_sum = field.add(claimed_sum, contribution);
        states.push(TermState {
            contribution,
            next_round: 0,
        });
        for &variable in &term.variables {
            events.push((variable, term_id));
        }
    }
    events.sort_unstable();

    let mut transcript = HashChainTranscript::new(
        field.modulus(),
        num_vars,
        terms.len(),
        max_degree,
        claimed_sum,
        &polynomial_digest,
    );
    let inverse_two = field.inv(2);
    let mut inverse_two_powers = vec![1u64];
    let mut running_claim = claimed_sum;
    let mut rounds = Vec::with_capacity(num_vars);
    let mut event_cursor = 0usize;
    let mut active_terms = Vec::new();

    for round_idx in 0..num_vars {
        active_terms.clear();
        let mut a = 0u64;
        while event_cursor < events.len() && events[event_cursor].0 == round_idx {
            let term_id = events[event_cursor].1;
            let state = states[term_id];
            let gap = round_idx - state.next_round;
            ensure_power(&mut inverse_two_powers, gap, inverse_two, field);
            let current = field.mul(state.contribution, inverse_two_powers[gap]);
            a = field.add(a, current);
            active_terms.push((term_id, current));
            event_cursor += 1;
        }
        let b = field.mul(field.sub(running_claim, a), inverse_two);
        let expected_round = (a, b);
        if let Some(provided) = supplied_rounds {
            if provided.get(round_idx).copied() != Some(expected_round) {
                return Err(SparseProofError::RoundMismatch(round_idx));
            }
        }
        rounds.push(expected_round);

        let challenge = transcript.round(field, a, b);
        for &(term_id, current) in &active_terms {
            states[term_id] = TermState {
                contribution: field.mul(current, challenge),
                next_round: round_idx + 1,
            };
        }
        running_claim = field.add(b, field.mul(a, challenge));
    }

    let mut final_evaluation = 0u64;
    for state in states {
        let tail = num_vars - state.next_round;
        ensure_power(&mut inverse_two_powers, tail, inverse_two, field);
        final_evaluation = field.add(
            final_evaluation,
            field.mul(state.contribution, inverse_two_powers[tail]),
        );
    }
    if running_claim != final_evaluation {
        return Err(SparseProofError::FinalEvaluationMismatch);
    }

    Ok(SparseExecution {
        claimed_sum,
        rounds,
        final_evaluation,
        polynomial_digest,
        transcript_digest: transcript.state,
        term_incidences,
    })
}

fn derive_terms(spec: &SeededSparseSpec, field: &Field) -> Vec<SparseTerm> {
    let mut seed_hasher = Blake2b256::new();
    absorb_bytes(&mut seed_hasher, POLYNOMIAL_DOMAIN);
    seed_hasher.update(usize_word(spec.num_vars).to_be_bytes());
    seed_hasher.update(usize_word(spec.num_terms).to_be_bytes());
    seed_hasher.update(usize_word(spec.max_degree).to_be_bytes());
    absorb_bytes(&mut seed_hasher, &spec.seed);
    let mut prng = SparsePrng::from_seed_bytes(finish_hash(seed_hasher));

    let mut terms = Vec::with_capacity(spec.num_terms);
    for _ in 0..spec.num_terms {
        let degree = if spec.max_degree == 1 {
            1
        } else {
            2 + prng.gen_mod((spec.max_degree - 1) as u64) as usize
        };
        let coefficient = 1 + prng.gen_mod(field.modulus() - 1);
        let mut variables = Vec::with_capacity(degree);
        while variables.len() < degree {
            let candidate = prng.gen_mod(spec.num_vars as u64) as usize;
            if !variables.contains(&candidate) {
                variables.push(candidate);
            }
        }
        variables.sort_unstable();
        terms.push(SparseTerm {
            coefficient,
            variables,
        });
    }
    terms
}

fn digest_terms(spec: &SeededSparseSpec, terms: &[SparseTerm]) -> TranscriptDigest {
    let mut hasher = Blake2b256::new();
    absorb_bytes(&mut hasher, POLYNOMIAL_DOMAIN);
    hasher.update(usize_word(spec.num_vars).to_be_bytes());
    hasher.update(usize_word(spec.num_terms).to_be_bytes());
    hasher.update(usize_word(spec.max_degree).to_be_bytes());
    absorb_bytes(&mut hasher, &spec.seed);
    for term in terms {
        hasher.update(term.coefficient.to_be_bytes());
        hasher.update(usize_word(term.variables.len()).to_be_bytes());
        for &variable in &term.variables {
            hasher.update(usize_word(variable).to_be_bytes());
        }
    }
    finish_hash(hasher)
}

fn digest_committed_polynomial(polynomial: &CommittedSparsePolynomial) -> TranscriptDigest {
    let mut hasher = Blake2b256::new();
    absorb_bytes(&mut hasher, COMMITTED_POLYNOMIAL_DOMAIN);
    absorb_bytes(&mut hasher, &polynomial.to_bytes());
    finish_hash(hasher)
}

fn ensure_power(powers: &mut Vec<u64>, exponent: usize, base: u64, field: &Field) {
    while powers.len() <= exponent {
        let next = field.mul(*powers.last().expect("power table is never empty"), base);
        powers.push(next);
    }
}

fn digest_mod_field(digest: &TranscriptDigest, field: &Field) -> u64 {
    digest.iter().fold(0u64, |accumulator, &byte| {
        field.add(field.mul(accumulator, 256), byte as u64)
    })
}

fn absorb_bytes(hasher: &mut Blake2b256, bytes: &[u8]) {
    hasher.update(usize_word(bytes.len()).to_be_bytes());
    hasher.update(bytes);
}

fn finish_hash(hasher: Blake2b256) -> TranscriptDigest {
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&hasher.finalize());
    digest
}

fn usize_word(value: usize) -> u64 {
    u64::try_from(value).expect("value must fit in a certificate word")
}

fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_be_bytes());
}

struct CertificateReader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> CertificateReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], SparseProofError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(SparseProofError::InvalidEncoding("length overflow"))?;
        if end > self.bytes.len() {
            return Err(SparseProofError::InvalidEncoding("unexpected end"));
        }
        let slice = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(slice)
    }

    fn u64(&mut self) -> Result<u64, SparseProofError> {
        let mut word = [0u8; 8];
        word.copy_from_slice(self.take(8)?);
        Ok(u64::from_be_bytes(word))
    }

    fn usize(&mut self) -> Result<usize, SparseProofError> {
        usize::try_from(self.u64()?)
            .map_err(|_| SparseProofError::InvalidEncoding("size does not fit usize"))
    }

    fn digest(&mut self) -> Result<TranscriptDigest, SparseProofError> {
        let mut digest = [0u8; 32];
        digest.copy_from_slice(self.take(32)?);
        Ok(digest)
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enumerate_sum(spec: &SeededSparseSpec, field: &Field) -> u64 {
        let terms = derive_terms(spec, field);
        let mut total = 0u64;
        for assignment in 0..(1usize << spec.num_vars) {
            let mut value = 0u64;
            for term in &terms {
                if term
                    .variables
                    .iter()
                    .all(|&variable| (assignment >> variable) & 1 == 1)
                {
                    value = field.add(value, term.coefficient);
                }
            }
            total = field.add(total, value);
        }
        total
    }

    #[test]
    fn sparse_claim_matches_dense_enumeration() {
        let field = Field::new(1_000_000_007);
        let spec = SeededSparseSpec::new(8, 24, 5, b"dense-equivalence".to_vec());
        let proof = SeededSparseProof::prove(spec.clone(), &field);
        assert_eq!(proof.claimed_sum, enumerate_sum(&spec, &field));
        assert!(proof.verify(&field).is_ok());
    }

    #[test]
    fn sparse_certificate_roundtrip_and_tamper_rejection() {
        let field = Field::new(1_000_000_007);
        let spec = SeededSparseSpec::new(256, 128, 8, b"roundtrip".to_vec());
        let proof = SeededSparseProof::prove(spec, &field);
        let encoded = proof.to_bytes();
        let decoded = SeededSparseProof::from_bytes(&encoded).expect("certificate must decode");
        assert_eq!(decoded, proof);
        assert!(decoded.verify(&field).is_ok());

        let mut tampered = decoded;
        tampered.rounds[127].0 = field.add(tampered.rounds[127].0, 1);
        assert_eq!(
            tampered.verify(&field),
            Err(SparseProofError::RoundMismatch(127))
        );
    }

    #[test]
    fn sparse_certificate_handles_large_dimension_without_hypercube() {
        let field = Field::new(1_000_000_007);
        let spec = SeededSparseSpec::new(10_000, 512, 8, b"large-dimension".to_vec());
        let proof = SeededSparseProof::prove(spec, &field);
        let report = proof
            .verify(&field)
            .expect("large sparse proof must verify");
        assert_eq!(report.rounds_verified, 10_000);
        assert!(report.term_incidences >= 1024);
        assert_eq!(proof.rounds.len(), 10_000);
    }

    fn committed_fixture() -> CommittedSparsePolynomial {
        CommittedSparsePolynomial::new(
            16,
            vec![
                SparseMonomial::new(17, vec![0, 3, 9]).unwrap(),
                SparseMonomial::new(29, vec![1, 4]).unwrap(),
                SparseMonomial::new(41, vec![2, 5, 8, 13]).unwrap(),
                SparseMonomial::new(53, vec![6, 7, 10, 11, 15]).unwrap(),
            ],
        )
        .unwrap()
    }

    #[test]
    fn committed_polynomial_and_certificate_roundtrip() {
        let field = Field::new(1_000_000_007);
        let polynomial = committed_fixture();
        let polynomial_bytes = polynomial.to_bytes();
        let decoded_polynomial = CommittedSparsePolynomial::from_bytes(&polynomial_bytes).unwrap();
        assert_eq!(decoded_polynomial, polynomial);
        assert_eq!(decoded_polynomial.commitment(), polynomial.commitment());

        let proof = CommittedSparseProof::prove(&decoded_polynomial, &field).unwrap();
        let proof_bytes = proof.to_bytes();
        let decoded_proof = CommittedSparseProof::from_bytes(&proof_bytes).unwrap();
        let report = decoded_proof.verify(&decoded_polynomial, &field).unwrap();
        assert_eq!(report.rounds_verified, 16);
        assert_eq!(report.term_incidences, 14);
        assert_eq!(report.polynomial_digest, polynomial.commitment());
    }

    #[test]
    fn committed_certificate_rejects_external_workload_tampering() {
        let field = Field::new(1_000_000_007);
        let polynomial = committed_fixture();
        let proof = CommittedSparseProof::prove(&polynomial, &field).unwrap();
        let mut terms = polynomial.terms().to_vec();
        terms[2] = SparseMonomial::new(42, terms[2].variables().to_vec()).unwrap();
        let tampered = CommittedSparsePolynomial::new(polynomial.num_vars(), terms).unwrap();

        assert_eq!(
            proof.verify(&tampered, &field),
            Err(SparseProofError::PolynomialDigestMismatch)
        );
    }

    #[test]
    fn untrusted_lengths_are_rejected_before_allocation() {
        let mut polynomial = Vec::new();
        polynomial.extend_from_slice(POLYNOMIAL_MAGIC);
        push_u64(&mut polynomial, 8);
        push_u64(&mut polynomial, u64::MAX);
        assert!(matches!(
            CommittedSparsePolynomial::from_bytes(&polynomial),
            Err(SparseProofError::InvalidEncoding(_))
        ));

        let mut certificate = Vec::new();
        certificate.extend_from_slice(COMMITTED_CERTIFICATE_MAGIC);
        push_u64(&mut certificate, 1_000_000_007);
        push_u64(&mut certificate, 1_000_000_000);
        push_u64(&mut certificate, 1);
        push_u64(&mut certificate, 1);
        certificate.extend_from_slice(&[0u8; 32]);
        push_u64(&mut certificate, 0);
        push_u64(&mut certificate, 1_000_000_000);
        assert!(matches!(
            CommittedSparseProof::from_bytes(&certificate),
            Err(SparseProofError::InvalidEncoding(_))
        ));
    }
}

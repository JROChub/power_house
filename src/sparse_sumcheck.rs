//! Event-driven sum-check certificates for public seeded sparse polynomials.
//!
//! The polynomial family is a sum of square-free monomials:
//!
//! ```text
//! f(x_0, ..., x_{n-1}) = sum_t c_t * product_{j in S_t} x_j
//! ```
//!
//! A public seed deterministically defines the coefficients and supports
//! `S_t`. The prover and verifier process only the declared variables and the
//! nonzero term incidences. They never allocate the `2^n` Boolean hypercube.

use crate::{prng::SimplePrng, Field, TranscriptDigest};
use blake2::digest::{consts::U32, Digest};
use std::fmt;

type Blake2b256 = blake2::Blake2b<U32>;

const POLYNOMIAL_DOMAIN: &[u8] = b"power_house:v1:seeded-sparse-polynomial";
const TRANSCRIPT_DOMAIN: &[u8] = b"power_house:v1:sparse-sumcheck-transcript";
const CHALLENGE_DOMAIN: &[u8] = b"power_house:v1:sparse-sumcheck-challenge";
const RESPONSE_DOMAIN: &[u8] = b"power_house:v1:sparse-sumcheck-response";
const CERTIFICATE_MAGIC: &[u8; 8] = b"PHSPv1\0\0";

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

/// Verification result for a seeded sparse certificate.
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

/// Errors returned while decoding or verifying sparse certificates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SparseProofError {
    /// Certificate bytes are malformed.
    InvalidEncoding(&'static str),
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

#[derive(Debug, Clone)]
struct SparseTerm {
    coefficient: u64,
    variables: Vec<usize>,
}

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
struct HashChainTranscript {
    state: TranscriptDigest,
    counter: u64,
}

impl HashChainTranscript {
    fn new(
        p: u64,
        spec: &SeededSparseSpec,
        claimed_sum: u64,
        polynomial_digest: &TranscriptDigest,
    ) -> Self {
        let mut hasher = Blake2b256::new();
        absorb_bytes(&mut hasher, TRANSCRIPT_DOMAIN);
        hasher.update(p.to_be_bytes());
        hasher.update(usize_word(spec.num_vars).to_be_bytes());
        hasher.update(usize_word(spec.num_terms).to_be_bytes());
        hasher.update(usize_word(spec.max_degree).to_be_bytes());
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
        let seed = reader.take(seed_len)?.to_vec();
        let claimed_sum = reader.u64()?;
        let polynomial_digest = reader.digest()?;
        let round_count = reader.usize()?;
        if round_count != num_vars {
            return Err(SparseProofError::RoundCountMismatch);
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

fn execute_sparse(
    spec: &SeededSparseSpec,
    field: &Field,
    supplied_rounds: Option<&[(u64, u64)]>,
) -> Result<SparseExecution, SparseProofError> {
    let terms = derive_terms(spec, field);
    let polynomial_digest = digest_terms(spec, &terms);
    let term_incidences = terms.iter().map(|term| term.variables.len()).sum();
    let mut events = Vec::with_capacity(term_incidences);
    let mut states = Vec::with_capacity(terms.len());
    let mut claimed_sum = 0u64;

    for (term_id, term) in terms.iter().enumerate() {
        let exponent = spec.num_vars - term.variables.len();
        let contribution = field.mul(term.coefficient, field.pow(2, usize_word(exponent)));
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

    let mut transcript =
        HashChainTranscript::new(field.modulus(), spec, claimed_sum, &polynomial_digest);
    let inverse_two = field.inv(2);
    let mut inverse_two_powers = vec![1u64];
    let mut running_claim = claimed_sum;
    let mut rounds = Vec::with_capacity(spec.num_vars);
    let mut event_cursor = 0usize;
    let mut active_terms = Vec::new();

    for round_idx in 0..spec.num_vars {
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
        let tail = spec.num_vars - state.next_round;
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
    let mut prng = SimplePrng::from_seed_bytes(finish_hash(seed_hasher));

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
}

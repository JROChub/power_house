# Sparse Certificate Security Model

Status: active security statement for Power House v0.3.14 sparse formats.

This document defines what `PHSPv1`, `PHSMv1`, and `PHCPv1` establish and what
they do not establish.

## Statement

For a prime field `F_p`, the committed workflow represents a public sparse
multilinear polynomial

```text
f(x_0, ..., x_(n-1)) = sum_t c_t * product_(j in S_t) x_j
```

and records a deterministic sum-check transcript for its sum over `{0,1}^n`.
`PHSMv1` contains the canonical polynomial. `PHCPv1` contains its
domain-separated BLAKE2b-256 commitment, claimed sum, round messages, final
evaluation, and transcript digest.

## Current Verification Profile

The v1 verifier reads the complete public polynomial and deterministically
recomputes:

1. the polynomial commitment,
2. the Boolean-hypercube sum,
3. every expected round polynomial,
4. every Fiat-Shamir challenge,
5. the final evaluation and transcript digest.

Acceptance therefore means that the supplied files match the deterministic v1
derivation. It is a conformance and reproducibility check, not a succinct
delegation result.

For this polynomial representation, the claimed sum also has the closed form

```text
sum_t c_t * 2^(n - |S_t|) mod p
```

so a verifier that already reads every sparse term can calculate the statement
without the million-round certificate. The certificate is useful as a stable,
cross-language stress artifact, but it does not reduce verifier asymptotic work.

## Security Properties

- **Canonical binding:** changing canonical `PHSMv1` bytes changes the
  BLAKE2b-256 commitment except in the event of a hash collision.
- **Deterministic transcript integrity:** changing certificate metadata,
  rounds, final evaluation, or transcript state is rejected by full replay.
- **Prime-field enforcement:** Rust and Python now reject composite moduli
  using deterministic primality checks over the complete `u64` range.
- **Memory safety objective:** length fields are checked against available
  input before encoded collections are allocated. Decoders cap inputs at 16
  million variables, one million terms, one million variables per monomial, 1
  MiB seeds, and a 64 million-incidence work budget.
- **Cross-language conformance:** separate Rust and Python implementations parse and
  replay the stable formats.

## Non-Goals

The v1 formats do not provide:

- succinct verification,
- a polynomial commitment opening proof,
- zero knowledge or witness hiding,
- proof of arbitrary computation,
- protection from a malicious local verifier implementation,
- post-quantum security certification,
- an established novelty or world-first claim.

## Probabilistic Sum-Check Boundary

A conventional multilinear sum-check verifier checks round consistency and one
final polynomial evaluation instead of recomputing each expected round. Its
classical interactive soundness error is bounded by approximately `n / |F|`
for one repetition.

For the published parameters:

```text
n = 1,000,000
|F| = 1,000,000,007
error <= approximately 9.99999993e-4
security >= approximately 9.97 bits
```

That is not an acceptable cryptographic soundness target. BLAKE2b-derived
Fiat-Shamir challenges do not enlarge the field or remove this algebraic bound.
The estimate can be reproduced with:

```bash
python3 scripts/soundness_budget.py
```

Using the 64-bit prime `18446744073709551557` raises one-repetition security to
approximately 44.07 bits at one million rounds. Three independently
domain-separated repetitions would exceed a 128-bit classical union-bound
target, before accounting for the Fiat-Shamir transform's random-oracle model.

## Threats and Mitigations

| Threat | v1 treatment | Remaining limitation |
| --- | --- | --- |
| Workload substitution | Domain-separated hash commitment | Not a polynomial commitment |
| Certificate bit corruption | Exact replay and transcript digest | Depends on verifier correctness |
| Composite modulus | Deterministic primality rejection | Construction still panics on invalid Rust input |
| Length-based allocation abuse | Input-size checks before encoded allocations | Applications should still impose file-size limits |
| Common-mode implementation error | Rust/Python conformance and property tests | Both implementations remain project-authored |
| Malicious prover in standard sum-check | Not the v1 verification profile | Requires a v2 protocol and soundness budget |

## v2 Security Target

A research-grade successor must:

1. use a field/repetition plan with at least 128 bits of stated algebraic
   soundness,
2. separate prover logic from verifier logic,
3. check standard sum-check identities rather than exact prover replay,
4. bind final evaluation through a reviewed polynomial commitment or a clearly
   stated public-data oracle model,
5. domain-separate every repetition and protocol version,
6. publish malformed-input, differential, and external-reproduction results.

The project conformance corpus is in `conformance/v1`. Its manifest fixes
digests and expected outputs. Rust and Python require every single-byte XOR
mutation of the small canonical vectors to reject.

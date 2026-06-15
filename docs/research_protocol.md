# Research Protocol

Status: active research and evidence protocol for Power House v0.3.7.

This document turns Power-House development into a falsifiable research
program. Scale demonstrations remain useful, but exponent size alone is not a
research contribution.

## Baseline: v1 Deterministic Replay

The released `PHSMv1`/`PHCPv1` pair is the baseline:

- one million variables,
- 8,192 sparse monomials,
- 57,546 variable incidences,
- 16,000,128 certificate bytes,
- Rust and Python deterministic replay,
- no Boolean-hypercube allocation.

The baseline is a conformance benchmark. It is not the proposed novel result.

## Falsifiable v2 Claim

The working claim for investigation is:

> For a public or committed multilinear workload with one million logical
> variables, Power-House v2 provides at least 128 bits of stated algebraic
> soundness while reducing verifier access to the workload below full replay,
> with independently reproducible prover memory, proof size, and verification
> time.

This claim fails if any of the following is true:

- the verifier must read all workload terms,
- the final evaluation is not bound by a reviewed commitment/opening method,
- the soundness calculation is below 128 bits,
- the comparison excludes a materially faster or smaller established system,
- an unaffiliated implementation cannot reproduce acceptance and rejection
  vectors.

## Work Packages

### WP1: Specification

- Freeze canonical byte encodings and transcript domains.
- Specify field arithmetic, challenge sampling, and repetition separation.
- State completeness and soundness propositions.
- Publish parser resource limits.

### WP2: Differential Verification

- Maintain structurally separate Rust and Python verifiers.
- Publish valid and invalid conformance vectors.
- Run property tests against dense enumeration for small domains.
- Mutate every byte of small canonical artifacts and require rejection.
- Fuzz all untrusted decoders with allocation and timeout limits.

### WP3: Soundness Upgrade

- Evaluate the 64-bit prime `18446744073709551557`.
- Use at least three independently domain-separated repetitions for a
  one-million-round multilinear protocol if relying on the classical
  `n/|F|` bound.
- Prefer an extension field or reviewed PCS when it reduces proof size or gives
  a cleaner security argument.
- Treat Fiat-Shamir security separately from the interactive algebraic bound.

### WP4: Workload and Commitment

- Replace the closed-form sparse-monomial sum with a useful computation or
  streaming relation.
- Select a reviewed multilinear polynomial commitment.
- Measure verifier workload access, not only elapsed time.
- Document trusted setup, hiding, and post-quantum assumptions.

### WP5: Reproduction

- Produce machine-readable benchmark reports.
- Archive source, toolchains, vectors, and checksums.
- Obtain two unaffiliated reproductions.
- Submit the specification and claim to specialist review before publicity.

## Acceptance Matrix

| Gate | Required evidence |
| --- | --- |
| Correctness | Dense-equivalence property tests and cross-language vectors |
| Robustness | Mutation corpus, fuzzing, no decoder panic/OOM |
| Soundness | Explicit bound, field proof, repetitions, Fiat-Shamir model |
| Efficiency | Median and variance across pinned benchmark runs |
| Comparison | Same workload and security target against primary baselines |
| Independence | Unaffiliated implementation and public reproduction logs |
| Claim discipline | Prior-art review and exact falsifiable wording |

## Reproducible Commands

```bash
cargo run --example conformance_vectors
cargo test --test sparse_protocol
python3 scripts/test_sparse_verifier.py

python3 scripts/soundness_budget.py
python3 scripts/soundness_budget.py \
  --field 18446744073709551557 \
  --repetitions 3

python3 scripts/benchmark_sparse.py \
  --repeats 3 \
  --output target/research-benchmark.json
```

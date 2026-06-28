# Public Sparse Computation Certificate

Status: active format guide for Power House v0.3.13.

Power-House now includes an event-driven sum-check prover for a public seeded
sparse multilinear polynomial:

```text
f(x_0, ..., x_(n-1)) = sum_t c_t * product_(j in S_t) x_j
```

The public seed deterministically defines every nonzero coefficient `c_t` and
square-free support set `S_t`. The engine processes variable incidences rather
than allocating the `2^n` Boolean evaluation table.

## Million-Variable Artifact

Run:

```bash
cargo run --release --example sparse_record
```

Default parameters:

```text
variables:             1,000,000
Boolean domain:        2^1,000,000
domain decimal digits: 301,030
sparse terms:          8,192
maximum term degree:   12
term incidences:       57,093
certificate bytes:     16,000,171
```

Reference certificate:

```text
polynomial_digest:
8ce5e85d9946cbe3434dba2b34714f84dc3315a1827a8815e40fb6a3d4b1a9b6

transcript_digest:
70dcfd35bdea8ec037582f0e3b3a60a6f8df5e532ebc0821ef87f1f299bc12d5

SHA-256:
2b219ba189c3a38f1073c7797629e9aaf44a36820abb64c7628129480eb43f3b
```

The Rust example writes:

```text
target/power_house_sparse_record.phsp
```

## Independent Verifier

The repository includes a separate Python implementation using only the
standard library:

```bash
python3 scripts/verify_sparse_certificate.py \
  target/power_house_sparse_record.phsp
```

The separately implemented Python verifier:

1. decodes the stable `PHSPv1` binary format,
2. derives the sparse polynomial from the public seed,
3. recomputes the polynomial digest and claimed sum,
4. replays every sum-check round,
5. reconstructs Fiat-Shamir challenges,
6. checks the final evaluation and transcript digest.

## Complexity

Let `n` be the number of variables and `I` the number of variable incidences
across all nonzero terms.

- certificate size: `O(n)`
- prover transcript work: `O(n + I log n)`
- verifier transcript work: `O(n + I log n)`
- polynomial description: `O(I)`
- Boolean-domain allocation: none

The verifier remains linear in the number of rounds. It is independent of the
`2^n` domain size.

## Scope

This certificate verifies a public, compactly described polynomial. The
[`PHSMv1`/`PHCPv1` workflow](committed_workload.md) extends it to separately
stored external public data. Neither format yet provides a succinct
multilinear polynomial opening or a hidden-witness argument.

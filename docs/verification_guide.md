# Verification Guide

This guide reproduces the Power House v0.3.14 provenance and proof workflows.

## Requirements

- Rust stable with Cargo
- Python 3.10 or newer for the cross-language verifier
- approximately 100 MB of free disk space for generated certificates

Run all commands from the repository root.

## 1. Test the implementation

```bash
cargo fmt --check
cargo test --all-targets --locked
cargo test --all-targets --features net --locked
cargo clippy --all-targets --all-features -- -D warnings
python3 -m py_compile scripts/*.py
```

## 2. Verify `.pha` and Rootprint

Regenerate the canonical artifacts:

```bash
cargo run --example pha_conformance_vectors
cargo run --example identity_conformance_vectors
git diff --exit-code -- \
  conformance/pha-v1 \
  publicpower/artifacts/rootprint-valid.json
```

Run Rust and Python core-independence tests:

```bash
cargo test --test provenance_protocol --test rootprint_cli
cargo test --test identity_cli
PYTHONPATH=sdk/python python3 -m unittest discover -s sdk/python/tests -v
```

Run the library workflow:

```bash
cargo run --example rootprint_workflow
```

The tests require all Power House core mutations to reject. EPA payload
mutation must preserve `.pha` and Rootprint core validity while failing the
separate explicit EPA integrity check.

## 3. Verify the human-observable sidecar

Regenerate the independent `slbit` packet binding:

```bash
cargo run --example slbit_conformance_vectors
git diff --exit-code -- \
  conformance/slbit-v1 \
  publicpower/artifacts/luminous-valid.json
```

Run the isolation and CLI tests:

```bash
cargo test --test observatory_protocol --test observatory_cli
cargo run --example slbit_observatory

julian observatory verify \
  conformance/pha-v1/rootprint-valid.json \
  conformance/slbit-v1/observatory-valid.json
```

The final command verifies Power House core state first, then the optional
sidecar. Semantic mutation must reject sidecar integrity without changing the
underlying Rootprint verification result.

## 4. Verify more than one sextillion points

```bash
cargo run --release --example sextillion_verify
```

This proves the Boolean-hypercube sum of a constant polynomial over `2^70`
points. The proof and verifier each process 70 rounds.

## 5. Verify a seeded non-constant polynomial

```bash
cargo run --release --example hyperscale_affine
```

The public seed defines an affine multilinear polynomial over `2^4096` points.
The verifier derives the same coefficients, replays every Fiat-Shamir round,
and rejects a different seed or modified round.

## 6. Generate a million-round sparse certificate

```bash
cargo run --release --example sparse_record

python3 scripts/verify_sparse_certificate.py \
  target/power_house_sparse_record.phsp
```

The default certificate has one million rounds and describes a public seeded
sparse polynomial over `2^1,000,000` points.

## 7. Bind a proof to external data

```bash
cargo run --release --example committed_workload -- generate
cargo run --release --example committed_workload -- prove
cargo run --release --example committed_workload -- verify

python3 scripts/verify_sparse_certificate.py \
  target/external_interaction_model.phcp \
  --polynomial target/external_interaction_model.phsm
```

The `PHCPv1` proof commits to the separate `PHSMv1` workload. The Rust and
Python verifiers both require the exact workload bytes.

Run the differential mutation suite after generating both million-round
artifacts:

```bash
python3 scripts/test_sparse_verifier.py
```

The test consumes the canonical `conformance/v1` files, validates their
manifest, and requires rejection after XOR-mutating every individual byte.

## 8. Confirm tamper rejection

```bash
cp target/external_interaction_model.phsm /tmp/tampered.phsm
printf '\001' | dd of=/tmp/tampered.phsm bs=1 seek=40 count=1 conv=notrunc

python3 scripts/verify_sparse_certificate.py \
  target/external_interaction_model.phcp \
  --polynomial /tmp/tampered.phsm
```

The final command must fail. Unit tests also cover modified proof rounds,
wrong public seeds, malformed lengths, workload changes, and transcript digest
changes.

## Complexity and scope

For `n` variables and `I` sparse term incidences:

- proof size: `O(n)`
- prover work: `O(n + I log n)`
- verifier work: `O(n + I log n)`
- expanded `2^n` table: never allocated

The current external-data commitment is public and non-hiding. Verification
reads the complete sparse workload. It is not a succinct polynomial opening,
general virtual-machine proof, or hidden-witness argument.

The v1 verifier also recomputes every expected round from the public sparse
polynomial. This is deterministic conformance replay, not a conventional
probabilistic sum-check verifier. The current field and one-million-round count
would provide only approximately 9.97 bits under the classical one-repetition
`n/|F|` soundness bound:

```bash
python3 scripts/soundness_budget.py
```

## Reporting results

Record:

- Git commit and crate version
- `rustc --version` and `python3 --version`
- CPU model and memory
- generated file sizes and SHA-256 digests
- prover and verifier timings

External reproductions should publish these values independently rather than
copying the reference artifact metadata.

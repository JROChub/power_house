# power_house

[![CI](https://img.shields.io/github/actions/workflow/status/JROChub/power_house/ci.yml?branch=main&label=CI)](https://github.com/JROChub/power_house/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/power_house)](https://crates.io/crates/power_house)
[![docs.rs](https://img.shields.io/docsrs/power_house)](https://docs.rs/power_house)
[![license](https://img.shields.io/crates/l/power_house)](LICENSE)

`power_house` is a Rust verification stack for deterministic sum-check proofs,
commitment-bound sparse workloads, transcript anchoring, and optional quorum
networking.

## Verified Scale

The repository contains four reproducible proof modes:

| Mode | Public domain | Verifier work | Command |
| --- | ---: | ---: | --- |
| Constant polynomial | `2^70` points | `O(70)` | `cargo run --release --example sextillion_verify` |
| Seeded affine polynomial | `2^4096` points | `O(4096)` | `cargo run --release --example hyperscale_affine` |
| Seeded sparse polynomial | `2^1,000,000` points | `O(n + I log n)` | `cargo run --release --example sparse_record` |
| External committed sparse polynomial | `2^1,000,000` points | `O(n + I log n)` | `cargo run --release --example committed_workload` |

Here `n` is the number of variables and `I` is the number of nonzero variable
incidences. None of these modes allocates the expanded Boolean hypercube.

The external workflow stores the polynomial and proof separately:

- `PHSMv1`: canonical sparse polynomial
- `PHCPv1`: proof containing a domain-separated BLAKE2b-256 commitment

Verification requires both files and rejects workload substitution.

## Install

```bash
cargo add power_house
cargo install power_house --features net
```

## Reproduce

```bash
cargo test --all-targets
cargo test --all-targets --features net

cargo run --release --example sextillion_verify
cargo run --release --example hyperscale_affine
cargo run --release --example sparse_record
cargo run --release --example committed_workload

python3 scripts/verify_sparse_certificate.py \
  target/power_house_sparse_record.phsp

python3 scripts/verify_sparse_certificate.py \
  target/external_interaction_model.phcp \
  --polynomial target/external_interaction_model.phsm

python3 scripts/test_sparse_verifier.py
python3 scripts/soundness_budget.py
```

The full procedure, formats, expected outputs, and failure tests are in
[Verification Guide](docs/verification_guide.md).

Small canonical files in `conformance/v1` are checked by both languages. Every
single-byte XOR mutation of each vector must reject.

## Library

```rust
use power_house::{Field, GeneralSumProof};

let field = Field::new(1_000_000_007);
let proof = GeneralSumProof::prove_seeded_affine(
    4096,
    &field,
    b"public reproducible workload",
);

assert!(proof.verify_seeded_affine(
    &field,
    b"public reproducible workload",
));
```

Primary APIs:

- `GeneralSumProof`: dense, streaming, constant, and seeded-affine sum-check
- `SeededSparseProof`: stable `PHSPv1` seeded sparse certificates
- `CommittedSparsePolynomial`: canonical external sparse workloads
- `CommittedSparseProof`: stable `PHCPv1` commitment-bound certificates
- `ProofLedger`: transcript logs, anchors, and quorum reconciliation

## Network

The `net` feature enables the `julian` CLI, libp2p transport, signed envelopes,
data availability endpoints, governance policies, stake accounting, and token
migration commands.

```bash
julian keygen ed25519://<seed> --out ./keys/node.identity

julian net start \
  --node-id <node_id> \
  --log-dir ./logs/<node_id> \
  --listen /ip4/0.0.0.0/tcp/0 \
  --bootstrap /dns4/mfenx.com/tcp/7002/p2p/<PEER_ID> \
  --quorum 2 \
  --key ed25519://<seed>
```

The native wallet lane accepts signed EIP-1559 transfers and exposes only
quorum-finalized blocks, balances, nonces, transactions, and receipts:

```bash
julian net start \
  --node-id <node_id> \
  --log-dir ./logs/<node_id> \
  --blob-dir ./data/<node_id> \
  --listen /ip4/0.0.0.0/tcp/7001 \
  --policy ./config/native-validators.json \
  --quorum 2 \
  --evm-chain-id 177155 \
  --evm-rpc-listen 127.0.0.1:8545 \
  --key ed25519://<seed>
```

Run `scripts/test_native_rpc_cluster.sh` to verify three independent replicas
produce the same finalized block hash, state root, balances, and receipt.
Use `scripts/generate_rpc_cluster.py` to create a sealed quorum-2 production
bundle for three validators. The production runbook provisions the validators
and global HTTPS edge on DigitalOcean.

Operations and migration procedures are documented in
[Operations](docs/ops.md) and [Mainnet Launch](docs/mainnet_launch.md).

## Documentation

- [Verification Guide](docs/verification_guide.md)
- [JULIAN Protocol](JULIAN_PROTOCOL.md)
- [Committed Workload Format](docs/committed_workload.md)
- [Million-Round Sparse Certificate](docs/sparse_record.md)
- [Hyperscale Seeded-Affine Proof](docs/hyperscale_proof.md)
- [Prior-Art Review](docs/prior_art_review.md)
- [Sparse Certificate Security Model](docs/security_model.md)
- [Research Protocol](docs/research_protocol.md)
- [Orbital Observatory](docs/orbital_observatory.md)
- [Operations](docs/ops.md)
- [RPC Operations](docs/rpc_operations.md)
- [Production RPC Deployment](docs/production_rpc_deployment.md)

## License

MIT OR BSD-2-Clause.

# Power House

[![CI](https://img.shields.io/github/actions/workflow/status/JROChub/power_house/ci.yml?branch=main&label=CI)](https://github.com/JROChub/power_house/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/power_house)](https://crates.io/crates/power_house)
[![docs.rs](https://img.shields.io/docsrs/power_house)](https://docs.rs/power_house)
[![license](https://img.shields.io/crates/l/power_house)](LICENSE)

Power House is a deterministic verification and provenance system for portable
computational identities.

Power House 0.3.13 introduces Memory Capsules: self-verifying proof-memory
objects that bind `.pha` artifacts, Rootprint lineage, replay state, optional
witness receipts, challenge vectors, and non-core semantic packets into one
offline-verifiable bundle.

`slbit` is the independent semantic layer: it shows what verified proof memory
means without changing core proof identity.

Current release: **v0.3.13**

Production reliability evidence is published on the dedicated
[72-hour campaign page](https://mfenx.com/campaign.html).

The primary workflow is **Power House Identity + Rootprint**:

- **Identity** provides immutable create, fork, merge, verify, replay, and
  equivalence operations over `.pha` and Rootprint.
- **Power House Archive (`.pha`)** binds proof data and provenance to a
  deterministic `phx_fingerprint`.
- **Rootprint** provides verifiable navigation, forks, merges, and equivalence
  over `.pha` core identities.
- **External proof attachments (EPA)** are optional transport data and remain
  outside the Power House core fingerprint and Rootprint branch identity.
- **Observatory sidecars** optionally bind human-readable semantic packets to
  verified Rootprint replay state without changing proof identity.

## Quick Start

```bash
cargo add power_house
```

```rust
use power_house::{prove_with_rootprint, provenance::PhaArtifact};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let artifact = PhaArtifact::new(
        json!({"producer": "example"}),
        "power-house/example/v1",
        json!({"claim": 7}),
        json!({"accepted": true}),
    )?;

    let graph = prove_with_rootprint!(
        label: "main",
        artifact: artifact,
    )?;
    graph.verify()?;
    Ok(())
}
```

Install the `julian` CLI with the network feature:

```bash
cargo install power_house --features net
```

The primary identity commands are:

```bash
julian identity create main.pha --label main \
  --identity-output main.identity.json \
  --rootprint-output proof.rootprint.json
julian identity verify main.identity.json proof.rootprint.json
julian identity replay main.identity.json proof.rootprint.json
```

Create a portable proof-memory capsule:

```bash
julian memory create \
  --pha main.pha \
  --rootprint proof.rootprint.json \
  --sidecar proof.observatory.json \
  --output earth-001.phm
julian memory verify earth-001.phm
julian memory replay earth-001.phm
julian memory challenge earth-001.phm --all
```

## Human-Observable Proofs

[`slbit`](https://crates.io/crates/slbit) is an independent zero-dependency
crate for luminous claims, semantic transcripts, and visualization packets.
Power House remains the verification and provenance authority; `slbit` adds
human-readable meaning beside it.

```bash
cargo add power_house
cargo add slbit
cargo run --example slbit_observatory
```

The optional sidecar can be verified offline:

```bash
julian observatory verify \
  proof.rootprint.json \
  proof.observatory.json
```

See the [Power House + slbit Observatory guide](docs/slbit.md) for the complete
Rust workflow, schemas, trust boundary, browser rendering, and conformance
vectors.

## Verification Profiles

| Profile | Public statement | Verifier path | Reproduce |
| --- | --- | --- | --- |
| Constant sum-check | `2^70` Boolean points | 70 field rounds | `cargo run --release --example sextillion_verify` |
| Seeded affine sum-check | `2^4096` Boolean points | 4,096 field rounds | `cargo run --release --example hyperscale_affine` |
| Seeded sparse certificate | `2^1,000,000` Boolean points | `O(n + I log n)` deterministic replay | `cargo run --release --example sparse_record` |
| Committed sparse workload | External `PHSMv1` + `PHCPv1` files | Commitment-bound deterministic replay | `cargo run --release --example committed_workload` |
| Portable provenance | `.pha` core + Rootprint DAG | Fingerprint and graph replay | `cargo run --example rootprint_workflow` |

Here `n` is the number of variables and `I` is the number of nonzero variable
incidences. The proof modes operate on compact algebraic descriptions and do
not allocate the expanded Boolean hypercube.

## Core Formats

| Format | Purpose |
| --- | --- |
| `.pha` v1 | Portable proof, public inputs, provenance, and core fingerprint |
| Rootprint v1 | Deterministic proof-history graph with forks and merges |
| `.phm` Memory Capsule v1 | Portable proof memory with core, lineage, replay, semantic bindings, witnesses, and challenges |
| Observatory sidecar v1 | Non-core binding from replay state and branch IDs to semantic packets |
| `PHSPv1` | Seeded sparse polynomial certificate |
| `PHSMv1` | Canonical external sparse polynomial |
| `PHCPv1` | Certificate bound to a `PHSMv1` commitment |

Rust and Python consume the same canonical vectors under `conformance/`.
Mutation tests require core changes to reject while proving that EPA mutation
does not alter Power House core validity.

## Reproduce

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
cargo test --all-targets --locked
cargo test --all-targets --features net --locked
cargo test --test memory_capsule --test memory_cli --locked

cargo run --example pha_conformance_vectors
cargo run --example slbit_conformance_vectors
cargo run --example rootprint_workflow
cargo run --example slbit_observatory
cargo run --release --example sextillion_verify
cargo run --release --example hyperscale_affine
cargo run --release --example sparse_record
cargo run --release --example committed_workload

PYTHONPATH=sdk/python python3 -m unittest discover -s sdk/python/tests -v
python3 scripts/test_sparse_verifier.py
python3 scripts/test_observatory_contract.py
python3 scripts/soundness_budget.py
```

The complete procedure and expected rejection behavior are documented in the
[Verification Guide](docs/verification_guide.md).

## Primary Rust APIs

- [`Identity`](https://docs.rs/power_house/latest/power_house/identity/struct.Identity.html):
  immutable `.pha` and Rootprint identity abstraction.
- [`PhaArtifact`](https://docs.rs/power_house/latest/power_house/provenance/pha/struct.PhaArtifact.html):
  portable Power House core identity.
- [`Rootprint`](https://docs.rs/power_house/latest/power_house/provenance/rootprint/struct.Rootprint.html):
  deterministic proof-history branching and verification.
- [`ObservatorySidecar`](https://docs.rs/power_house/latest/power_house/observatory/struct.ObservatorySidecar.html):
  non-core semantic packet binding to verified Rootprint replay state.
- [`MemoryCapsule`](https://docs.rs/power_house/latest/power_house/memory/struct.MemoryCapsule.html):
  portable proof memory with replay and challenge verification.
- [`MemoryCapsuleBuilder`](https://docs.rs/power_house/latest/power_house/memory/struct.MemoryCapsuleBuilder.html):
  safe construction interface for `.phm` bundles.
- [`prove_with_rootprint!`](https://docs.rs/power_house/latest/power_house/macro.prove_with_rootprint.html):
  recommended provenance-aware construction interface.
- [`GeneralSumProof`](https://docs.rs/power_house/latest/power_house/sumcheck/struct.GeneralSumProof.html):
  dense, streaming, constant, and seeded-affine sum-check.
- [`SeededSparseProof`](https://docs.rs/power_house/latest/power_house/sparse_sumcheck/struct.SeededSparseProof.html):
  stable `PHSPv1` certificates.
- [`CommittedSparsePolynomial`](https://docs.rs/power_house/latest/power_house/sparse_sumcheck/struct.CommittedSparsePolynomial.html)
  and
  [`CommittedSparseProof`](https://docs.rs/power_house/latest/power_house/sparse_sumcheck/struct.CommittedSparseProof.html):
  external workload binding.
- [`ProofLedger`](https://docs.rs/power_house/latest/power_house/julian/struct.ProofLedger.html):
  transcript logs, anchors, and quorum reconciliation.
- [`ValidatorRegistry`](https://docs.rs/power_house/latest/power_house/net/validator_registry/struct.ValidatorRegistry.html):
  signed identity admission and monitoring discovery records.

## Python SDK

The bundled zero-dependency Python SDK defaults to pure Power House + Rootprint:

```python
from power_house import create_artifact, new_rootprint, verify_rootprint

artifact = create_artifact(
    {"source": "python"},
    "power-house/example/v1",
    {"claim": 7},
    {"accepted": True},
)
graph = new_rootprint("main", artifact)
verify_rootprint(graph)
```

EPA helpers require an explicit secondary import:

```python
from power_house.external import attach_external_proof
```

See [SDKs](docs/sdk.md) for installation and interoperability tests.

## Network And RPC

The optional `net` feature enables libp2p transport, signed envelopes, data
availability services, governance policies, stake accounting, migration tools,
and a quorum-finalized native JSON-RPC lane.

| Public network | Value |
| --- | --- |
| RPC name | **LAX MFENX RPC** |
| Chain ID | `177155` (`0x2b403`) |
| Canonical endpoint | `https://rpc.mfenx.com` |
| ChainList endpoint | `https://rpc.mfenx.com` |
| Status | `https://mfenx.com/status.html` |

The production edge uses health-aware global routing across validators in
New York, San Francisco, and Amsterdam. Public traffic is rate-limited at
Nginx and removed from a backend automatically when `/healthz` fails. Signed
validator registrations bind each admitted public key to its derived peer ID
and live identity metrics; validator totals are not inferred from peer links.

```bash
julian net start \
  --node-id validator-1 \
  --log-dir ./logs/validator-1 \
  --blob-dir ./data/validator-1 \
  --listen /ip4/0.0.0.0/tcp/7001 \
  --policy ./configs/governance.stake.json \
  --quorum 2 \
  --evm-chain-id 177155 \
  --evm-rpc-listen 127.0.0.1:8545 \
  --key ed25519://<seed>
```

Use `scripts/test_native_rpc_cluster.sh` to verify replica finality and
`scripts/check_rpc.py` to run the external publication gate.

## Documentation

Start with the [Documentation Index](docs/README.md).

- [Identity Layer](docs/identity.md)
- [Power House + slbit Observatory](docs/slbit.md)
- [Power House Archive v1](docs/pha_spec.md)
- [Rootprint v1](docs/rootprint.md)
- [Provenance Security Model](docs/provenance_security.md)
- [Verification Guide](docs/verification_guide.md)
- [SDKs](docs/sdk.md)
- [JULIAN Protocol](JULIAN_PROTOCOL.md)
- [Sparse Security Model](docs/security_model.md)
- [RPC Operations](docs/rpc_operations.md)
- [Production RPC Deployment](docs/production_rpc_deployment.md)
- [Stable Public Network Roadmap](docs/network_roadmap.md)
- [Signed Validator Registry](docs/validator_registry.md)
- [Node Operator Guide](docs/node_operator.md)
- [Incident Response](docs/incident_response.md)
- [Load Testing](docs/load_testing.md)
- [Testnet to Mainnet](docs/testnet_mainnet.md)
- [Orbital Observatory](docs/orbital_observatory.md)
- [v0.3.0 Benchmark Report](benchmarks/v0.3.0/report.json)

## Public Surfaces

- API documentation: <https://docs.rs/power_house>
- Package: <https://crates.io/crates/power_house>
- Repository: <https://github.com/JROChub/power_house>
- Public verifier: <https://mfenx.com>
- Network status: <https://mfenx.com/status.html>

## License

Power House v0.3.6 and later is licensed under
[AGPL-3.0-only](LICENSE). Earlier releases retain their original licenses; see
[LICENSE-CHANGE.md](LICENSE-CHANGE.md).

# Power House

[![CI](https://img.shields.io/github/actions/workflow/status/JROChub/power_house/ci.yml?branch=main&label=CI)](https://github.com/JROChub/power_house/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/power_house)](https://crates.io/crates/power_house)
[![docs.rs](https://img.shields.io/docsrs/power_house)](https://docs.rs/power_house)
[![license](https://img.shields.io/crates/l/power_house)](LICENSE)

Power House is a deterministic verification and provenance system for portable
computational identities.

Power House includes the opt-in Sovereign Fractal Computation Substrate (SFCS):
source text maps directly into deterministic computational-fractal graphs,
executable dense integer and memory traces replay into digest-bound `.pha`
artifacts, VM executions can be recorded through a deterministic RV32I trace,
and synthesis plans record connected sub-fractal regions that route to the
Sovereign fast path. Rootprint v1 and existing `.pha` identity rules remain
unchanged.

The SFCS objective is to make direct source-to-fractal execution the native
Power House path so traditional circuit compilers and zkVM workflows become
unnecessary and unwise as the default for the workloads Power House targets.
The current release is a guarded milestone toward that objective, not final
SFCS compliance.

The release also retains Memory Capsules: self-verifying proof-memory objects
that bind `.pha` artifacts, Rootprint lineage, replay state, optional witness
receipts, challenge vectors, and non-core semantic packets into one
offline-verifiable bundle.

`slbit` is the independent semantic layer: it shows what verified proof memory
means without changing core proof identity.

Current release: **v0.3.19**

Production reliability evidence is published on the dedicated
[72-hour campaign page](https://mfenx.com/campaign.html).

The primary workflow is **Power House Identity + Rootprint**:

- **Identity** provides immutable create, fork, merge, verify, replay, and
  equivalence operations over `.pha` and Rootprint.
- **Power House Archive (`.pha`)** binds proof data and provenance to a
  deterministic `phx_fingerprint`.
- **Rootprint** provides verifiable navigation, forks, merges, and equivalence
  over `.pha` core identities.
- **SFCS draft primitives** are opt-in through `--features sfcs` and provide
  direct fractal parsing, dense integer and memory execution traces, synthesis
  plans, a deterministic RV32I VM execution foundation, public VM transition
  constraint proofs with memory/range coverage, broader Rust-subset,
  LLVM-style SSA, and WASM-style stack compiler paths, the first
  privacy-preserving private-add and private-VM proof profiles, verifier-side
  private linear transition checks, zero-knowledge u32 range proofs for
  committed VM values, an offline `julian sfcs` CLI, and `.pha` embedding
  verification without mutating Rootprint v1.
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
Power House remains the verification and provenance authority; `slbit` 3.1 adds
the Meaning Observatory inspection layer, deterministic ask reports, and
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

Run the opt-in SFCS VM foundation:

```bash
cargo install power_house --features sfcs
julian sfcs vm-run rv32i.program.json \
  --inputs rv32i.inputs.json \
  --artifact-output rv32i.execution.pha \
  --report rv32i.report.json
julian sfcs verify-vm-pha rv32i.execution.pha
```

Prove public VM transition, memory consistency, and range-check coverage:

```bash
julian sfcs vm-constraints rv32i.program.json \
  --inputs rv32i.inputs.json \
  --artifact-output rv32i.constraints.pha \
  --report rv32i.constraints.report.json
julian sfcs verify-vm-constraints-pha rv32i.constraints.pha
```

Compile broader public source frontends directly into SFCS graphs:

```bash
julian sfcs rust-public score.rs --graph-output score.graph.json
julian sfcs llvm-ir score.ll --graph-output score-llvm.graph.json
julian sfcs wasm-stack score.wasmstack --graph-output score-wasm.graph.json
```

The VM foundation is not a complete zkVM release. The real zero-knowledge
privacy layer for arbitrary private VM execution and the full unrestricted
Rust/LLVM/WASM compiler family are release-gated until they are implemented,
audited, and tested end to end
without changing `.pha` or Rootprint identity rules. The `sfcs-zk` feature
currently provides two auditable privacy milestones: a constrained Rust-subset
`u32 + u32 -> u32` compiler with a private no-overflow RV32I add proof, and a
general private-VM profile that hides private inputs and trace data while
  publishing public outputs, digest commitments, verifier-side homomorphic
  transition checks for linear/no-overflow VM relations, zero-knowledge u32 range
  proofs for those committed VM values, private read-after-write memory
  consistency proofs, private memory access/register value binding proofs, and
  equality-branch proofs for covered `beq`/`bne` cases, and constraint
  coverage.

```bash
cargo install power_house --features sfcs-zk
julian sfcs rust-private-add private_add.rs \
  --lhs-value 144 \
  --rhs-value 233 \
  --lhs-blinding 1111111111111111111111111111111111111111111111111111111111111111 \
  --rhs-blinding 2222222222222222222222222222222222222222222222222222222222222222 \
  --artifact-output private-add.pha \
  --rootprint-output private-add.rootprint.json \
  --sidecar-output private-add.observatory.json \
  --capsule-output private-add.phm \
  --report private-add.report.json
julian sfcs verify-zk-pha private-add.pha
julian memory verify private-add.phm
```

```bash
julian sfcs zk-private-vm rv32i.program.json \
  --witness private-vm.witness.json \
  --artifact-output private-vm.pha \
  --report private-vm.report.json
julian sfcs verify-zk-pha private-vm.pha
```

See the [SFCS zkVM gate](docs/sfcs_zkvm.md).

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
| SFCS executable draft | Computational fractal source, trace, and synthesis plan committed through `.pha` | Graph digest, execution trace replay, synthesis-plan replay, Rootprint-safe bridge | `cargo test --features sfcs --test sfcs --test sfcs_cli` |

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
| `power-house/sfcs-fractal/v1-draft` | Opt-in computational-fractal draft graph |
| `power-house/sfcs-execution/v1-draft` | Opt-in graph + trace + synthesis plan committed through `.pha` |
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
cargo test --features sfcs --test sfcs --locked
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
- [`SfcsGraph`](https://docs.rs/power_house/latest/power_house/sfcs/struct.SfcsGraph.html):
  opt-in computational-fractal draft graph behind the `sfcs` feature, including
  `SfcsGraph::from_source(...)` for native expression-to-fractal lowering.
- [`SfcsExecutionTrace`](https://docs.rs/power_house/latest/power_house/sfcs/struct.SfcsExecutionTrace.html):
  deterministic executable trace for the SFCS source-to-fractal subset.
- [`SfcsSynthesisPlan`](https://docs.rs/power_house/latest/power_house/sfcs/struct.SfcsSynthesisPlan.html):
  deterministic fast-path extraction and dense-boundary synthesis plan.
- [`verify_sfcs_pha_embedding`](https://docs.rs/power_house/latest/power_house/fn.verify_sfcs_pha_embedding.html):
  explicit SFCS `.pha` embedding verifier.
- [`verify_sfcs_execution_embedding`](https://docs.rs/power_house/latest/power_house/fn.verify_sfcs_execution_embedding.html):
  explicit SFCS graph, trace, synthesis, output, and invariant verifier.
- [`SfcsVmConstraintProof`](https://docs.rs/power_house/latest/power_house/struct.SfcsVmConstraintProof.html):
  transparent public VM transition, memory consistency, range coverage, and
  execution-fractal proof object.
- [`compile_private_add_source`](https://docs.rs/power_house/latest/power_house/fn.compile_private_add_source.html):
  constrained Rust-subset compiler for the first `sfcs-zk` private-add profile.
- [`compile_public_rust_source`](https://docs.rs/power_house/latest/power_house/fn.compile_public_rust_source.html):
  public Rust-subset compiler that lowers multi-parameter expressions directly
  into SFCS graphs.
- [`compile_llvm_ir_source`](https://docs.rs/power_house/latest/power_house/fn.compile_llvm_ir_source.html):
  deterministic LLVM-style SSA subset compiler for i32 arithmetic,
  comparisons, `select`, and explicit returns into SFCS graphs.
- [`compile_wasm_stack_source`](https://docs.rs/power_house/latest/power_house/fn.compile_wasm_stack_source.html):
  deterministic WASM-style stack IR compiler that lowers i32 stack operations
  directly into SFCS graphs.
- [`SfcsZkPrivateAddProof`](https://docs.rs/power_house/latest/power_house/struct.SfcsZkPrivateAddProof.html):
  first privacy-preserving SFCS proof profile, proving committed private add
  inputs match a public output without exposing the inputs.
- [`SfcsZkPrivateVmProof`](https://docs.rs/power_house/latest/power_house/struct.SfcsZkPrivateVmProof.html):
  general private VM proof profile for supported RV32I executions, hiding
  private inputs and trace data while binding public outputs, digest
  commitments, linear transition proofs, u32 range proofs, private memory
  consistency proofs, private memory value binding proofs, equality-branch
  proofs, and coverage counters.
- [`SfcsZkPrivateVmLinearRelationProof`](https://docs.rs/power_house/latest/power_house/struct.SfcsZkPrivateVmLinearRelationProof.html):
  homomorphic verifier-side proof for private `add`, `addi`, `sub`, `subi`,
  and no-overflow public-scale VM relations.
- [`SfcsZkPrivateVmRangeProof`](https://docs.rs/power_house/latest/power_house/struct.SfcsZkPrivateVmRangeProof.html):
  zero-knowledge 32-bit bit-decomposition and recomposition proof for committed
  private VM values.
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
- [SFCS Draft](docs/sfcs.md)
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

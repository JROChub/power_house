# Power House Documentation

This index is the authoritative map for Power House v0.4.1 documentation.

## Start Here

| Document | Purpose |
| --- | --- |
| [Repository README](../README.md) | Installation, architecture, examples, and public links |
| [Verification Guide](verification_guide.md) | Reproduce proof modes, conformance vectors, and rejection tests |
| [Identity Layer](identity.md) | Immutable identity API, CLI, `.pha` binding, verification, and replay |
| [Memory Capsule v1](memory_capsule.md) | Portable `.phm` proof memory, replay, semantic binding, and challenge vectors |
| [Truth Boundary](truth_boundary.md) | What is core proof, what is semantic explanation, and what must not be overclaimed |
| [Power House + slbit Observatory](slbit.md) | Independent semantic packets, non-core sidecar binding, CLI, and browser workflow |
| [SFCS](sfcs.md) | Native fractal execution, public replay, transactional Origin, and whole-program private proof architecture |
| [SFCS Whole-Program Private VM](sfcs_zkvm.md) | RISC Zero proof boundary, deterministic identity, CLI, capsule verification, and scoped compatibility profiles |
| [Power House Archive v1](pha_spec.md) | Normative `.pha` core identity and optional EPA format |
| [Rootprint v1](rootprint.md) | Normative branching, merging, navigation, and graph verification |
| [Provenance Security Model](provenance_security.md) | Integrity boundary, assumptions, mutation behavior, and EPA isolation |
| [SDKs](sdk.md) | Rust and Python interfaces and cross-language interoperability |
| [JULIAN Protocol](../JULIAN_PROTOCOL.md) | Transcript anchoring, quorum reconciliation, and network architecture |
| [Stable Public Network Roadmap](network_roadmap.md) | Current deployment limits, retired-chain evidence, and remaining decentralization work |

## Proof Systems And Formats

- [Sextillion-Scale Certificate](sextillion_proof.md)
- [Hyperscale Seeded-Affine Proof](hyperscale_proof.md)
- [Million-Round Sparse Certificate](sparse_record.md)
- [Committed Sparse Workload](committed_workload.md)
- [Sparse Certificate Security Model](security_model.md)
- [Research Protocol](research_protocol.md)
- [Prior-Art Review](prior_art_review.md)

## Operations

- [Production RPC Deployment](production_rpc_deployment.md): current network identity checks and retained deployment design
- [Incident Response](incident_response.md)
- [Load Testing](load_testing.md)
- [Testnet to Mainnet](testnet_mainnet.md)
- [Orbital Observatory](orbital_observatory.md)

Operational commands must be tested against the release identified at the top
of each active guide. Secrets, access tokens, private keys, and unredacted
production configuration must never be committed.

MFENX Native 2026 uses chain `2026092601` and separately maintained validator
runtime `20261008.3`. Its three validator processes share one host; public
observer and validator enrollment is disabled. Retained chain `177155` tooling
does not join or replace that deployment. Current status and wallet setup are
at [mfenx.com/status.html](https://mfenx.com/status.html).

## Benchmarks And Conformance

- [`conformance/pha-v1`](../conformance/pha-v1): `.pha` and Rootprint vectors
- [`conformance/identity-v1`](../conformance/identity-v1): identity and replay vectors
- [`conformance/slbit-v1`](../conformance/slbit-v1): semantic sidecar and Rootprint binding vectors
- Memory Capsule conformance is covered by `cargo test --test memory_capsule`
  and `cargo test --test memory_cli`.
- [`conformance/v1`](../conformance/v1): sparse proof vectors
- [`benchmarks/v0.3.0/report.json`](../benchmarks/v0.3.0/report.json):
  measured v0.3.0 report
- [`benchmarks/v0.4.1/verified-observatory.json`](../benchmarks/v0.4.1/verified-observatory.json):
  one measured workload comparing independent and reused graph verification
- [`benchmarks/v0.4.1/wallet-finality-observation.json`](../benchmarks/v0.4.1/wallet-finality-observation.json):
  one finalized zero-value transaction observed through both current endpoints
- [`benchmarks/v0.4.1/dependency-audit-scope.json`](../benchmarks/v0.4.1/dependency-audit-scope.json):
  dependency updates, remaining advisories, and assessed scope
- [`benchmarks/README.md`](../benchmarks/README.md): benchmark reproduction

Timing reports are environment-specific measurements. Protocol complexity and
verification scope are defined in the corresponding specifications.

## Historical Material

The following documents describe retired deployments or retained tooling.
They are not authority for the current public validator deployment:

- [Retained RPC Operations](rpc_operations.md)
- [Retained Network Operations](ops.md)
- [Retained Node Operator Guide](node_operator.md)
- [Retained Signed Validator Registry](validator_registry.md)
- [Retained Public Observer Registry](observer_registry.md)
- [Retired 72-Hour Reliability Campaign](reliability_campaign.md)
- [Power-House Protocol Manual v0.1.57](book_of_power.md)
- [Training Binder v0.1.54](training_binder.md)
- [v0.1.54 Design Plan](roadmap_v0.1.54.md)
- [Legacy Mainnet Launch Guide v0.1.54](mainnet_launch.md)
- [Legacy Community Onboarding](community_onboarding.md)
- [Legacy Permissionless Join Guide](permissionless_join.md)
- [Legacy Tokenomics Notes](tokenomics.md)
- [Legacy Uptime Bounty Policy](bounty_policy.md)
- [Legacy Promotion Pack](promotion_pack.md)

For current behavior, use the source code, conformance vectors, CI workflow,
and active documents listed above.

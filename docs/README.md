# Power House Documentation

This index is the authoritative map for Power House v0.3.17 documentation.

## Start Here

| Document | Purpose |
| --- | --- |
| [Repository README](../README.md) | Installation, architecture, examples, and public links |
| [Verification Guide](verification_guide.md) | Reproduce proof modes, conformance vectors, and rejection tests |
| [Identity Layer](identity.md) | Immutable identity API, CLI, `.pha` binding, verification, and replay |
| [Memory Capsule v1](memory_capsule.md) | Portable `.phm` proof memory, replay, semantic binding, and challenge vectors |
| [Truth Boundary](truth_boundary.md) | What is core proof, what is semantic explanation, and what must not be overclaimed |
| [Power House + slbit Observatory](slbit.md) | Independent semantic packets, non-core sidecar binding, CLI, and browser workflow |
| [SFCS Draft](sfcs.md) | Experimental computational-fractal design gate and opt-in draft primitives |
| [Power House Archive v1](pha_spec.md) | Normative `.pha` core identity and optional EPA format |
| [Rootprint v1](rootprint.md) | Normative branching, merging, navigation, and graph verification |
| [Provenance Security Model](provenance_security.md) | Integrity boundary, assumptions, mutation behavior, and EPA isolation |
| [SDKs](sdk.md) | Rust and Python interfaces and cross-language interoperability |
| [JULIAN Protocol](../JULIAN_PROTOCOL.md) | Transcript anchoring, quorum reconciliation, and network architecture |
| [Stable Public Network Roadmap](network_roadmap.md) | Production topology, completion evidence, and remaining decentralization work |
| [Signed Validator Registry](validator_registry.md) | Identity-bound validator enrollment, health reconciliation, and dynamic monitoring discovery |
| [Public Observer Registry](observer_registry.md) | Permissionless observer enrollment, identity health checks, and public peer telemetry |

## Proof Systems And Formats

- [Sextillion-Scale Certificate](sextillion_proof.md)
- [Hyperscale Seeded-Affine Proof](hyperscale_proof.md)
- [Million-Round Sparse Certificate](sparse_record.md)
- [Committed Sparse Workload](committed_workload.md)
- [Sparse Certificate Security Model](security_model.md)
- [Research Protocol](research_protocol.md)
- [Prior-Art Review](prior_art_review.md)

## Operations

- [RPC Operations](rpc_operations.md)
- [Production RPC Deployment](production_rpc_deployment.md)
- [Network Operations](ops.md)
- [Node Operator Guide](node_operator.md)
- [Incident Response](incident_response.md)
- [Load Testing](load_testing.md)
- [72-Hour Reliability Campaign](reliability_campaign.md)
- [Testnet to Mainnet](testnet_mainnet.md)
- [Orbital Observatory](orbital_observatory.md)

Operational commands must be tested against the release identified at the top
of each active guide. Secrets, access tokens, private keys, and unredacted
production configuration must never be committed.

## Benchmarks And Conformance

- [`conformance/pha-v1`](../conformance/pha-v1): `.pha` and Rootprint vectors
- [`conformance/identity-v1`](../conformance/identity-v1): identity and replay vectors
- [`conformance/slbit-v1`](../conformance/slbit-v1): semantic sidecar and Rootprint binding vectors
- Memory Capsule conformance is covered by `cargo test --test memory_capsule`
  and `cargo test --test memory_cli`.
- [`conformance/v1`](../conformance/v1): sparse proof vectors
- [`benchmarks/v0.3.0/report.json`](../benchmarks/v0.3.0/report.json):
  measured v0.3.0 report
- [`benchmarks/README.md`](../benchmarks/README.md): benchmark reproduction

Timing reports are environment-specific measurements. Protocol complexity and
verification scope are defined in the corresponding specifications.

## Historical Material

The following documents are retained for traceability and are not the current
release specification or deployment authority:

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

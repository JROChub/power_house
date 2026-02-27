# power_house

[![git](https://img.shields.io/badge/git-power__house-6f2da8?logo=github&logoColor=white)](https://github.com/JROChub/power_house)
[![tests](https://img.shields.io/github/actions/workflow/status/JROChub/power_house/ci.yml?label=tests&logo=github&logoColor=white&color=39ff14)](https://github.com/JROChub/power_house/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/power_house?label=crates.io&color=blue)](https://crates.io/crates/power_house)
[![docs.rs](https://img.shields.io/docsrs/power_house?label=docs.rs)](https://docs.rs/power_house)
[![license](https://img.shields.io/crates/l/power_house?label=license)](LICENSE)

`power_house` is a Rust protocol stack for deterministic proof generation, quorum state reconciliation, and stake-aware settlement.

Maintainer: **MFENX LLC**

## What It Delivers

- Deterministic multilinear sum-check proofs with transcript hashing.
- Quorum finality and anchor reconciliation for auditable state progression.
- Optional P2P networking via `julian net ...` with policy gates.
- DA commitments with `share_root` and `pedersen_root` support.
- Stake registry, slashing evidence, and fee distribution controls.

## Live References

- Website: https://mfenx.com
- Repository: https://github.com/JROChub/power_house
- Protocol spec: `JULIAN_PROTOCOL.md`
- Operations runbook: `docs/ops.md`
- Launch guidance: `docs/mainnet_launch.md`

## Install

```bash
cargo install power_house
cargo install power_house --features net
```

## Build And Test

```bash
cargo test
cargo test --features net
```

## Local Quick Start

```bash
cargo run --example demo
cargo run --example scale_sumcheck
```

## Network Mode (`--features net`)

Generate an identity and start a node:

```bash
julian keygen ed25519://<seed> --out ./keys/node.identity

julian net start \
  --node-id <node_id> \
  --log-dir ./logs/<node_id> \
  --listen /ip4/0.0.0.0/tcp/0 \
  --bootstrap /dns4/mfenx.com/tcp/7002/p2p/<BOOTSTRAP_PEER_ID> \
  --broadcast-interval 5000 \
  --quorum 2 \
  --key ed25519://<seed>
```

Common flags:

- `--metrics :9100`
- `--policy configs/governance.stake.json`
- `--policy-allowlist configs/governance.multisig.json`
- `--allow-open-membership`
- `--gossip-shard 1`
- `--bft --bft-round-ms 5000`
- `--token-mode <native|TOKEN_ID>`
- `--token-oracle <RPC_URL>`

## DA HTTP API

Endpoints:

- `POST /submit_blob`
- `GET /commitment/<namespace>/<hash>`
- `GET /sample/<namespace>/<hash>?count=N`
- `GET /prove_storage/<namespace>/<hash>/<idx>`

Example:

```bash
curl -X POST http://127.0.0.1:8181/submit_blob \
  -H 'X-Namespace: default' \
  -H 'X-Fee: 10' \
  --data-binary @file.bin
```

## Governance And Staking

Use explicit governance policy files under `configs/`.

- Stake-backed DA attestation and slashing write evidence to `evidence_outbox.jsonl`.
- Stake registry balances are updated deterministically by command handlers.
- Open membership is opt-in (`--allow-open-membership`).

## Token Migration Workflow

The migration workflow is deterministic and idempotent.

Freeze mutable ingress during migration:

```bash
export PH_MIGRATION_MODE=freeze
```

Run finalize pipeline:

```bash
julian migration finalize \
  --registry ./path/to/stake_registry.json \
  --height 12345 \
  --log-dir ./logs/nodeA \
  --output-dir ./migration-out \
  --token-contract native://julian \
  --conversion-ratio 1 \
  --treasury-mint 0 \
  --amount-source total
```

Validate state:

```bash
julian migration verify-state \
  --registry ./path/to/stake_registry.json \
  --claims ./migration-out/migration_claims.json \
  --state ./migration-out/migration_apply_state.json \
  --require-complete
```

Run packaged checks:

```bash
./scripts/token_migration_dry_run.sh
./scripts/verify_migration_contract.sh
./scripts/smoke_net.sh --with-migration
```

## Documentation

- `JULIAN_PROTOCOL.md`
- `docs/book_of_power.md`
- `docs/ops.md`
- `docs/permissionless_join.md`
- `docs/community_onboarding.md`
- `docs/tokenomics.md`

## Notes On Freshness

This README intentionally avoids hard-coded release/version statements that go stale.
Use Git tags, the releases page, and CI status for current build/version truth.

## License

`power_house` is dual-licensed under MIT OR BSD-2-Clause. See `LICENSE`.

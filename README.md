# Power-House

[![git](https://img.shields.io/badge/git-power__house-6f2da8?logo=github&logoColor=white)](https://github.com/JROChub/power_house)
[![tests](https://img.shields.io/github/actions/workflow/status/JROChub/power_house/ci.yml?label=tests&logo=github&logoColor=white&color=39ff14)](https://github.com/JROChub/power_house/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/power_house?label=crates.io&color=blue)](https://crates.io/crates/power_house)
[![docs.rs](https://img.shields.io/docsrs/power_house?label=docs.rs)](https://docs.rs/power_house)
[![license](https://img.shields.io/crates/l/power_house?label=license)](LICENSE)

power_house provides deterministic sum-check proofs, transcript logging, and quorum-ledger tooling in Rust. The `julian` CLI adds an optional P2P layer and data-availability (DA) services.

Author: lexluger
Last update: 02/18/2026

Operations docs: `docs/book_of_power.md` (protocol manual) and `docs/ops.md` (runbook).

## Install

```
cargo install power_house
cargo install power_house --features net
```

## Tests

```
cargo test
cargo test --features net
```

## Features

- Deterministic multilinear sum-check proofs with transcript hashing.
- Quorum finality and anchor reconciliation for audit-friendly ledgers.
- Optional P2P gossipsub networking (`julian net ...`) with policy gates.
- Data-availability (DA) commitments with `share_root` and `pedersen_root`.
- Stake-backed governance, slashing evidence, and fee splitting.

## Local quick start

```
cargo run --example demo
cargo run --example scale_sumcheck
```

## Network mode (feature `net`)

Generate a deterministic identity and start a node:

```
julian keygen ed25519://<seed> --out ./keys/node.identity
julian net start \
  --node-id <your_name> \
  --log-dir ./logs/<your_name> \
  --listen /ip4/0.0.0.0/tcp/0 \
  --bootstrap /ip4/137.184.33.2/tcp/7001/p2p/12D3KooWLASw1JVBdDFNATYDJMbAn69CeWieTBLxAKaN9eLEkh3q \
  --bootstrap /ip4/146.190.126.101/tcp/7002/p2p/12D3KooWRLM7PJrtjRM6NZPX8vmdu4YGJa9D6aPoEnLcE1o6aKCd \
  --broadcast-interval 5000 \
  --quorum 2 \
  --key ed25519://<seed>
```

Optional flags: `--metrics :9100`, `--policy governance.json`, `--gossip-shard 1`, `--bft --bft-round-ms 5000`.

## DA commitments (dual roots)

`POST /submit_blob` returns both `share_root` (legacy) and `pedersen_root` (ZK-friendly). Sampling and storage proofs expose `pedersen_root` plus `pedersen_proof` for rollup verification.

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

## Governance and staking

Use `--policy governance.json` for static, stake, or multisig membership. Sample descriptors live in `configs/`. Stake-backed DA attestation and slashing write evidence to `evidence_outbox.jsonl` and update the registry balances.

## Operations

The runbook in `docs/ops.md` includes systemd templates, environment layout, health checks, and backup timers. Keep deterministic seeds and unit files in a private infra repo or secrets manager.

## Genesis (pinned)

The A2 testnet ledger is frozen to these domain-separated BLAKE2b-256 digests:

```
statement: JULIAN::GENESIS          hash: 139f1985df5b36dae23fa509fb53a006ba58e28e6dbb41d6d71cc1e91a82d84a
statement: Dense polynomial proof   hash: ded75c45b3b7eedd37041aae79713d7382e000eb4d83fab5f6aca6ca4d276e8c
statement: Hash anchor proof        hash: c72413466b2f76f1471f2e7160dadcbf912a4f8bc80ef1f2ffdb54ecb2bb2114
```

Verify an anchor from local logs:

```
julian node run mynode ./logs/mynode mynode.anchor.txt
julian node reconcile ./logs/mynode boot1.anchor.txt 2
```

## License

power_house is dual-licensed under MIT OR BSD-2-Clause. See `LICENSE`.

# Power-House

[![git](https://img.shields.io/badge/git-JROChub%2Fpower__house-6f2da8?logo=github&logoColor=white)](https://github.com/JROChub/power_house)
[![tests](https://img.shields.io/github/actions/workflow/status/JROChub/power_house/ci.yml?label=tests&logo=github&logoColor=white&color=39ff14)](https://github.com/JROChub/power_house/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/power_house?label=crates.io&color=blue)](https://crates.io/crates/power_house)
[![docs.rs](https://img.shields.io/docsrs/power_house?label=docs.rs)](https://docs.rs/power_house)
[![license](https://img.shields.io/crates/l/power_house?label=license)](LICENSE)

power_house delivers deterministically reproducible multilinear sum-check proofs, deterministic PRNG wiring, and quorum ledger tooling for transparent transcript auditing—implemented end-to-end in Rust.

Author: lexluger
Last update: 02/08/2026

Need the full operations guide?
Read the Power-House Protocol Manual (docs/book_of_power.md) and the VPS/network runbook (docs/ops.md).

## DA commitments (dual roots)

- Blob ingest (`POST /submit_blob`) now returns both `share_root` (legacy) and `pedersen_root` for ZK circuits.
- Commitments, sampling, and storage proofs expose `pedersen_root` plus `pedersen_proof` (siblings as hex) for clients.
- Rollup verifiers must use `pedersen_root` + `pedersen_proof`; legacy `share_root` remains for light clients.

## Evidence, gating, and settlement (stake-aware)

- Anchors are gated on DA commitments with stake-weighted attestation QC; QC files are persisted per blob.
- Availability faults enqueue evidence to `evidence_outbox.jsonl`, get gossiped, and trigger slashing via the stake registry.
- Blob fees are debited from the payer, credited to the operator, and split with attestors; DA attestors get a reward when QC is persisted.
- Rollup settlement helpers return fault evidence on verification failure and can split fees between operator/attesters (`settle_rollup_with_rewards`). Faults are appended to `evidence_outbox.jsonl` (default: sibling to the stake registry if `--outbox` not provided, or `evidence_outbox.jsonl` under the blob service base_dir).
- Evidence handling rejects senders not permitted by membership policy and applies slashing on validated evidence. Stake registry accounts track `{balance, stake, slashed}`; fees debit `balance`, rewards credit `balance`, bonding moves `balance -> stake`, slashing zeroes stake and marks `slashed`.

## Quick Join (Public Net)

```
cargo install power_house --features net
# optional: export a deterministic identity (ed25519://your-seed) or use an encrypted identity file
julian net start \
  --node-id <your_name> \
  --log-dir ./logs/<your_name> \
  --listen /ip4/0.0.0.0/tcp/0 \
  --bootstrap /dns4/boot1.jrocnet.com/tcp/7001/p2p/12D3KooWLASw1JVBdDFNATYDJMbAn69CeWieTBLxAKaN9eLEkh3q \
  --bootstrap /dns4/boot2.jrocnet.com/tcp/7002/p2p/12D3KooWRLM7PJrtjRM6NZPX8vmdu4YGJa9D6aPoEnLcE1o6aKCd \
  --broadcast-interval 5000 \
  --quorum 2 \
  --key ed25519://<seed>
```

Optional Prometheus metrics: add `--metrics :9100` (or another port) when starting a node.
Optional governance: add `--policy governance.json` to enforce a membership policy.

## Quick API / CLI examples

- Submit a blob (HTTP):
  ```bash
  curl -X POST http://127.0.0.1:8181/submit_blob \
    -H 'X-Namespace: default' \
    -H 'X-Fee: 10' \
    --data-binary @file.bin
  ```
- Fetch a commitment:
  ```bash
  curl http://127.0.0.1:8181/commitment/default/<hash>
  ```
- Sample shares:
  ```bash
  curl "http://127.0.0.1:8181/sample/default/<hash>?count=2"
  ```
- Prove storage for a share:
  ```bash
  curl http://127.0.0.1:8181/prove_storage/default/<hash>/0
  ```
- Stake registry (CLI):
  ```bash
  julian stake show /path/to/registry.json
  julian stake fund /path/to/registry.json <pubkey_b64> 1000
  julian stake bond /path/to/registry.json <pubkey_b64> 500
  ```
- Rollup settlement (CLI):
  ```bash
  julian rollup settle /path/to/registry.json default <share_root> <payer_b64> 1000 optimistic
  # with ZK proof files:
  julian rollup settle /path/to/registry.json default <share_root> <payer_b64> 1000 zk \
    --proof=proof.bin --public-inputs=inputs.bin --merkle-path=path.bin
  ```

## Identity Governance (descriptor-driven)

Supply `--policy governance.json` to load a governance descriptor (static allowlist, referenced file, stake-backed, or multisig).
Legacy: `--policy-allowlist allow.json` with base64 ed25519 keys still works.
Checkpoints: add `--checkpoint-interval 100` to emit signed anchor checkpoints every 100 broadcasts under `./logs/<node>/checkpoints`.

Sample descriptor (`--policy`):

```json
{
  "backend": "static",
  "allowlist": [
    "mbnfAp950/gQfEPc2J27MEvc+TPkY65/AJ6Xs0NjYew=",
    "5o2IL90EOYBUPvXMgCwFoo94UDYe9mAvZBCAwtasJ+I="
  ]
}
```

Multisig descriptors point to a state file containing:
`{"threshold":2,"signers":[...],"members":[...]}`.
The helper verifies that at least K authorized signers approve a rotation before writing the updated membership to disk.

Encrypted identity file (instead of `--key`):
Create a file containing the base64 result of XORing your 32-byte secret key with the first 32 bytes of SHA-512(passphrase), then:

```
julian net start --identity /path/to/file
```

You’ll be prompted for the passphrase at startup.

Bootstrap multiaddrs:

* `/dns4/boot1.jrocnet.com/tcp/7001/p2p/12D3KooWLASw1JVBdDFNATYDJMbAn69CeWieTBLxAKaN9eLEkh3q`
* `/dns4/boot2.jrocnet.com/tcp/7002/p2p/12D3KooWRLM7PJrtjRM6NZPX8vmdu4YGJa9D6aPoEnLcE1o6aKCd`

`boot1.jrocnet.com` and `boot2.jrocnet.com` resolve to the current public ingress addresses.
Update DNS—not this README—if underlying IPs move.

Local smoke test (two-node quorum; ports 7211/7212):

```
scripts/smoke_net.sh
```

## Data Availability API (HTTP)

Start the blob service with `--blob-dir` and `--blob-listen :8181` (plus optional `--blob-policy policy.json`). Endpoints:
- `POST /submit_blob` (headers: `X-Namespace`, optional `X-Fee`, `X-Publisher`) body = raw bytes. Returns share_root, hash, shard counts.
- `GET /commitment/<namespace>/<hash>` returns commitment metadata + attestations.
- `GET /sample/<namespace>/<hash>?count=N` returns sampled shares + Merkle proofs.
- `GET /prove_storage/<namespace>/<hash>/<idx>` proves a specific share (slashes publisher if missing).

Anchors carry `da_commitments` (namespace, blob_hash, share_root) and are rejected unless QC attestation quorum is met.

Stake registry tooling: manage balances/stake with `julian stake show|fund|bond|unbond|reward <registry.json> ...`; the blob server debits fees, shares operator/attestor rewards (bps per namespace), and slashing writes evidence to `evidence.jsonl`/`evidence_outbox.jsonl`.

## Operations Toolkit

See `docs/ops.md` for the full runbook (systemd templates, env layout, healthcheck/backup timers). Use `infra/ops_hosts.example.toml` as a starting point for host metadata. Package and deploy the `julian` binary with your preferred tooling (scp/rsync). If systemd is unavailable, use a custom restart command.

## Genesis Anchor (Pinned)

The A2 testnet ledger is frozen to these statements and domain-separated BLAKE2b-256 digests (hex).
Every node should reproduce these values from its local logs:

```
statement: JULIAN::GENESIS          hash: 139f1985df5b36dae23fa509fb53a006ba58e28e6dbb41d6d71cc1e91a82d84a
statement: Dense polynomial proof   hash: ded75c45b3b7eedd37041aae79713d7382e000eb4d83fab5f6aca6ca4d276e8c
statement: Hash anchor proof        hash: c72413466b2f76f1471f2e7160dadcbf912a4f8bc80ef1f2ffdb54ecb2bb2114
```

Boot nodes run with deterministic seeds (`ed25519://boot1-seed`, `ed25519://boot2-seed`) so libp2p Peer IDs remain constant.

## Verify Your Anchor

```
# Produce an anchor file from your local logs
julian node run mynode ./logs/mynode mynode.anchor.txt

# Inspect and compare to pinned digests above
cat mynode.anchor.txt

# Reconcile against a published anchor (example with boot1)
julian node reconcile ./logs/mynode boot1.anchor.txt 2
```

Recreate the bootstrap anchors:

```
julian node run boot1 ./logs/boot1 boot1.anchor.txt
julian node run boot2 ./logs/boot2 boot2.anchor.txt

julian node reconcile ./logs/boot1 boot2.anchor.txt 2
julian node reconcile ./logs/boot2 boot1.anchor.txt 2
```

Note: On systemd-managed hosts using the provided template, logs are under `/var/lib/jrocnet/<node>/logs` (for example, `/var/lib/jrocnet/boot1/logs`). Adjust the commands accordingly on your VPS.

## License

power_house is dual-licensed under either of:

* MIT License
* BSD 2-Clause License

See `LICENSE` for the full text.

## Overview

power_house explores interactive proof systems, finite-field arithmetic, and deterministic pseudo-randomness in pure Rust with a focus on reproducibility. It emulates the sum-check protocol, demonstrates a quorum finality primitive, and backs the JULIAN Protocol ledger.

## Motivation

Interactive proof techniques underpin modern verifiable compute and consensus. This crate distills those ideas into a standalone lab for experimentation, verification, and operations. It provides transcript-anchored proofs, a deterministic audit path, and a quorum primitive for anchor finality.

## Features

* **Finite Field Arithmetic**
  Prime-mod arithmetic via `Field`. Deterministic, no external deps.

* **Sum-Check Protocol Demo**
  Prover certifies polynomial sums over the Boolean hypercube; verifier checks with negligible soundness error.

* **Deterministic PRNG (Fiat–Shamir)**
  BLAKE2b-based expander for challenge derivation. No OS entropy. Reproducible.
  (The `crt_chain` example intentionally uses an LCG for CRT illustration.)

* **Generalized Multilinear Sum-Check**
  `MultilinearPolynomial`, `Transcript`, `GeneralSumClaim` enable non-interactive proofs for arbitrary multilinear polynomials.

* **Transcript & Chaining Toolkit**
  Capture per-round sums, challenges, final evaluations; chain proofs; feed them into the ledger. All entries carry domain-separated BLAKE2b-256 integrity digests.

* **Streaming Proof Generation**
  Build large proofs via streaming evaluators; benchmarking CLI reports per-round timing.

* **Ledger Transcript Logging with Integrity Hashes**
  Proofs persist as plain ASCII dossiers with deterministic digests. Anchors are append-only commitments; a ledger state is valid iff statement strings and ordered hash lists match.

* **Quorum Finality for JULIAN**
  `reconcile_anchors_with_quorum` defines finality: once ≥ q nodes publish matching anchors, the state is final. Divergent anchors are identified by `verify_logs`.

* **Consensus Primitive**
  Quorum-based agreement logic reflecting core BFT ideas.

* **ALIEN Ledger Blueprint**
  A scaffold that unifies proofs, deterministic randomness, and anchor reconciliation.

## CLI Workflow

The `julian` binary exposes local ledger tooling and the optional `JROC-NET` networking stack.

### Local ledger (`julian node …`)

These commands require only the standard library:

* `julian node run <node_id> <log_dir> <output>`
  Recompute transcript hashes from `<log_dir>`, prepend JULIAN genesis, and write a machine-readable anchor file.

* `julian node anchor <log_dir>`
  Print a formatted ledger anchor derived from logs.

* `julian node reconcile <log_dir> <peer_anchor> <quorum>`
  Recompute local anchor, load peer anchor, check quorum finality.

* `julian node prove <log_dir> <entry_index> <leaf_index> [output.json]`
  Emit a Merkle proof for a specific transcript digest.

* `julian node verify-proof <anchor_file> <proof_file>`
  Verify a Merkle proof against a stored anchor (non-zero exit on failure).

End-to-end anchor example (after `cargo run --example hash_pipeline`):

```
# Prepare node log directories
mkdir -p ./logs/nodeA ./logs/nodeB
cp /tmp/power_house_anchor_a/* ./logs/nodeA/
cp /tmp/power_house_anchor_b/* ./logs/nodeB/

# Produce anchors and reach quorum
julian node run nodeA ./logs/nodeA nodeA.anchor
julian node run nodeB ./logs/nodeB nodeB.anchor
julian node reconcile ./logs/nodeA nodeB.anchor 2
```

### Network mode (`julian net …`, feature `net`)

The network subcommands pull in optional dependencies (`libp2p`, `ed25519-dalek`, `tokio`). Build with the feature:

```
cargo install --path . --features net
# or, for local runs
cargo run --features net --bin julian -- net ...
```

Supported commands:

* `julian net start --node-id <id> --log-dir <path> --listen <multiaddr> --bootstrap <multiaddr>... --broadcast-interval <ms> --quorum <q> [--key <spec>]`
  • `--key` accepts `ed25519://deterministic-seed`, or a path to raw/hex/base64 secret key bytes; omitted ⇒ fresh key.
  • `--identity` loads an encrypted identity file (XOR of secret key with SHA-512(passphrase), first 32 bytes).
  • `--metrics [:port]` exposes Prometheus metrics (binds `0.0.0.0:<port>` if prefixed with a colon).
  • `--policy governance.json` loads a governance descriptor (`backend: static | static-file | stake | multisig`) and enforces the membership set.
  • `--policy-allowlist allow.json` restricts quorum counting to listed ed25519 keys.
  • `--checkpoint-interval N` writes signed anchor checkpoints every N broadcasts.

* `julian net anchor --log-dir <path> [--node-id <id>] [--quorum <q>]`
  Emit a machine-readable JSON anchor.

* `julian net verify-envelope --file <path> --log-dir <path> [--quorum <q>]`
  Validate a signed envelope, decode the anchor payload, and perform the quorum check against local logs.

Two local nodes with deterministic keys:

```
# Terminal 1 – nodeA
cargo run --features net --bin julian -- net start \
  --node-id nodeA \
  --log-dir ./logs/nodeA \
  --listen /ip4/127.0.0.1/tcp/7001 \
  --broadcast-interval 5000 \
  --quorum 2 \
  --key ed25519://nodeA-seed

# Terminal 2 – nodeB (bootstraps to nodeA locally)
cargo run --features net --bin julian -- net start \
  --node-id nodeB \
  --log-dir ./logs/nodeB \
  --listen /ip4/127.0.0.1/tcp/7002 \
  --bootstrap /ip4/127.0.0.1/tcp/7001 \
  --broadcast-interval 5000 \
  --quorum 2 \
  --key ed25519://nodeB-seed
```

Each node recomputes anchors from its log directory, signs them, gossips envelopes, and logs finality once quorum succeeds.

Local smoke: `scripts/smoke_net.sh` (ports 7211/7212). Confirms signed anchor broadcasts and finality; exits non-zero on failure.

## Boot Node Operations (systemd)

Treat public ingress nodes as long-lived services.

1. Build: `cargo build --release --features net --bin julian`
2. Ship:  `scp target/release/julian root@host:/root/julian.new && sudo install -m 0755 /root/julian.new /usr/local/bin/julian`
3. Unit: copy the systemd template from `docs/ops.md`; set node-specific `/etc/jrocnet/powerhouse-bootN.env` and shared `/etc/jrocnet/powerhouse-common.env`; use explicit `/ip4/<peer-ip>/tcp/<port>/p2p/<peer-id>` for `PH_BOOTSTRAPS` so the service dials the right ingress even if DNS lags.
4. Start: `systemctl daemon-reload && systemctl enable --now powerhouse-bootN.service`
5. Health: `journalctl -u powerhouse-bootN.service -n 40 -f` → see `QSYS|mod=ANCHOR|evt=STANDBY` then alternating `QSYS|mod=ANCHOR|evt=BROADCAST` and `QSYS|mod=QUORUM|evt=FINALIZED`.
6. Reachability: from each host `nc -vz <other-ip> 7001` / `7002`; failures imply firewall/routing, not libp2p.

Keep customized unit files and deterministic seeds in your infra repo or secrets manager (do not commit live service defs publicly).

## Governance Descriptor Reference

`--policy` accepts a JSON descriptor with a `backend` key:

* `static`       — inline allowlist via `allowlist: ["base64", ...]`
* `static-file`  — pointer to legacy allowlist JSON (`{"allowed":[...]}`)
* `stake`        — bond-backed membership loaded from a staking state file
* `multisig`     — pointer to state tracking K-of-N signers and active members

Example multisig state:

```json
{
  "threshold": 2,
  "signers": [
    "mbnfAp950/gQfEPc2J27MEvc+TPkY65/AJ6Xs0NjYew=",
    "5o2IL90EOYBUPvXMgCwFoo94UDYe9mAvZBCAwtasJ+I=",
    "pslM5tF63E6Zb9P4uM7V6ZJZr/E4YjX8pB7k5wBfF7A="
  ],
  "members": [
    "mbnfAp950/gQfEPc2J27MEvc+TPkY65/AJ6Xs0NjYew=",
    "5o2IL90EOYBUPvXMgCwFoo94UDYe9mAvZBCAwtasJ+I="
  ]
}
```

Stake-backed example:

```json
{
  "threshold": 2,
  "bond_threshold": 100,
  "signers": [
    "mbnfAp950/gQfEPc2J27MEvc+TPkY65/AJ6Xs0NjYew=",
    "5o2IL90EOYBUPvXMgCwFoo94UDYe9mAvZBCAwtasJ+I="
  ],
  "entries": [
    {"public_key": "mbnfAp950/gQfEPc2J27MEvc+TPkY65/AJ6Xs0NjYew=", "bond": 150, "slashed": false},
    {"public_key": "5o2IL90EOYBUPvXMgCwFoo94UDYe9mAvZBCAwtasJ+I=", "bond": 120, "slashed": false}
  ]
}
```

## Membership Rotation Checklist

* Fetch current descriptor (e.g., `scp root@boot1:/etc/jrocnet/governance.json ./`).
* Edit offline; confirm the new membership list.
* Multisig: craft a `GovernanceUpdate` with the new members; collect ≥ threshold ed25519 signatures.
* Stake: embed bond deposits and explicit slashes in the update metadata; conflicting anchors are auto-slashed at runtime—review and re-affirm the registry after incidents.
* Distribute the signed update + refreshed state file to each boot node (archive prior version under `logs/policy/`).
* Restart with the same `--policy` argument; `julian net start` loads the new membership immediately.

## Anchor JSON Schema (reference)

```json
{
  "schema": "jrocnet.anchor.v1",
  "network": "JROC-NET",
  "node_id": "nodeA",
  "genesis": "JULIAN::GENESIS",
  "challenge_mode": "mod",
  "fold_digest": "c87282dddb8d85a8b09a9669a1b2d97b30251c05b80eae2671271c432698aabe",
  "crate_version": "0.1.54",
  "entries": [
    {
      "statement": "JULIAN::GENESIS",
      "hashes": ["139f1985df5b36dae23fa509fb53a006ba58e28e6dbb41d6d71cc1e91a82d84a"],
      "merkle_root": "09c0673e5d1a15ea98da1e7188d64e4db53f46982810d631264dbbd001ad995a"
    },
    {
      "statement": "Dense polynomial proof",
      "hashes": ["ded75c45b3b7eedd37041aae79713d7382e000eb4d83fab5f6aca6ca4d276e8c"],
      "merkle_root": "80e7cb9d1721ce47f6f908f9ac01098d9c035f1225fff84083a6e1d0828144f4"
    },
    {
      "statement": "Hash anchor proof",
      "hashes": ["c72413466b2f76f1471f2e7160dadcbf912a4f8bc80ef1f2ffdb54ecb2bb2114"],
      "merkle_root": "637aeed7e8fbb42747c39c82dfe1eb242bda92fead2a24abaf8c5ffc45ff8e82"
    }
  ],
  "quorum": 2,
  "timestamp_ms": 1730246400000
}
```

## Signed Envelope Format

```json
{
  "schema": "jrocnet.envelope.v1",
  "public_key": "<base64-ed25519-pk>",
  "node_id": "nodeA",
  "payload": "<base64-raw-json-of-anchor>",
  "signature": "<base64-sign(payload)>"
}
```

Validation steps: ensure the schema matches, base64-decode the payload, verify the ed25519 signature, parse the embedded anchor JSON, then reconcile with local logs.

## JROC-NET Public Net (current snapshot)

1. Topics & networking

   * Gossipsub topic: `jrocnet/anchors/v1`.
   * Bootstrap multiaddrs: publish `/ip4/<BOOT>/tcp/7001/p2p/<PEER_ID>` per public node (or DNS4 equivalents).

2. Anchor schema

   * Machine-readable anchors follow `jrocnet.anchor.v1` (see schema above).

3. Signed envelopes

   * `jrocnet.envelope.v1` provides tamper-evident anchor broadcasts (ed25519 over raw anchor JSON).

4. CLI flags / behavior

   * `julian net start` supports `--bootstrap`, `--key`, `--broadcast-interval`, `--quorum`, `--policy`, `--metrics`.
   * `julian net anchor` / `julian net verify-envelope` cover audit and validation.

5. Libp2p behavior

   * TCP + Noise + Yamux; Gossipsub for gossip; Kademlia for peer discovery; Identify for metadata.

6. Security hygiene

   * Message-id cache (SHA-256 of payload), strict validation, per-topic rate limiting, schema/network checks before reconciliation.

7. Observability

   * Prometheus `--metrics` endpoint exports:
     `anchors_received_total`, `anchors_verified_total`, `invalid_envelopes_total`, `lrucache_evictions_total`, `finality_events_total`, `gossipsub_rejects_total`.

   * Starter Grafana dashboard: `contrib/grafana/jroc_net_dashboard.json`.

8. Launch playbook (community join)

```
cargo install power_house --features net
julian net start \
  --node-id <your_name> \
  --log-dir ./logs/<your_name> \
  --listen /ip4/0.0.0.0/tcp/0 \
  --bootstrap /dns4/boot1.jrocnet.com/tcp/7001/p2p/12D3KooWLASw1JVBdDFNATYDJMbAn69CeWieTBLxAKaN9eLEkh3q \
  --bootstrap /dns4/boot2.jrocnet.com/tcp/7002/p2p/12D3KooWRLM7PJrtjRM6NZPX8vmdu4YGJa9D6aPoEnLcE1o6aKCd \
  --broadcast-interval 5000 \
  --quorum 2 \
  --key ed25519://<seed>
```

Bootstrap multiaddrs (A2 reference):

* `/dns4/boot1.jrocnet.com/tcp/7001/p2p/12D3KooWLASw1JVBdDFNATYDJMbAn69CeWieTBLxAKaN9eLEkh3q`
* `/dns4/boot2.jrocnet.com/tcp/7002/p2p/12D3KooWRLM7PJrtjRM6NZPX8vmdu4YGJa9D6aPoEnLcE1o6aKCd`

The testnet keeps every transcript, proof, and anchor transparent so auditors can replay history end-to-end.

## Examples

Sum-check verification

```rust
use power_house::{Field, SumClaim};

let field = Field::new(101);
let claim = SumClaim::prove_demo(&field, 8);
assert!(claim.verify_demo());
```

Run the executable demo:

```
cargo run --example demo
```

CRT chain showcase (LCG for CRT illustration)

```
cargo run --example crt_chain
```

Prints a 12-round trace with reproducible totals and hash pairs; shows how components compose into a heavier protocol.

General multilinear sum-check

```rust
use power_house::{Field, GeneralSumClaim, MultilinearPolynomial};

let field = Field::new(97);
let poly = MultilinearPolynomial::from_evaluations(3, vec![
    0, 1, 4, 5, 7, 8, 11, 23,
]);
let claim = GeneralSumClaim::prove(&poly, &field);
assert!(claim.verify(&poly, &field));
```

Interactive variant:

```
cargo run --example general_sumcheck
```

Transcript outputs include deterministic Fiat–Shamir challenges; each record carries a domain-separated BLAKE2b-256 integrity hash for tamper-evident storage.

Mega sum-check & chaining

```
cargo run --example mega_sumcheck
```

Build 10-variable polynomials, record per-round timings, chain multiple proofs, hand to the ledger scaffold.

Scaling benchmark

```
cargo run --example scale_sumcheck
```

Prints a timing table for increasing numbers of variables; set `POWER_HOUSE_SCALE_OUT=/path/to/results.csv` to emit machine-readable timing data.

Transcript hash verification

```
cargo run --example verify_logs -- /tmp/power_house_ledger_logs
```

Replays ledger logs, recomputes integrity hashes, prints pass/fail summary.

Hash pipeline & anchor reconciliation

```
cargo run --example hash_pipeline
```

Streams per-proof hashes into constant-time anchors, folds them with domain-separated BLAKE2b-256, and reconciles across multiple ledgers. This is the reference JULIAN pipeline.

## Whitepaper

See `JULIAN_PROTOCOL.md`.

## CLI node commands (quick ref)

```
cargo run --bin julian -- node run <node_id> <log_dir> <output_anchor>
cargo run --bin julian -- node anchor <log_dir>
cargo run --bin julian -- node reconcile <log_dir> <peer_anchor> <quorum>
```

These commands replay transcript logs, derive JULIAN anchors, and check quorum finality using only the Rust standard library.

---

End of README.txt

Rollup settlement: `julian rollup settle <registry.json> <namespace> <share_root> <payer_b64> <fee> [zk|optimistic]` debits fees and emits a receipt (proof verification is stubbed for now; optimistic faults are rejected).

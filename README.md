<p align="center">
  <img src="assets/JROC-NET.jpeg" alt="JROC NET" width="420">
</p>

# JROC NET
### The JULIAN Protocol Network
Transparent Proof-Derived Consensus via Ledger Anchors

Engine: power_house  
Protocol: JULIAN  
Network: JROC NET

---

# Power-House

[![Crates.io](https://img.shields.io/crates/v/power_house.svg)](https://crates.io/crates/power_house)
[![docs.rs](https://docs.rs/power_house/badge.svg)](https://docs.rs/power_house)
[![Build Status](https://img.shields.io/badge/tests-passing-brightgreen.svg)](#)

**Author:** Julian Christian Sanders 
**Email:** [lexluger.dev@proton.me](mailto:lexluger.dev@proton.me) 
**Date** 10/16/2025 

---

## Launch Announcement – JROC NET

JROC NET is now live. The JULIAN Protocol ledger is fully operational with genesis anchors, transcript hashing, quorum-based finality, and multi-node reconciliation. All proofs, transcripts, and anchors remain fully transparent and reproducible, ensuring verifiable computation across distributed nodes.

---

## Quick Join (Public Testnet A2)

```bash
cargo install power_house --features net
# optional: export a deterministic identity (ed25519://your-seed) or use an encrypted identity file
julian net start \
  --node-id <your_name> \
  --log-dir ./logs/<your_name> \
  --listen /ip4/0.0.0.0/tcp/0 \
  --bootstrap /ip4/76.33.137.42/tcp/7001/p2p/12D3KooWLASw1JVBdDFNATYDJMbAn69CeWieTBLxAKaN9eLEkh3q \
  --bootstrap /ip4/76.33.137.42/tcp/7002/p2p/12D3KooWRLM7PJrtjRM6NZPX8vmdu4YGJa9D6aPoEnLcE1o6aKCd \
  --broadcast-interval 5000 \
  --quorum 2 \
  --key ed25519://<seed>

# expose Prometheus metrics (optional)
# --metrics :9100
```

To load an encrypted identity instead of `--key`, create a file containing the base64 result of XORing your 32-byte secret key with the first 32 bytes of `SHA-512(passphrase)`, then run `julian net start --identity /path/to/file`. You’ll be prompted for the passphrase at startup.

Bootstrap multiaddrs (loopback defaults shown above):

- `/ip4/76.33.137.42/tcp/7001/p2p/12D3KooWLASw1JVBdDFNATYDJMbAn69CeWieTBLxAKaN9eLEkh3q`
- `/ip4/76.33.137.42/tcp/7002/p2p/12D3KooWRLM7PJrtjRM6NZPX8vmdu4YGJa9D6aPoEnLcE1o6aKCd`

Run `scripts/smoke_net.sh` for a local two-node quorum smoke test (ports 7211/7212, 8s runtime).

## Genesis Anchor (Pinned)

The A2 testnet ledger is frozen to the following statements and 64-bit digests. Every node should
reproduce these values from its local logs:

```
statement: JULIAN::GENESIS          hash: 17942395924573474124
statement: Dense polynomial proof   hash: 1560461912026565426
statement: Hash anchor proof        hash: 17506285175808955616
```

Boot nodes run with deterministic seeds (`ed25519://boot1-seed`, `ed25519://boot2-seed`) so their
libp2p Peer IDs remain constant. Do not rotate keys or ports unless you intend to publish a new
network version.

### Verify Your Anchor

```bash
# Produce an anchor file from your local logs
julian node run mynode ./logs/mynode mynode.anchor.txt

# Inspect the statements and compare to the pinned digests above
cat mynode.anchor.txt

# Reconcile against a published anchor (example with boot1)
julian node reconcile ./logs/mynode boot1.anchor.txt 2
```

To recreate the bootstrap anchors themselves:

```bash
julian node run boot1 ./logs/boot1 boot1.anchor.txt
julian node run boot2 ./logs/boot2 boot2.anchor.txt

julian node reconcile ./logs/boot1 boot2.anchor.txt 2
julian node reconcile ./logs/boot2 boot1.anchor.txt 2
```

---

## License

Copyright © 2025 Julian Christian Sanders

Permission is hereby granted, free of charge, to any person obtaining a copy of
this software and associated documentation files (the "Software"), to use,
reproduce, and distribute the Software, subject to the following conditions:

1. 
   The Software may be used, reproduced, and distributed solely for non-profit,
   academic, or personal purposes. Commercial exploitation of any kind—defined as
   use of the Software or derivative works for direct or indirect financial gain,
   advertising, or sale—is expressly prohibited without the prior written consent
   of the copyright holder.

2. 
   Redistributions of the Software in source or binary form must retain this
   copyright notice, the conditions of use, and the following disclaimer in all
   such distributions.

3. 
   Derivative works that incorporate substantial portions of the Software must
   not be used or distributed for any profit-seeking activity. Any derivative
   must also prominently reproduce this notice and the non-commercial
   restrictions.

4. 
   THE SOFTWARE IS PROVIDED “AS IS,” WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
   IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
   FITNESS FOR A PARTICULAR PURPOSE, AND NON-INFRINGEMENT. IN NO EVENT SHALL THE
   AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES, OR OTHER
   LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT, OR OTHERWISE, ARISING FROM,
   OUT OF, OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
   SOFTWARE.

This license statement does not constitute legal advice. Consult a legal
professional for questions about your specific situation.

---

## Power-House Overview

**Power-House** is a paradigmatic exploration of interactive proof systems, finite-field arithmetic, and deterministic pseudo-randomness—all implemented in pure Rust without external dependencies. It offers a microcosm of proof verification processes inspired by seminal work in probabilistically checkable proofs (PCPs) and modern zero-knowledge architectures.

### Motivation

Interactive proof techniques underpin cutting-edge cryptographic protocols and blockchain consensus.
This crate distills those advanced concepts into a standalone laboratory for experimentation, formal verification, and pedagogy.
It emulates the essential features of the **sum-check protocol**, exhibits a **rudimentary Byzantine consensus mechanism**, and now powers the **JULIAN Protocol**—a proof-transparent ledger that anchors folding transcripts into verifiable consensus states.

### Features

-  **Finite Field Arithmetic:**
  A lean yet robust implementation of arithmetic modulo a prime, essential for homomorphic operations and algebraic proofs.

-  **Sum-Check Protocol Demo:**
  Illustrates how a prover can succinctly certify a polynomial’s evaluation over a Boolean hypercube, while the verifier checks integrity with negligible soundness error.

-  **Deterministic PRNG:**
  A compact linear-congruential generator serving as a deterministic source of challenge derivation, thereby eliminating external entropy dependencies.

-  **Generalized Multilinear Sum-Check:**
  The `MultilinearPolynomial`, `Transcript`, and `GeneralSumClaim` types enable non-interactive proofs for arbitrary multilinear polynomials—still without any external crates.

-  **Transcript & Chaining Toolkit:**
  Capture Fiat–Shamir challenges, per-round sums, and final evaluations, then chain proofs together or feed them directly into the ALIEN ledger scaffold for deterministic auditing.

-  **Streaming Proof Generation:**
  Build massive sum-checks via streaming evaluators (no full hypercube allocation), with per-round timing exported by the benchmarking CLI.

-  **Ledger Transcript Logging with Integrity Hashes:**
  Persist proofs as ASCII dossiers tagged with built-in hash digests so transcripts remain self-authenticating without external crates. Ledger anchors are append-only commitments to those transcripts; a ledger state is valid iff every anchor agrees on the statement string and ordered hash list.

-  **Quorum Finality for the JULIAN Protocol:**
  `reconcile_anchors_with_quorum` formalises finality: once ≥ *q* nodes publish matching anchors, the JULIAN ledger state is final. Divergent anchors are immediately pinpointed by re-running `verify_logs`.

-  **Consensus Primitive:**
  Demonstrates quorum-based agreement logic reflective of Byzantine fault tolerance in distributed systems.

-  **ALIEN Ledger Blueprint:**
  A scaffold for integrating proofs, consensus, and randomness into a unified verification ledger, pointing toward PSPACE-level expressive power and quantum-assisted extensions.

## JULIAN CLI Workflows

The `julian` binary exposes both local ledger tooling and the optional `JROC-NET` networking stack.

### Local ledger (`julian node …`)

These commands are always available and require only the standard library:

- `julian node run <node_id> <log_dir> <output>` – recomputes transcript hashes from `<log_dir>`, prepends the JULIAN genesis anchor, and writes a machine-readable anchor file.
- `julian node anchor <log_dir>` – prints a formatted ledger anchor derived from the logs.
- `julian node reconcile <log_dir> <peer_anchor> <quorum>` – recomputes the local anchor, loads a peer’s anchor file, and checks quorum finality.

End-to-end anchor example (after running `cargo run --example hash_pipeline`):

```bash
# Prepare node log directories.
mkdir -p ./logs/nodeA ./logs/nodeB
cp /tmp/power_house_anchor_a/* ./logs/nodeA/
cp /tmp/power_house_anchor_b/* ./logs/nodeB/

# Produce anchors and reach quorum.
julian node run nodeA ./logs/nodeA nodeA.anchor
julian node run nodeB ./logs/nodeB nodeB.anchor
julian node reconcile ./logs/nodeA nodeB.anchor 2
```

### Network mode (`julian net …`, feature `net`)

The networking subcommands pull in optional dependencies (`libp2p`, `ed25519-dalek`, `tokio`). Build with the feature enabled:

```bash
cargo install --path . --features net
# or, for local runs
cargo run --features net --bin julian -- net ...
```

Supported commands:

- `julian net start --node-id <id> --log-dir <path> --listen <multiaddr> --bootstrap <multiaddr>... --broadcast-interval <ms> --quorum <q> [--key <spec>]`
  * `--key` accepts `ed25519://deterministic-seed`, or a path to raw/hex/base64 secret key bytes; omitted ⇒ fresh key.
  * `--identity` loads an encrypted identity file (XOR of the secret key with `SHA-512(passphrase)`); the CLI prompts for the passphrase.
  * `--metrics [:port]` exposes Prometheus metrics (defaults to `0.0.0.0:<port>` when prefixed with a colon).
- `julian net anchor --log-dir <path> [--node-id <id>] [--quorum <q>]` emits a machine-readable JSON anchor.
- `julian net verify-envelope --file <path> --log-dir <path> [--quorum <q>]` validates a signed envelope, decodes the anchor payload, and performs the quorum check against local logs.

Example session with two local nodes and deterministic keys:

```bash
# Terminal 1 – nodeA
cargo run --features net --bin julian -- net start \
  --node-id nodeA \
  --log-dir ./logs/nodeA \
  --listen /ip4/127.0.0.1/tcp/7001 \
  --broadcast-interval 5000 \
  --quorum 2 \
  --key ed25519://nodeA-seed

# Terminal 2 – nodeB
cargo run --features net --bin julian -- net start \
  --node-id nodeB \
  --log-dir ./logs/nodeB \
  --listen /ip4/127.0.0.1/tcp/7002 \
  --bootstrap /ip4/127.0.0.1/tcp/7001/p2p/<NODEA_PEER_ID> \
  --broadcast-interval 5000 \
  --quorum 2 \
  --key ed25519://nodeB-seed
```

Each node recomputes anchors from its log directory, signs them, broadcasts envelopes over Gossipsub, and logs finality events once the quorum predicate succeeds.

Run `scripts/smoke_net.sh` to exercise the two-node quorum workflow locally; the script boots nodes on ports 7211/7212, waits for signed anchor broadcasts, confirms finality, and exits non-zero on failure.

#### Anchor JSON schema

```json
{
  "schema": "jrocnet.anchor.v1",
  "network": "JROC-NET",
  "node_id": "nodeA",
  "genesis": "JULIAN::GENESIS",
  "entries": [
    { "statement": "JULIAN::GENESIS", "hashes": [17942395924573474124] },
    { "statement": "Dense polynomial proof", "hashes": [1560461912026565426] },
    { "statement": "Hash anchor proof", "hashes": [17506285175808955616] }
  ],
  "quorum": 2,
  "timestamp_ms": 1730246400000
}
```

#### Signed envelope format

```json
{
  "schema": "jrocnet.envelope.v1",
  "public_key": "<base64-ed25519-pk>",
  "node_id": "nodeA",
  "payload": "<base64-raw-json-of-anchor>",
  "signature": "<base64-sign(payload)>"
}
```

Validation steps: ensure the schema matches, base64-decode the payload, verify the ed25519 signature, parse the embedded anchor JSON, then reconcile with the local ledger.

### JROC-NET Public Testnet (A2) roadmap

1. **Topics & networking**
   - Gossip topics: `jrocnet/anchors/v1`, optional `jrocnet/ping/v1`, `jrocnet/peers/v1`.
   - Bootstrap multiaddrs: `/ip4/<BOOT>/tcp/7001/p2p/<PEER_ID>` defined per public node.
2. **Anchor schema** – Machine-readable anchors follow `jrocnet.anchor.v1` as shown above.
3. **Signed envelopes** – `jrocnet.envelope.v1` ensures tamper-evident broadcasts (ed25519 signatures over the raw anchor JSON).
4. **CLI flags** – `julian net start` accepts `--bootstrap`, `--key`, `--broadcast-interval`, `--quorum`, mirroring the launch playbook; `julian net anchor`/`verify-envelope` cover audit tooling.
5. **Libp2p behaviour** – TCP + Noise + Yamux transports, Gossipsub for anchor gossip, Kademlia for peer discovery, Identify for metadata.
6. **Security hygiene** – Message-id cache (SHA256 payload hash), strict validation, per-topic rate limiting, and schema/network checks before reconciliation.
7. **Observability** – Console summaries plus the optional `--metrics` Prometheus endpoint exporting `anchors_received_total`, `anchors_verified_total`, `invalid_envelopes_total`, `lrucache_evictions_total`, `finality_events_total`, and `gossipsub_rejects_total`.
   - Import `contrib/grafana/jroc_net_dashboard.json` into Grafana for a starter dashboard.
8. **Launch playbook** – Run at least two bootstrap nodes, publish their multiaddrs, then let community nodes join via:

   ```bash
   cargo install power_house --features net
   julian net start \
     --node-id <your_name> \
     --log-dir ./logs/<your_name> \
     --listen /ip4/0.0.0.0/tcp/0 \
     --bootstrap /ip4/127.0.0.1/tcp/7001/p2p/12D3KooWLASw1JVBdDFNATYDJMbAn69CeWieTBLxAKaN9eLEkh3q \
     --bootstrap /ip4/127.0.0.1/tcp/7002/p2p/12D3KooWRLM7PJrtjRM6NZPX8vmdu4YGJa9D6aPoEnLcE1o6aKCd \
     --broadcast-interval 5000 \
     --quorum 2 \
     --key ed25519://<seed>
   ```

   Bootstrap multiaddrs (A2 testnet reference):

   - `/ip4/127.0.0.1/tcp/7001/p2p/12D3KooWLASw1JVBdDFNATYDJMbAn69CeWieTBLxAKaN9eLEkh3q`
   - `/ip4/127.0.0.1/tcp/7002/p2p/12D3KooWRLM7PJrtjRM6NZPX8vmdu4YGJa9D6aPoEnLcE1o6aKCd`

The testnet keeps every transcript, proof, and anchor transparent so auditors can replay history end-to-end.

---

## Examples

### Sum-check verification

```rust
use power_house::{Field, SumClaim};

let field = Field::new(101);
let claim = SumClaim::prove_demo(&field, 8);
assert!(claim.verify_demo());
```

Run the executable variant to see the non-interactive sum-check in action:

```bash
cargo run --example demo
```

The program exits with a non-zero status if verification ever fails, making it easy to embed inside scripts or CI checks.

### CRT chain showcase

The `crt_chain` example threads three large primes through a deterministic LCG, combines the outputs
with the Chinese Remainder Theorem, and emits transcript digests derived from the `Field` arithmetic:

```bash
cargo run --example crt_chain
```

It prints a 12-round trace with reproducible totals and hash pairs, highlighting how Power-House components compose into a heavier protocol.

### General multilinear sum-check

```rust
use power_house::{Field, GeneralSumClaim, MultilinearPolynomial};

let field = Field::new(97);
let poly = MultilinearPolynomial::from_evaluations(3, vec![
    0, 1, 4, 5, 7, 8, 11, 23,
]);
let claim = GeneralSumClaim::prove(&poly, &field);
assert!(claim.verify(&poly, &field));
```

Re-run it interactively with:

```bash
cargo run --example general_sumcheck
```

The example exercises the Fiat–Shamir transcript helper and the generalized sum-check prover/verifier against a three-variable polynomial.

Transcript outputs include deterministic Fiat–Shamir challenges; when logged via the ledger, each record carries a 64-bit integrity hash for tamper-evident storage.

### Mega sum-check & chaining demo

```bash
cargo run --example mega_sumcheck
```

This walkthrough builds 10-variable polynomials, records per-round timings, and chains multiple proofs together before handing them off to the ALIEN ledger scaffold.

### Scaling benchmark

```bash
cargo run --example scale_sumcheck
```

Prints a timing table for increasing numbers of variables, helping you profile how multilinear proofs scale as the hypercube size grows.
Set `POWER_HOUSE_SCALE_OUT=/path/to/results.csv` to emit machine-readable timing data alongside the console output.

### Transcript hash verification

```bash
cargo run --example verify_logs -- /tmp/power_house_ledger_logs
```

Replays ledger log files, recomputes their integrity hashes, and prints a pass/fail summary so archived transcripts remain tamper-evident.

### Hash pipeline & anchor reconciliation

```bash
cargo run --example hash_pipeline
```

Streams per-proof hashes into constant-time anchors, aggregates them (mode selectable via `POWER_HOUSE_HASH_MODE=xor|sum`), and reconciles the anchors across multiple ledgers while emitting tamper-evident logs. This example is the reference JULIAN Protocol pipeline: nodes replay transcript logs, exchange `LedgerAnchor` structures, and call `reconcile_anchors_with_quorum` to reach finality.

### Whitepaper

The full JULIAN Protocol write-up lives in [`JULIAN_PROTOCOL.md`](JULIAN_PROTOCOL.md).

### CLI node commands

```bash
cargo run --bin julian -- node run <node_id> <log_dir> <output_anchor>
cargo run --bin julian -- node anchor <log_dir>
cargo run --bin julian -- node reconcile <log_dir> <peer_anchor> <quorum>
```

These commands replay transcript logs, derive JULIAN anchors, and check quorum finality using nothing beyond the Rust standard library.

<p style="margin:0;padding:0;">
  <img src="https://raw.githubusercontent.com/JROChub/power_house/master/assets/JROC-NET.jpeg" alt="power_house" style="display:block;width:100%;max-width:100%;margin:0;padding:0;">
</p>

<div style="font-family:'IBM Plex Mono',monospace;font-size:0.68rem;line-height:1.6;letter-spacing:0.03em;">
<h1 style="font-size:0.9rem;margin:1.4rem 0 0.6rem;">Power-House</h1>

<p style="margin:0.9rem 0 0.6rem;">power_house delivers deterministically reproducible multilinear sum-check proofs, deterministic PRNG wiring, and quorum ledger tooling for transparent transcript auditing—all authored in pure Rust.</p>

<p style="margin:0.4rem 0;">
  <a href="https://crates.io/crates/power_house"><img src="https://img.shields.io/crates/v/power_house.svg" alt="Crates.io badge" style="height:0.9rem;vertical-align:middle;"></a>
  <a href="https://docs.rs/power_house"><img src="https://docs.rs/power_house/badge.svg" alt="docs.rs badge" style="height:0.9rem;vertical-align:middle;margin-left:0.4rem;"></a>
  <img src="https://img.shields.io/badge/tests-passing-brightgreen.svg" alt="tests passing" style="height:0.9rem;vertical-align:middle;margin-left:0.4rem;">
</p>

<p style="margin:0.3rem 0;">Author: <strong style="font-weight:600;">lexluger</strong> &nbsp;|&nbsp; Email: <a href="mailto:lexluger.dev@proton.me">lexluger.dev@proton.me</a> &nbsp;|&nbsp; Site: <a href="https://jrocnet.com">jrocnet.com</a> &nbsp;|&nbsp; Last update: 2025‑10‑16</p>

<p style="margin:0.6rem 0;font-size:0.7rem;">Need the full operations guide? Read the <a href="./docs/book_of_power.md">Book of Power — Condensed Graviton Edition</a> for the alien-grade user manual bundled with this crate.</p>

<h2 style="font-size:0.82rem;margin:1.2rem 0 0.5rem;">Quick Join (Public Testnet A2)</h2>
<pre style="font-size:0.66rem;line-height:1.5;padding:0.8rem;background:#0d0d0d;color:#f0f0f0;border-radius:0.3rem;overflow:auto;">cargo install power_house --features net
# optional: export a deterministic identity (ed25519://your-seed) or use an encrypted identity file
julian net start \
  --node-id &lt;your_name&gt; \
  --log-dir ./logs/&lt;your_name&gt; \
  --listen /ip4/0.0.0.0/tcp/0 \
  --bootstrap /dns4/boot1.jrocnet.com/tcp/7001/p2p/12D3KooWLASw1JVBdDFNATYDJMbAn69CeWieTBLxAKaN9eLEkh3q \
  --bootstrap /dns4/boot2.jrocnet.com/tcp/7002/p2p/12D3KooWRLM7PJrtjRM6NZPX8vmdu4YGJa9D6aPoEnLcE1o6aKCd \
  --broadcast-interval 5000 \
  --quorum 2 \
  --key ed25519://&lt;seed&gt;
</pre>

<p style="margin:0.3rem 0;">Optional Prometheus metrics: add <code style="font-size:0.66rem;">--metrics :9100</code> (or another port) when starting a node.</p>

<p style="margin:0.3rem 0;">Identity governance now supports descriptor-driven policies. Supply <code style="font-size:0.66rem;">--policy governance.json</code> to load either a static allowlist, a referenced allowlist file, or a multisig-governed membership set. Legacy deployments can keep using <code style="font-size:0.66rem;">--policy-allowlist allow.json</code> with base64 ed25519 keys. Persist signed snapshots by adding <code style="font-size:0.66rem;">--checkpoint-interval 100</code> (for example) to emit checkpoints every <em>100</em> broadcasts under <code>./logs/&lt;node&gt;/checkpoints</code>.</p>

<p style="margin:0.3rem 0;">Sample descriptor (<code style="font-size:0.66rem;">--policy</code>):</p>
<pre style="font-size:0.66rem;line-height:1.5;padding:0.8rem;background:#0d0d0d;color:#f0f0f0;border-radius:0.3rem;overflow:auto;">{
  "backend": "static",
  "allowlist": [
    "mbnfAp950/gQfEPc2J27MEvc+TPkY65/AJ6Xs0NjYew=",
    "5o2IL90EOYBUPvXMgCwFoo94UDYe9mAvZBCAwtasJ+I="
  ]
}</pre>
<p style="margin:0.3rem 0;">Multisig descriptors point to a state file containing <code style="font-size:0.66rem;">{"threshold":2,"signers":[...],"members":[...]}</code>; the helper verifies that at least <code>K</code> authorised signers approve a membership rotation before writing it back to disk.</p>

<p>To load an encrypted identity instead of <code style="font-size:0.66rem;">--key</code>, create a file containing the base64 result of XORing your 32-byte secret key with the first 32 bytes of <code style="font-size:0.66rem;">SHA-512(passphrase)</code>, then run <code style="font-size:0.66rem;">julian net start --identity /path/to/file</code>. You’ll be prompted for the passphrase at startup.</p>

<ul style="margin:0.3rem 0 0.8rem 1.1rem;">
  <li style="margin:0.2rem 0;"><code style="font-size:0.66rem;">/dns4/boot1.jrocnet.com/tcp/7001/p2p/12D3KooWLASw1JVBdDFNATYDJMbAn69CeWieTBLxAKaN9eLEkh3q</code></li>
  <li style="margin:0.2rem 0;"><code style="font-size:0.66rem;">/dns4/boot2.jrocnet.com/tcp/7002/p2p/12D3KooWRLM7PJrtjRM6NZPX8vmdu4YGJa9D6aPoEnLcE1o6aKCd</code></li>
</ul>

<p><code style="font-size:0.66rem;">boot1.jrocnet.com</code> and <code style="font-size:0.66rem;">boot2.jrocnet.com</code> resolve to the current public ingress addresses. Update DNS—not this README—if underlying IPs move.</p>

<p>Run <code style="font-size:0.66rem;">scripts/smoke_net.sh</code> for a local two-node quorum smoke test (ports 7211/7212, 8 s runtime).</p>

<h2 style="font-size:0.82rem;margin:1.2rem 0 0.5rem;">Genesis Anchor (Pinned)</h2>
<p>The A2 testnet ledger is frozen to the following statements and domain-separated BLAKE2b-256 digests (hex). Every node should reproduce these values from its local logs:</p>

<pre style="font-size:0.66rem;line-height:1.5;padding:0.8rem;background:#0d0d0d;color:#f0f0f0;border-radius:0.3rem;overflow:auto;">statement: JULIAN::GENESIS          hash: 139f1985df5b36dae23fa509fb53a006ba58e28e6dbb41d6d71cc1e91a82d84a
statement: Dense polynomial proof   hash: ded75c45b3b7eedd37041aae79713d7382e000eb4d83fab5f6aca6ca4d276e8c
statement: Hash anchor proof        hash: 0f50904f7be06930a5500c2c54cfb6c2df76241507ebd01ab0a25039d2f08f9b</pre>

<p>Boot nodes run with deterministic seeds (<code style="font-size:0.66rem;">ed25519://boot1-seed</code>, <code style="font-size:0.66rem;">ed25519://boot2-seed</code>) so their libp2p Peer IDs remain constant.</p>

<h3 style="font-size:0.78rem;margin:1rem 0 0.4rem;">Verify Your Anchor</h3>
<pre style="font-size:0.66rem;line-height:1.5;padding:0.8rem;background:#0d0d0d;color:#f0f0f0;border-radius:0.3rem;overflow:auto;"># Produce an anchor file from your local logs
julian node run mynode ./logs/mynode mynode.anchor.txt

# Inspect the statements and compare to the pinned digests above
cat mynode.anchor.txt

# Reconcile against a published anchor (example with boot1)
julian node reconcile ./logs/mynode boot1.anchor.txt 2</pre>

<p>To recreate the bootstrap anchors themselves:</p>
<pre style="font-size:0.66rem;line-height:1.5;padding:0.8rem;background:#0d0d0d;color:#f0f0f0;border-radius:0.3rem;overflow:auto;">julian node run boot1 ./logs/boot1 boot1.anchor.txt
julian node run boot2 ./logs/boot2 boot2.anchor.txt

julian node reconcile ./logs/boot1 boot2.anchor.txt 2
julian node reconcile ./logs/boot2 boot1.anchor.txt 2</pre>

<h2 style="font-size:0.82rem;margin:1.2rem 0 0.5rem;">License</h2>
<p>power_house ships under the <strong style="font-weight:600;">Alien Public License 3.0 (APL‑3.0)</strong>:</p>
<ul style="margin:0.3rem 0 0.8rem 1.1rem;">
  <li style="margin:0.2rem 0;">Keep provenance: ship source, logs, and proof transcripts with every redistribution.</li>
  <li style="margin:0.2rem 0;">Attribute <em>“power_house — JULIAN Protocol”</em> in docs, consoles, and research.</li>
  <li style="margin:0.2rem 0;">Disclose fixes, audits, and benchmark data within 30 days of discovery.</li>
  <li style="margin:0.2rem 0;">Ask first for commercial deployment (SaaS, resale, embedded products).</li>
</ul>
<p>See <a href="./LICENSE">LICENSE</a> for the full legal text.</p>

<h2 style="font-size:0.82rem;margin:1.2rem 0 0.5rem;">power_house overview</h2>
<p><strong style="font-weight:600;">power_house</strong> is a paradigmatic exploration of interactive proof systems, finite-field arithmetic, and deterministic pseudo-randomness—all implemented in pure Rust with a focus on reproducibility. It emulates the <em>sum-check protocol</em>, demonstrates a quorum finality primitive, and now backs the JULIAN Protocol ledger.</p>

<h3 style="font-size:0.78rem;margin:1rem 0 0.4rem;">Motivation</h3>

Interactive proof techniques underpin cutting-edge cryptographic protocols and blockchain consensus.
This crate distills those advanced concepts into a standalone laboratory for experimentation, formal verification, and pedagogy.
It emulates the essential features of the **sum-check protocol**, exhibits a **rudimentary Byzantine consensus mechanism**, and now powers the **JULIAN Protocol**—a proof-transparent ledger that anchors folding transcripts into verifiable consensus states.

<h3 style="font-size:0.78rem;margin:1rem 0 0.4rem;">Features</h3>

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

<h2 style="font-size:0.82rem;margin:1.2rem 0 0.5rem;">CLI Workflow</h2>

The `julian` binary exposes both local ledger tooling and the optional `JROC-NET` networking stack.

<h3 style="font-size:0.78rem;margin:1rem 0 0.4rem;">Local ledger (<code style="font-size:0.66rem;">julian node …</code>)</h3>

These commands are always available and require only the standard library:

- `julian node run <node_id> <log_dir> <output>` – recomputes transcript hashes from `<log_dir>`, prepends the JULIAN genesis anchor, and writes a machine-readable anchor file.
- `julian node anchor <log_dir>` – prints a formatted ledger anchor derived from the logs.
- `julian node reconcile <log_dir> <peer_anchor> <quorum>` – recomputes the local anchor, loads a peer’s anchor file, and checks quorum finality.
- `julian node prove <log_dir> <entry_index> <leaf_index> [output.json]` – emits a Merkle proof for a specific transcript digest.
- `julian node verify-proof <anchor_file> <proof_file>` – checks a proof against a stored anchor and exits non-zero on failure.

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

<h3 style="font-size:0.78rem;margin:1rem 0 0.4rem;">Network mode (<code style="font-size:0.66rem;">julian net …</code>, feature <code style="font-size:0.66rem;">net</code>)</h3>

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
  * `--policy governance.json` loads a governance descriptor (`backend: static | static-file | multisig`) and enforces the returned membership set.
  * `--policy-allowlist allow.json` restricts quorum counting to the listed ed25519 keys.
  * `--checkpoint-interval N` writes signed anchor checkpoints every <code>N</code> broadcasts.
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
  --bootstrap /dns4/boot1.jrocnet.com/tcp/7001/p2p/12D3KooWLASw1JVBdDFNATYDJMbAn69CeWieTBLxAKaN9eLEkh3q \
  --broadcast-interval 5000 \
  --quorum 2 \
  --key ed25519://nodeB-seed
```

Each node recomputes anchors from its log directory, signs them, broadcasts envelopes over Gossipsub, and logs finality events once the quorum predicate succeeds.

Run `scripts/smoke_net.sh` to exercise the two-node quorum workflow locally; the script boots nodes on ports 7211/7212, waits for signed anchor broadcasts, confirms finality, and exits non-zero on failure.

<h4 style="font-size:0.72rem;margin:0.9rem 0 0.3rem;">Governance descriptor reference</h4>
<p style="margin:0.3rem 0;">The <code style="font-size:0.66rem;">--policy</code> flag accepts a JSON descriptor with a <code style="font-size:0.66rem;">backend</code> key:</p>
<ul style="margin:0.3rem 0 0.6rem 1.1rem;font-size:0.66rem;line-height:1.6;">
  <li><code>static</code> &mdash; inline allowlist via <code>allowlist: [&quot;base64&quot;,...]</code>.</li>
  <li><code>static-file</code> &mdash; pointer to a legacy allowlist JSON (<code>{"allowed":[...]}</code>).</li>
  <li><code>multisig</code> &mdash; pointer to a state file tracking K-of-N signers and active members.</li>
</ul>
<p style="margin:0.3rem 0;">Example multisig state file:</p>
<pre style="font-size:0.66rem;line-height:1.5;padding:0.8rem;background:#0d0d0d;color:#f0f0f0;border-radius:0.3rem;overflow:auto;">{
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
}</pre>
<p style="margin:0.3rem 0;">To rotate membership, craft a <code style="font-size:0.66rem;">GovernanceUpdate</code> JSON (new member list plus metadata), collect the required signatures offline, and feed it to your operational tooling before replacing the state file.</p>

<h4 style="font-size:0.72rem;margin:0.9rem 0 0.3rem;">Anchor JSON schema</h4>

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

<h4 style="font-size:0.72rem;margin:0.9rem 0 0.3rem;">Signed envelope format</h4>

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

<h3 style="font-size:0.78rem;margin:1rem 0 0.4rem;">JROC-NET Public Testnet (A2) roadmap</h3>

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
     --bootstrap /dns4/boot1.jrocnet.com/tcp/7001/p2p/12D3KooWLASw1JVBdDFNATYDJMbAn69CeWieTBLxAKaN9eLEkh3q \
     --bootstrap /dns4/boot2.jrocnet.com/tcp/7002/p2p/12D3KooWRLM7PJrtjRM6NZPX8vmdu4YGJa9D6aPoEnLcE1o6aKCd \
     --broadcast-interval 5000 \
     --quorum 2 \
     --key ed25519://<seed>
   ```

   Bootstrap multiaddrs (A2 testnet reference):

- `/dns4/boot1.jrocnet.com/tcp/7001/p2p/12D3KooWLASw1JVBdDFNATYDJMbAn69CeWieTBLxAKaN9eLEkh3q`
- `/dns4/boot2.jrocnet.com/tcp/7002/p2p/12D3KooWRLM7PJrtjRM6NZPX8vmdu4YGJa9D6aPoEnLcE1o6aKCd`

The testnet keeps every transcript, proof, and anchor transparent so auditors can replay history end-to-end.

<hr style="border:0;border-top:1px solid #333;margin:1.6rem 0;">

<h2 style="font-size:0.82rem;margin:1.2rem 0 0.5rem;">Examples</h2>

<h3 style="font-size:0.78rem;margin:1rem 0 0.4rem;">Sum-check verification</h3>

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

<h3 style="font-size:0.78rem;margin:1rem 0 0.4rem;">CRT chain showcase</h3>

The `crt_chain` example threads three large primes through a deterministic LCG, combines the outputs
with the Chinese Remainder Theorem, and emits transcript digests derived from the `Field` arithmetic:

```bash
cargo run --example crt_chain
```

It prints a 12-round trace with reproducible totals and hash pairs, highlighting how Power-House components compose into a heavier protocol.

<h3 style="font-size:0.78rem;margin:1rem 0 0.4rem;">General multilinear sum-check</h3>

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

Transcript outputs include deterministic Fiat–Shamir challenges; when logged via the ledger, each record carries a domain-separated BLAKE2b-256 integrity hash for tamper-evident storage.

<h3 style="font-size:0.78rem;margin:1rem 0 0.4rem;">Mega sum-check &amp; chaining demo</h3>

```bash
cargo run --example mega_sumcheck
```

This walkthrough builds 10-variable polynomials, records per-round timings, and chains multiple proofs together before handing them off to the ALIEN ledger scaffold.

<h3 style="font-size:0.78rem;margin:1rem 0 0.4rem;">Scaling benchmark</h3>

```bash
cargo run --example scale_sumcheck
```

Prints a timing table for increasing numbers of variables, helping you profile how multilinear proofs scale as the hypercube size grows.
Set `POWER_HOUSE_SCALE_OUT=/path/to/results.csv` to emit machine-readable timing data alongside the console output.

<h3 style="font-size:0.78rem;margin:1rem 0 0.4rem;">Transcript hash verification</h3>

```bash
cargo run --example verify_logs -- /tmp/power_house_ledger_logs
```

Replays ledger log files, recomputes their integrity hashes, and prints a pass/fail summary so archived transcripts remain tamper-evident.

<h3 style="font-size:0.78rem;margin:1rem 0 0.4rem;">Hash pipeline &amp; anchor reconciliation</h3>

```bash
cargo run --example hash_pipeline
```

Streams per-proof hashes into constant-time anchors, folds them with domain-separated BLAKE2b-256, and reconciles the anchors across multiple ledgers while emitting tamper-evident logs. This example is the reference JULIAN Protocol pipeline: nodes replay transcript logs, exchange `LedgerAnchor` structures, and call `reconcile_anchors_with_quorum` to reach finality.

<h3 style="font-size:0.78rem;margin:1rem 0 0.4rem;">Whitepaper</h3>

The full JULIAN Protocol write-up lives in [`JULIAN_PROTOCOL.md`](JULIAN_PROTOCOL.md).

<h3 style="font-size:0.78rem;margin:1rem 0 0.4rem;">CLI node commands</h3>

```bash
cargo run --bin julian -- node run <node_id> <log_dir> <output_anchor>
cargo run --bin julian -- node anchor <log_dir>
cargo run --bin julian -- node reconcile <log_dir> <peer_anchor> <quorum>
```

These commands replay transcript logs, derive JULIAN anchors, and check quorum finality using nothing beyond the Rust standard library.

</div>

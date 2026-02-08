JROC NET: The JULIAN Protocol Network
Version 0.1.54 — February 2026


# JULIAN Protocol: Proof-Transparent Consensus via Folding-Derived Anchors

## Abstract

We introduce the **JULIAN Protocol**, a fully self-contained Rust implementation of a
proof-transparent ledger that derives consensus from folding-style interactive proofs.
Streaming sum-check provers produce deterministic Fiat–Shamir transcripts that are hashed into
domain-separated BLAKE2b-256 anchors. These anchors are captured inside a ledger structure and
reconciled across peers with a quorum predicate: once at least *q* replicas expose identical anchor
sequences, the ledger state is final. The implementation retains a minimal dependency surface while
leaning on BLAKE2b for transcript authenticity, and ships with reproducible logging and verification
tooling.

## 1. Introduction

Modern proof systems often resemble black boxes: a verifier accepts or rejects proofs, but the
intermediate checks and state are rarely exposed, making multi-party verification difficult.
The JULIAN Protocol takes the opposite approach. Inspired by prior work on transparent,
composable ledgers, we construct a pipeline in which the transcript of every proof
is captured, hashed, and chained into a consensus anchor. Because transcript generation is
deterministic, independently replaying the verification yields identical anchors, enabling multiple
nodes to agree on a ledger’s state simply by comparing ordered hash lists. Polynomial evaluation,
sum-check folding, randomness derivation, and hashing are implemented directly inside the crate,
with BLAKE2b-256 providing deterministic commitments to each transcript.

## 2. System Overview

The JULIAN Protocol is implemented in the `power_house` crate. The key architectural pieces are:

| Component | Responsibility | Key Files |
|-----------|----------------|-----------|
| `StreamingPolynomial` | On-demand evaluation of multilinear polynomials without allocating the entire hypercube | `src/streaming.rs` |
| `GeneralSumProof` | Non-interactive sum-check prover/verifier with deterministic Fiat–Shamir transcripts | `src/sumcheck.rs` |
| `ProofLedger` | Ledger that stores statements, proofs, transcript hashes, Merkle roots, and audit logs | `src/alien.rs` |
| Ledger Anchors | `EntryAnchor` (statements, transcripts, Merkle root) aggregated into `LedgerAnchor` | `src/alien.rs` |
| Quorum Reconciliation | `reconcile_anchors` and `reconcile_anchors_with_quorum` determine validity/finality | `src/alien.rs` |
| Tooling | Examples for benchmarking (`scale_sumcheck`), chaining (`mega_sumcheck`), hashing (`hash_pipeline`), and log verification (`verify_logs`) | `examples/*.rs` |

The prover workflow is:

1. A `StreamingPolynomial` describes the witness polynomial.
2. `GeneralSumProof::prove_streaming_with_stats` folds the polynomial, producing the claimed sum,
   round transcripts, and timing statistics.
3. The ledger records the proof, hashes the transcript, and writes an ASCII log file summarising
   the transcript for external auditing (`write_transcript_record`).
4. Nodes exchange anchor structures and run the reconciliation predicate. If the predicate passes
   with the configured quorum, the ledger state is final for that round.

## 3. Transcript Hashing

During verification the prover records, for each sum-check round:

- the random challenge issued by the Fiat–Shamir transform,
- the running sum claim,
- the final evaluation of the folded polynomial.

The helper in `src/data.rs` converts this tuple into three ASCII lines:

```
transcript:<challenge_0> … <challenge_{k-1}>
round_sums:<s_0> … <s_{k-1}>
final:<value>
```

It then derives a digest by hashing the concatenated `u64` values with a domain-separated
BLAKE2b-256 capacity expander. The resulting 32-byte digest is stored in `LedgerEntry::hashes`
and emitted in the log file as a lowercase hex string (`hash:<digest>`).

This approach has three properties:

1. **Determinism** – For honest executions the hash is independent of the verifier, because the
   Fiat–Shamir challenges are derived from the transcript itself (`Transcript::challenge`).
2. **Tamper evidence** – Any alteration to the log file changes the digest, causing
   `verify_transcript_lines` to reject.
3. **Append-only anchoring** – Transcript hashes are chained in an ordered vector; removing or reordering
   entries is detectable because the index in the chain no longer matches the statement list.

The BLAKE2b-256 domain tag `JROC_TRANSCRIPT` ensures transcripts, anchor folds, and challenge
derivations remain distinct contexts while sharing the same primitive.

Every ledger entry also stores a Merkle root computed over its transcript digests. The root is
exposed alongside the hash list, enabling compact inclusion proofs (`julian node prove` and
`julian node verify-proof`) and keeping anchors stable even when new transcript formats appear.

## 4. Ledger Anchors and Finality

A **ledger anchor** is formalised in `src/alien.rs` as:

- `EntryAnchor { statement: String, hashes: Vec<TranscriptDigest>, merkle_root: TranscriptDigest }`
- `LedgerAnchor { entries: Vec<EntryAnchor> }`

The **validity predicate** `Valid(ledger_anchor)` holds when every hash equals the digest
recomputed from its transcript (`reconcile_anchors`). The **finality predicate**
`Final(anchor_set, quorum)` is implemented by `reconcile_anchors_with_quorum`:

> Finality is achieved when at least `quorum` anchors in `anchor_set` have identical sequences of
> `(statement, hashes)` for every ledger entry.

A typical deployment might set `quorum = f + 1` in a crash-fault tolerant model or `quorum = 2f + 1`
for Byzantine tolerance.

## 5. Multi-Node Reconciliation Protocol

1. **Local verification** – Nodes execute the prover/verifier, generating log files via
   `write_transcript_record`. The `verify_logs` example provides a CLI for replaying log files and
   checking their hashes.
2. **Anchor exchange** – Nodes serialise their local `LedgerAnchor` (e.g., as JSON or CBOR) and broadcast
   it to peers.
3. **Quorum check** – Every node runs `reconcile_anchors_with_quorum`. If the predicate returns
   `Ok(()))`, finality holds; otherwise the function indicates which entry diverged.
4. **Forensics** – Errant anchors are investigated by retrieving the corresponding ASCII log and
   re-running `verify_logs`; this isolates the exact transcript responsible for divergence.

Because all primitives (hashing, polynomial evaluation, transcript generation) use only the standard
library, the protocol remains lightweight and deterministic across platforms.

## 6. Implementation Details

- **Streaming polynomials** (`src/streaming.rs`): avoid allocating `2^n` evaluations by defining an
  evaluator closure `Fn(usize) -> u64`.
- **Sum-check folding** (`src/sumcheck.rs`): `GeneralSumProof::prove_streaming_with_stats` records
  per-round timings, claimed sums, and challenges; `verify_general_sum_streaming` replays the transcript.
- **Ledger** (`src/alien.rs`): accepts `ProofKind::General` and `ProofKind::StreamingGeneral`, emits logs,
  maintains in-memory hashes, and exposes `LedgerAnchor` structures.
- **Logging** (`src/data.rs`, `src/io.rs`): ASCII output with transcript, sums, final value, and hash.
- **Merkle accumulation** (`src/merkle.rs`): every entry records a BLAKE2b-256 Merkle root, allowing
  inclusion proofs without shipping the full transcript list; the CLI exposes `julian node prove`
  and `julian node verify-proof` for auditors.
- **Governance policies** (`src/net/governance.rs`): the networking layer now loads membership
  backends via `--policy` descriptors (static inline lists, referenced allowlist files, multisig
  state machines, or stake-backed registries). Legacy deployments may still pass
  `--policy-allowlist` for a simple read-only set.
- **Checkpoints** (`src/net/checkpoint.rs`): nodes may emit signed anchor checkpoints every
  <code>N</code> broadcasts (`--checkpoint-interval N`), enabling fast bootstrap by replaying only
  logs newer than the last checkpoint.
- **Examples**:
  - `hash_pipeline`: illustrates the end-to-end protocol on two nodes, including log directories.
  - `scale_sumcheck`: streaming benchmark with optional CSV output (`POWER_HOUSE_SCALE_OUT`).
  - `verify_logs`: CLI to validate stored transcripts.

## 7. Performance Evaluation

`cargo run --example scale_sumcheck -- 20` benchmarks streaming sum-check proofs up to 20 variables
(one million evaluations) on a standard laptop CPU. Sample output:

| Vars | Points | Total (ms) | Avg (ms) | Max Round (ms) | Final Eval |
|------|--------|-----------:|---------:|---------------:|-----------:|
| 8    | 256    | 0.35       | 0.04     | 0.15           | 38         |
| 12   | 4096   | 6.49       | 0.54     | 2.84           | 145        |
| 16   | 65536  | 126.79     | 7.92     | 57.27          | 27         |
| 20   | 1,048,576 | 2399.86 | 119.99   | 1135.87        | 24         |

The streaming prover scales linearly with the number of points, proving viability for large
polynomials. The `mega_sumcheck` example demonstrates chained proofs (10 → 6 → 5 variables) and logs
per-round timings for each link in the chain.

## 8. Security Considerations

- **Tamper detection** – Because transcript hashes capture the full Fiat–Shamir transcript, any
  modification of the log file is detected by `verify_logs`. Ledger reconciliation spots divergent
  anchors immediately.
- **Hash collisions** – Transcript digests default to domain-separated BLAKE2b-256. The
  `hash_pipeline` example demonstrates folding those digests into further BLAKE2b-256 anchors,
  and the code is structured so alternative hashers or commitment schemes can be slotted in if
  needed.
- **Quorum assumptions** – `reconcile_anchors_with_quorum` groups votes by anchor digest and counts
  distinct identities (ed25519 public keys) per anchor. Honest deployments must ensure each node
  signs its envelope once per broadcast; offline reconciliation can supply placeholder identities
  when signatures are unavailable.
- **Admission control** – Supply a governance descriptor with `--policy` (or a legacy allowlist with
  `--policy-allowlist`) so only authorised ed25519 keys count toward quorum. Stake-backed identities
  must satisfy the configured bond threshold; nodes automatically slash keys that broadcast
  conflicting anchors and persist that evidence in the staking registry.
- **Checkpoints** – Periodic signed checkpoints reduce replay cost for new nodes. Ensure checkpoint
  files are stored securely (they contain anchor snapshots plus signatures).
- **Streaming closure trust** – Nodes supplying streaming evaluators must ensure identical logic;
  cross-node reconciliation is necessary to catch inconsistencies.

## 9. Use Cases and Future Work

1. **Audit trails** – ASCII logs provide human-readable transcripts; long-term storage can retain
   both the logs and ledger anchors.
2. **Cross-node reconciliation** – The anchor exchange protocol enables different organisations to
   validate shared proof data without sharing the underlying witness.
3. **Recursive proof layering** – Because anchors are deterministic, they can themselves become inputs
   to higher-level proofs (e.g., commitments or succinct proofs).
4. **Hash agility** – Replace or supplement the default hash with cryptographic hash functions or
   polynomial commitments while keeping the same ledger and reconciliation logic.

## 10. Networking Layer (`net` Feature)

To support distributed reconciliation, the crate now ships an optional networking module gated by the
`net` feature flag:

- `net::schema` – serde-compatible structs for `jrocnet.anchor.v1` and `jrocnet.envelope.v1`, plus
  conversion helpers to and from `LedgerAnchor`.
- `net::sign` – ed25519 key loading, deterministic seed derivation, base64 helpers, and signature
  verification (backed by `ed25519-dalek`).
- `net::swarm` – libp2p stack (TCP + Noise + Yamux) with Gossipsub gossip, Kademlia discovery, and
  Identify metadata. Nodes recompute anchors from local logs, sign them, broadcast envelopes, and run
  quorum reconciliation upon receipt. Envelope handling now enforces schema/network identifiers,
  caps payloads to 64 KB/10k entries, maintains an LRU of recently seen payload hashes, and fumes
  invalid senders after repeated mistakes.
- `net::governance` – membership policy trait plus static, multisig, and stake-backed implementations
  (`--policy`).
- `net::policy` – legacy allowlist helper retained for read-only deployments (`--policy-allowlist`).
- `net::checkpoint` – periodic signed checkpoints (`--checkpoint-interval`) for fast ledger bootstrap.

The `julian net` CLI subcommands—`start`, `anchor`, and `verify-envelope`—exercise this layer. They
remain opt-in so the base library stays dependency-free by default.

## 11. Network Hardening

The networking layer now incorporates several safeguards:

- **Envelope versioning** – `schema_version` is embedded in every envelope and rejected if it
  advertises a newer major version than the node understands. Older envelopes default to version `1`
  and continue to work.
- **Resource caps** – Envelopes larger than 64 KB or containing more than 10 k anchor entries are
  dropped before decoding. A SHA-256 LRU cache suppresses duplicate payloads, emitting metrics on
  eviction.
- **Peer hygiene** – Nodes track invalid submissions per peer and log when thresholds are exceeded,
  helping operators downscore abusive sources.
- **Prometheus metrics** – Optional `--metrics` flag exposes counters for received/verified anchors,
  invalid envelopes, LRU evictions, finality events, and Gossipsub rejects to feed observability
  dashboards.
- **Identity hygiene** – Operators may supply passphrase-protected identity files (`--identity`),
  keeping long-lived signing keys off disk in plaintext while still deriving deterministic peer IDs.

## 12. JROC-NET Public Testnet Plan

The A2 testnet targets transparent, tamper-evident anchor gossip:

1. **Topics & behaviour** – Gossip on `jrocnet/anchors/v1`, optional ping/peer topics for liveness;
   Kademlia DHT plus Identify for peer metadata.
2. **Genesis anchoring** – Every broadcast anchor starts with `JULIAN::GENESIS`; the JSON schema
   enforces this invariant alongside the network identifier.
3. **Signed envelopes** – ed25519 signatures cover the raw anchor JSON; peers verify signatures and
   schema before attempting reconciliation.
4. **CLI network mode** – `julian net start` exposes `--listen`, repeated `--bootstrap`, `--key`,
   `--broadcast-interval`, and `--quorum`; audit tooling lives in `net anchor` / `net verify-envelope`.
5. **Security hygiene** – SHA-256 payload-based message IDs, replay caches, strict schema/network
   checks, 64 KB envelope caps, 10k-entry limits, LRU duplicate suppression, and basic invalid-peer
   counters to rate-limit abusive senders.
6. **Observability** – Console summaries track peer counts and recent finality events; the optional
   Prometheus endpoint exports `anchors_received_total`, `anchors_verified_total`,
   `invalid_envelopes_total`, `lrucache_evictions_total`, `finality_events_total`, and
   `gossipsub_rejects_total`.
7. **Launch playbook** – Operate at least two bootstrap nodes, publish their multiaddrs, and share the
   join snippet so community operators can participate with deterministic key seeds.

## 13. Genesis Commitment

The JROC-NET A2 testnet anchors every ledger to a fixed genesis bundle:

- `statement: JULIAN::GENESIS` → `17942395924573474124`
- `statement: Dense polynomial proof` → `1560461912026565426`
- `statement: Hash anchor proof` → `17506285175808955616`

Bootstrap nodes operate with deterministic seeds (`ed25519://boot1-seed`, `ed25519://boot2-seed`) so
their libp2p Peer IDs remain constant across restarts. Operators joining the network should derive
their local anchor via `julian node run` and compare the resulting statements/digests against the
values above before accepting finality.

## 13. Conclusion

The JULIAN Protocol now spans two layers: a dependency-free proof/ledger core and an optional
libp2p-based networking shell. Deterministic transcripts, append-only anchors, and quorum
reconciliation continue to guarantee auditability, while the JROC-NET tooling demonstrates how those
primitives generalise to a public testnet. Future work will focus on richer observability, stronger
hash modes, and commitment layers that compress anchor histories even further.

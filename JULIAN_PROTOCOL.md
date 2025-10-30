JROC NET: The JULIAN Protocol Network
Version 0.1.6 — February 2025


# JULIAN Protocol: Proof-Transparent Consensus via Folding-Derived Anchors

## Abstract

We introduce the **JULIAN Protocol**, a fully self-contained Rust implementation of a
proof-transparent ledger that derives consensus from folding-style interactive proofs.
Streaming sum-check provers produce deterministic Fiat–Shamir transcripts that are hashed into
64-bit anchors. These anchors are captured inside a ledger structure and reconciled across peers
with a quorum predicate: once at least *q* replicas expose identical anchor sequences, the ledger
state is final. The protocol relies exclusively on the Rust standard library—no external hashers or
cryptographic crates—and ships with reproducible logging and verification tooling.

## 1. Introduction

Modern proof systems often resemble black boxes: a verifier accepts or rejects proofs, but the
intermediate checks and state are rarely exposed, making multi-party verification difficult.
The JULIAN Protocol takes the opposite approach. Inspired by the ALIEN theorem’s vision of
transparent, composable ledgers, we construct a pipeline in which the transcript of every proof
is captured, hashed, and chained into a consensus anchor. Because transcript generation is
deterministic, independently replaying the verification yields identical anchors, enabling multiple
nodes to agree on a ledger’s state simply by comparing ordered hash lists. All components—polynomial
evaluation, sum-check folding, randomness derivation, and hashing—use only the Rust standard library,
creating a minimal yet practical foundation for transparent proof ledgers.

## 2. System Overview

The JULIAN Protocol is implemented in the `power_house` crate. The key architectural pieces are:

| Component | Responsibility | Key Files |
|-----------|----------------|-----------|
| `StreamingPolynomial` | On-demand evaluation of multilinear polynomials without allocating the entire hypercube | `src/streaming.rs` |
| `GeneralSumProof` | Non-interactive sum-check prover/verifier with deterministic Fiat–Shamir transcripts | `src/sumcheck.rs` |
| `ProofLedger` | Ledger that stores statements, proofs, transcript hashes, and audit logs | `src/alien.rs` |
| Ledger Anchors | `EntryAnchor` and `LedgerAnchor` aggregate statements with transcript hashes | `src/alien.rs` |
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

It then derives a digest by hashing the concatenated `u64` values with the standard-library
`DefaultHasher`. The resulting `u64` hash is stored in `LedgerEntry::hashes` and emitted in the
log file as `hash:<digest>`.

This approach has three properties:

1. **Determinism** – For honest executions the hash is independent of the verifier, because the
   Fiat–Shamir challenges are derived from the transcript itself (`Transcript::challenge`).
2. **Tamper evidence** – Any alteration to the log file changes the digest, causing
   `verify_transcript_lines` to reject.
3. **Append-only anchoring** – Transcript hashes are chained in an ordered vector; removing or reordering
   entries is detectable because the index in the chain no longer matches the statement list.

Although `DefaultHasher` is not collision-resistant in the cryptographic sense, it is sufficient
for demonstrating the protocol’s mechanics; stronger hash modes can be added (see §9).

## 4. Ledger Anchors and Finality

A **ledger anchor** is formalised in `src/alien.rs` as:

- `EntryAnchor { statement: String, hashes: Vec<u64> }`
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
- **Hash collisions** – The default 64-bit hash is selectable; the `hash_pipeline` example supports
  `xor` and `sum` modes, and the code is structured to introduce stronger hashers while preserving
  the pure-std property.
- **Quorum assumptions** – `reconcile_anchors_with_quorum` assumes a benign communication setting:
  anchors are compared post-transmission. Integrating network transport or signatures is left for
  future work.
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

## 10. Conclusion

The JULIAN Protocol provides an auditable, dependency-free foundation for proof-transparent ledgers.
By streaming sum-check transcripts, hashing them into ledger anchors, and defining finality via a
quorum predicate, we enable multiple nodes to agree on ledger state without opaque verifier behaviour.
Future work will explore stronger hash functions, integration with networking stacks, and recursive
commitment layers.

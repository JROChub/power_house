# Power-House

[![Crates.io](https://img.shields.io/crates/v/power_house.svg)](https://crates.io/crates/power_house)
[![docs.rs](https://docs.rs/power_house/badge.svg)](https://docs.rs/power_house)
[![Build Status](https://img.shields.io/badge/tests-passing-brightgreen.svg)](#)

**Author:** Julian Christian Sanders  
**Email:** [lexluger.dev@proton.me](mailto:lexluger.dev@proton.me)  
**Date** 10/16/2025 

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

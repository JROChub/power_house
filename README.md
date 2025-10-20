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
It emulates the essential features of the **sum-check protocol**, exhibits a **rudimentary Byzantine consensus mechanism**, and outlines a theoretical **blueprint toward a future-proof ledger** as envisioned in the **ALIEN theorem**.

### Features

-  **Finite Field Arithmetic:**
  A lean yet robust implementation of arithmetic modulo a prime, essential for homomorphic operations and algebraic proofs.

-  **Sum-Check Protocol Demo:**
  Illustrates how a prover can succinctly certify a polynomial’s evaluation over a Boolean hypercube, while the verifier checks integrity with negligible soundness error.

-  **Deterministic PRNG:**
  A compact linear-congruential generator serving as a deterministic source of challenge derivation, thereby eliminating external entropy dependencies.

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

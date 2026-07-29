# SFCS RISC Zero Conformance Programs

These checked binaries are deterministic transport fixtures for the
`sfcs-risc0` release gate. They are rebuilt from `risc0-methods/guest` with
RISC Zero 3.0.6 and the guest-builder image pinned in
`risc0-methods/build.rs`.

| Fixture | Source | SHA-256 | RISC Zero image ID | SFCS graph digest |
|---|---|---|---|---|
| `private-sum-v1.bin` | `risc0-methods/guest/src/main.rs` | `216478c6c063c16858f2ef99f4f7880f47feee913047b5a7e74734e98d1dd014` | `risc0:c21cdb0fdc114bfdf531b58b3468014f08e62df52bd0c53a4a1c1e36c407dab2` | `sha256:33f529a4c811b5af442005bf28ffe73a4aef2a40ff5eb7c11b224e0f39e26d10` |
| `private-general-v1.bin` | `risc0-methods/guest/src/general.rs` | `430e550df277c7d9bd70d9d54f19c075feb552154139e5139dd37966d700c9ce` | `risc0:f8df79aab0e78d6f9eb2a95b3c9ee86571a3b348a89abf60d5f230248efc3922` | `sha256:4ff178ff541c72718cb505b13a2b0a3191568dc8837a44548987e80b458e00cc` |

The general fixture covers functions, bounded loops, dynamic word-memory
addressing, byte and halfword updates, signed and unsigned comparisons,
equality and non-equality branches, bitwise operations, rotations,
multiplication, and wrapping arithmetic. `tests/sfcs_risc0_general.rs`
recomputes its public journal independently and requires the fixed vector to
enter every declared branch class.

CI performs all of the following:

1. rebuilds both binaries from a clean locked workspace in the pinned image;
2. compares the rebuilt bytes exactly with these fixtures;
3. compares RISC Zero image IDs and deterministic SFCS graph digests;
4. generates real receipts with development mode disabled;
5. verifies `.pha`, Rootprint, Memory Capsule, and SLBIT separation invariants.

Print an identity locally with:

```bash
cargo run --locked --features sfcs-risc0 \
  --example sfcs_risc0_program_identity -- \
  conformance/sfcs-risc0/private-general-v1.bin
```

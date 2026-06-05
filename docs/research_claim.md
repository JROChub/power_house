# Research Claim Standard

Power-House must not call a result "the first in history" merely because its
implicit domain is extremely large. Classical sum-check, GKR, Spartan, Jolt,
STARKs, and multilinear polynomial commitments already verify computations
whose expanded representations are infeasible to materialize.

Relevant prior art includes:

- Justin Thaler, *Proofs, Arguments, and Zero-Knowledge*:
  https://people.cs.georgetown.edu/jthaler/ProofsArgsAndZK.html
- *A Time-Space Tradeoff for the Sumcheck Prover*:
  https://eprint.iacr.org/2024/524
- Spartan:
  https://github.com/microsoft/Spartan
- Jolt:
  https://eprint.iacr.org/2023/1217
- LiLAC multilinear polynomial commitments:
  https://eprint.iacr.org/2024/1943
- Recent sub-linear GKR work:
  https://eprint.iacr.org/2025/717

## Current Established Result

Power-House has a reproducible public engineering artifact:

> A Rust/Python reproducible, one-million-round deterministic sum-check
> transcript for a separately stored, hash-bound sparse multilinear polynomial
> over a one-million-variable Boolean domain, with a stable 16 MB certificate
> and no hypercube allocation.

This is an engineering result, not an established world-first claim. The v1
verifier reads the complete workload and recomputes the exact expected
transcript. The represented sum also has a closed form. These facts prevent a
succinct-verification or novel-protocol claim. See
[Security Model](security_model.md) and [Prior-Art Review](prior_art_review.md).

## Historical Claim Gates

The project may use a world-first or historical claim only after all gates are
complete:

1. **Protocol specification**  
   Publish the polynomial derivation, transcript, binary certificate format,
   field assumptions, complexity analysis, and security limitations.

2. **Immutable artifact**  
   Publish source, certificate, manifest, checksums, compiler version, hardware
   details, and exact commands in a timestamped release or archival DOI.

3. **Independent implementation**  
   Obtain verification by an implementation authored by an unaffiliated team.
   The bundled Python verifier is useful cross-language conformance evidence
   but is not an independent external audit.

4. **Prior-art comparison**  
   Compare the result directly against sum-check, sparse-dense sum-check, GKR,
   Spartan, Jolt, Scribe, and multilinear commitment systems. Domain size alone
   is not a valid comparison metric.

5. **External reproduction**  
   At least two unaffiliated parties must reproduce the certificate digest and
   publish machine details and timings.

6. **Cryptographic scope**  
   The `PHSMv1`/`PHCPv1` workflow binds public external data with BLAKE2b-256.
   For a general or succinct verifiable-computation claim, replace full
   workload replay with a reviewed multilinear polynomial commitment and an
   opening proof for an externally supplied computation or witness. A standard
   one-repetition sum-check at the published million-round field parameters has
   only about 9.97 bits under the classical `n/|F|` bound.

7. **Public review**  
   Publish a technical preprint and obtain specialist review or a formal audit.

## Claim Levels

- **Allowed now:** "Power-House deterministically replays a separately stored,
  hash-bound sparse polynomial transcript over a one-million-variable Boolean
  domain through a million-round reproducible certificate."
- **Allowed after external reproduction:** "Power-House publishes an
  independently reproduced million-round sparse sum-check artifact."
- **Allowed after novelty review:** A narrowly worded "first" claim matching
  exactly what the literature review and audit establish.
- **Not currently allowed:** "Quantum computers were previously required" or
  "first system to verify computations beyond sextillion scale."

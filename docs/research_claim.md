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

## Current Candidate Result

Power-House has a candidate public engineering record:

> A cross-language reproducible, one-million-round sum-check certificate for a
> separately stored, commitment-bound sparse multilinear polynomial over
> `2^1,000,000` Boolean points, with a stable 16 MB certificate and no
> hypercube allocation.

This wording is a candidate, not an established world-first claim. An exhaustive
literature and artifact search has not yet been completed, and no independent
external party has reproduced the result.

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
   Obtain verification by an implementation not authored from the Rust code.
   The bundled Python verifier is useful cross-language evidence but is not an
   independent external audit.

4. **Prior-art comparison**  
   Compare the result directly against sum-check, sparse-dense sum-check, GKR,
   Spartan, Jolt, Scribe, and multilinear commitment systems. Domain size alone
   is not a valid comparison metric.

5. **External reproduction**  
   At least two unaffiliated parties must reproduce the certificate digest and
   publish machine details and timings.

6. **Cryptographic scope**  
   The `PHSMv1`/`PHCPv1` workflow now binds public external data with
   BLAKE2b-256. For a general or succinct verifiable-computation claim, replace
   full workload replay with a proven multilinear polynomial commitment and an
   opening proof for an externally supplied computation or witness.

7. **Public review**  
   Publish a technical preprint and obtain specialist review or a formal audit.

## Claim Levels

- **Allowed now:** "Power-House verifies a separately stored,
  commitment-bound sparse polynomial over `2^1,000,000` points through a
  million-round reproducible certificate."
- **Allowed after external reproduction:** "Power-House publishes an
  independently reproduced million-round sparse sum-check artifact."
- **Allowed after novelty review:** A narrowly worded "first" claim matching
  exactly what the literature review and audit establish.
- **Not currently allowed:** "Quantum computers were previously required" or
  "first system to verify computations beyond sextillion scale."

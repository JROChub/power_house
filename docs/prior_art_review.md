# Prior-Art Review

Status: active technical review for Power House v0.3.15.

## Question Under Review

The narrow candidate question is:

> Does Power-House introduce a novel proof protocol, or does it publish a
> distinctive reproducibility artifact built from established sum-check
> techniques?

The evidence currently supports the second description.

## Primary Sources

| Work | Relevant result | Consequence for Power-House |
| --- | --- | --- |
| Lund, Fortnow, Karloff, and Nisan, 1992, [Algebraic Methods for Interactive Proof Systems](https://doi.org/10.1145/146585.146605) | Introduced the sum-check protocol used to verify sums of low-degree multivariate polynomials over exponentially large domains. | An enormous implicit Boolean domain is established prior art. |
| Goldwasser, Kalai, and Rothblum, [Delegating Computation: Interactive Proofs for Muggles](https://dl.acm.org/doi/10.1145/2699436) | Applies layered arithmetization and sum-check to delegated computation. | Efficient verification of structured computation is established prior art. |
| Setty, [Spartan](https://eprint.iacr.org/2019/550) | Builds transparent arguments for R1CS using multilinear extensions and sum-check techniques. | General-purpose transparent arguments already use multilinear sum-check machinery. |
| Ben-Sasson et al., [Scalable, Transparent, and Post-Quantum Secure Computational Integrity](https://eprint.iacr.org/2018/046) | Introduces the STARK framework for transparent scalable computational integrity. | Large-scale classical verification does not inherently require quantum hardware. |
| Setty, Thaler, and Wahby, [Unlocking the Lookup Singularity with Lasso](https://eprint.iacr.org/2023/1216) | Uses sum-check-oriented techniques for efficient lookup arguments and includes sparse-structure optimizations. | Sparse structure and sum-check optimization are active established areas. |
| Arun et al., [Jolt](https://eprint.iacr.org/2023/1217) | Constructs a virtual-machine argument primarily from lookups and sum-check. | Modern systems already verify rich computations over implicit structures. |
| Chiesa, Fedele, Fenzi, and Zitek-Estrada, [A Time-Space Tradeoff for the Sumcheck Prover](https://eprint.iacr.org/2024/524) | Studies prover memory and running-time tradeoffs for multilinear sum-check. | Prover scaling and streaming constraints are not new research questions. |
| Baweja et al., [Scribe: Low-memory SNARKs via Read-Write Streaming](https://www.usenix.org/conference/usenixsecurity26/presentation/baweja) | Builds and evaluates a low-memory SNARK using a read-write streaming model and disk-backed prover state. | A low-memory claim must compare against modern streaming proof systems. |

## Feature Comparison

| Property | Power-House v1 | Established systems |
| --- | --- | --- |
| Exponential implicit domain | Yes | Core sum-check property since LFKN |
| Sparse multilinear representation | Yes | Common in modern multilinear protocols |
| Fiat-Shamir transcript | Yes | Standard non-interactive compilation technique |
| Public-data hash binding | Yes | Standard collision-resistant commitment pattern |
| Succinct verifier in workload size | No | Provided by several argument/PCS systems |
| Hidden witness | No | Supported by zero-knowledge argument systems |
| General computation arithmetization | No | GKR, Spartan, STARKs, Jolt, and others |
| Stable million-round public artifact | Yes | Potential benchmark distinction; novelty unverified |
| Cross-language deterministic replay | Rust and Python | Engineering evidence, not protocol novelty |

## Current Conclusion

No protocol-level historical claim is justified by the current evidence.
Specifically:

- `2^1,000,000` is a description of an implicit domain, not executed work.
- the sparse monomial sum is available in closed form,
- the v1 verifier reads the entire public workload,
- exact transcript replay is deterministic conformance checking,
- BLAKE2b binding is not a multilinear polynomial commitment.

The strongest supportable statement is:

> Power-House publishes a stable, cross-language reproducible million-round
> deterministic sum-check transcript for a separately stored, hash-bound sparse
> multilinear polynomial over a one-million-variable Boolean domain.

Whether that artifact is the largest or first public artifact of its exact kind
requires a broader artifact search and independent review.

## Novelty Path

A potentially publishable contribution needs at least one property not supplied
by the v1 artifact:

1. a genuinely lower-memory or faster prover with a proved complexity
   improvement over current sparse/streaming sum-check methods,
2. a commitment/opening construction that avoids full public-workload replay,
3. a useful computation arithmetization whose verification advantage survives
   comparison with GKR, Spartan, STARK, Lasso/Jolt, and current sum-check work,
4. independently measured engineering results that establish a reproducible
   record without presenting that record as a new protocol.

## Review Procedure

Before any novelty claim:

1. search IACR ePrint, DBLP, ACM, IEEE, USENIX, and artifact repositories using
   protocol properties rather than only the phrase "million-round";
2. record inclusion criteria and negative search results;
3. send the exact claim and protocol specification to at least two unaffiliated
   specialists;
4. publish reviewer conflicts and requested corrections;
5. phrase any surviving claim narrowly enough to be falsifiable.

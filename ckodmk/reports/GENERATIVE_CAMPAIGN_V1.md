# CKODMK deterministic generative campaign v1 results

Run date: 2026-08-10  
Report SHA-256: `14efaaee77b2fa885e163f6fd3d08c19b31e21b96174d16de1d2ef32ef5e9ac7`  
Runner SHA-256: `5b489df4880ee5fd346fd56252e8b9f4b7ac9cbe60c736294c7353221c0c653d`

## Result

The retained deterministic run completed **1,000,000 / 1,000,000 cases** with
zero observed failures.

| Family | Cases | Principal observed coverage |
|---|---:|---|
| Restricted JSON parser | 250,000 | 31,250 each across two valid and six invalid byte grammars |
| Normalization | 50,000 | 45,956 reductions; 4,044 unchanged-size fixed points; 1,639 box-scoped and 48,361 global statements |
| Rational equivalence | 400,000 | 12,592 box-scoped and 387,408 global source/target point comparisons |
| Certificate mutation | 150,000 | 25,000 each across source digest, target digest, scope, evidence kind, specification, and unknown-field mutations |
| Composition | 150,000 | 27,256 accepted and 122,744 rejected; 37,500 each across exact/exact, bound/bound, property/property, and unregistered exact/bound pairs |

The Rust exact checker was additionally invoked on 128 generated genuine
certificates and 128 generated mutated certificates. It accepted all genuine
samples and rejected all hostile samples. The checker bytes were:

- filename: `ckodmk-check`;
- byte count: `975296`;
- SHA-256: `2cb4c08d532f1c6122bd4b62e0e4582b114298a202c7bcc316b4bafaf6f5ac54`.

The authoritative result is
[`GENERATIVE_CAMPAIGN_V1.json`](GENERATIVE_CAMPAIGN_V1.json). The method,
allocation, oracle construction, determinism rules, and reproduction command
are in the [campaign specification](../specs/generative-campaign-v1.md).

## Interpretation

This closes the internal million-case denominator in rubric D2 at evidence
level E2. It does not earn external E3 credit and does not satisfy hard gate H5.
The generated run is not a proof, a qualified audit, a non-author-controlled
campaign, a deployed-model test, or evidence of universal correctness.

Only 256 certificate cases crossed into the separately implemented Rust
checker. The remaining certificate denominator establishes distinct canonical
mutations, not 150,000 Rust executions. Composition cases test the registered
rule shape against a second predicate; they do not establish Python/Rust
conformance with the Coq model. These limitations are part of the result.

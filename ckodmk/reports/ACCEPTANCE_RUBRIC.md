# CKODMK evidence-weighted acceptance rubric

Rubric version: 1.0  
Target artifact line: CKODMK 0.2.x and successors  
Threshold for a rating above 9/10: **at least 91/100 and every hard gate passed**

## Purpose

This rubric evaluates CKODMK as a research and deployment-assurance product. It
does not score ambition, prose, test count, or developer confidence. Points are
earned by preserved evidence tied to a frozen artifact.

A score above 9/10 means that CKODMK met this rubric for one declared version
and evaluation scope. It does not mean universal correctness, universal model
equivalence, perfect security, or fitness for every deployment.

## Evaluation protocol

Before scoring, the evaluator MUST freeze and record:

- source revision or archive digest;
- Python and Rust package/build lockfiles;
- checker binary digest;
- schema and specification versions;
- test, model, dataset, mutation-corpus, toolchain, and device digests;
- evaluator identity, conflicts, and independence relationship;
- evaluation start time and preregistered thresholds.

Results produced before this freeze may be useful diagnostics but do not count
as blinded or independent evidence. A behavior-changing change invalidates the
affected measurements and requires a new evidence snapshot.

The evaluator MUST preserve failures, timeouts, unsupported cases, exclusions,
and raw results. Post hoc removal of difficult cases invalidates the affected
category.

## Evidence weights

Each criterion has a maximum point value. Unless its row imposes a stronger
requirement, the following multiplier applies to the evidence supporting it:

| Evidence level | Required record | Multiplier |
|---|---|---:|
| E0 — assertion | Prose, screenshot, unrecorded command, or author statement | 0% |
| E1 — inspectable | Versioned source/specification or static artifact, but no reproducible result | 25% |
| E2 — internally reproducible | One-command run with inputs, environment, raw output, and digests | 50% |
| E3 — independently established | Mechanically checked theorem where applicable, or reproduction/audit by a qualified non-author using frozen public artifacts | 100% |

The maximum points in a rubric row describe the full-credit E3 result. A row
that explicitly requires external evidence receives no more than half credit
from internal evidence, regardless of test volume.

Independent means the evaluator did not implement the feature under test, did
not receive hidden answers, and reports organizational and financial conflicts.
Using a second process, language, model, or agent controlled by the same author
is useful diversity but not external independence.

## Hard gates

All hard gates must pass before CKODMK can qualify above 9/10. Points cannot
compensate for a failed or missing gate.

| Gate | Passing evidence | Status |
|---|---|---|
| H1 — claim hygiene | Public materials contain no universal "same intelligence," category-novelty, or exactness claim outside a declared semantics, scope, and boundary. | `PASS — INTERNAL REVIEW`: current materials explicitly retire those claims; external review is still required for E3 credit. |
| H2 — mechanized core | A proof assistant checks artifact continuity, scope non-expansion, registered relation composition, evidence non-escalation, and trust propagation for the core assurance calculus. | `PASS — INTERNAL STRICT SUBSET`: Coq 8.20.1 checks the modeled core; the [hash-bound report](formal/proof-report.json) explicitly excludes implementation conformance, leaf-checker soundness, and the unmodeled specification. |
| H3 — checker independence | A qualified external audit finds no shared implementation authority between producer and checker and validates semantic agreement with the specification. | `PENDING` |
| H4 — soundness defects | No unresolved critical or high-severity false-accept defect exists in the frozen release; all historical reproduced failures have regression evidence. | `PASS — INTERNAL SNAPSHOT`: final exact, Gate, adjudicator, and independent-checker reviews found no open critical/high defect; this is not a qualified external audit. |
| H5 — blinded hostile campaign | A non-author controls at least 1,000 preregistered semantics-changing mutations; CKODMK returns zero false `PASS` decisions in the declared supported class and reports an exact confidence bound. | `PENDING` |
| H6 — real deployment | The frozen protocol covers at least six real trained models, two architecture families, two independent transformation toolchains, and two named physical target profiles. | `PENDING — TOOLCHAIN PORTION INTERNALLY EXECUTED`: ONNX Runtime and ONNXScript candidates now cover six models/two families; a second named physical target and external reproduction remain absent. |
| H7 — matched baselines | CKODMK is compared at matched evaluation cost with hash-only provenance, ordinary differential testing, and the strongest applicable existing validator. | `PENDING` |
| H8 — external reproduction | A qualified third party reproduces conformance verdicts and aggregate real-device results from the published package without private guidance. | `PENDING — SIGNED HANDOFF READY`: the evaluator checklist, phone-report consumer, strict signed-return schema, OpenSSH identity verification, and hostile return-consumer tests exist; no third party has executed or signed a result. |
| H9 — reproducible disclosure | Public or auditor-accessible artifacts include raw data, failures, exclusions, scripts, dependency locks, licenses, SBOM, and digests sufficient to repeat the evaluation. | `PENDING — PRIVATE PACKAGE COMPLETE INTERNALLY`: the deterministic archive requires both labs, both Python locks, all SBOMs, raw campaign evidence, governance, the evaluator handoff, and the signed-return consumer; external access and reproduction remain unconfirmed. |
| H10 — consequential use | At least two qualified external teams use CKODMK to govern a real release decision and document integration cost, false alarms, interpretation, and retention intent. | `PENDING` |

`PENDING` is not failure. It means no qualifying evidence has yet been entered.

## Scored criteria

### A. Thesis and prior-art discipline — 12 points

| ID | Criterion | Full-credit evidence | Points |
|---|---|---|---:|
| A1 | Falsifiable thesis | One precise research hypothesis, formal target theorem, explicit nonclaims, and kill criteria agree across code, specification, paper, and marketing. | 3 |
| A2 | Closest prior art | Claim-by-claim comparison includes proportional lumping, LEO/LEO++, TASO, TENSAT, MLIR-TV, neural equivalence/proof-transfer work, independent proof checking, TorchLean, composable MLOps assurance, and proof-carrying code using primary sources. | 4 |
| A3 | Defensible delta | The claimed delta is limited to relational artifact binding, semantic-profile reconciliation, no-claim-escalation, and deployment-boundary evaluation; no ingredient is called universally novel. | 3 |
| A4 | Search transparency | A systematic search log records databases, queries, dates, inclusion/exclusion rules, related patents, reviewer, and unresolved overlaps. | 2 |

Minimum category score for 9+ qualification: **10/12**.

### B. Formal assurance model — 22 points

| ID | Criterion | Full-credit evidence | Points |
|---|---|---|---:|
| B1 | Operational semantics | Executable or formal semantics cover all constructs in the proved subset, including numeric behavior and edge cases; unspecified behavior cannot receive exact status. | 5 |
| B2 | Typed evidence vector | Artifact, relation, parameters, scope, semantics, boundary, evidence kind, provenance, TCB, and verdict are mandatory and machine validated; there is no fake global lattice. | 4 |
| B3 | Mechanized no-escalation theorem | A proof assistant checks composition soundness, domain non-expansion, evidence non-promotion, and conservative trust propagation. | 8 |
| B4 | Explicit trust/threat model | Every parser, checker, importer, runtime, kernel, device, data source, cryptographic assumption, and opaque step is included or explicitly excluded. | 3 |
| B5 | Refusal semantics | `PASS`, `FAIL`, `INCONCLUSIVE`, and malformed-input rejection are disjoint, documented, and covered by executable cases. | 2 |

Minimum category score for 9+ qualification: **18/22**. B3 is also a hard gate.

### C. Checker and artifact integrity — 18 points

| ID | Criterion | Full-credit evidence | Points |
|---|---|---|---:|
| C1 | Independent checker | Consumer-side checker shares no parser, evaluator, optimizer, or certificate-authority implementation with the producer; semantic agreement is independently reviewed. | 5 |
| C2 | Unambiguous representation | Duplicate keys, floats where forbidden, noncanonical rationals, Unicode/cross-runtime ordering ambiguity, type confusion, unknown fields, and trailing input are rejected consistently. | 3 |
| C3 | Fail-closed publication | Candidate generation, coefficient growth, output paths, checker failure, timeout, and partial writes cannot publish an accepted bundle. | 3 |
| C4 | Cross-implementation conformance | Two implementations agree on 100% of a versioned positive/negative corpus, including boundary and resource-limit cases. | 4 |
| C5 | Supply-chain integrity | Hermetic or locked builds, reproducible release procedure, SBOM, license manifest, source/binary binding, signatures, and key policy are published and audited. | 3 |

Minimum category score for 9+ qualification: **15/18**.

### D. Adversarial correctness and robustness — 14 points

| ID | Criterion | Full-credit evidence | Points |
|---|---|---|---:|
| D1 | Historical regressions | Every reproduced predecessor defect has a minimal regression: second-pass cancellation, producer/checker coefficient closure, swallowed counterexample, malformed type acceptance, digest/profile substitution, and false metric evidence. | 3 |
| D2 | Generative testing | At least 1,000,000 seeded parser, normalization, equivalence, certificate, and composition cases run with coverage and failure minimization; oracle construction is documented. | 4 |
| D3 | Blinded semantic mutation | At least 1,000 non-author-controlled semantic mutations yield zero false passes in supported cases; unsupported cases are counted, not discarded, and exact confidence bounds are reported. | 5 |
| D4 | Resource safety | Time, bytes, dimensions, scalar slots, nesting, coefficient/intermediate growth, memory, subprocess, and output-file limits are attacked and fail closed. | 2 |

Minimum category score for 9+ qualification: **12/14**.

### E. Real-world external validity — 18 points

| ID | Criterion | Full-credit evidence | Points |
|---|---|---|---:|
| E1 | Real models | At least six unmodified, publicly identified trained models from two architecture families are evaluated; synthetic redundancy is reported separately. | 3 |
| E2 | Independent toolchains | At least two non-CKODMK transformation/compiler pipelines are evaluated with exact versions, flags, and intermediate artifacts. | 4 |
| E3 | Named physical targets | At least two retail or otherwise independently obtainable devices are measured with OS, runtime, driver, firmware, power, temperature, and repeat protocol fixed. | 4 |
| E4 | Fair baselines | Hash-only provenance, matched-budget differential testing, and the strongest applicable prior validator use preregistered budgets and common fault sets. | 3 |
| E5 | Coverage and failures | Static and dynamic operator/stage coverage, false pass, false block, inconclusive, timeout, unsupported, and per-fault-class results include confidence intervals and negative findings. | 4 |

Minimum category score for 9+ qualification: **15/18**.

### F. Efficiency and reproducibility — 8 points

| ID | Criterion | Full-credit evidence | Points |
|---|---|---|---:|
| F1 | Practical overhead | On the preregistered corpus, p95 checking overhead is below 10% of build time and evidence is below 5% of deployed artifact bytes, or external users document why a larger cost is acceptable. | 3 |
| F2 | One-command reproduction | A clean, pinned environment regenerates schemas, examples, tests, reports, wheels/binaries, manifests, and aggregate tables with documented sources of nondeterminism. | 3 |
| F3 | Honest negative results | Timeouts, unsupported operators, failed hypotheses, outliers, and deviations from protocol are preserved in machine-readable form. | 2 |

Minimum category score for 9+ qualification: **6/8**.

### G. Product usefulness and governance — 8 points

| ID | Criterion | Full-credit evidence | Points |
|---|---|---|---:|
| G1 | Consequential external use | At least two qualified teams use CKODMK for a real release decision; evidence records defects blocked, false alarms, integration labor, and retention or payment intent. | 3 |
| G2 | Workflow integration | A documented CI/release integration cannot be bypassed silently and preserves signed, content-addressed reports. | 2 |
| G3 | Claim comprehension | A preregistered user study shows intended users distinguish exact, finite, statistical, observational, integrity, and inconclusive results without material overclaiming. | 2 |
| G4 | Governance | License compatibility, vulnerability reporting, schema evolution, key rotation/revocation, support policy, and responsible disclosure are documented. | 1 |

Minimum category score for 9+ qualification: **5/8**.

## Point summary

| Category | Maximum | 9+ minimum | Earned |
|---|---:|---:|---:|
| A. Thesis and prior art | 12 | 10 | `PENDING` |
| B. Formal assurance model | 22 | 18 | `PENDING` |
| C. Checker and artifact integrity | 18 | 15 | `PENDING` |
| D. Adversarial correctness | 14 | 12 | `PENDING` |
| E. Real-world external validity | 18 | 15 | `PENDING` |
| F. Efficiency and reproducibility | 8 | 6 | `PENDING` |
| G. Product usefulness and governance | 8 | 5 | `PENDING` |
| **Total** | **100** | **91 overall** | **NOT SCORED** |

All category minima and all hard gates apply in addition to the 91-point total.

## Current evaluation worksheet

The following internal snapshot is recorded so that development evidence is not
mistaken for a qualification evaluation:

| Item | Current value |
|---|---|
| Real-model lab manifest | `sha256:e0abf8731a4bd9b5ae54fa09734c822e2bfaa94d9263119950c84ce988b0e250` |
| Real-model lab summary | `sha256:fab6200e0e75c19d81d9c5f7aaa9f7bf193af12fd689647f11214f0e722c0303` |
| Internal protocol | `sha256:77f3dd5096db758cc1b31666dcc1f904048b0ca5da9699a9958a7bbcdb6d56fc` |
| Rust Gate adjudicator | `sha256:58c0cd14fc5d5e96f0a4f1e98ff3a225362de34e5c5a261b7c795bac3ec14447` |
| Real-model scope | 3 retained MLPs plus 3 browser RBF models, 2 families, 1 dataset, browser execution in Chromium and WebKit, ONNX Runtime and ONNXScript external candidate generation, CKODMK browser generation, 1 physical CPU; second-device evidence remains absent |
| Second-toolchain lab | ONNXScript 0.7.1, 9 Gemm fusions, 6/6 Gate/Rust/Polygraphy passes, 3 NumPy-profile passes and 3 explicit unsupported RBF results; manifest `sha256:e8ddce2d7a9a3a0b5a0dbe8afa31f6cac8eed914698ced1cca4b4ba7a257d978`; internal E2 only |
| External evaluator handoff | Deterministic private archive requires both retained labs, both locks, all SBOMs and raw evidence; `AUDITOR_HANDOFF.md` defines clean execution, non-author mutation control, denominator retention, and a signed return package; `verify_external_evaluation.py` authenticates caller-trusted OpenSSH identity and recomputes the minimum profile without claiming to re-execute the evidence |
| Governance | `GOVERNANCE.md` defines schema evolution, support, vulnerability handling, release authority, signing and revocation requirements, license/data governance, retention, and change control; current releases remain unsigned and protected-branch/private-attestation enforcement is unavailable on the current repository plan |
| Browser physical-target return path | Phone-study v2 adds an opt-in evaluator-entered target record, a random session nonce, and a strict consumer that recomputes all schedules and summaries; entered target and timing authenticity remain unattested |
| Exact suites | Python 18/18; Rust exact checker 2/2 |
| Gate suites | Python Gate 15/15; Rust adjudicator 18/18 |
| Independent ONNX checker | NumPy implementation 38/38; no Gate, ONNX Runtime, or Polygraphy execution code; external audit absent |
| Mechanized core | Coq 8.20.1; 29/29 exported theorems closed under the recorded global context |
| Internal hostile outcome | V3 passed: 120 target faults (100 `BLOCK`, 20 expected `INCONCLUSIVE`, 0 false `PASS`), 9/9 controls `PASS`, one evidence fabrication blocked |
| Internal campaign report | `sha256:41fae9b583100a267047968e3ff237d4584647aa92dc519d61ef07cee3f3ce2d` |
| Internal generative campaign | 1,000,000/1,000,000 cases passed; report `sha256:14efaaee77b2fa885e163f6fd3d08c19b31e21b96174d16de1d2ef32ef5e9ac7`; internal E2 only |
| Customer/external use | 0 customers, 0 qualified interviews, 0 pilots, 0 external reproductions |

The Rust Gate adjudicator is separately implemented decision logic but does not
execute ONNX; raw outputs, timings, and replay remain Python-runner assertions.
The independent NumPy implementation re-executes a narrow ONNX subset but does
not evaluate device or latency claims. The training history is producer-reported.
The V3 campaign was internally agent-separated, not controlled by a qualified
non-author. These facts cap the current material at internal E2 evidence and do
not pass gates that require E3 evidence.

This qualification block remains incomplete and may be filled only after a
frozen independent evaluation and evidence review.

```text
CKODMK qualification archive digest:     PENDING
Specification version/digest:            PENDING
Checker source and binary digests:        PENDING
Evaluation protocol digest:               PENDING
Evaluator and independence declaration:   PENDING
Evaluation start/end:                      PENDING
Raw evidence index:                        PENDING

Hard gates passed:                         PENDING / 10
Raw weighted score:                        PENDING / 100
Category minimums satisfied:               PENDING
Unresolved critical/high defects:          PENDING
External reproduction result:              PENDING

Qualification above 9/10:                  NOT YET EVALUATED
```

`NOT YET EVALUATED` is not evidence of failure and must not be rewritten as a
pass. Internal test results may populate E2 evidence, but they cannot substitute
for the hard external gates.

## Score interpretation

| Result | Interpretation |
|---|---|
| 91–100, all gates and minima passed | Qualifies above 9/10 for the frozen scope and artifact only. |
| 80–90, or any hard gate missing | Strong research prototype, but not qualified above 9/10. |
| 65–79 | Promising prototype with material evidence or external-validity gaps. |
| 40–64 | Early implementation; claims should remain laboratory-scoped. |
| 0–39 | Thesis or evidence base is not yet credible. |

The numerical score must always be published beside gate status. Reporting a
high raw score while hiding a failed hard gate is itself a claim-hygiene failure.

## Re-evaluation rule

Any change to semantic rules, parser, checker, normalizer, evidence format,
composition logic, dependency versions, runtime profile, or release policy
requires impact analysis. Affected criteria return to `PENDING` until their
evidence is regenerated. Historical scores remain attached to their original
artifact digests and must not be carried forward by version number alone.

# CKODMK internal readiness score

Artifact: `v0.2.0-evidence.4` at `965600821fef3c4a2dd580be3296ee0f5f1b1dbe`  
Evaluation time: `2026-08-10T23:59:00Z`  
Status: internal self-assessment; not an external qualification

**Weighted internal score: 41.2125 / 100 (4.12125 / 10).**
Hard gates with internal pass status: **3 / 10**.
Qualification above 9/10: **NO**.

The score applies the rubric's evidence multipliers and an explicit scope-coverage factor. Internal E2 evidence receives at most half credit. Missing external evidence receives no substitute credit.

## Category results

| Category | Awarded | Maximum | 9+ minimum | Minimum met |
|---|---:|---:|---:|---|
| A. Thesis and prior art | 4.5 | 12 | 10 | NO |
| B. Formal assurance model | 12.35 | 22 | 18 | NO |
| C. Checker and artifact integrity | 8.775 | 18 | 15 | NO |
| D. Adversarial correctness | 4.8 | 14 | 12 | NO |
| E. Real-world external validity | 6.8 | 18 | 15 | NO |
| F. Efficiency and reproducibility | 2.6875 | 8 | 6 | NO |
| G. Product usefulness and governance | 1.3 | 8 | 5 | NO |

## Criterion ledger

| ID | Level | Coverage | Awarded / max | Missing evidence |
|---|---|---:|---:|---|
| A1 | E2 | 100.00% | 1.5 / 3 | External thesis review. |
| A2 | E1 | 100.00% | 1 / 4 | Independent literature review. |
| A3 | E2 | 100.00% | 1.5 / 3 | External review of the surviving delta. |
| A4 | E1 | 100.00% | 0.5 / 2 | Professional patent search and independent reviewer. |
| B1 | E3 | 45.00% | 2.25 / 5 | Mechanized semantics for certificates, parsers, resource limits, every assurance construct, Float32 runtime, and deployment kernels. |
| B2 | E2 | 100.00% | 2 / 4 | External conformance review. |
| B3 | E3 | 70.00% | 5.6 / 8 | Mechanized parser/resource semantics, leaf-checker soundness, and Python/Rust implementation conformance. |
| B4 | E2 | 100.00% | 1.5 / 3 | External threat-model review. |
| B5 | E2 | 100.00% | 1 / 2 | External negative-case reproduction. |
| C1 | E2 | 100.00% | 2.5 / 5 | Qualified external audit. |
| C2 | E2 | 100.00% | 1.5 / 3 | External parser differential campaign. |
| C3 | E2 | 100.00% | 1.5 / 3 | External filesystem and failure-injection review. |
| C4 | E2 | 100.00% | 2 / 4 | Independent corpus execution and a mechanized source-code refinement proof. |
| C5 | E2 | 85.00% | 1.275 / 3 | MFENX owner-root authorization, protected branch, and externally audited provenance. |
| D1 | E2 | 100.00% | 1.5 / 3 | External replay. |
| D2 | E2 | 100.00% | 2 / 4 | Non-author generation and coverage review. |
| D3 | E2 | 12.00% | 0.3 / 5 | 880 additional mutations and non-author control of the full campaign. |
| D4 | E2 | 100.00% | 1 / 2 | External resource-exhaustion audit. |
| E1 | E2 | 100.00% | 1.5 / 3 | External reproduction and broader model provenance. |
| E2 | E2 | 100.00% | 2 / 4 | Independent execution. |
| E3 | E2 | 50.00% | 1 / 4 | A second independently obtainable physical target with full telemetry. |
| E4 | E2 | 60.00% | 0.9 / 3 | Fully matched external budgets and customer-current-process baseline. |
| E5 | E2 | 70.00% | 1.4 / 4 | External coverage study with the full required denominator. |
| F1 | E1 | 25.00% | 0.1875 / 3 | Preregistered overhead calculation or external acceptance of a larger cost. |
| F2 | E2 | 100.00% | 1.5 / 3 | Independent clean-environment reproduction. |
| F3 | E2 | 100.00% | 1 / 2 | External confirmation that exclusions are complete. |
| G1 | E0 | 0.00% | 0 / 3 | Two qualified teams governing real release decisions. |
| G2 | E2 | 80.00% | 0.8 / 2 | Non-bypassable protected release authority and signed reports. |
| G3 | E0 | 0.00% | 0 / 2 | Target-user study with retained questions, responses, scoring, and overclaiming threshold. |
| G4 | E2 | 100.00% | 0.5 / 1 | External governance and legal review. |

## Hard gates

| Gate | Status | Missing evidence |
|---|---|---|
| H1 | `PASS_INTERNAL` | Qualified external claim review. |
| H2 | `PASS_INTERNAL_SUBSET` | Concrete Python/Rust conformance to the rational rewrite and box definitions, plus the unmodeled specification. |
| H3 | `PENDING` | Qualified external checker audit and independent semantic-agreement review. |
| H4 | `PASS_INTERNAL` | Qualified external confirmation that no critical or high false-accept defect remains. |
| H5 | `PENDING` | At least 1,000 preregistered mutations controlled by a non-author. |
| H6 | `PENDING` | A second named physical target and external reproduction. |
| H7 | `PENDING` | Externally controlled, fully matched evaluation budgets and customer-current-process baseline. |
| H8 | `PENDING` | A qualified third-party signed reproduction return. |
| H9 | `PENDING` | Independent confirmation that the auditor-accessible archive is sufficient to reproduce the evaluation. |
| H10 | `PENDING` | Two qualified external teams making consequential release decisions. |

This score must not be rounded up, marketed as an external result, or carried to a changed artifact. The machine-readable report contains the exact integer arithmetic and evidence paths.

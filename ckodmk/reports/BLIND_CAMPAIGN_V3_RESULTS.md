# CKODMK internal blinded campaign v3

This file is generated from the sealed machine-readable final report. Do not edit it by hand.

Final report SHA-256: `sha256:41fae9b583100a267047968e3ff237d4584647aa92dc519d61ef07cee3f3ce2d`  
Campaign ID: `ckodmk-v3-internal-agent-separated-20260810T141620Z-ed6f88e64511228e`  
Run-manifest SHA-256: `sha256:5ddde0e91340086ccccaf4dac9074c937ab7cad27ccc3031d5a613ec108fe9e7`  
Plan SHA-256: `sha256:6b59cd72f9edf2c47cc1c9b43fcfe6a4bfc14900a1be0c7c7c5585ca2686f011`  
Commitment: `sha256:af8c0b9eea4e3bab07afab9d2bcc63b7d37cc6da7e443792302f52fe4e0a0838`  
Execution wall interval: 54m 30s (`2026-08-10T14:20:30Z` to `2026-08-10T15:15:00Z`)

## Verdict

**The preregistered internal campaign acceptance predicate passed.**

- CKODMK returned zero `PASS` decisions on 120 target faults: 100 `BLOCK` and 20 correctly scoped `INCONCLUSIVE` runtime/device-drift decisions.
- CKODMK returned `PASS` on all 9 valid controls.
- CKODMK had no crash, timeout, missing decision, protocol error, wrong expected decision, or internal sandbox failure.
- The fresh Rust adjudicator matched every required CKODMK decision, and its plan-report channel rejected the separate label/evidence fabrication case with the required `BOUND_DATASET_LABEL_MISMATCH` category.
- This is internal agent-separated evidence over a selected 130-case corpus. It is not an external audit, customer result, formal implementation proof, or production failure-rate estimate.

For 0 false `PASS` decisions among 120 target faults, the preregistered two-sided 95% Clopper–Pearson upper bound is 3.027%. For 0 false blocks among only 9 controls, the corresponding upper bound is 33.627%; the control sample is therefore too small for a strong field false-alarm claim.

## Aggregate comparison

| Tool | Target-fault BLOCK | INCONCLUSIVE | PASS | Crash/timeout/protocol | Controls PASS | Scope note |
|---|---:|---:|---:|---:|---:|---|
| CKODMK Gate | 100 | 20 | 0 | 0 | 9/9 | all six strata |
| Rust adjudicator | 100 | 20 | 0 | 0 | 9/9 | recomputes report logic; does not execute ONNX |
| Independent NumPy profile | 100 | 0 | 0 | 0 | 9/9 | 100 comparable faults; 20 device-drift cases excluded |
| Composite public baseline | 90 | 0 | 30 | 0 | 9/9 | strongest union of the public component baselines |
| Polygraphy | 89 | 0 | 30 | 1 | 9/9 | shares ONNX Runtime with the oracle |
| ONNX full checker | 10 | 0 | 110 | 0 | 9/9 | structural validation only |
| ONNX Runtime smoke | 9 | 0 | 110 | 1 | 9/9 | one batch-one execution |

CKODMK had 30 favorable discordant decisions against the composite baseline and zero unfavorable ones across the 129 shared fault/control cases (two-sided exact McNemar `p=1.86264514923e-09`, Bonferroni threshold `0.00833333333333`). This establishes superiority only on this selected internal corpus and predicate.

The independent NumPy implementation agreed with CKODMK on all 100 comparable semantic faults and all 9 controls. It deliberately passed all 20 synthetic runtime/device-drift cases because that boundary is outside its profile; those cases are excluded from paired scoring.

## CKODMK by stratum

| Stratum | BLOCK | INCONCLUSIVE | PASS | Safe non-PASS 95% CI |
|---|---:|---:|---:|---:|
| `weight_bias_corruption` | 20 | 0 | 0 | 83.157%–100.000% |
| `class_output_permutation` | 20 | 0 | 0 | 83.157%–100.000% |
| `input_feature_transform` | 20 | 0 | 0 | 83.157%–100.000% |
| `graph_operator_shape` | 20 | 0 | 0 | 83.157%–100.000% |
| `binding_substitution` | 20 | 0 | 0 | 83.157%–100.000% |
| `runtime_device_drift` | 0 | 20 | 0 | 83.157%–100.000% |

## Execution cost

Durations are child process wall observations on the recorded private test host, not portable performance guarantees. They were affected by observed transient filesystem waits and must not be used as stable device benchmarks.

| Tool | Median per case | p95 per case | Sum over 130 cases |
|---|---:|---:|---:|
| CKODMK Gate | 1.326s | 1.735s | 169.6s |
| Independent NumPy profile | 1.476s | 1.832s | 180.3s |
| ONNX full checker | 0.417s | 0.573s | 58.9s |
| ONNX Runtime smoke | 0.366s | 0.525s | 50.2s |
| Polygraphy | 21.232s | 22.319s | 2563.3s |
| Rust adjudicator | 0.134s | 0.224s | 19.3s |
| Rust plan-report channel | 0.134s | 0.267s | 19.6s |

## Negative and exceptional results

- ONNX Runtime smoke and Polygraphy each crashed on the same deliberately invalid graph/shape case `33f9e7c0965bf18afac3a58e354309e6`; CKODMK and both consumer-side checkers blocked it. The crash is retained and counted as an abnormal result, not a detection success.
- CKODMK abnormal results: `0`; Rust adjudicator: `0`; independent NumPy profile: `0`.
- The graph optimized retained model artifacts remain larger than their sources, and INT8 remains slower on the recorded private test host despite being smaller; the blind campaign does not reverse those retained negative findings.
- The campaign tested three team-trained MLPs, one dataset, one physical CPU, and one primary runtime family. It did not test transformers, mobile hardware, a second architecture family, customer artifacts, or an external evaluator.

## Integrity

| Binding | SHA-256 |
|---|---|
| Release tree | `sha256:2933f0d6f8066230735ebdf76d53f76636c2e2d64cc2c89e5fca51f99e2377a7` |
| Environment tree | `sha256:0c3370b63329d004ef7d5427324998fd89a3617e6efd211569014a8db25498bc` |
| Canonical lab tree | `sha256:1dde0623d57df3c7da3d5a1512563f4b913396b8f1c4ec87b0d41bbca6d18d99` |
| Protocol | `sha256:ac86810ba09f075ce26acc189fb267cf27577756f9771f880cd356a9dd773454` |
| Config | `sha256:6daa6f4d67733d8b80068ce3f2f1d6db8ec6809865d40b80db06847bf5bbf175` |
| Corpus manifest | `sha256:03a813951cc694f5edde83e47875080db6b5c4ff7ab266d411ede3c20ccf363d` |
| Oracle | `sha256:a3bfcfa42edfb897c322fd80b18567b49705d29f645204b9a6b3445b071eeaa7` |
| Execution-start record | `sha256:dc2ad96fe175cc0ac22d868bc89dd7885c7f76cb642816f2f29c9d0c7869b993` |
| Execution-complete record | `sha256:2f943c6dede38315d1934bb56110efe1ab2d6400575cef3bd072749e2a786198` |

## Recorded limitations

- internal agent-separated evidence; not external, independent, customer, or formal-verification evidence
- faults are conditionally selected on three models and are not IID production failures
- the nine-control census includes three byte-identical path-handling controls plus six transformed controls
- uniform plan-report Rust invocations on non-target cases are blinding decoys and are not assigned detection or false-positive success predicates
- Clopper-Pearson intervals are protocol summaries, not production guarantees
- ONNX Runtime is a shared execution trusted base for Gate, Polygraphy, and the evaluator oracle
- runtime/device drift uses a synthetic evaluator shim and is not a shipped-CLI host-drift test
- the Rust adjudicator checks bound evidence but does not independently execute ONNX
- the independent NumPy ONNX checker evaluates its declared independent profile and caller-pinned bindings only; its full-contract decision is not a CKODMK release decision and runtime/device drift is excluded from paired scoring
- local directory permissions and agent separation provide procedural, not cryptographic, label blinding
- local hashes and timestamps provide integrity links but no signer identity, trusted time, or external append-only publication
- the environment inventory hashes the interpreter and pinned direct campaign distributions, not the Python standard library or the entire OS, kernel, libc, firmware, or hardware stack
- v3 permits exactly one execution attempt; interruption or infrastructure failure is a terminal campaign result and requires a fresh version and seed

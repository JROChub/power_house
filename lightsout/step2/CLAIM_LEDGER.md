# Step 2 addendum claim ledger

This is the ledger for the Step 2 addendum. A claim below is a signed release
claim only when all routed evidence is frozen, the canonical manifest is
present, its detached signature verifies under the dedicated namespace, and
`verify-addendum.sh` passes.

| ID | Signed status | Evidence state | Exact scope and qualification |
| --- | --- | --- | --- |
| S2-CL-001 | Established | Canonical binary, source inventory, two-build record, and audit frozen | A separately invocable reference verifier owns its wire types, canonicalization, digest derivations, storage reader, resource formulas, result checks, and arithmetic loop. It does not import or execute the MFENX executor or tensor-store crates. It remains in the same repository, follows the same contract specification, uses the same Rust toolchain, and is not independently authored or separately governed. |
| S2-CL-002 | Established | Frozen W0 report routed by exact digest | On the frozen W0 subject, the verifier checks the final image, result, manifests, chunks, certificate derivations, partitions, storage and I/O claims, performs a fresh ascending-k wrapping-`i32` GEMM, and compares all 3,145,728 output values covering 352,321,536 useful integer operations. This is final-state verification, not historical checkpoint, recovery, or kill-event attestation. |
| S2-CL-003 | Established | Sealed verifier audit and validation records routed | The frozen verifier validation record covers 27 unit tests, locked offline builds, Clippy with warnings denied, malformed-input mutations, independent BLAKE3 checks, and certificate derivation. These tests are finite evidence, not a formal proof or an independent security review. |
| S2-CL-004 | Established | Revisioned archive and extracted validation closure frozen | A revision-distinct archive retains the Step 1 accepted executor unchanged and adds the standalone verifier plus the source-only Step 2 harness materials. Its signed scope record still reports zero executed scaling attempts and zero executed adversarial cases. |
| S2-CL-005 | Plan only | Frozen source design; 0 attempts executed | The fixed-work 1, 2, 4, 8, and 16 lane design contains 100 planned attempts. `executed_attempts=0`, `evidence_validation_implemented=false`, and `scaling_result_established=false`. No speedup, throughput scaling, core residency, or 1–16 lane performance result is claimed. |
| S2-CL-006 | Plan only | Frozen source design; 0 cases executed | The adversarial design enumerates 214 cases: 192 mutations and 22 kill windows. `executed_cases=0`, `evidence_validation_implemented=false`, and `complete_sweep_established=false`. The suite is category-enumerated, not exhaustive; 14 kill windows require executor failpoints. |
| S2-CL-007 | Established with scope limits | Checksum-closed same-host capture routed | Two clean remapped builds in independent directories on one physical host reproduce the accepted executor byte-for-byte. Two retained raw controls differ and expose path dependence. No unrelated-machine, cross-host, cross-distribution, or general reproducible-build result is claimed. |
| S2-CL-008 | Established only by a valid detached signature | Signature and namespace are checked by `verify-addendum.sh` | A valid detached signature establishes continuity with the Step 1 project key only after the key fingerprint is independently pinned. It does not establish third-party certification, security, performance, or hardware-backed key custody. |
| S2-CL-009 | Established with scope limits | The v1 and v2 evidence is sealed and routed | For the same frozen workload and output root, one accepted fresh v2 run took exactly 5.608764486 seconds externally versus 276.103268901 seconds for one accepted v1 run, a wall-time ratio of 49.2271104608x rounded to 10 decimal places. The v2 acceptance record reports 1.392552037 seconds execution, 1.717503054 seconds finalization, 1.722296839 seconds replay/verification, and 5.375569420 seconds internal end-to-end. This is a single-run cross-release observation, not a statistical distribution, lane-scaling result, universal speedup, or projection to another workload or machine. |
| S2-CL-010 | Established with scope limits | Twice-generated SBOM/provenance and sealed validation record routed | The addendum includes a CycloneDX dependency SBOM for the standalone verifier plus explicit first-party executor and harness references, and a post-hoc in-toto Statement v1 using the SLSA provenance v1 predicate shape. The SBOM is dependency metadata rather than a binary-composition or operating-system scan. The provenance claims no SLSA build level and was not emitted by an independently controlled build service. |

## Work that remains open

- complete lifecycle verification, including historical checkpoint and recovery
  event attestation;
- execute and publish the full mutation and kill-sweep population, including
  unsupported and failed cases;
- execute and publish the complete 1–16 lane scaling population;
- complete three unrelated-machine reproductions;
- complete an independent code and security review;
- expand the sampling profiles and continue optimizing ingestion, finalization,
  and replay beyond the already retained cross-release observation, then
  remeasure without generalizing a single run;
- execute one real external workload; and
- complete commercial evaluation, verifier/executor boundary, paper, and paid
  design-partner work.

Absence from this ledger is not evidence. Claims may be promoted only by a new
signed manifest that names the exact supporting artifacts and preserves all
qualifications above.

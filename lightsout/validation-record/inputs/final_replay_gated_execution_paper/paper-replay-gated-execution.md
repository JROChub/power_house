# Replay-Gated Execution: A Contract for Withholding Acceptance Until Exact Recalculation

Draft technical paper — not peer reviewed  
Contract: `mfenx-replay-gated-execution-contract/v1`

Artifact status: this paper binds the signed validation candidate, final claim
ledger, sealed A3 adversarial sweep, supply-chain records, final-candidate
external workload, unfiltered scaling population, performance profile,
three-host closure, both failed automated-security runs, and a procedurally
separate nonhuman review. Independent human review is not performed; request
#117 remains open. No proprietary executor, partner, payment, SLA, counsel
review, or paid evaluation is claimed. The reference executor and standalone
verifier are Apache-2.0; optional paid work covers services rather than
different software rights.

## Abstract

Execution systems commonly return a result together with metadata asserting
how the result was produced. Integrity checks can authenticate stored bytes,
but they do not establish that those bytes are the correct output of the
declared computation. This paper presents one contribution: a replay-gated
execution contract in which a result is not accepted until a separately
invocable verifier authenticates the declared inputs and output, rederives the
typed identities and resource claims, freshly recalculates the complete
operation, and compares every output value. The contract separates execution
from acceptance and makes the acceptance predicate portable across executor
implementations. A reference instance covers canonical, streamed,
ascending-inner-index wrapping-i32 matrix multiplication over a
content-addressed local store. We specify its state transition, byte boundary,
failure semantics, and threat limits, then report bounded implementation
evidence without treating tests as proof or performance results as universal.

## 1. Problem

Let an executor receive a declared program `P` and inputs `I`, and return a
result `R`. A conventional success response often conflates three statements:

1. the executor finished;
2. the returned bytes are intact; and
3. those bytes equal the specified computation.

The first is an execution event. The second is an authentication property. The
third is a semantic property. A checksum over `R` proves neither that `P` was
the intended program nor that `R = P(I)`. Self-reported timing, lane assignment,
checkpoint reuse, and verification fields add useful diagnostics but remain
claims made by the producer unless another path rederives them.

The design goal is deliberately narrow: define the point at which a caller may
change a final result from *produced* to *accepted*. Scheduling, distributed
consensus, probabilistic checking, novel arithmetic, and accelerator design are
outside this paper's claimed contribution.

## 2. Contribution

The replay-gated execution contract makes acceptance an explicit predicate:

```text
Accept(P, I, R, S) =
    Canonical(P, I, R)
  ∧ Authenticate(I, R, S)
  ∧ BindIdentities(P, I, R)
  ∧ RederiveClaims(P, I, R)
  ∧ Replay(P, I) = Output(R)
```

`S` is the content-addressed store. Every conjunction is fail-closed. An
unsupported version, unknown field, noncanonical integer, malformed shape,
overflow, absent object, digest mismatch, incorrect resource certificate,
incomplete partition, false counter, or unequal output rejects the result.

The key separation is:

```text
executor:  (P, I) -> candidate R
verifier:  (P, I, candidate R, S) -> accept | reject
```

The verifier is separately invocable and does not ask the executor whether the
candidate is correct. The frozen reference verifier owns its wire types,
canonicalization, object reader, identity derivations, resource formulas,
partition checks, I/O derivations, arithmetic loop, and comparison. It neither
imports nor spawns the executor. Separate invocation is not the same as
independent authorship, governance, hardware, or formal verification; those
remain distinct questions.

## 3. Contract state machine

The externally meaningful states are:

```text
unadmitted -> admitted -> executing -> candidate -> replaying -> accepted
                    \-> failed      \-> rejected   \-> rejected
```

Only `accepted` is a successful terminal state for consumption. A candidate
may exist durably before replay, but publication interfaces must label it as a
candidate. Restart may reuse authenticated pieces, yet reuse cannot bypass the
final complete replay gate.

Admission validates the complete immutable input manifests and resource
certificate before primary arithmetic. Execution writes piece data and receipts
to private checkpoint storage. Finalization assembles a candidate output tensor
and commits its manifest. Replay then opens the declared inputs and output
through a fresh verifier-owned reader, recalculates all values, compares every
canonical output byte, and rederives the claims in the result. Failure at any
stage leaves no accepted result.

This distinction matters for crash recovery. A valid receipt can establish that
a named piece is eligible for reuse under the checkpoint rules. It cannot
establish that the final set of pieces is globally complete and correct. The
replay gate checks the complete final output regardless of which pieces were
executed or reused.

## 4. Canonical byte boundary

Contract v1 uses strict typed JSON control objects and canonical little-endian
row-major i32 tensor payloads. Control readers enforce bounded regular files,
integer-only numeric lexemes, owned schemas with unknown-field rejection, and
equality between raw JSON values and their typed projection. Typed program,
image, resource-certificate, and result identities are reserialized and
rehashed rather than copied from claimed fields.

Tensor handles name canonical manifests. A manifest binds type, encoding,
logical byte length, chunk geometry, ordered chunk digests, and a content root.
Before exposing a requested range, the reader loads and authenticates the full
referenced chunk. Shape products, byte lengths, offsets, partitions, operation
counts, and resource formulas use checked arithmetic and bounded metadata.

Canonicalization is part of the security boundary. Without one byte projection,
two parsers could accept semantically similar but byte-distinct objects, or an
identity could bind a representation different from the one actually executed.

## 5. Exact replay instance

The reference instance admits one operation:

```text
C[i,j] = fold k=0..K-1 (C[i,j] + A[i,k] * B[k,j]) modulo 2^32
```

Values are interpreted as signed i32 at the boundary. Multiplication and
addition wrap modulo `2^32`, and `k` increases in a fixed order. Because the
arithmetic is integer and the order is specified, the verifier can compare all
output values exactly; no tolerance policy or floating-point environment is
needed.

The executor may stream right-hand panels and divide output rows into
deterministic lane pieces. The verifier does not accept lane concurrency as
evidence of arithmetic correctness. It rederives piece coverage and assignments
and then performs its own complete calculation. The format binds resource and
I/O claims so a candidate cannot silently substitute a different shape or
cheaper declared workload.

The cost is intentional. Complete replay performs the useful arithmetic again.
The contract targets cases where exact acceptance, auditable local execution,
and recovery are worth that cost. It does not claim to replace sampling or
proof systems for workloads where full recalculation is unacceptable.

## 6. Failure and publication semantics

The contract treats errors as evidence, not records to discard. A conforming
acceptance harness retains command status, stdout, stderr, candidate objects,
mutation inputs, verifier diagnostics, and checksums. Existing destinations are
not silently overwritten. Missing tools, unavailable resources, incomplete
telemetry, or unsupported environments are failures or explicitly unavailable
cases, never passing samples.

Publication requires a closed inventory. A SHA-256 manifest detects changed or
missing bundle bytes but provides no publisher identity by itself. A detached
signature or workflow attestation can authenticate that inventory to a pinned
identity. Such a signature authenticates bytes and provenance within its stated
trust model; it does not certify semantic correctness, reviewer independence,
or hardware.

## 7. Bounded implementation evidence

The evidence model supports a specific implementation and release, not a
general theorem. The public claim ledger controls memory, offline execution,
concurrency, timing, scaling, signature, review, and reproduction statements.
A completed test does not repair a missing identity, and a later candidate does
not inherit an earlier candidate's result.

A standalone verifier artifact adds a separately invoked final-state check and
retains its source and tests. It shares project authorship, specification,
language, and toolchain, so it is not described as an independent security
authority.

The bound implementation is signed validation candidate
`mfenx-local-v2-validation-candidate-20260822-a1`, not a final, production, or
commercial release. Its executor SHA-256 is
`92e48bfe615ad5241202d2e49fac51d52e21d66f3d0c84c273af042d5852dac0`;
the standalone verifier SHA-256 is
`f3714660b9deeef3bd8ecef716c40580c7596c0903f05e89d555ed0d32e9b9fa`.
The canonical manifest SHA-256 is
`fb4023a172927ba7555376f0217f84c3dd2bcb057ce11d59e7b2697d02ab6229`,
its detached signature SHA-256 is
`f4ccf040df310dea9820e6ddab2c4c7b3ca0db6caabffc38463859b25a106458`,
and the matching signed archive SHA-256 is
`9485bba9d5bb7a10e911db6070fd081f8f6ade2b8894460eea2729fbe868405f`.
The signature uses namespace `mfenx-validation-candidate`, principal
`mfenx-release`, and pinned fingerprint
`SHA256:Uhj/Ci2+3KA2JN/H8+Sl6nhAiTeD76zvajqvxLOYTTc`. This authenticates the
manifest bytes to that policy; it does not certify correctness or production
fitness.

The fresh A3 adversarial run enumerated 192 mutation cases and 22 SIGKILL
restart-boundary cases. All 214 were attempted exactly once, and the independent
evidence validator recorded 214 passed and zero failed. Its run inventory,
post-seal validation, and final-attestation SHA-256 values are, respectively,
`455c7408b1099174bdfe20d0a193cc04075e137ea6873cb172bd8b271b44228a`,
`ab3b2d369f8acc4fc681815ef21fe08ecca98be3b601b5a8b719254441b42fe9`,
and `08255a52606665d63bb43f1812afe928aa3f55e2ccea0248b596f190f757c716`.
The failed A1 and superseded A2 histories remain retained; A3 does not erase
them.

The validation-candidate supply-chain bundle is closed by
`6a69c7eb301b054248d32ce09c2036acf387808da5cc3cf1ddf08ed47e4e075f`.
It contains SBOM
`a3c23510be8e0ba33ff7e6ef05621a848b093652539bbc69717870f881a3a392`,
provenance
`a4314ebbc44113dfc46a5b6aac957bb6afc65d8f6c1c182929376d44241e7971`,
and review-build inventory
`1cf134446069379f2a5a4b8e3d9a8814ca5e4b600df17d6750ea1d6c4a917e52`.
That inventory is qualified: the review build and tests passed, but arbitrary
extraction-path executor byte identity is unclaimed; the verifier's two
byte-identical builds were on the same physical host; no cross-host
reproducibility or independent builder is established. The SBOM describes Cargo
metadata, not complete binary, operating-system, firmware, or hardware
composition, and the provenance claims no SLSA level.

The final-candidate UCI Iris feature table was scaled exactly to integers and
transformed into `transpose(X10) * X10`. The candidate executor completed the
4-by-150 by 150-by-4 operation, executor replay accepted, the standalone
verifier accepted, and all 16 outputs matched the fixed oracle. The evidence
inventory SHA-256 is
`311aafe7b75d8d3a2a5bc5ca553b20e13940cd653f9252cc9dcfa165f6f7ff42`,
the summary SHA-256 is
`32b870a07c771199be685464c02df8f0c052d21a582085831047daaa466e5270`,
the output manifest root is
`82150d2bac89c31bee0b18cfd30be4feaa0fcfcaa87e2b4189958457dc5775e5`,
and the canonical i32le oracle SHA-256 is
`90f359758f26cc028c4ea52ed99e30c629512b3efb6e53dfd18bb6b669c36f35`.
This is one small correctness integration, not production suitability,
performance, classification quality, or workload generality.

The MFENX Validation Record claim ledger is bound by SHA-256
`3b6f39020bfec3f2c57cd78cc98a23c7af8ee57981a6b2d215b8319ee96de5d8`.
Its neutral claim identifiers are `MVR-CL-001` through `MVR-CL-011`.
It is the lifecycle authority for the following measured statements and their
qualifications.

### 7.1 Single-host fixed-work scaling

The v3 inventory SHA-256 is
`b69942c3be110cb9c42adbc4c8a88a1d1224101120ad64d3c35090aa107c41fc`;
the results and final-attestation SHA-256 values are
`bb1fd8bfe6158f2e68e028b9f3085359fcf7288d09cfd59e74a9083ef4360608`
and
`39a9a0f353e6053a65f1f85db7c9dc98b9c9d71529d5ff7b72fb76e37fafa21d`.
Post-seal validation and independent seal-audit records are
`8b65fd73306cc0a2992eeb1df3554c67061b2b7a26487fc23714b71f8591d423`
and
`01970563ecfdf3a45c3e0737393e6202d8edd762dc180dce752e3ba5bbf1e9c6`.

All 100 preregistered attempts were retained: ten per lane/temperature cell,
100 succeeded, zero failed, no sample filtering was permitted or applied, and
every output root was
`63ed4dd0918ee5553c5c473a568b4c2a1c9d0326c1b83e60f9c3dc23ec70d462`.
The primary metric was external executor wall time for a fixed
1,879,048,192-useful-integer-operation workload.

| Temperature | Lanes | Topology | Median wall time | Lane-1 median speedup |
| --- | ---: | --- | ---: | ---: |
| cold_unprimed | 1 | 1 physical core | 14.5623164435 s | 1.000000x |
| cold_unprimed | 2 | 2 physical cores | 12.294541476 s | 1.184454x |
| cold_unprimed | 4 | 4 physical cores | 11.072497582 s | 1.315179x |
| cold_unprimed | 8 | 8 lanes on 4 cores | 11.899606585 s | 1.223765x |
| cold_unprimed | 16 | 16 lanes on 4 cores | 12.8122589605 s | 1.136592x |
| warm_primed | 1 | 1 physical core | 14.017605601 s | 1.000000x |
| warm_primed | 2 | 2 physical cores | 12.094623824 s | 1.158995x |
| warm_primed | 4 | 4 physical cores | 11.5473317155 s | 1.213926x |
| warm_primed | 8 | 8 lanes on 4 cores | 11.6321247915 s | 1.205077x |
| warm_primed | 16 | 16 lanes on 4 cores | 12.200400739 s | 1.148946x |

Only 1, 2, and 4 lanes are physical-core scaling points. Eight and sixteen are
oversubscribed deterministic lane configurations, not 8-core or 16-core
measurements. `cold_unprimed` means no harness-prime checkpoint, not reset
operating-system or CPU caches. This is one host and supports neither an
inferential, cross-machine, universal speedup nor a hardware-class or
institutional supercomputer designation.

The performance-profile inventory SHA-256 is
`0669fd12cd392c6c3877aa642fdf81af2cf5d0dadf0f831926394d87b342cadb`.
Across those 100 observations, finalization was the largest instrumented phase:
its overall median was 7.156610520 seconds, 61.185269% of the sum of phase
medians. Embedded exact replay was 2.400651941 seconds and primary execution
1.901046064 seconds by the same calculation. This is observational phase
attribution, not proof that a particular instruction, filesystem operation,
cache state, or device caused the cost. A historical one-observation-per-release
ratio was 49.2271104608x for a same-root v1/v2 pair; it is explicitly not a
candidate speedup, statistical result, universal speedup, or causal estimate.

### 7.2 Hosted reproduction

GitHub Actions run `32571937955` completed three distinct GitHub-hosted Ubuntu
24.04 VM jobs. All three accepted the signed release workload and reference
replay and produced output root
`c4620971a11a6873de7f45f79a93a277fd2cf3f58a89ed4600e6167afed40606`.
The locally closed inventory SHA-256 is
`8bf1dedf91080c5db2d56e44b1cacb343ce214c371961e1a8ac790c6d5d85847`.
This shows release/workload reproduction across three ephemeral jobs from one
runner-image family. Runner identifiers are reuse checks, not hardware
attestation; the evidence is not source-to-binary reproducibility, three
administrative domains, or a performance comparison.

### 7.3 Security review, including failures

Automated-security A1 failed and is retained under inventory
`4e5c8f22227835e81fb79d55ae5b8b50973853c61a28658d81480356048e2a29`.
Its root failures included an unsupported CodeQL Rust build mode, an attempted
write into a read-only signed-source copy, and a workspace adapter mismatch.

A2 is retained under inventory
`99f003ae694740c6b9a5061b39850bbf6a6aedc6e37b56fe514d560b8f691890`.
It produced all 17 required tool outcomes, including candidate/signature
verification, the recorded writable analysis overlay, dependency checks,
format/lint/tests, two bounded fuzz campaigns, static policy, CodeQL, and
attestation records. Its final gate nevertheless failed on one unwaived CodeQL
`rust/cleartext-logging` result at
`crates/rarecomp-mfenx-local/src/main.rs:466`. Completing tools is not the same
as passing their findings gate; A2 is a failure.

A procedurally separate AI-assisted code/security review is closed under
inventory
`414956bf0cea315f99e7fa8d1aa3a923aa6edaacfb560b95fcaf0234eae22f23`
with disposition `pass_with_findings`. It found one open low-severity issue:
the standalone verifier's caller-controlled report publication follows
symlinks and permits destructive input overlap. It also recorded one open
informational documentation-completeness issue. The review had no human
reviewer, reviewer signature, or organizational independence. It does not
satisfy the still-open independent-human-review request at
<https://github.com/JROChub/power_house/issues/117>.

## 8. Threat limits

Replay gating addresses false final results only inside its admitted operation,
formats, arithmetic, and trusted verifier environment. Contract v1 does not by
itself protect against:

- a malicious or compromised verifier binary;
- hostile concurrent replacement inside the caller's private store;
- kernel, firmware, compiler, CPU, filesystem, or cryptographic-library flaws;
- leakage through timing, access patterns, logs, or host compromise;
- sudden-power-loss behavior beyond documented filesystem guarantees;
- denial of service within admitted time and storage bounds;
- false physical telemetry or proof of lane/core residency;
- rollback to an older, still-valid signed release; or
- errors in the frozen specification shared by executor and verifier.

Independent review, diverse implementations, transparency, rollback policy,
sandboxing, and hardware-backed attestation can address different portions of
that list. They are complementary work, not properties smuggled into the word
“replay.”

## 9. Evaluation protocol

Evaluation should separate correctness, recovery, security, portability, and
performance claims. For correctness, publish canonical positive and negative
vectors plus exact roots. For recovery, enumerate kill windows and retain every
outcome. For security, publish mutations, fuzz duration/corpus, static-analysis
versions and findings, and an independent human report. For portability,
require distinct runner identities and signed per-host evidence. For scaling,
fix the workload, pin CPU affinity, publish every attempt, and wait for measured
1–16 lane results before changing claims.

These experiments test an implementation of the contract. None converts finite
observations into proof of all executions.

## 10. Conclusion

The replay-gated execution contract contributes one explicit rule: producing a
candidate is not acceptance; acceptance occurs only after a separate path
authenticates the declared state, rederives its bindings and claims, freshly
recalculates the complete admitted operation, and compares the exact result.
That rule yields a concrete byte boundary, state transition, and failure policy
that an open verifier can enforce independently of any executor implementation.
Its value and cost are both visible: exact semantic checking is
straightforward to audit, and it repeats the computation. Claims beyond that
bounded contribution require their own evidence.

The reference implementation and verifier are Apache-2.0. A technical
evaluation may be supported as paid human work, but payment cannot alter the
acceptance predicate or restrict independent verifier use. This paper makes no
claim that a proprietary implementation, commercial relationship, SLA, or
production support system currently exists.

## Artifact map

- Contract: `docs/EXECUTION_CONTRACT_V1.md`
- Canonical formats: `docs/CANONICAL_FORMATS_V1.md`
- Threat model: `docs/THREAT_MODEL.md`
- Claim ledger: `docs/CLAIM_LEDGER.md`
- Conformance vectors: `conformance/execution-contract-v1/`
- Standalone verifier: `crates/mfenx-contract-v1-verifier/`
- External workload contract: `packaging/external-workloads/uci-iris/`
- Technical-evaluation profile:
  `packaging/commercial-evaluation/technical-evaluation-profile.json`
- Signed validation candidate:
  `release/mfenx-local-v2-validation-candidate-20260822-a1/`
- Candidate supply-chain capture:
  `evidence/mfenx-validation-candidate-supply-chain-20260822-a1/`
- A3 adversarial capture:
  `evidence/mfenx-local-v2-adversarial-sweep-20260822-a3/`
- Final-candidate external-workload capture:
  `evidence/external-workload-uci-iris-final-candidate-20260822-a1/`
- Final scaling capture:
  `evidence/mfenx-local-scaling-v3-a3-signed-20260822-a1/`
- Performance profile:
  `evidence/mfenx-local-performance-profile-20260822-a1/`
- Three-host closure: exact inventory and local-verification paths in the
  technical-evaluation profile
- Automated-security A1/A2 closures: exact historical capture paths in the
  technical-evaluation profile
- Procedurally separate nonhuman review:
  `evidence/mfenx-independent-code-security-review-20260822-a1/`

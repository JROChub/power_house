# MFENX validation and adoption roadmap

This roadmap starts from one immutable subject: the accepted MFENX Local v2 release
captured at `evidence/mfenx-software-supercomputer-v2-20260821-a1`.

Roadmap items are gates, not aspirations. A gate is complete only when its required
artifacts exist, their digests are bound to the release identity, and the documented
verification command succeeds. A failed run is retained and reported with the same
visibility as a successful run.

## Step 0: establish the subject under test

**Outcome:** every later claim, signature, reproduction, review, benchmark, and commercial
evaluation names the same source, binary, formats, and evidence.

The baseline identity is:

- release: MFENX Local v2;
- machine class: `software_defined_local_supercomputer_v2`;
- image / ISA / certificate / result: schema 3 / ISA 6 / schema 5 / schema 3;
- source-tree SHA-256:
  `e3c87a14c466e3a335f13f86738a72459bff1629c5b5d969733f0d7f544bb9ca`;
- Linux x86_64 executable SHA-256:
  `a1043e568704163b9dedf536c5feb60b0b7fd23097a2a8f0504d55d7ddcb1e3c`;
- sealed-evidence `SHA256SUMS` SHA-256:
  `71033d917be233ea260417a1f7521c8098f27715475ad0ddf8318a5ecf2fd966`;
- accepted output root:
  `4691a345a8818af410da311bc4d79131dcd093ff47384e71d4832ca08fed638c`.

Step 0 exits when a machine-readable release record repeats these values, the sealed
manifest verifies, and no active document silently substitutes another build or result.

## Step 1: freeze the public contract and trust material

**Outcome:** a third party can determine exactly what was claimed, what bytes were
signed, what formats are accepted, and which threats are or are not covered.

Required artifacts:

1. Execution Contract v1, including arithmetic, scheduling, storage, memory, recovery,
   verification, durability, and rejection rules.
2. A claim ledger mapping every active product claim to evidence and a confidence class:
   proven, observed, self-reported, scoped, or not yet established.
3. Canonical formats and positive/negative conformance vectors.
4. A complete threat model with trust boundaries and residual risks.
5. A signature over a canonical release subject, plus public verification material.
6. An SBOM and build-provenance statement bound to the exact source and binary digests.
7. A signed reproduction record for the accepted local run.

Exit checks:

- signatures verify from a clean directory using only the published public key;
- every signed subject digest resolves to an existing immutable artifact;
- the SBOM describes the released executor rather than the entire optional workspace;
- conformance vectors have both accept and reject cases;
- the claim ledger contains no unqualified performance, hardware, remote-attestation, or
  portability claim.

## Step 2: separate verification and broaden adversarial evidence

**Outcome:** verification does not require trusting the production executor, and the
release process is repeatable and measurable.

Required artifacts:

1. A standalone reference verifier with a narrower dependency and privilege boundary
   than the executor.
2. An independently packaged acceptance harness whose expected identities and formulas
   are not imported from executor output.
3. A reproducible-build recipe and at least two clean build records. Equality is judged
   on the documented reproducibility target: complete binary bytes or a stated normalized
   representation.
4. A scaling harness with fixed work, counterbalanced order, raw samples, CPU placement,
   and fail-closed validation.
5. A complete mutation and kill-point sweep across manifests, chunks, images,
   certificates, plans, pieces, receipts, results, publication boundaries, and recovery.
6. Profiles for ingestion, primary execution, finalization, and replay, followed by
   optimizations that retain all correctness and recovery gates.

Exit checks:

- the verifier rejects every published negative vector and accepts every positive vector;
- the acceptance harness can be obtained and run without the executor source tree;
- all build attempts and benchmark samples are retained, including failures;
- optimization claims report the workload, host, sample population, statistic, and raw
  records.

## Step 3: obtain independent reproduction and review

**Outcome:** the release no longer depends on one host, one operator, or only internal
review.

Required artifacts:

1. Three reproductions on unrelated machines, with distinct hardware/OS identities and
   no shared build directory or tensor store.
2. An independent code and security review with scope, reviewer identity, findings,
   dispositions, and unresolved risks.
3. An append-only results index containing all successes and failures.
4. Fixed-work 1, 2, 4, 8, and 16 lane measurements where the host has that many eligible
   lanes; unsupported lane counts are reported as unavailable, never extrapolated.

Exit checks:

- each reproduction independently builds or verifies the same signed release subject;
- output roots and conformance results agree;
- no failed reproduction or scaling sample is omitted;
- public claims are updated only after the evidence index is sealed.

## Step 4: prove usefulness and define the product boundary

**Outcome:** one outside user solves a real problem, can evaluate the product safely, and
can independently verify its outputs.

Required artifacts:

1. One real external workload with a named problem, input contract, correctness oracle,
   operational constraints, and user-visible result.
2. A commercial evaluation package with installation, licensing, support boundary,
   telemetry policy, upgrade policy, and rollback instructions.
3. A documented open-verifier / commercial-executor boundary. Result verification and
   accepted formats must remain usable without a commercial executor license.
4. A technical paper centered on one contribution: the replay-gated execution contract.
5. A design-partner process with explicit evaluation success criteria; payment or intent
   is recorded separately from technical acceptance.

Exit checks:

- the external workload owner confirms the result and the published verifier accepts it;
- the evaluation package installs and uninstalls on its stated platforms;
- verifier source, format specifications, vectors, and verification instructions remain
  publicly obtainable;
- the paper's measurements resolve to the append-only evidence index;
- commercial language does not expand technical claims beyond the signed evidence.

## Change control

Any change to arithmetic, address derivation, schemas, scheduling, checkpoint semantics,
verification method, or resource accounting creates a new execution-contract version.
A faster implementation may retain Contract v1 only when it produces the same canonical
artifacts and satisfies every Contract v1 conformance rule. Old evidence is never
rewritten; a new release links to it as a comparator.

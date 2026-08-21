# MFENX Local v2 claim ledger

Ledger version: 1  
Evidence cutoff: 2026-08-21  
Scope: `mfenx-replay-gated-execution-contract/v1` and the frozen MFENX Local v2 release

This is the controlling list of public technical claims. A claim may be repeated elsewhere
only with the scope and qualification recorded here. “Supported” means the cited frozen
artifact directly supports the stated, bounded claim; it does not mean independent peer
review or proof beyond the cited environment.

Evidence root: `evidence/mfenx-software-supercomputer-v2-20260821-a1/`

Bundle anchors:

- `acceptance.json`: SHA-256
  `4f101f8ec4b592b39b4024f198dd9f80cef79d2612556acf377191bee6270ebc`
- `SHA256SUMS`: SHA-256
  `71033d917be233ea260417a1f7521c8098f27715475ad0ddf8318a5ecf2fd966`
- `bin/mfenx-local`: SHA-256
  `a1043e568704163b9dedf536c5feb60b0b7fd23097a2a8f0504d55d7ddcb1e3c`
- captured source tree: SHA-256
  `e3c87a14c466e3a335f13f86738a72459bff1629c5b5d969733f0d7f544bb9ca`

## Supported claims

| ID | Approved claim | Direct evidence | Required qualification |
| --- | --- | --- | --- |
| CL-001 | MFENX Local v2 implements the machine class `software_defined_local_supercomputer_v2`. | `artifacts/gemm.mfx.json` (SHA-256 `7f929d844081a35c02ec7d58bacde73895ac535c86846905022d281e8bc10587`); `acceptance.json` `.machine` | “Supercomputer” is an architectural/product-class label: owned ISA/compiler, deterministic lanes, storage-backed bounded execution, recovery, and replay. It is not TOP500 or custom-silicon status. |
| CL-002 | The accepted workload executed through the local CPU-lane backend without a VPS, cloud service, product network syscall, or GPU device path. | `provenance/trace-summary.json` (SHA-256 `98b9ebf03d55b1fe82158ccd23618049e383e987962ddc644d28cb6af0d9ab6c`); `provenance/containment.txt`; `.build`, `.containment`, and `.syscall_trace` in `acceptance.json` | Applies to the frozen binary and captured run. It is not proof about every optional crate, host service, or future build. |
| CL-003 | Two named MFENX lane tasks overlapped during the accepted primary execution. | `provenance/lane-overlap.json` (SHA-256 `51354461ec3a9333a680c802bfa507d15808091f0cedddb7fb4ccaf474912461`); `memory/uninterrupted-run.lane-tasks.tsv` | This is task overlap on one host, not proof of simultaneous residency on two physical cores or 1–16 lane scaling. |
| CL-004 | The accepted workload used exact wrapping-`i32` GEMM and passed complete replay with output root `4691a345a8818af410da311bc4d79131dcd093ff47384e71d4832ca08fed638c`. | `artifacts/uninterrupted.result.json` (SHA-256 `a210d08613838ea40e13988d4aaa565cc505422b658d9da95170af42c66e4ad4`); `artifacts/uninterrupted.verify.json` (SHA-256 `9133a6ac6432ced66482acded3f5d6e9649cb70b7a7e43864f444c401f42d6a8`); `.uninterrupted_output_root` in `acceptance.json` | Exact for this admitted operation/workload. “Independent” means a separately addressed replay path inside the same binary, not an independently developed verifier or security domain. |
| CL-005 | The accepted input was 234,881,696 logical bytes and the rederived managed peak was 42,411,200 bytes, a 5.538:1 input-to-managed ratio. | `provenance/workload-contract.json` (SHA-256 `33a19658e0ba3cec98840418feafc61e9953d66ed586b886f9e25942478a70be`); image certificate in `artifacts/gemm.mfx.json` | Managed peak covers named algorithm-owned allocations, not RSS/VM/page cache. Durable bytes are storage, not RAM. |
| CL-006 | The accepted run stayed within an externally imposed 256 MiB address-space limit; sampled maximums included 36,820 KiB VmHWM and 224,724 KiB VmPeak. | `provenance/memory-summary.json` (SHA-256 `a5b7da9a4b619df04736e2574e5a5e529f920770cfda6aee229aa42b5538b4c9`); `memory/*.limits.txt`; `memory/*.tsv` | One observed Linux run. Samples are not a proof of unobserved instantaneous RSS or a portable memory guarantee. |
| CL-007 | One SIGKILL run left exactly one reusable piece; restart reused that piece, executed the missing piece, and produced the same output root as a fresh run. | `provenance/post-kill-receipts.json`; `provenance/post-resume-checkpoint.json`; `artifacts/resumed.result.json`; `checkpoints/killed-and-resumed/`; `.recovery` in `acceptance.json` | One selected kill boundary, not a complete kill sweep or universal power-loss guarantee. |
| CL-008 | Five targeted corruptions were rejected: input chunk, checkpoint piece, checkpoint receipt, image, and result I/O counter. | `mutations/corrupt-*.exit-status` (each SHA-256 `4355a46b19d348dc2f57c046f8ef63d4538ebb936000f3c9ee954a27460dd865`, the bytes `1\n`); `logs/corrupt-*`; `.corruption_rejection` in `acceptance.json` | A five-case targeted suite, not a complete mutation corpus, fuzz campaign, or security proof. |
| CL-009 | The accepted uninterrupted v2 run took 5.608764486 seconds by an external monotonic process observer. | `provenance/memory-summary.json` `.external_timings["uninterrupted-run"]`; `memory/uninterrupted-run.external-timing.json`; `.performance` in `acceptance.json` | One accepted run on one machine. Do not present it as a distribution, service level, or universal throughput result. |
| CL-010 | On the same frozen workload, the accepted v2 wall time was 49.23x lower than the accepted v1 wall time while both produced the same output root. | v2 evidence above; v1 `evidence/mfenx-software-supercomputer-20260820-a1/acceptance.json` (SHA-256 `b4fb33fffa4362c6a7d9a9af963e1b1af4e5a32634d6ac5916a43b87868814be`) and v1 `provenance/memory-summary.json` (SHA-256 `6a32e31aa1f8be2eec04e3eb9d43bd7b34a26da300505f79d49b4d994e6ae1c2`) | Say “49.23x same-workload cross-release single accepted-run ratio” (`276.103268901 / 5.608764486`). Do not say universal speedup, multi-run benchmark, scaling, or 49.23x faster on other workloads/machines. |
| CL-011 | Deterministic admission/primary/replay I/O accounts in the accepted result equal an independent manifest/range/cache-state derivation. | `provenance/uninterrupted-io-counters.json`; `.io_accounting` in `acceptance.json` | This independently derives the counter model; it does not independently observe physical device traffic. |
| CL-012 | The release evidence is internally closed by a 1,307-entry SHA-256 manifest and reports release acceptance `PASS`. | `SHA256SUMS`; `acceptance.json` | Hash closure detects changed bundle bytes. Publisher continuity is established separately by CL-013 and still requires independent fingerprint pinning. |
| CL-013 | The canonical release manifest has a valid OpenSSH Ed25519 signature in the `mfenx-release` namespace under key fingerprint `SHA256:Uhj/Ci2+3KA2JN/H8+Sl6nhAiTeD76zvajqvxLOYTTc`. | `release/mfenx-local-v2-20260821-a1/RELEASE-MANIFEST.canonical.json`; sibling `.sig`, `release-signing-key.pub`, and `allowed_signers`; `verify-release.sh` | This is a locally created project-continuity key. The signature authenticates bytes to the key only after the fingerprint is obtained through an independently trusted channel; it is not third-party identity, a trusted timestamp, hardware-backed signing, or transparency-log inclusion. |
| CL-014 | The release includes a CycloneDX 1.5 SBOM for the locked Rust normal/build dependency closure of `rarecomp-mfenx-local` for `x86_64-unknown-linux-gnu`. | `release/mfenx-local-v2-20260821-a1/mfenx-local-v2.sbom.cdx.json`; `recreate-sbom.sh` | The 46-component SBOM is dependency metadata, not a binary-composition scan. It excludes operating-system libraries, dev-only dependencies, runtime data, and unrelated workspace members. |
| CL-015 | The release includes an in-toto Statement v1 using the SLSA provenance v1 predicate shape, binding the accepted executable to the frozen source inventory, lockfile, toolchain constraint, build command, evidence, and SBOM. | `release/mfenx-local-v2-20260821-a1/mfenx-local-v2.provenance.intoto.json`; signed release manifest | It was assembled post hoc from sealed evidence, not emitted by an isolated builder. It claims no SLSA build level and is not yet a reproducible-build result. |
| CL-016 | The signed local reproduction record preserves the accepted Local v2 run and its exact workload, output root, containment results, and failure tests. | `release/mfenx-local-v2-20260821-a1/local-reproduction-record.json`; signed release manifest; sealed evidence paths cited by the record | This is one local accepted run, not one of the three unrelated-machine reproductions, an independent review, or a general performance distribution. |

## Qualified architecture statements

These phrases are permitted only with their stated meaning:

| Phrase | Approved meaning | Forbidden implication |
| --- | --- | --- |
| “software-defined local supercomputer” | The product owns the local ISA, compiler, deterministic CPU-lane schedule, bounded storage-backed execution, checkpoint contract, and replay gate. | TOP500 qualification, custom silicon, GPU equivalence, or a guaranteed performance class. |
| “independent exact replay” | A fresh complete computation using verifier-owned addressing/decoding loops inside the same binary. | Independently authored code, diverse implementation, remote verifier, separate protection domain, or formal proof. |
| “bounded memory” | The rederived certificate covers explicitly named implementation-managed allocations. | Hard RSS, VM, allocator, kernel, filesystem-cache, or whole-machine memory cap. |
| “offline/local” | The accepted executor needs no network service and the captured product trace observed no network syscall. | Air-gap certification, host-wide network absence, or a statement about other repository products. |
| “crash recovery” | Valid synchronized receipts are reused after ordinary process death; missing work is recomputed. | Exhaustive kill safety, malicious concurrent writers, or power-loss durability on every platform/filesystem. |
| “deterministic” | Inputs/options bind image, schedule, arithmetic, output, and deterministic counters. | Deterministic timing, OS scheduling, CPU placement, or identical telemetry. |

## Not established; do not claim

The current public record does **not** establish:

- a literal or measured 1,000x result;
- TOP500 eligibility or performance-class supercomputer status;
- fabricated MFENX CPU/GPU/FPGA silicon;
- 1–16 lane scaling or efficient scaling beyond the captured two-lane witness;
- three unrelated-machine reproductions;
- reproducible binary builds;
- an independent code or security review;
- a standalone independently developed verifier;
- a complete mutation corpus, fuzz campaign, or kill-point sweep;
- external-workload or commercial-production validation;
- cryptographic attestation of runtime telemetry or physical hardware;
- protection against hostile concurrent writers in the private store; or
- universal sudden-power-loss durability.

When new evidence exists, add a new ledger row with immutable paths and digests before
changing public language. Failed experiments are evidence too and MUST be retained and
published with the successful runs in the applicable benchmark or reproduction record.

## Claim-change procedure

1. State one falsifiable claim and its exact scope.
2. Produce an immutable artifact that directly measures or verifies it.
3. Record artifact paths, digests, machine/toolchain context, and failures.
4. Have a verifier not responsible for the executor path check the derivation where
   feasible.
5. Add or amend this ledger before updating the README, website, release notes, or paper.

Absence from the “Supported claims” table means the claim is not approved for current
public use.

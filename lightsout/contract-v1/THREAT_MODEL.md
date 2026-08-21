# MFENX Local v2 threat model

Status: complete for `mfenx-replay-gated-execution-contract/v1`  
Last reviewed: 2026-08-21

This threat model covers the portable `mfenx-local` image-3/ISA-6 execution path. It does
not cover the repository's SSH fabric, browser service, accelerator adapters, QPU paths,
or historical releases.

## Protected properties

The contract is designed to protect:

- **input identity:** an admitted tensor has the type, length, chunk order, and bytes bound
  by its handle and manifest root;
- **program identity:** the accepted image has the exact typed program, inputs, instruction,
  schedule, and resource certificate bound by its image digest;
- **arithmetic integrity:** the accepted output equals exact wrapping-`i32` GEMM for the
  admitted inputs;
- **partition integrity:** every output row belongs to exactly one deterministic piece and
  lane assignment;
- **restart integrity:** only complete, image-bound, content-authenticated pieces are reused;
- **result integrity:** structural fields, operation counts, storage fields, deterministic
  I/O counters, and replay claims are rederived before acceptance; and
- **managed-resource admission:** the implementation's explicitly managed buffers fit the
  rederived certificate.

Availability, confidentiality, publisher identity, host identity, and performance are not
implied by those properties.

## Trust assumptions

The following are trusted for one local execution:

- the exact executable and dynamic libraries selected by the operator;
- the Rust compiler, linker, build host, and dependency sources used to produce it;
- the CPU's documented integer and memory behavior;
- the kernel, filesystem implementation, allocator, and hardware below the process;
- the user account controlling the private tensor-store and checkpoint roots; and
- BLAKE3 collision and second-preimage resistance for content identities.

A release signature can authenticate published bytes to a key, but cannot remove these
runtime assumptions or prove the claims inside an evidence record.

## Adversary model

Within the supported boundary, an adversary may:

- supply malformed, oversized, reordered, stale, or schema-incompatible JSON;
- alter, truncate, extend, delete, or swap stored chunks and manifests between runs;
- alter an image, result, checkpoint plan, receipt, or committed piece at rest;
- replay checkpoint material from another image or bind a piece to the wrong lane/range;
- kill the process at an arbitrary instruction, including between piece and receipt
  publication;
- exhaust admitted disk capacity or cause ordinary I/O failures; and
- present self-reported timing or recovery fields that should not be trusted as external
  observations.

The contract does not claim protection when an adversary can:

- modify the running process, executable, compiler, dependencies, kernel, or CPU;
- write concurrently inside the supposedly private store or checkpoint namespace;
- control privileged tracing, clocks, process accounting, or the acceptance harness;
- exploit an undiscovered defect shared by the primary and replay paths;
- recover secrets through timing, cache, power, electromagnetic, or filesystem side
  channels; or
- prevent progress indefinitely by denying CPU, memory, or storage service.

## Attack surfaces and controls

| Surface | Control | Residual risk |
| --- | --- | --- |
| Tensor handle and manifest | Bounded reads, strict typed fields, exact byte-canonical manifest, rederived root and hints | BLAKE3 is identity, not issuer authentication; a concurrently hostile private store is excluded |
| Tensor chunks | Regular-file/length checks, full-chunk domain-separated digest before exposure, full admission pass | A hostile writer racing after authentication can create time-of-check/time-of-use risk |
| Image | Schema/profile gating, typed projection, program/image digests, exact input/instruction binding | Same binary parses and executes; parser/compiler common-mode bugs remain |
| Resource certificate | Complete deterministic rederivation and equality check | Covers named managed buffers, not total RSS/VM, page cache, kernel memory, or denial of service |
| Lane dispatch | Fixed piece partition, `piece % lanes`, named fixed-size thread pool and barrier | Does not force simultaneous CPU residency or prove physical-core separation |
| Checkpoint root | Real private directory, store-overlap rejection, exclusive lock, strict namespace | Same-user/root interference and hostile filesystem/kernel are outside the boundary |
| Plan and receipts | Image/type/schedule/range binding, exact filenames, bounded typed JSON | JSON identity is not signed; rollback of a whole valid checkpoint is not prevented |
| Piece bytes | Domain-separated digest including piece/lane/range metadata, exact length, reread on finalization | Digest collision assumptions and concurrent privileged mutation remain |
| Final tensor | Sequential authenticated commit into the content-addressed store | Disk-full and power-loss behavior depends on filesystem guarantees |
| Result | Typed schema plus rederived bindings, partitions, counts, storage, and I/O | Timing and recovery telemetry remain self-reported inside the result |
| Exact replay | Fresh complete recomputation, separately derived addresses/decoding, full byte comparison | Same codebase, compiler, process, host, and algorithm can share defects |
| CLI outputs | Same-directory temporary, file sync, create-if-absent hard-link publication | Existing outputs are not replaced; hard-link support is required |
| Release bundle | SHA-256 closure plus signing/provenance when published | Hash lists alone do not authenticate a publisher or reproduce a build |

## Failure and crash analysis

Ordinary process death releases the advisory lock. A synchronized piece without a receipt
is not committed work and is recomputed. A valid piece plus receipt is reused only after
all bindings and content are checked. Invalid committed metadata fails closed rather than
being silently discarded as successful work.

On Unix, the implementation synchronizes files and containing directories at commit
boundaries. This reduces the power-loss window but is still subject to the actual
filesystem, storage-controller, and hardware guarantees. On non-Unix targets, stable Rust
does not provide the same portable directory-sync primitive, so only ordinary process
crash—not universal sudden-power-loss durability—is claimed.

The frozen release demonstrates one SIGKILL boundary with exactly one durable piece and a
successful missing-only restart. It is not a complete kill-point sweep and must not be
described as one.

## Verification independence

The primary and verifier use different loop/address/decode paths and the verifier
recomputes every element. This protects against many corrupt outputs, checkpoint errors,
and primary-path mistakes. Both paths still share:

- image and tensor parsers;
- data types and selected utility functions;
- one compiled binary and toolchain;
- one operating system, filesystem, CPU, and process trust domain; and
- the same mathematical specification.

Accordingly, `parallel_independently_addressed_exact_replay_v2` means implementation-path
independence, not organizational independence, N-version programming, remote attestation,
or a separate security boundary. The standalone verifier planned for the next stage is
needed to reduce this common-mode risk.

## Resource and denial-of-service boundary

The resource certificate is checked arithmetic over implementation-owned allocations. It
does not sandbox memory, CPU time, storage growth, file count outside the admitted
namespace, or kernel resources. External `RLIMIT_AS` and process sampling in the release
evidence are observations of one run, not part of the portable execution contract.

Malformed inputs are bounded before large control allocation, and arithmetic uses checked
size derivation. Nevertheless, a valid admitted job can consume its declared storage,
compute time, and I/O. Disk exhaustion, slow media, priority starvation, and forced
termination are availability failures, not integrity bypasses.

## Network, GPU, and containment boundary

The local crate's direct product graph uses no network or vendor-GPU API, and the accepted
release run observed zero product network syscalls and zero GPU device paths in a separate
network namespace with a synthetic device view. This supports a claim about that binary
and capture. It does not prove that every future build, optional repository component, OS
service, or untraced process is network/GPU free.

The binary still relies on the local OS and filesystem. “Offline” means it needs no remote
service for the accepted execution, not that the host is an air-gapped security appliance.

## Cryptographic boundary

Unkeyed BLAKE3 identities detect content changes under standard collision/second-preimage
assumptions. SHA-256 identifies evidence-bundle bytes. Neither authenticates an author.
A detached release signature authenticates only the signed digest or artifact to the
corresponding key and verification policy.

No contract artifact is encrypted. Tensor values, shapes, access patterns, filenames,
sizes, and timings are not confidential. There is no rollback counter, trusted timestamp,
hardware root of trust, secure boot measurement, or remote attestation.

## Telemetry and performance boundary

Internal timing and recovery-partition fields are explicitly `self_reported`. The frozen
acceptance harness independently measured process wall time, memory samples, lane-task
overlap, traces, and post-kill state on one host. Those observations are evidence, not
portable guarantees.

The term “software-defined local supercomputer” is an architecture/product-class label.
It is not a TOP500, performance-class, custom-silicon, or universal speedup claim. The
cross-release 49.23x figure is one same-workload accepted-run comparison and does not
establish a distribution, scaling law, or performance on unrelated machines.

## Evidence coverage and open gaps

The frozen release evidence covers:

- complete bundle hash closure;
- one accepted larger-than-managed-RAM workload under an external address-space limit;
- external observation of two overlapping named lane tasks;
- exact uninterrupted and resumed output-root agreement;
- one SIGKILL/restart boundary; and
- five targeted corruptions: input chunk, checkpoint piece, checkpoint receipt, image,
  and result I/O counter.

It does not yet cover:

- a complete mutation corpus or exhaustive kill-point sweep;
- reproducible builds across independent builders;
- unrelated-machine reproduction;
- independent source/security review;
- 1–16 lane scaling results;
- malicious concurrent access to the private store;
- side-channel resistance; or
- an external production workload.

These omissions are tracked as pending work, not implied successes. Public language MUST
follow [CLAIM_LEDGER.md](CLAIM_LEDGER.md).

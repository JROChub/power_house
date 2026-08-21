# MFENX replay-gated execution contract v1

Status: **frozen**  
Contract identifier: `mfenx-replay-gated-execution-contract/v1`  
First conforming product release: MFENX Local v2  
Freeze date: 2026-08-21

This document is the normative execution contract for the current `mfenx-local`
product boundary. It freezes what an accepted execution means. It does not freeze
performance, command-line spelling, implementation language, or every optional subsystem
in this repository.

The words **MUST**, **MUST NOT**, **SHOULD**, and **MAY** are normative.

## Frozen profile

An execution claiming this contract MUST use this complete tuple:

| Field | Frozen value |
| --- | --- |
| Machine-image schema | `3` |
| MFENX ISA revision | `6` |
| Machine class | `software_defined_local_supercomputer_v2` |
| Backend | `rarecomp_mfenx_local_cpu_lane_engine` |
| Opcode | `streamed_i32_gemm` |
| Dataflow | `contiguous_right_panels_v2` |
| Program schema | `1` |
| Tensor-manifest schema | `1` |
| Resource-certificate schema | `5` |
| Lane-schedule schema | `1` |
| Lane policy | `deterministic_striped_v1` |
| Checkpoint-plan schema | `2` |
| Piece-receipt schema | `2` |
| Result schema | `3` |
| Verification policy | `exact_replay` |
| Verification method | `parallel_independently_addressed_exact_replay_v2` |

A change to any value in this tuple, accepted arithmetic, digest preimage, piece
partition law, certificate derivation, result-validation rule, or replay acceptance rule
requires a new execution-contract version. Adding a performance result or release
signature does not.

The frozen reference release is identified by:

- source-tree SHA-256
  `e3c87a14c466e3a335f13f86738a72459bff1629c5b5d969733f0d7f544bb9ca`;
- executable SHA-256
  `a1043e568704163b9dedf536c5feb60b0b7fd23097a2a8f0504d55d7ddcb1e3c`;
- release `acceptance.json` SHA-256
  `4f101f8ec4b592b39b4024f198dd9f80cef79d2612556acf377191bee6270ebc`;
  and
- release `SHA256SUMS` SHA-256
  `71033d917be233ea260417a1f7521c8098f27715475ad0ddf8318a5ecf2fd966`.

Those digests identify one implementation and evidence capture. They are not themselves
signatures. Release authentication is a separate supply-chain property.

## Admitted computation

The only admitted operation is dense rank-two matrix multiplication with both inputs and
the output encoded as canonical row-major little-endian `i32` values.

For left shape `[M, K]` and right shape `[K, N]`:

- `M`, `K`, and `N` MUST be nonzero;
- `M`, `K`, and `N` MUST fit every checked size and address calculation;
- the configured lane count MUST be in `1..=64` and MUST NOT exceed `M`;
- the output MUST have shape `[M, N]`;
- the output partition MUST contain at most 4,096 pieces; and
- the output tensor MUST be representable by at most 16,384 store chunks.

For every output coordinate `(m, n)`, the value is:

```text
acc = 0_i32
for k in 0..K:
    acc = acc.wrapping_add(left[m,k].wrapping_mul(right[k,n]))
output[m,n] = acc
```

This is modulo-`2^32` two's-complement arithmetic expressed through Rust's explicit
wrapping `i32` operations. It is not saturating arithmetic, mathematical unbounded-integer
GEMM, or floating-point GEMM. The useful operation count is `2 * M * K * N`: one multiply
and one add per inner-loop step. Verification work is recorded separately and is not
useful primary work.

## Tensor identity and admission

Each tensor handle binds an authenticated manifest through `manifest_digest`, and repeats
its type and byte length as checked hints. A conforming consumer MUST:

1. validate the handle and schema limits;
2. load the referenced regular manifest file under the private store;
3. require the exact canonical manifest byte encoding;
4. rederive the manifest content root;
5. require the handle type and byte length to match the manifest; and
6. authenticate every input chunk before execution or replay admission.

Every touched chunk MUST be read in full and its domain-separated BLAKE3 digest MUST match
before any requested bytes from that chunk are exposed. A valid manifest proves the
identity of expected bytes; it does not prove who created them.

The tensor payload byte length is the checked product of all dimensions and the element
width. Store limits are: rank at most 32, manifest at most 4 MiB, at most 16,384 chunks,
and chunk/I/O extent at most 16 MiB. A compiled local image binds one specific I/O extent;
`run` and `verify` MUST use that extent.

## Image admission

A conforming executor MUST reject an image unless all of the following hold:

- every frozen-profile identifier matches;
- the typed semantic program validates and its compact typed-JSON BLAKE3 equals
  `program_digest`;
- `inputs` contains exactly the `left` and `right` bindings;
- there is exactly one machine instruction and it repeats those exact bindings and the
  derived output type;
- there is exactly one schedule for instruction index zero;
- schedule lanes and piece count equal the rederived resource certificate;
- the semantic program is exactly the three-instruction input/input/matrix-product program
  derived from the stored input types; and
- complete input authentication and every checked derivation succeed.

The resource certificate MUST be rederived from the authenticated manifests, shapes,
chunk extents, configured I/O extent, lane count, managed-byte admission, and exact replay
policy. An executor MUST NOT trust certificate fields supplied by the image.

## Resource meaning

`certified_managed_peak_bytes` is the maximum of the rederived execution, finalization,
and verification phase formulas. The formulas cover the explicitly managed tensor caches,
range staging, panels, output accumulators, manifest allowances, lane stacks and controls,
coordinator reserve, and piece metadata used by this profile.

It is an algorithm-owned allocation certificate. It is **not** a hard limit on RSS,
virtual address space, executable mappings, allocator metadata, kernel page cache,
filesystem metadata, or another process. Durable input, output, and checkpoint capacity
is separately reported and MUST NOT be described as RAM.

Compilation MUST fail when the rederived managed peak exceeds `max_managed_bytes`, when
one lane cannot be admitted, or when output/checkpoint geometry exceeds a format limit.

## Deterministic partition and lane dispatch

The compiler derives `rows_per_piece` and contiguous output-row pieces. For piece `p`:

```text
row_start = p * rows_per_piece
row_count = min(rows_per_piece, M - row_start)
lane_index = p mod lanes
```

Pieces MUST cover `[0, M)` exactly once, in ascending piece-index order, without gaps or
overlap. Every configured lane owns at least one piece. A lane executes its missing pieces
in stripe order. OS scheduling order and wall time are not deterministic contract fields.

The primary reads the left rows for one piece and traverses the complete right tensor in
ascending contiguous panels. It writes one complete bounded row piece before committing
that piece's receipt.

## Checkpoint commit and restart

The checkpoint root MUST be a private real directory, MUST NOT be the tensor store or lie
inside it, and MUST be held through an exclusive job lock for admission, execution,
finalization, and replay. The namespace admits only the lock, one plan, canonical piece
and receipt names, and bounded runtime temporary names.

The plan binds the image digest, output type, schedule, row geometry, and piece count. A
piece receipt binds the image digest, piece and lane indices, row range, byte length, and
domain-separated piece-content digest.

A piece is reusable only when its plan, receipt, filename, geometry, lane assignment,
length, and content digest all validate. A piece without a committed receipt is an orphan
and MUST be recomputed. A malformed or contradictory committed receipt MUST fail closed;
it MUST NOT silently become completed work.

Files are synchronized before publication. On Unix, containing directories are also
synchronized at commit boundaries. On targets without a portable directory-sync
operation, this contract covers ordinary process-death restart but does not promise
directory-entry survival across sudden power loss.

## Finalization

After every piece has a valid receipt, finalization MUST reread the pieces in ascending
order, reauthenticate each piece while reading, and stream their exact bytes into a new
content-addressed output tensor. It MUST reject trailing, truncated, or changed piece
bytes. Publication is create-if-absent; an existing caller output path is not replaced.

For identical inputs and compile options, successful fresh and resumed executions MUST
produce the same output tensor root. Timings, process identifiers, and the split between
reused and executed pieces MAY differ.

## Replay gate

An execution result is accepted only after exact replay succeeds. Replay MUST:

- revalidate the image, certificate, result structure, input objects, and output object;
- recompute every output value using verifier-owned loops and address derivation;
- retain the same ascending `K` arithmetic order required by wrapping arithmetic;
- compare every canonical output `i32` value;
- cover every piece and exactly the output byte length; and
- reproduce all deterministic I/O counters and operation counts.

Replay lanes use the frozen striped partition, but the verifier derives addresses and
decodes bytes separately from the primary kernel. Here, **independent** means a separate
implementation path and fresh complete recomputation inside the same binary. It does not
mean a separately developed verifier, a separate protection domain, diverse hardware, or
protection from a common-mode compiler or implementation defect.

The replay is deterministic and exact, not probabilistic. Any mismatch MUST reject the
result. The standalone `verify` command MUST perform fresh replay and MUST NOT trust the
stored `verified` Boolean.

## Result acceptance

A conforming result validator MUST rederive and compare at least:

- image and resource-certificate digests;
- machine class and backend;
- the single output handle and its exact type;
- complete, ordered, disjoint reused/executed piece partitions and lane assignments;
- useful, executed-primary, verification, and physical operation counts;
- retained-storage and certified-managed-peak fields;
- admission, primary, replay-input, and replay-output `ReaderIoCounters`; and
- replay method, piece coverage, byte coverage, and verified status.

`ReaderIoCounters` fields are logical requested bytes, successfully authenticated chunk
bytes, chunk-load attempts, and single-reader cache hits. Their values are deterministic
for the manifests, request sequence, and one-chunk reader-cache model. They are validated
claims, not timing telemetry.

Internal timing fields and the runtime's recovery split are self-reported. A verifier MAY
record a new replay duration, and that duration need not equal the result's prior duration.
Signatures, external timing observations, host identity, and policy evidence belong to an
evidence envelope outside the execution result.

## Failure semantics

Malformed JSON, unknown typed fields, unsupported schemas, invalid digests, noncanonical
manifests, corrupt objects, invalid arithmetic geometry, allocation failure, certificate
mismatch, checkpoint conflict, I/O-account mismatch, and replay mismatch MUST return
failure and MUST NOT publish an accepted result.

The contract guarantees fail-closed detection only within the trust assumptions in
[THREAT_MODEL.md](THREAT_MODEL.md). In particular, it does not defend a private store from
a hostile process with concurrent write access, compromise of the running binary or OS,
or a common-mode defect shared by primary and replay.

## Conformance

A conforming implementation MUST pass the positive and negative vectors in
[`conformance/execution-contract-v1`](../conformance/execution-contract-v1), validate the
frozen release artifacts, and reject every specified mutation. Passing these vectors is
necessary but not sufficient for security or independent review.

Canonical encodings and digest preimages are specified in
[CANONICAL_FORMATS_V1.md](CANONICAL_FORMATS_V1.md). Evidence-backed public claims are
controlled by [CLAIM_LEDGER.md](CLAIM_LEDGER.md).

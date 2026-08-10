# CKODMK browser phone study v2

Status: implemented profile for CKODMK 0.2.x
Schema: `mfenx/ckodmk-browser-phone-study/v2`
Execution profile: `onnxruntime-web-wasm-f32-batch1-paired-timing/v1`

## Purpose

This profile records paired source and candidate inference time inside the
browser that completed a CKODMK browser verification. It is a finite local
observation. It is not a native-runtime benchmark, device attestation, energy
measurement, thermal study, or proof of performance superiority.

## Preconditions

The study button remains disabled until the browser Gate has completed a full
run over the currently selected source, candidate, canonical NPZ, and contract.
The study then independently:

1. freezes the four selected `Blob` values into bounded `ArrayBuffer` values;
2. rehashes all four values with SHA-256;
3. requires the caller-entered canonical contract digest to match;
4. strictly parses and validates the browser contract;
5. requires the source, candidate, and data-set digests to match that contract;
6. strictly reparses the canonical NPZ;
7. creates fresh source and candidate ONNX Runtime Web sessions.

Changing any selected file or the expected contract digest revokes study
authorization and deletes the in-memory study result.

## Execution method

- Backend: ONNX Runtime Web 1.27.0 WASM.
- Threads: one.
- Execution: sequential.
- Runtime graph optimization: disabled.
- Batch size: one.
- Warmup: 12 source/candidate pairs.
- Measurement: 60 source/candidate pairs.
- Order: source then candidate on even rounds; candidate then source on odd
  rounds.
- Sample index: `(round * 977) mod samples`.
- Clock observation: 128 repeated calls to one session on one input, measured as
  one `performance.now()` interval. The stored per-inference integer
  nanoseconds equal the rounded group interval divided by 128.
- Summary: nearest-rank p50 and p95 over the 60 positive integer observations
  for each artifact.

Every measured inference must return a finite Float32 vector with exactly the
contract class count. A missing, zero, noninteger, or greater-than-60-second
per-inference observation is rejected. Cancellation is checked between pairs.
The complete study has a five-minute wall limit checked between pairs. An
opaque WASM call already in progress cannot be forcibly interrupted by this
main-thread implementation.

## Report fields

The report contains:

- schema, creation time, and `authentication: none`;
- a 256-bit random session nonce generated locally before execution;
- an explicit privacy record;
- either `target_declaration: null` or an explicitly enabled evaluator-entered
  retail model, operating system, browser, and relationship classification;
- runtime profile, version, backend, threads, optimization, warmup count,
  measured count, inner repetition count, order, and clock method;
- SHA-256 digests of the source, candidate, NPZ, and contract;
- contract ID, sample count, and class count;
- 60 records containing order, sample index, source nanoseconds, and candidate
  nanoseconds;
- source and candidate nearest-rank p50 and p95 nanoseconds;
- the interpretation string `finite browser timing observations only`.

The report is unsigned. Its digest bindings establish which local bytes were
used, not who ran the study, which physical device ran it, or whether its clock
was honest. The session nonce distinguishes completed reports but supplies no
identity, trusted time, or proof of execution. A target declaration is a human
statement, not automatic detection or remote attestation.

## Privacy

The implementation does not automatically read or include:

- processor or device model;
- operating-system name or version;
- hostname or filesystem path;
- browser user-agent string;
- IP or other network address;
- account or user identifier;
- hardware concurrency or device-memory hints.

No report is uploaded or submitted. It exists in memory until the user chooses
download or operating-system share. The destination selected by the user is
outside this application profile.

An evaluator may explicitly opt in to four manually entered target fields.
They are ignored when the checkbox is clear. The form rejects control
characters and values outside 2 to 96 Unicode characters. Serial numbers,
hostnames, account identifiers, and network addresses are neither requested
nor needed.

## Failure semantics

No partial report is downloadable. A parse, digest, runtime, output, timing,
resource, cancellation, or wall-limit failure clears the in-memory result and
sets the UI state to `STOPPED`. Only a complete study enables download or
share.

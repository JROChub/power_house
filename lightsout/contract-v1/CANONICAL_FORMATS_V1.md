# MFENX execution-contract v1 canonical formats

This document defines the byte and typed-value formats used by
`mfenx-replay-gated-execution-contract/v1`. It is normative together with
[EXECUTION_CONTRACT_V1.md](EXECUTION_CONTRACT_V1.md).

## General rules

- JSON text is UTF-8.
- Object member names are exact and case-sensitive.
- Enum strings are lowercase `snake_case` as shown here.
- Integers are base-10 JSON integers without fractions or exponents.
- Digests are exactly 64 lowercase hexadecimal characters representing 32 bytes in normal
  byte order.
- Array order is significant.
- Map keys are serialized in ascending UTF-8 lexical order. The frozen image input map is
  therefore `left`, then `right`.
- Unknown fields, missing fields, wrong JSON types, out-of-range integers, and unsupported
  enum values are nonconforming.

There are two deliberately different JSON rules.

1. **Stored tensor manifests are byte-canonical.** The file bytes MUST equal the compact
   typed serialization exactly. Whitespace, reordered members, or a trailing newline is
   rejected even when it represents the same JSON value.
2. **Control and checkpoint JSON is value-canonical.** The parser compares the decoded raw
   JSON value with the value projected from its strict typed schema. Insignificant
   whitespace and object-member order do not change acceptance. Identity digests are still
   computed from the compact typed serialization described below, never from caller file
   bytes.

The command-line producer writes control JSON in a pretty form. Pretty file bytes are not
the image or result identity.

## Compact typed JSON

Compact typed JSON contains no insignificant whitespace and no trailing newline. Struct
members occur in the order listed below. Nested values follow their own listed order.

| Type | Member order |
| --- | --- |
| `TensorType` | `element`, `shape` |
| `StoredTensor` | `manifest_digest`, `ty`, `byte_length` |
| `ChunkRef` | `offset`, `length`, `digest` |
| `ChunkedTensorManifest` | `schema_version`, `ty`, `encoding`, `byte_length`, `chunk_bytes`, `chunks`, `content_root` |
| `Program` | `version`, `name`, `instructions`, `outputs` |
| IR `Instruction` | `id`, `ty`, `operation` |
| IR input operation | `op`, `name` |
| IR matrix-product operation | `op`, `lhs`, `rhs` |
| `streamed_i32_gemm` | `opcode`, `left`, `right`, `output_type` |
| `LocalLaneSchedule` | `schema_version`, `instruction_index`, `policy`, `lanes`, `piece_count` |
| `LocalMachineImage` | `schema_version`, `isa_version`, `machine_class`, `backend`, `execution_dataflow`, `program_digest`, `program`, `inputs`, `instructions`, `schedule`, `resource_certificate`, `verification` |
| `ReaderIoCounters` | `requested_bytes`, `authenticated_chunk_bytes`, `chunk_loads`, `cache_hits` |
| `LocalPieceAssignment` | `piece_index`, `lane_index` |
| `LocalOutput` | `index`, `tensor` |
| `LocalExecutionResult` | `schema_version`, `image_digest`, `machine_class`, `backend`, `resource_certificate_digest`, `outputs`, `metrics`, `verification` |
| Checkpoint plan | `schema_version`, `image_digest`, `output_type`, `schedule`, `rows_per_piece`, `piece_count` |
| Piece receipt | `schema_version`, `image_digest`, `index`, `lane_index`, `row_start`, `row_count`, `byte_length`, `content_blake3` |

`GemmResourceCertificate` member order is:

```text
schema_version
max_managed_bytes
max_io_bytes
lanes
piece_count
rows_per_piece
panel_elements
input_caches_per_lane_bytes
output_piece_bytes_per_lane
left_piece_bytes_per_lane
right_panel_bytes_per_lane
panel_decode_bytes_per_lane
primary_authenticated_range_staging_bytes_per_lane
primary_manifest_working_set_bytes_per_lane
managed_bytes_per_lane
coordinator_base_reserve_bytes
piece_metadata_reserve_bytes
lane_stack_bytes
lane_control_reserve_bytes
aggregate_execution_peak_bytes
finalization_peak_bytes
verification_peak_bytes
certified_managed_peak_bytes
retained_input_storage_bytes
retained_output_storage_bytes
primary_requested_input_bytes_upper_bound
primary_authenticated_input_bytes_upper_bound
verification_requested_input_bytes_upper_bound
verification_authenticated_input_bytes_upper_bound
verification_requested_output_bytes
verification_authenticated_output_bytes_upper_bound
verification_chunk_caches_per_lane_bytes
verification_authenticated_range_staging_bytes_per_lane
verification_manifest_working_set_bytes_per_lane
checkpoint_temporary_piece_storage_upper_bound_bytes
checkpoint_temporary_json_storage_upper_bound_bytes
retained_checkpoint_storage_upper_bound_bytes
```

`LocalExecutionMetrics` member order is:

```text
end_to_end_ns
execution_ns
finalization_ns
useful_integer_operations
executed_primary_integer_operations
physical_integer_operations
primary_input_io
total_pieces
reused_pieces
executed_pieces
reused_assignments
executed_assignments
lanes
certified_managed_peak_bytes
retained_storage_bytes
gpu_devices_required
network_transports_required
runtime_telemetry_attestation
recovery_partition_attestation
directory_metadata_sync
process_crash_recovery
```

`LocalVerificationReport` member order is:

```text
verified
method
pieces_checked
bytes_checked
verification_integer_operations
admission_input_io
input_io
output_io
verification_ns
```

The frozen local program contains exactly these operation tags: `input` and `mat_mul`.
Other operations defined by the broader IR are outside this contract.

## Tensor payload encoding

The local profile admits only `i32`. Each logical value is encoded as its two's-complement
bit pattern in four little-endian bytes. Values are in dense row-major order, with no
header, alignment padding, or row padding.

For a `[2,2]` tensor containing `[1,2,3,4]`, payload bytes are:

```text
01000000 02000000 03000000 04000000
```

## Tensor chunk identity

For exact chunk payload bytes `P`:

```text
chunk_digest = BLAKE3(
    UTF8("rarecomp-mfenx/tensor-chunk/v1") || 00 || P
)
```

The digest is stored as lowercase hex. Chunk files are regular files named
`chunks/<first-two-hex>/<digest>.bin`, have exactly the manifest length, and contain only
`P`.

## Tensor manifest identity

The manifest root is BLAKE3 over this binary preimage, in order:

```text
UTF8("rarecomp-mfenx/chunked-tensor-manifest/v1") || 00
schema_version                              u32 little-endian
element_tag                                 u8
rank                                        u64 little-endian
each dimension                              u64 little-endian
encoding_tag                                u8
byte_length                                 u64 little-endian
chunk_bytes                                 u32 little-endian
chunk_count                                 u64 little-endian
for each ordered chunk:
    offset                                  u64 little-endian
    length                                  u32 little-endian
    decoded 32-byte chunk digest
```

Element tags are `u8=0`, `i32=1`, `i64=2`, `u64=3`, `f32=4`, `f16=5`, `bf16=6`,
`f64=7`, and `bool=8`. The only encoding tag is
`canonical_le_row_major=0`.

Chunk `i` has canonical offset `i * chunk_bytes`. Every non-final chunk has length
`chunk_bytes`; the final chunk has the exact remainder. Empty payloads have no chunks.
The manifest path is
`manifests/<first-two-root-hex>/<content_root>.json`. Its exact file bytes MUST be the
compact typed JSON, including `content_root`, with no newline.

## Program, image, certificate, and result identities

The following identities use unkeyed BLAKE3 directly over compact typed JSON:

```text
program_digest              = BLAKE3(compact_typed_json(program))
image_digest                = BLAKE3(compact_typed_json(image))
resource_certificate_digest = BLAKE3(compact_typed_json(resource_certificate))
result_digest               = BLAKE3(compact_typed_json(result))
```

There is no additional domain string for these four identities in contract v1. Consumers
MUST first project through the exact typed schema, then serialize; hashing a pretty input
file is incorrect.

## Checkpoint piece identity

For a piece with exact payload bytes `P`:

```text
piece_digest = BLAKE3(
    UTF8("rarecomp-mfenx/local-output-piece/v2") || 00 ||
    piece_index u64 little-endian ||
    lane_index  u32 little-endian ||
    row_start   u64 little-endian ||
    row_count   u64 little-endian ||
    P
)
```

Piece files are named `piece-<eight-decimal-digits>.bin`; receipts are named
`receipt-<eight-decimal-digits>.json`. Receipt `content_blake3` is the piece
digest above, not the tensor-chunk digest.

## Release-envelope hashing

The frozen evidence bundle uses `SHA256SUMS` as a transport-integrity manifest. Paths are
relative to the evidence root. This SHA-256 layer is separate from the BLAKE3 execution
identities and separate from release signing. A consumer MUST NOT treat an unsigned hash
list as proof of publisher identity.

## Conformance vectors

[`vectors.json`](../conformance/execution-contract-v1/vectors.json) contains:

- a standalone tensor payload, chunk digest, manifest preimage fields, root, and exact
  canonical manifest string;
- expected identities for the frozen program, image, certificate, result, and checkpoint
  piece; and
- required rejection classes.

The vector file's own SHA-256 is recorded in its sibling `SHA256SUMS`. Implementations
SHOULD verify that checksum before consuming the values.

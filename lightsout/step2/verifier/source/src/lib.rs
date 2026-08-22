//! Separate standalone verifier for MFENX execution contract v1.
//!
//! This crate deliberately shares no code or crate dependency with the MFENX
//! executor or tensor store. It consumes only their frozen public byte formats.

#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::fmt;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Frozen execution-contract identifier verified by this crate.
pub const CONTRACT_ID: &str = "mfenx-replay-gated-execution-contract/v1";

const REPORT_SCHEMA: u32 = 1;
const IMAGE_SCHEMA: u32 = 3;
const ISA_REVISION: u32 = 6;
const PROGRAM_SCHEMA: u32 = 1;
const MANIFEST_SCHEMA: u32 = 1;
const CERTIFICATE_SCHEMA: u32 = 5;
const SCHEDULE_SCHEMA: u32 = 1;
const RESULT_SCHEMA: u32 = 3;
const MAX_CONTROL_BYTES: u64 = 4 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 4 * 1024 * 1024;
const MAX_CHUNK_BYTES: u64 = 16 * 1024 * 1024;
const MAX_CHUNKS: usize = 16_384;
const MAX_RANK: usize = 32;
const MAX_LANES: u32 = 64;
const MAX_PIECES: u64 = 4_096;
const COORDINATOR_RESERVE: u64 = 2 * 1024 * 1024;
const PIECE_METADATA_RESERVE: u64 = 2 * 1024 * 1024;
const LANE_STACK: u64 = 1024 * 1024;
const LANE_CONTROL: u64 = 64 * 1024;
const PLAN_STORAGE_BOUND: u64 = 64 * 1024;
const RECEIPT_STORAGE_BOUND: u64 = 1024;
const CHUNK_DOMAIN: &[u8] = b"rarecomp-mfenx/tensor-chunk/v1\0";
const MANIFEST_DOMAIN: &[u8] = b"rarecomp-mfenx/chunked-tensor-manifest/v1\0";

/// Verification failure with a fail-closed diagnostic.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct VerifyError(String);

impl VerifyError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for VerifyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for VerifyError {}

type Result<T> = std::result::Result<T, VerifyError>;

/// Independently derived typed identities.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct IdentityReport {
    /// BLAKE3 of the compact typed program JSON.
    pub program_blake3: String,
    /// BLAKE3 of the compact typed machine-image JSON.
    pub image_blake3: String,
    /// BLAKE3 of the compact typed resource-certificate JSON.
    pub resource_certificate_blake3: String,
    /// BLAKE3 of the compact typed execution-result JSON.
    pub result_blake3: String,
    /// Authenticated left tensor manifest root.
    pub left_manifest_root: String,
    /// Authenticated right tensor manifest root.
    pub right_manifest_root: String,
    /// Authenticated output tensor manifest root.
    pub output_manifest_root: String,
}

/// Shape and arithmetic coverage of the fresh replay.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ArithmeticReport {
    /// Frozen operation implemented by the reference verifier.
    pub operation: &'static str,
    /// Frozen arithmetic rule.
    pub arithmetic: &'static str,
    /// Left/output row count.
    pub rows: u64,
    /// Contracted dimension.
    pub inner: u64,
    /// Right/output column count.
    pub columns: u64,
    /// Number of output values compared exactly.
    pub values_checked: u64,
    /// Number of multiply-plus-add integer operations recomputed.
    pub integer_operations: u128,
    /// Number of deterministic output pieces covered.
    pub pieces_checked: u64,
    /// Number of canonical output bytes compared.
    pub bytes_checked: u64,
}

/// Deterministic I/O counters observed by the fresh reference replay.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IoCounters {
    /// Logical bytes requested by valid range reads.
    pub requested_bytes: u64,
    /// Full chunk bytes successfully authenticated before exposure.
    pub authenticated_chunk_bytes: u64,
    /// Chunk loads reaching the content-addressed store.
    pub chunk_loads: u64,
    /// Reads served from one reader's authenticated single-chunk cache.
    pub cache_hits: u64,
}

/// Successful standalone verification report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct VerificationReport {
    /// Report schema owned by this standalone verifier.
    pub schema_version: u32,
    /// Frozen contract identifier.
    pub contract: &'static str,
    /// Stable identity of this verifier implementation boundary.
    pub verifier: &'static str,
    /// True only after every validation and exact output comparison succeeds.
    pub accepted: bool,
    /// Independently recomputed identities.
    pub identities: IdentityReport,
    /// Exact arithmetic coverage.
    pub arithmetic: ArithmeticReport,
    /// Full immutable-input admission authentication performed in this process.
    pub admission_input_io: IoCounters,
    /// Input I/O performed by the fresh standalone arithmetic replay.
    pub reference_replay_input_io: IoCounters,
    /// Output I/O performed by the fresh standalone comparison.
    pub reference_replay_output_io: IoCounters,
    /// Wall duration of this process's arithmetic replay and comparison.
    pub reference_replay_ns: u128,
    /// Validation boundaries that completed before acceptance.
    pub checks: CheckReport,
}

/// Explicit coverage flags for a successful report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[allow(clippy::struct_excessive_bools)]
pub struct CheckReport {
    /// Image and result were projected through strict owned wire schemas.
    pub strict_typed_control_json: bool,
    /// All tensor manifests matched their exact compact typed bytes.
    pub canonical_manifest_bytes: bool,
    /// Manifest roots and every touched chunk digest were rederived.
    pub authenticated_tensor_store: bool,
    /// Program, image, certificate, and result identities were rederived.
    pub typed_identities: bool,
    /// The complete resource certificate was independently rederived.
    pub resource_certificate: bool,
    /// Piece partition, counts, storage, and deterministic result I/O were rederived.
    pub result_claims: bool,
    /// Every output value matched a fresh wrapping-`i32` GEMM.
    pub exact_output_replay: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ElementType {
    I32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TensorType {
    element: ElementType,
    shape: Vec<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredTensor {
    manifest_digest: String,
    ty: TensorType,
    byte_length: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChunkRef {
    offset: u64,
    length: u32,
    digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TensorEncoding {
    CanonicalLeRowMajor,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    schema_version: u32,
    ty: TensorType,
    encoding: TensorEncoding,
    byte_length: u64,
    chunk_bytes: u32,
    chunks: Vec<ChunkRef>,
    content_root: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct ValueId(u32);

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
enum ProgramOperation {
    Input { name: String },
    MatMul { lhs: ValueId, rhs: ValueId },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProgramInstruction {
    id: ValueId,
    ty: TensorType,
    operation: ProgramOperation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Program {
    version: u32,
    name: String,
    instructions: Vec<ProgramInstruction>,
    outputs: Vec<ValueId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MachineClass {
    SoftwareDefinedLocalSupercomputerV2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ExecutionDataflow {
    ContiguousRightPanelsV2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum VerificationPolicy {
    ExactReplay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LanePolicy {
    DeterministicStripedV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LaneSchedule {
    schema_version: u32,
    instruction_index: u32,
    policy: LanePolicy,
    lanes: u32,
    piece_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "opcode", rename_all = "snake_case", deny_unknown_fields)]
enum MachineInstruction {
    StreamedI32Gemm {
        left: StoredTensor,
        right: StoredTensor,
        output_type: TensorType,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceCertificate {
    schema_version: u32,
    max_managed_bytes: u64,
    max_io_bytes: u64,
    lanes: u32,
    piece_count: u64,
    rows_per_piece: u64,
    panel_elements: u64,
    input_caches_per_lane_bytes: u64,
    output_piece_bytes_per_lane: u64,
    left_piece_bytes_per_lane: u64,
    right_panel_bytes_per_lane: u64,
    panel_decode_bytes_per_lane: u64,
    primary_authenticated_range_staging_bytes_per_lane: u64,
    primary_manifest_working_set_bytes_per_lane: u64,
    managed_bytes_per_lane: u64,
    coordinator_base_reserve_bytes: u64,
    piece_metadata_reserve_bytes: u64,
    lane_stack_bytes: u64,
    lane_control_reserve_bytes: u64,
    aggregate_execution_peak_bytes: u64,
    finalization_peak_bytes: u64,
    verification_peak_bytes: u64,
    certified_managed_peak_bytes: u64,
    retained_input_storage_bytes: u64,
    retained_output_storage_bytes: u64,
    primary_requested_input_bytes_upper_bound: u64,
    primary_authenticated_input_bytes_upper_bound: u64,
    verification_requested_input_bytes_upper_bound: u64,
    verification_authenticated_input_bytes_upper_bound: u64,
    verification_requested_output_bytes: u64,
    verification_authenticated_output_bytes_upper_bound: u64,
    verification_chunk_caches_per_lane_bytes: u64,
    verification_authenticated_range_staging_bytes_per_lane: u64,
    verification_manifest_working_set_bytes_per_lane: u64,
    checkpoint_temporary_piece_storage_upper_bound_bytes: u64,
    checkpoint_temporary_json_storage_upper_bound_bytes: u64,
    retained_checkpoint_storage_upper_bound_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Inputs {
    left: StoredTensor,
    right: StoredTensor,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MachineImage {
    schema_version: u32,
    isa_version: u32,
    machine_class: MachineClass,
    backend: String,
    execution_dataflow: ExecutionDataflow,
    program_digest: String,
    program: Program,
    inputs: Inputs,
    instructions: Vec<MachineInstruction>,
    schedule: LaneSchedule,
    resource_certificate: ResourceCertificate,
    verification: VerificationPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Output {
    index: usize,
    tensor: StoredTensor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PieceAssignment {
    piece_index: u64,
    lane_index: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AttestationStatus {
    IndependentlyAttested,
    SelfReported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DirectoryMetadataSync {
    Available,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ProcessCrashRecovery {
    Supported,
    Unsupported,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecutionMetrics {
    end_to_end_ns: u128,
    execution_ns: u128,
    finalization_ns: u128,
    useful_integer_operations: u128,
    executed_primary_integer_operations: u128,
    physical_integer_operations: u128,
    primary_input_io: IoCounters,
    total_pieces: u64,
    reused_pieces: u64,
    executed_pieces: u64,
    reused_assignments: Vec<PieceAssignment>,
    executed_assignments: Vec<PieceAssignment>,
    lanes: u32,
    certified_managed_peak_bytes: u64,
    retained_storage_bytes: u64,
    gpu_devices_required: u32,
    network_transports_required: u32,
    runtime_telemetry_attestation: AttestationStatus,
    recovery_partition_attestation: AttestationStatus,
    directory_metadata_sync: DirectoryMetadataSync,
    process_crash_recovery: ProcessCrashRecovery,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaimedVerification {
    verified: bool,
    method: String,
    pieces_checked: u64,
    bytes_checked: u64,
    verification_integer_operations: u128,
    admission_input_io: IoCounters,
    input_io: IoCounters,
    output_io: IoCounters,
    verification_ns: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecutionResult {
    schema_version: u32,
    image_digest: String,
    machine_class: MachineClass,
    backend: String,
    resource_certificate_digest: String,
    outputs: Vec<Output>,
    metrics: ExecutionMetrics,
    verification: ClaimedVerification,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PieceSpec {
    index: u64,
    lane_index: u32,
    row_start: u64,
    row_count: u64,
}

#[derive(Clone, Debug)]
struct LoadedTensor {
    manifest: Manifest,
    canonical_bytes: u64,
}

/// Verify one image/result/store tuple without invoking any MFENX product code.
///
/// # Errors
///
/// Returns a fail-closed error for any parse, canonicalization, identity,
/// resource, store-authentication, result-claim, or arithmetic mismatch.
pub fn verify(
    image_path: &Path,
    result_path: &Path,
    store_root: &Path,
) -> Result<VerificationReport> {
    let image: MachineImage = read_control(image_path, "machine image")?;
    let result: ExecutionResult = read_control(result_path, "execution result")?;
    require_store_directory(store_root)?;

    verify_loaded(&image, &result, store_root)
}

fn read_control<T>(path: &Path, label: &str) -> Result<T>
where
    T: DeserializeOwned + Serialize,
{
    let bytes = read_regular_bounded(path, MAX_CONTROL_BYTES, label)?;
    require_integer_lexemes(&bytes, label)?;
    let raw: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| VerifyError::new(format!("invalid {label} JSON: {error}")))?;
    let typed: T = serde_json::from_slice(&bytes)
        .map_err(|error| VerifyError::new(format!("invalid strict {label} schema: {error}")))?;
    let projected = serde_json::to_value(&typed)
        .map_err(|error| VerifyError::new(format!("could not project {label}: {error}")))?;
    if raw != projected {
        return Err(VerifyError::new(format!(
            "{label} contains a value outside its strict typed projection"
        )));
    }
    Ok(typed)
}

fn read_regular_bounded(path: &Path, limit: u64, label: &str) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        VerifyError::new(format!(
            "cannot inspect {label} {}: {error}",
            path.display()
        ))
    })?;
    if !metadata.file_type().is_file() {
        return Err(VerifyError::new(format!(
            "{label} is not a regular non-symlink file: {}",
            path.display()
        )));
    }
    if metadata.len() > limit {
        return Err(VerifyError::new(format!(
            "{label} size {} exceeds bound {limit}",
            metadata.len()
        )));
    }
    let file = File::open(path).map_err(|error| {
        VerifyError::new(format!("cannot open {label} {}: {error}", path.display()))
    })?;
    if !file
        .metadata()
        .map_err(|error| VerifyError::new(format!("cannot inspect open {label}: {error}")))?
        .is_file()
    {
        return Err(VerifyError::new(format!(
            "open {label} is not a regular file"
        )));
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            VerifyError::new(format!("cannot read {label} {}: {error}", path.display()))
        })?;
    if u64::try_from(bytes.len()).map_err(|_| VerifyError::new("file length does not fit u64"))?
        > limit
    {
        return Err(VerifyError::new(format!(
            "{label} grew beyond bound {limit}"
        )));
    }
    Ok(bytes)
}

fn require_store_directory(path: &Path) -> Result<()> {
    require_real_directory(path, "tensor store")
}

fn require_real_directory(path: &Path, label: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        VerifyError::new(format!(
            "cannot inspect {label} {}: {error}",
            path.display()
        ))
    })?;
    if !metadata.file_type().is_dir() {
        return Err(VerifyError::new(format!(
            "{label} is not a real non-symlink directory: {}",
            path.display()
        )));
    }
    Ok(())
}

fn require_integer_lexemes(bytes: &[u8], label: &str) -> Result<()> {
    let mut index = 0_usize;
    let mut in_string = false;
    let mut escaped = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if byte == b'"' {
            in_string = true;
            index += 1;
            continue;
        }
        if byte == b'-' || byte.is_ascii_digit() {
            let start = index;
            if byte == b'-' {
                index += 1;
            }
            let digits = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            if digits == index
                || (bytes[digits] == b'0' && index - digits > 1)
                || (index < bytes.len() && matches!(bytes[index], b'.' | b'e' | b'E' | b'+'))
            {
                let token_end = bytes[index..]
                    .iter()
                    .position(|candidate| is_json_delimiter(*candidate))
                    .map_or(bytes.len(), |offset| index + offset);
                return Err(VerifyError::new(format!(
                    "{label} contains a noncanonical integer token at byte {start}: {}",
                    String::from_utf8_lossy(&bytes[start..token_end])
                )));
            }
            continue;
        }
        index += 1;
    }
    Ok(())
}

const fn is_json_delimiter(byte: u8) -> bool {
    matches!(
        byte,
        b' ' | b'\t' | b'\r' | b'\n' | b',' | b':' | b']' | b'}'
    )
}

fn typed_digest<T: Serialize>(value: &T, label: &str) -> Result<String> {
    let encoded = serde_json::to_vec(value)
        .map_err(|error| VerifyError::new(format!("could not serialize typed {label}: {error}")))?;
    Ok(blake3::hash(&encoded).to_hex().to_string())
}

fn validate_digest(value: &str, label: &str) -> Result<[u8; 32]> {
    if value.len() != 64
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(VerifyError::new(format!(
            "{label} must be exactly 64 lowercase hexadecimal characters"
        )));
    }
    let mut decoded = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        decoded[index] = (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]);
    }
    Ok(decoded)
}

const fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => 0,
    }
}

fn chunk_digest(payload: &[u8]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(CHUNK_DOMAIN);
    hasher.update(payload);
    hasher.finalize().to_hex().to_string()
}

fn manifest_root(manifest: &Manifest) -> Result<String> {
    let mut hasher = blake3::Hasher::new();
    hasher.update(MANIFEST_DOMAIN);
    hasher.update(&manifest.schema_version.to_le_bytes());
    hasher.update(&[1]);
    hasher.update(
        &u64::try_from(manifest.ty.shape.len())
            .map_err(|_| VerifyError::new("tensor rank does not fit u64"))?
            .to_le_bytes(),
    );
    for dimension in &manifest.ty.shape {
        hasher.update(&dimension.to_le_bytes());
    }
    hasher.update(&[0]);
    hasher.update(&manifest.byte_length.to_le_bytes());
    hasher.update(&manifest.chunk_bytes.to_le_bytes());
    hasher.update(
        &u64::try_from(manifest.chunks.len())
            .map_err(|_| VerifyError::new("chunk count does not fit u64"))?
            .to_le_bytes(),
    );
    for chunk in &manifest.chunks {
        hasher.update(&chunk.offset.to_le_bytes());
        hasher.update(&chunk.length.to_le_bytes());
        hasher.update(&validate_digest(&chunk.digest, "chunk digest")?);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn tensor_byte_length(ty: &TensorType) -> Result<u64> {
    ty.shape
        .iter()
        .try_fold(1_u64, |total, dimension| total.checked_mul(*dimension))
        .and_then(|values| values.checked_mul(4))
        .ok_or_else(|| VerifyError::new("tensor byte length overflows u64"))
}

fn validate_manifest(manifest: &Manifest, max_io_bytes: u64) -> Result<()> {
    if manifest.schema_version != MANIFEST_SCHEMA
        || manifest.ty.shape.len() > MAX_RANK
        || manifest.byte_length != tensor_byte_length(&manifest.ty)?
        || manifest.chunk_bytes == 0
        || u64::from(manifest.chunk_bytes) > max_io_bytes
        || u64::from(manifest.chunk_bytes) > MAX_CHUNK_BYTES
    {
        return Err(VerifyError::new(
            "tensor manifest violates schema, shape, length, or I/O bounds",
        ));
    }
    validate_digest(&manifest.content_root, "manifest content root")?;
    let chunk_extent = u64::from(manifest.chunk_bytes);
    let expected_count = if manifest.byte_length == 0 {
        0
    } else {
        ((manifest.byte_length - 1) / chunk_extent) + 1
    };
    if u64::try_from(manifest.chunks.len())
        .map_err(|_| VerifyError::new("chunk count does not fit u64"))?
        != expected_count
        || manifest.chunks.len() > MAX_CHUNKS
    {
        return Err(VerifyError::new(
            "tensor manifest has a noncanonical chunk count",
        ));
    }
    for (index, chunk) in manifest.chunks.iter().enumerate() {
        validate_digest(&chunk.digest, "chunk digest")?;
        let expected_offset = u64::try_from(index)
            .map_err(|_| VerifyError::new("chunk index does not fit u64"))?
            .checked_mul(chunk_extent)
            .ok_or_else(|| VerifyError::new("chunk offset overflows u64"))?;
        let expected_length = (manifest.byte_length - expected_offset).min(chunk_extent);
        if chunk.offset != expected_offset
            || u64::from(chunk.length) != expected_length
            || chunk.length == 0
        {
            return Err(VerifyError::new(format!(
                "tensor manifest chunk {index} has noncanonical geometry"
            )));
        }
    }
    if manifest_root(manifest)? != manifest.content_root {
        return Err(VerifyError::new("tensor manifest content root mismatch"));
    }
    Ok(())
}

fn object_path(root: &Path, category: &str, digest: &str, suffix: &str) -> Result<PathBuf> {
    validate_digest(digest, category)?;
    Ok(root
        .join(category)
        .join(&digest[..2])
        .join(format!("{digest}.{suffix}")))
}

fn validated_object_path(
    root: &Path,
    category: &str,
    digest: &str,
    suffix: &str,
) -> Result<PathBuf> {
    let path = object_path(root, category, digest, suffix)?;
    require_real_directory(root, "tensor store")?;
    require_real_directory(
        &root.join(category),
        &format!("tensor store {category} base"),
    )?;
    require_real_directory(
        &root.join(category).join(&digest[..2]),
        &format!("tensor store {category} prefix"),
    )?;
    Ok(path)
}

fn load_tensor(root: &Path, handle: &StoredTensor, max_io_bytes: u64) -> Result<LoadedTensor> {
    validate_digest(&handle.manifest_digest, "stored tensor manifest digest")?;
    if handle.ty.shape.len() > MAX_RANK || handle.byte_length != tensor_byte_length(&handle.ty)? {
        return Err(VerifyError::new(
            "stored tensor handle has an invalid type or byte length",
        ));
    }
    let path = validated_object_path(root, "manifests", &handle.manifest_digest, "json")?;
    let bytes = read_regular_bounded(&path, MAX_MANIFEST_BYTES, "tensor manifest")?;
    require_integer_lexemes(&bytes, "tensor manifest")?;
    let manifest: Manifest = serde_json::from_slice(&bytes)
        .map_err(|error| VerifyError::new(format!("invalid strict tensor manifest: {error}")))?;
    let canonical = serde_json::to_vec(&manifest)
        .map_err(|error| VerifyError::new(format!("cannot serialize tensor manifest: {error}")))?;
    if canonical != bytes {
        return Err(VerifyError::new(format!(
            "tensor manifest is not exact compact typed JSON: {}",
            path.display()
        )));
    }
    validate_manifest(&manifest, max_io_bytes)?;
    if manifest.content_root != handle.manifest_digest
        || manifest.ty != handle.ty
        || manifest.byte_length != handle.byte_length
    {
        return Err(VerifyError::new(
            "stored tensor handle does not match its authenticated manifest",
        ));
    }
    Ok(LoadedTensor {
        manifest,
        canonical_bytes: u64::try_from(bytes.len())
            .map_err(|_| VerifyError::new("manifest byte length does not fit u64"))?,
    })
}

struct TensorReader<'a> {
    root: &'a Path,
    manifest: &'a Manifest,
    max_io_bytes: usize,
    cached_index: Option<usize>,
    cached_payload: Vec<u8>,
    counters: IoCounters,
}

impl<'a> TensorReader<'a> {
    fn new(root: &'a Path, manifest: &'a Manifest, max_io_bytes: usize) -> Self {
        Self {
            root,
            manifest,
            max_io_bytes,
            cached_index: None,
            cached_payload: Vec::new(),
            counters: IoCounters::default(),
        }
    }

    fn verify_all(&mut self) -> Result<()> {
        self.cached_index = None;
        for index in 0..self.manifest.chunks.len() {
            self.load_chunk(index)?;
        }
        Ok(())
    }

    fn read_exact_at(&mut self, offset: u64, destination: &mut [u8]) -> Result<()> {
        if destination.len() > self.max_io_bytes {
            return Err(VerifyError::new(
                "tensor range read exceeds bound image I/O extent",
            ));
        }
        let length = u64::try_from(destination.len())
            .map_err(|_| VerifyError::new("range length does not fit u64"))?;
        let end = offset
            .checked_add(length)
            .ok_or_else(|| VerifyError::new("tensor range end overflows u64"))?;
        if end > self.manifest.byte_length {
            return Err(VerifyError::new("tensor range lies beyond payload"));
        }
        if destination.is_empty() {
            return Ok(());
        }
        self.counters.requested_bytes = checked_add(
            self.counters.requested_bytes,
            length,
            "reader requested bytes",
        )?;
        let mut staging = allocate_bytes(destination.len(), "authenticated range staging")?;
        let extent = u64::from(self.manifest.chunk_bytes);
        let mut logical = offset;
        let mut written = 0_usize;
        while logical < end {
            let index = usize::try_from(logical / extent)
                .map_err(|_| VerifyError::new("chunk index does not fit usize"))?;
            self.load_chunk(index)?;
            let chunk = self
                .manifest
                .chunks
                .get(index)
                .ok_or_else(|| VerifyError::new("range references absent chunk record"))?;
            let within = usize::try_from(logical - chunk.offset)
                .map_err(|_| VerifyError::new("in-chunk offset does not fit usize"))?;
            let available = self.cached_payload.len() - within;
            let remaining = staging.len() - written;
            let count = available.min(remaining);
            staging[written..written + count]
                .copy_from_slice(&self.cached_payload[within..within + count]);
            logical = checked_add(
                logical,
                u64::try_from(count)
                    .map_err(|_| VerifyError::new("copy count does not fit u64"))?,
                "range cursor",
            )?;
            written += count;
        }
        destination.copy_from_slice(&staging);
        Ok(())
    }

    fn load_chunk(&mut self, index: usize) -> Result<()> {
        if self.cached_index == Some(index) {
            self.counters.cache_hits =
                checked_add(self.counters.cache_hits, 1, "reader cache hits")?;
            return Ok(());
        }
        self.cached_index = None;
        let chunk = self
            .manifest
            .chunks
            .get(index)
            .ok_or_else(|| VerifyError::new(format!("missing chunk record {index}")))?;
        let path = validated_object_path(self.root, "chunks", &chunk.digest, "bin")?;
        self.counters.chunk_loads =
            checked_add(self.counters.chunk_loads, 1, "reader chunk loads")?;
        let payload = read_regular_bounded(&path, u64::from(chunk.length), "tensor chunk")?;
        if payload.len()
            != usize::try_from(chunk.length)
                .map_err(|_| VerifyError::new("chunk length does not fit usize"))?
        {
            return Err(VerifyError::new(format!(
                "tensor chunk has the wrong length: {}",
                path.display()
            )));
        }
        if chunk_digest(&payload) != chunk.digest {
            return Err(VerifyError::new(format!(
                "tensor chunk digest mismatch: {}",
                path.display()
            )));
        }
        self.counters.authenticated_chunk_bytes = checked_add(
            self.counters.authenticated_chunk_bytes,
            u64::from(chunk.length),
            "reader authenticated bytes",
        )?;
        self.cached_payload = payload;
        self.cached_index = Some(index);
        Ok(())
    }
}

fn allocate_bytes(length: usize, label: &str) -> Result<Vec<u8>> {
    let mut values = Vec::new();
    values.try_reserve_exact(length).map_err(|error| {
        VerifyError::new(format!("cannot allocate {label} ({length} bytes): {error}"))
    })?;
    values.resize(length, 0);
    Ok(values)
}

fn allocate_i32(length: usize, label: &str) -> Result<Vec<i32>> {
    let mut values = Vec::new();
    values.try_reserve_exact(length).map_err(|error| {
        VerifyError::new(format!(
            "cannot allocate {label} ({length} i32 values): {error}"
        ))
    })?;
    values.resize(length, 0);
    Ok(values)
}

fn checked_add(left: u64, right: u64, label: &str) -> Result<u64> {
    left.checked_add(right)
        .ok_or_else(|| VerifyError::new(format!("{label} overflows u64")))
}

fn checked_mul(left: u64, right: u64, label: &str) -> Result<u64> {
    left.checked_mul(right)
        .ok_or_else(|| VerifyError::new(format!("{label} overflows u64")))
}

fn add_io(left: IoCounters, right: IoCounters) -> Result<IoCounters> {
    Ok(IoCounters {
        requested_bytes: checked_add(
            left.requested_bytes,
            right.requested_bytes,
            "I/O requested bytes",
        )?,
        authenticated_chunk_bytes: checked_add(
            left.authenticated_chunk_bytes,
            right.authenticated_chunk_bytes,
            "I/O authenticated bytes",
        )?,
        chunk_loads: checked_add(left.chunk_loads, right.chunk_loads, "I/O chunk loads")?,
        cache_hits: checked_add(left.cache_hits, right.cache_hits, "I/O cache hits")?,
    })
}

#[allow(clippy::too_many_lines)]
fn verify_loaded(
    image: &MachineImage,
    result: &ExecutionResult,
    store_root: &Path,
) -> Result<VerificationReport> {
    validate_profile(image)?;
    let max_io_u64 = image.resource_certificate.max_io_bytes;
    let max_io = usize::try_from(max_io_u64)
        .map_err(|_| VerifyError::new("image I/O extent does not fit usize"))?;

    let left_handle = &image.inputs.left;
    let right_handle = &image.inputs.right;
    let left = load_tensor(store_root, left_handle, max_io_u64)?;
    let right = load_tensor(store_root, right_handle, max_io_u64)?;
    let output_type = validate_image_semantics(image, &left, &right)?;

    let expected_certificate = derive_resource_certificate(
        &left,
        &right,
        image.resource_certificate.max_managed_bytes,
        max_io_u64,
        image.resource_certificate.lanes,
    )?;
    if image.resource_certificate != expected_certificate {
        return Err(VerifyError::new(
            "resource certificate does not match independent derivation",
        ));
    }
    if image.schedule.lanes != expected_certificate.lanes
        || image.schedule.piece_count != expected_certificate.piece_count
    {
        return Err(VerifyError::new(
            "lane schedule does not match rederived certificate",
        ));
    }

    let program_digest = typed_digest(&image.program, "program")?;
    validate_digest(&image.program_digest, "image program digest")?;
    if image.program_digest != program_digest {
        return Err(VerifyError::new("program digest mismatch"));
    }
    let image_digest = typed_digest(image, "machine image")?;
    let certificate_digest = typed_digest(&image.resource_certificate, "resource certificate")?;

    let mut left_admission = TensorReader::new(store_root, &left.manifest, max_io);
    let mut right_admission = TensorReader::new(store_root, &right.manifest, max_io);
    left_admission.verify_all()?;
    right_admission.verify_all()?;
    let admission_input_io = add_io(left_admission.counters, right_admission.counters)?;

    let result_digest = typed_digest(result, "execution result")?;
    validate_result_envelope(image, result, &image_digest, &certificate_digest)?;
    let output_handle = &result.outputs[0].tensor;
    if output_handle.ty != output_type
        || output_handle.byte_length != tensor_byte_length(&output_type)?
    {
        return Err(VerifyError::new(
            "result output handle does not match derived GEMM output type",
        ));
    }
    let output = load_tensor(store_root, output_handle, max_io_u64)?;
    if u64::from(output.manifest.chunk_bytes) != max_io_u64 {
        return Err(VerifyError::new(
            "result output manifest chunk extent differs from the image-bound I/O extent",
        ));
    }
    let specs = piece_specs(image, &output_type)?;
    validate_result_claims(
        image,
        result,
        &left,
        &right,
        &output,
        &specs,
        admission_input_io,
    )?;

    let replay_started = Instant::now();
    let replay = exact_replay(store_root, image, &left, &right, &output, &specs, max_io)?;
    let replay_ns = replay_started.elapsed().as_nanos();
    if replay.input_io != result.verification.input_io
        || replay.output_io != result.verification.output_io
        || replay.pieces_checked != result.verification.pieces_checked
        || replay.bytes_checked != result.verification.bytes_checked
        || replay.operations != result.verification.verification_integer_operations
    {
        return Err(VerifyError::new(
            "fresh standalone replay coverage or deterministic I/O differs from result claim",
        ));
    }

    let values_checked = checked_mul(
        output_type.shape[0],
        output_type.shape[1],
        "output value count",
    )?;
    Ok(VerificationReport {
        schema_version: REPORT_SCHEMA,
        contract: CONTRACT_ID,
        verifier: "mfenx_contract_v1_reference_verifier_rust_v1",
        accepted: true,
        identities: IdentityReport {
            program_blake3: program_digest,
            image_blake3: image_digest,
            resource_certificate_blake3: certificate_digest,
            result_blake3: result_digest,
            left_manifest_root: left.manifest.content_root.clone(),
            right_manifest_root: right.manifest.content_root.clone(),
            output_manifest_root: output.manifest.content_root.clone(),
        },
        arithmetic: ArithmeticReport {
            operation: "streamed_i32_gemm",
            arithmetic: "ascending_k_wrapping_i32_modulo_2^32",
            rows: output_type.shape[0],
            inner: left.manifest.ty.shape[1],
            columns: output_type.shape[1],
            values_checked,
            integer_operations: replay.operations,
            pieces_checked: replay.pieces_checked,
            bytes_checked: replay.bytes_checked,
        },
        admission_input_io,
        reference_replay_input_io: replay.input_io,
        reference_replay_output_io: replay.output_io,
        reference_replay_ns: replay_ns,
        checks: CheckReport {
            strict_typed_control_json: true,
            canonical_manifest_bytes: true,
            authenticated_tensor_store: true,
            typed_identities: true,
            resource_certificate: true,
            result_claims: true,
            exact_output_replay: true,
        },
    })
}

fn validate_profile(image: &MachineImage) -> Result<()> {
    if image.schema_version != IMAGE_SCHEMA
        || image.isa_version != ISA_REVISION
        || image.machine_class != MachineClass::SoftwareDefinedLocalSupercomputerV2
        || image.backend != "rarecomp_mfenx_local_cpu_lane_engine"
        || image.execution_dataflow != ExecutionDataflow::ContiguousRightPanelsV2
        || image.verification != VerificationPolicy::ExactReplay
        || image.resource_certificate.schema_version != CERTIFICATE_SCHEMA
        || image.resource_certificate.max_io_bytes < 4
        || image.resource_certificate.max_io_bytes > MAX_CHUNK_BYTES
        || image.resource_certificate.max_managed_bytes == 0
        || image.resource_certificate.lanes == 0
        || image.resource_certificate.lanes > MAX_LANES
        || image.schedule.schema_version != SCHEDULE_SCHEMA
        || image.schedule.instruction_index != 0
        || image.schedule.policy != LanePolicy::DeterministicStripedV1
    {
        return Err(VerifyError::new(
            "machine image does not match the frozen contract-v1 profile",
        ));
    }
    Ok(())
}

fn gemm_output_type(left: &TensorType, right: &TensorType) -> Result<TensorType> {
    if left.shape.len() != 2
        || right.shape.len() != 2
        || left.shape[0] == 0
        || left.shape[1] == 0
        || right.shape[1] == 0
        || left.shape[1] != right.shape[0]
    {
        return Err(VerifyError::new(
            "inputs do not form a nonempty rank-two i32 GEMM",
        ));
    }
    Ok(TensorType {
        element: ElementType::I32,
        shape: vec![left.shape[0], right.shape[1]],
    })
}

fn expected_program(left: &TensorType, right: &TensorType, output: &TensorType) -> Program {
    Program {
        version: PROGRAM_SCHEMA,
        name: "manifest-backed-i32-gemm".into(),
        instructions: vec![
            ProgramInstruction {
                id: ValueId(0),
                ty: left.clone(),
                operation: ProgramOperation::Input {
                    name: "left".into(),
                },
            },
            ProgramInstruction {
                id: ValueId(1),
                ty: right.clone(),
                operation: ProgramOperation::Input {
                    name: "right".into(),
                },
            },
            ProgramInstruction {
                id: ValueId(2),
                ty: output.clone(),
                operation: ProgramOperation::MatMul {
                    lhs: ValueId(0),
                    rhs: ValueId(1),
                },
            },
        ],
        outputs: vec![ValueId(2)],
    }
}

fn validate_image_semantics(
    image: &MachineImage,
    left: &LoadedTensor,
    right: &LoadedTensor,
) -> Result<TensorType> {
    let left_handle = &image.inputs.left;
    let right_handle = &image.inputs.right;
    let output_type = gemm_output_type(&left.manifest.ty, &right.manifest.ty)?;
    if image.program != expected_program(&left.manifest.ty, &right.manifest.ty, &output_type)
        || image.instructions.len() != 1
    {
        return Err(VerifyError::new(
            "machine image semantic program is not the frozen three-instruction GEMM",
        ));
    }
    let MachineInstruction::StreamedI32Gemm {
        left,
        right,
        output_type: instruction_output,
    } = &image.instructions[0];
    if left != left_handle || right != right_handle || *instruction_output != output_type {
        return Err(VerifyError::new(
            "machine instruction does not repeat exact input/output bindings",
        ));
    }
    Ok(output_type)
}

fn manifest_working_set(tensor: &LoadedTensor) -> Result<u64> {
    checked_add(
        checked_mul(tensor.canonical_bytes, 4, "manifest encoded working set")?,
        checked_add(
            checked_mul(
                u64::try_from(tensor.manifest.chunks.len())
                    .map_err(|_| VerifyError::new("manifest chunk count does not fit u64"))?,
                128,
                "manifest record working set",
            )?,
            4096,
            "manifest fixed working set",
        )?,
        "manifest working set",
    )
}

fn projected_manifest_working_set(byte_length: u64, chunk_bytes: u64) -> Result<u64> {
    let chunks = ceil_div(byte_length, chunk_bytes, "projected output chunks")?;
    checked_add(
        checked_mul(chunks, 896, "projected manifest records")?,
        16 * 1024,
        "projected manifest working set",
    )
}

fn ceil_div(value: u64, divisor: u64, label: &str) -> Result<u64> {
    if divisor == 0 {
        return Err(VerifyError::new(format!("{label} has zero divisor")));
    }
    if value == 0 {
        return Ok(0);
    }
    Ok(checked_add(value, divisor - 1, label)? / divisor)
}

fn partition_authenticated_bound(logical: u64, chunk: u64, intervals: u64) -> Result<u64> {
    let boundary = checked_mul(
        checked_mul(chunk.saturating_sub(1), 2, "partition boundary slack")?,
        intervals,
        "partition boundary intervals",
    )?;
    let interval_bound = checked_add(logical, boundary, "partition interval bound")?;
    let object_bound = checked_mul(logical, intervals, "partition whole-object bound")?;
    Ok(interval_bound.min(object_bound))
}

#[allow(clippy::too_many_lines)]
fn derive_resource_certificate(
    left: &LoadedTensor,
    right: &LoadedTensor,
    max_managed: u64,
    max_io: u64,
    lanes_u32: u32,
) -> Result<ResourceCertificate> {
    if lanes_u32 == 0
        || lanes_u32 > MAX_LANES
        || max_managed == 0
        || !(4..=MAX_CHUNK_BYTES).contains(&max_io)
    {
        return Err(VerifyError::new("invalid resource planning limits"));
    }
    let rows = left.manifest.ty.shape[0];
    let inner = left.manifest.ty.shape[1];
    let columns = right.manifest.ty.shape[1];
    let lanes = u64::from(lanes_u32);
    if lanes > rows {
        return Err(VerifyError::new("lane count exceeds output rows"));
    }
    let left_chunk = u64::from(left.manifest.chunk_bytes);
    let right_chunk = u64::from(right.manifest.chunk_bytes);
    if left_chunk > max_io || right_chunk > max_io {
        return Err(VerifyError::new(
            "image I/O extent is smaller than an input chunk",
        ));
    }
    let panel_elements = columns.min(max_io / 4);
    if panel_elements == 0 {
        return Err(VerifyError::new("image I/O extent cannot hold one i32"));
    }
    let panel_bytes = checked_mul(panel_elements, 4, "panel bytes")?;
    let input_caches = checked_add(left_chunk, right_chunk, "input chunk caches")?;
    let primary_range_staging = checked_mul(max_io, 2, "primary range staging")?;
    let primary_manifest_set = checked_add(
        manifest_working_set(left)?,
        manifest_working_set(right)?,
        "primary manifest working set",
    )?;
    let primary_fixed = checked_add(
        checked_add(
            checked_add(
                input_caches,
                checked_mul(panel_bytes, 2, "primary panel buffers")?,
                "primary caches and panels",
            )?,
            primary_range_staging,
            "primary staging",
        )?,
        primary_manifest_set,
        "primary fixed lane bytes",
    )?;
    let lane_runtime = checked_add(LANE_STACK, LANE_CONTROL, "lane runtime reserve")?;
    let parallel_reserve = checked_add(
        checked_add(
            COORDINATOR_RESERVE,
            PIECE_METADATA_RESERVE,
            "parallel global reserve",
        )?,
        checked_mul(lane_runtime, lanes, "aggregate lane runtime reserve")?,
        "parallel reserve",
    )?;
    if parallel_reserve >= max_managed {
        return Err(VerifyError::new(
            "managed memory cannot admit lane reserves",
        ));
    }
    let budget_per_lane = (max_managed - parallel_reserve) / lanes;
    if primary_fixed >= budget_per_lane {
        return Err(VerifyError::new(
            "managed memory cannot admit primary fixed lane buffers",
        ));
    }
    let output_row_bytes = checked_mul(columns, 4, "output row bytes")?;
    let left_row_bytes = checked_mul(inner, 4, "left row bytes")?;
    let bytes_per_row = checked_add(
        output_row_bytes,
        left_row_bytes,
        "managed bytes per output row",
    )?;
    let left_bytes = left.manifest.byte_length;
    let right_bytes = right.manifest.byte_length;
    let output_bytes = checked_mul(rows, output_row_bytes, "output tensor bytes")?;
    let projected_output_manifest = projected_manifest_working_set(output_bytes, max_io)?;
    let verification_chunk_caches = checked_add(input_caches, max_io, "verification chunk caches")?;
    let verification_staging = checked_mul(max_io, 3, "verification range staging")?;
    let verification_manifest_set = checked_add(
        primary_manifest_set,
        projected_output_manifest,
        "verification manifest working set",
    )?;
    let verification_panels = checked_mul(panel_bytes, 3, "verification panel buffers")?;
    let verification_fixed = checked_add(
        checked_add(
            checked_add(
                verification_chunk_caches,
                verification_staging,
                "verification caches and staging",
            )?,
            verification_panels,
            "verification panels",
        )?,
        verification_manifest_set,
        "verification fixed lane bytes",
    )?;
    if verification_fixed >= budget_per_lane {
        return Err(VerifyError::new(
            "managed memory cannot admit verification fixed lane buffers",
        ));
    }
    let execution_rows = (budget_per_lane - primary_fixed) / bytes_per_row;
    let verification_rows = (budget_per_lane - verification_fixed) / bytes_per_row;
    let rows_per_lane = ceil_div(rows, lanes, "rows per lane")?;
    let rows_per_piece = execution_rows.min(verification_rows).min(rows_per_lane);
    if rows_per_piece == 0 {
        return Err(VerifyError::new(
            "managed memory cannot admit one output row per piece",
        ));
    }
    let output_piece = checked_mul(rows_per_piece, output_row_bytes, "output piece bytes")?;
    let left_piece = checked_mul(rows_per_piece, left_row_bytes, "left piece bytes")?;
    let managed_per_lane = checked_add(
        checked_add(primary_fixed, output_piece, "primary plus output piece")?,
        left_piece,
        "managed bytes per lane",
    )?;
    let execution_peak = checked_add(
        checked_mul(managed_per_lane, lanes, "aggregate execution lanes")?,
        parallel_reserve,
        "aggregate execution peak",
    )?;
    let non_execution_reserve = checked_add(
        COORDINATOR_RESERVE,
        PIECE_METADATA_RESERVE,
        "non-execution reserve",
    )?;
    let finalization_peak = checked_add(
        checked_add(
            checked_mul(max_io, 2, "finalization I/O buffers")?,
            projected_output_manifest,
            "finalization manifest",
        )?,
        non_execution_reserve,
        "finalization peak",
    )?;
    let verification_per_lane = checked_add(
        checked_add(
            verification_fixed,
            output_piece,
            "verification output piece",
        )?,
        left_piece,
        "verification lane bytes",
    )?;
    let verification_peak = checked_add(
        checked_mul(verification_per_lane, lanes, "aggregate verification lanes")?,
        parallel_reserve,
        "verification peak",
    )?;
    let certified_peak = execution_peak.max(finalization_peak).max(verification_peak);
    if certified_peak > max_managed {
        return Err(VerifyError::new(
            "rederived managed peak exceeds admitted bytes",
        ));
    }
    if output_bytes > checked_mul(max_io, MAX_CHUNKS as u64, "representable output bytes")? {
        return Err(VerifyError::new(
            "output exceeds flat manifest representation",
        ));
    }
    let piece_count = ceil_div(rows, rows_per_piece, "piece count")?;
    if piece_count > MAX_PIECES || piece_count > MAX_CHUNKS as u64 {
        return Err(VerifyError::new(
            "output piece count exceeds contract bounds",
        ));
    }
    let requested_input = checked_add(
        left_bytes,
        checked_mul(right_bytes, piece_count, "right input requests")?,
        "primary requested input",
    )?;
    let authenticated_input = checked_add(
        partition_authenticated_bound(left_bytes, left_chunk, piece_count)?,
        checked_mul(right_bytes, piece_count, "right authenticated input")?,
        "authenticated input upper bound",
    )?;
    let verification_output_bound =
        partition_authenticated_bound(output_bytes, max_io, piece_count)?.min(checked_mul(
            output_bytes,
            lanes,
            "lane output authentication bound",
        )?);
    let temporary_piece_storage = checked_mul(output_piece, lanes, "temporary piece storage")?;
    let temporary_json_storage = checked_add(
        checked_mul(lanes, RECEIPT_STORAGE_BOUND, "temporary receipt storage")?,
        PLAN_STORAGE_BOUND,
        "temporary JSON storage",
    )?;
    let retained_checkpoint = checked_add(
        checked_add(
            checked_add(
                checked_add(output_bytes, PLAN_STORAGE_BOUND, "checkpoint plan storage")?,
                checked_mul(piece_count, RECEIPT_STORAGE_BOUND, "retained receipts")?,
                "checkpoint receipts",
            )?,
            temporary_piece_storage,
            "checkpoint temporary pieces",
        )?,
        temporary_json_storage,
        "retained checkpoint storage",
    )?;
    Ok(ResourceCertificate {
        schema_version: CERTIFICATE_SCHEMA,
        max_managed_bytes: max_managed,
        max_io_bytes: max_io,
        lanes: lanes_u32,
        piece_count,
        rows_per_piece,
        panel_elements,
        input_caches_per_lane_bytes: input_caches,
        output_piece_bytes_per_lane: output_piece,
        left_piece_bytes_per_lane: left_piece,
        right_panel_bytes_per_lane: panel_bytes,
        panel_decode_bytes_per_lane: panel_bytes,
        primary_authenticated_range_staging_bytes_per_lane: primary_range_staging,
        primary_manifest_working_set_bytes_per_lane: primary_manifest_set,
        managed_bytes_per_lane: managed_per_lane,
        coordinator_base_reserve_bytes: COORDINATOR_RESERVE,
        piece_metadata_reserve_bytes: PIECE_METADATA_RESERVE,
        lane_stack_bytes: LANE_STACK,
        lane_control_reserve_bytes: LANE_CONTROL,
        aggregate_execution_peak_bytes: execution_peak,
        finalization_peak_bytes: finalization_peak,
        verification_peak_bytes: verification_peak,
        certified_managed_peak_bytes: certified_peak,
        retained_input_storage_bytes: checked_add(
            left_bytes,
            right_bytes,
            "retained input storage",
        )?,
        retained_output_storage_bytes: output_bytes,
        primary_requested_input_bytes_upper_bound: requested_input,
        primary_authenticated_input_bytes_upper_bound: authenticated_input,
        verification_requested_input_bytes_upper_bound: requested_input,
        verification_authenticated_input_bytes_upper_bound: authenticated_input,
        verification_requested_output_bytes: output_bytes,
        verification_authenticated_output_bytes_upper_bound: verification_output_bound,
        verification_chunk_caches_per_lane_bytes: verification_chunk_caches,
        verification_authenticated_range_staging_bytes_per_lane: verification_staging,
        verification_manifest_working_set_bytes_per_lane: verification_manifest_set,
        checkpoint_temporary_piece_storage_upper_bound_bytes: temporary_piece_storage,
        checkpoint_temporary_json_storage_upper_bound_bytes: temporary_json_storage,
        retained_checkpoint_storage_upper_bound_bytes: retained_checkpoint,
    })
}

fn validate_result_envelope(
    image: &MachineImage,
    result: &ExecutionResult,
    image_digest: &str,
    certificate_digest: &str,
) -> Result<()> {
    validate_digest(&result.image_digest, "result image digest")?;
    validate_digest(
        &result.resource_certificate_digest,
        "result resource certificate digest",
    )?;
    if result.schema_version != RESULT_SCHEMA
        || result.image_digest != image_digest
        || result.machine_class != image.machine_class
        || result.backend != image.backend
        || result.resource_certificate_digest != certificate_digest
        || result.outputs.len() != 1
        || result.outputs[0].index != 0
        || !result.verification.verified
        || result.metrics.gpu_devices_required != 0
        || result.metrics.network_transports_required != 0
        || result.metrics.runtime_telemetry_attestation != AttestationStatus::SelfReported
        || result.metrics.recovery_partition_attestation != AttestationStatus::SelfReported
        || result.metrics.process_crash_recovery != ProcessCrashRecovery::Supported
    {
        return Err(VerifyError::new(
            "execution result violates the frozen result envelope",
        ));
    }
    Ok(())
}

fn piece_specs(image: &MachineImage, output_type: &TensorType) -> Result<Vec<PieceSpec>> {
    let rows_per_piece = image.resource_certificate.rows_per_piece;
    let piece_count = ceil_div(output_type.shape[0], rows_per_piece, "result piece count")?;
    if piece_count == 0
        || piece_count != image.resource_certificate.piece_count
        || piece_count != image.schedule.piece_count
        || piece_count > MAX_PIECES
    {
        return Err(VerifyError::new(
            "result piece count does not match image schedule",
        ));
    }
    let capacity = usize::try_from(piece_count)
        .map_err(|_| VerifyError::new("piece count does not fit usize"))?;
    let mut specs = Vec::new();
    specs.try_reserve_exact(capacity).map_err(|error| {
        VerifyError::new(format!("cannot allocate piece specifications: {error}"))
    })?;
    for index in 0..piece_count {
        let row_start = checked_mul(index, rows_per_piece, "piece row start")?;
        let row_count = (output_type.shape[0] - row_start).min(rows_per_piece);
        specs.push(PieceSpec {
            index,
            lane_index: u32::try_from(index % u64::from(image.schedule.lanes))
                .map_err(|_| VerifyError::new("lane index does not fit u32"))?,
            row_start,
            row_count,
        });
    }
    Ok(specs)
}

fn validate_assignments(
    assignments: &[PieceAssignment],
    specs: &[PieceSpec],
) -> Result<BTreeSet<u64>> {
    let mut pieces = BTreeSet::new();
    let mut previous = None;
    for assignment in assignments {
        if previous.is_some_and(|value| value >= assignment.piece_index) {
            return Err(VerifyError::new(
                "piece assignments are not strictly increasing",
            ));
        }
        let spec = specs
            .get(
                usize::try_from(assignment.piece_index)
                    .map_err(|_| VerifyError::new("piece assignment index does not fit usize"))?,
            )
            .ok_or_else(|| VerifyError::new("piece assignment lies outside schedule"))?;
        if assignment.lane_index != spec.lane_index || !pieces.insert(assignment.piece_index) {
            return Err(VerifyError::new(
                "piece assignment has a wrong lane or duplicate index",
            ));
        }
        previous = Some(assignment.piece_index);
    }
    Ok(pieces)
}

fn operation_count(rows: u64, inner: u64, columns: u64) -> Result<u128> {
    u128::from(rows)
        .checked_mul(u128::from(inner))
        .and_then(|value| value.checked_mul(u128::from(columns)))
        .and_then(|value| value.checked_mul(2))
        .ok_or_else(|| VerifyError::new("integer operation count overflows u128"))
}

#[allow(clippy::too_many_lines)]
fn validate_result_claims(
    image: &MachineImage,
    result: &ExecutionResult,
    left: &LoadedTensor,
    right: &LoadedTensor,
    output: &LoadedTensor,
    specs: &[PieceSpec],
    admission_input_io: IoCounters,
) -> Result<()> {
    let reused = validate_assignments(&result.metrics.reused_assignments, specs)?;
    let executed = validate_assignments(&result.metrics.executed_assignments, specs)?;
    let expected = (0..u64::try_from(specs.len())
        .map_err(|_| VerifyError::new("piece count does not fit u64"))?)
        .collect::<BTreeSet<_>>();
    let union = reused.union(&executed).copied().collect::<BTreeSet<_>>();
    if !reused.is_disjoint(&executed)
        || union != expected
        || result.metrics.total_pieces != expected.len() as u64
        || result.metrics.reused_pieces != reused.len() as u64
        || result.metrics.executed_pieces != executed.len() as u64
    {
        return Err(VerifyError::new(
            "result reused/executed piece partition is invalid",
        ));
    }

    let expected_primary_io = simulate_primary_io(
        image,
        &left.manifest,
        &right.manifest,
        specs,
        &result.metrics.executed_assignments,
    )?;
    let (expected_replay_input, expected_replay_output) = simulate_replay_io(
        image,
        &left.manifest,
        &right.manifest,
        &output.manifest,
        specs,
    )?;
    let rows = left.manifest.ty.shape[0];
    let inner = left.manifest.ty.shape[1];
    let columns = right.manifest.ty.shape[1];
    let useful = operation_count(rows, inner, columns)?;
    let executed_rows =
        result
            .metrics
            .executed_assignments
            .iter()
            .try_fold(0_u64, |total, assignment| {
                let spec = specs
                    .get(
                        usize::try_from(assignment.piece_index)
                            .map_err(|_| VerifyError::new("piece index does not fit usize"))?,
                    )
                    .ok_or_else(|| VerifyError::new("executed piece lies outside schedule"))?;
                checked_add(total, spec.row_count, "executed row count")
            })?;
    let executed_primary = operation_count(executed_rows, inner, columns)?;
    let physical = executed_primary
        .checked_add(useful)
        .ok_or_else(|| VerifyError::new("physical operation count overflows u128"))?;
    let retained_storage = checked_add(
        checked_add(
            image.resource_certificate.retained_input_storage_bytes,
            image.resource_certificate.retained_output_storage_bytes,
            "retained input/output storage",
        )?,
        image
            .resource_certificate
            .retained_checkpoint_storage_upper_bound_bytes,
        "retained total storage",
    )?;
    let output_bytes = output.manifest.byte_length;
    if result.metrics.useful_integer_operations != useful
        || result.metrics.executed_primary_integer_operations != executed_primary
        || result.metrics.physical_integer_operations != physical
        || result.metrics.primary_input_io != expected_primary_io
        || result.metrics.lanes != image.resource_certificate.lanes
        || result.metrics.certified_managed_peak_bytes
            != image.resource_certificate.certified_managed_peak_bytes
        || result.metrics.retained_storage_bytes != retained_storage
        || result.verification.method != "parallel_independently_addressed_exact_replay_v2"
        || result.verification.pieces_checked != specs.len() as u64
        || result.verification.bytes_checked != output_bytes
        || result.verification.verification_integer_operations != useful
        || result.verification.admission_input_io != admission_input_io
        || result.verification.input_io != expected_replay_input
        || result.verification.output_io != expected_replay_output
        || expected_replay_input.requested_bytes
            != image
                .resource_certificate
                .verification_requested_input_bytes_upper_bound
        || expected_replay_input.authenticated_chunk_bytes
            > image
                .resource_certificate
                .verification_authenticated_input_bytes_upper_bound
        || expected_replay_output.requested_bytes
            != image
                .resource_certificate
                .verification_requested_output_bytes
        || expected_replay_output.authenticated_chunk_bytes
            > image
                .resource_certificate
                .verification_authenticated_output_bytes_upper_bound
    {
        return Err(VerifyError::new(
            "execution result deterministic claims do not match independent derivation",
        ));
    }
    Ok(())
}

struct IoSimulator<'a> {
    manifest: &'a Manifest,
    cached_index: Option<usize>,
    counters: IoCounters,
}

impl<'a> IoSimulator<'a> {
    const fn new(manifest: &'a Manifest) -> Self {
        Self {
            manifest,
            cached_index: None,
            counters: IoCounters {
                requested_bytes: 0,
                authenticated_chunk_bytes: 0,
                chunk_loads: 0,
                cache_hits: 0,
            },
        }
    }

    fn read(&mut self, offset: u64, length: u64) -> Result<()> {
        let end = checked_add(offset, length, "simulated range end")?;
        if end > self.manifest.byte_length {
            return Err(VerifyError::new(
                "simulated range lies beyond tensor payload",
            ));
        }
        self.counters.requested_bytes = checked_add(
            self.counters.requested_bytes,
            length,
            "simulated requested bytes",
        )?;
        let extent = u64::from(self.manifest.chunk_bytes);
        let mut logical = offset;
        while logical < end {
            let index = usize::try_from(logical / extent)
                .map_err(|_| VerifyError::new("simulated chunk index does not fit usize"))?;
            let chunk = self
                .manifest
                .chunks
                .get(index)
                .ok_or_else(|| VerifyError::new("simulated read references absent chunk"))?;
            if self.cached_index == Some(index) {
                self.counters.cache_hits =
                    checked_add(self.counters.cache_hits, 1, "simulated cache hits")?;
            } else {
                self.counters.chunk_loads =
                    checked_add(self.counters.chunk_loads, 1, "simulated chunk loads")?;
                self.counters.authenticated_chunk_bytes = checked_add(
                    self.counters.authenticated_chunk_bytes,
                    u64::from(chunk.length),
                    "simulated authenticated bytes",
                )?;
                self.cached_index = Some(index);
            }
            logical = end.min(checked_add(
                chunk.offset,
                u64::from(chunk.length),
                "chunk end",
            )?);
        }
        Ok(())
    }
}

fn simulate_panels(
    simulator: &mut IoSimulator<'_>,
    first_value: u64,
    value_count: u64,
    panel_elements: u64,
) -> Result<()> {
    if panel_elements == 0 {
        return Err(VerifyError::new("panel extent is zero"));
    }
    let mut consumed = 0_u64;
    while consumed < value_count {
        let count = (value_count - consumed).min(panel_elements);
        let offset = checked_mul(
            checked_add(first_value, consumed, "panel first value")?,
            4,
            "panel byte offset",
        )?;
        simulator.read(offset, checked_mul(count, 4, "panel byte count")?)?;
        consumed = checked_add(consumed, count, "panel cursor")?;
    }
    Ok(())
}

fn simulate_piece_input(
    image: &MachineImage,
    left: &Manifest,
    right: &Manifest,
    spec: PieceSpec,
    verifier_addressing: bool,
) -> Result<IoCounters> {
    let inner = left.ty.shape[1];
    let columns = right.ty.shape[1];
    let panel = image.resource_certificate.panel_elements;
    let mut left_io = IoSimulator::new(left);
    if verifier_addressing {
        for local_row in 0..spec.row_count {
            let first = checked_mul(
                checked_add(spec.row_start, local_row, "verifier left row")?,
                inner,
                "verifier left first value",
            )?;
            simulate_panels(&mut left_io, first, inner, panel)?;
        }
    } else {
        simulate_panels(
            &mut left_io,
            checked_mul(spec.row_start, inner, "primary left first value")?,
            checked_mul(spec.row_count, inner, "primary left values")?,
            panel,
        )?;
    }
    let mut right_io = IoSimulator::new(right);
    for contracted in 0..inner {
        simulate_panels(
            &mut right_io,
            checked_mul(contracted, columns, "right row first value")?,
            columns,
            panel,
        )?;
    }
    add_io(left_io.counters, right_io.counters)
}

fn simulate_primary_io(
    image: &MachineImage,
    left: &Manifest,
    right: &Manifest,
    specs: &[PieceSpec],
    assignments: &[PieceAssignment],
) -> Result<IoCounters> {
    assignments
        .iter()
        .try_fold(IoCounters::default(), |total, assignment| {
            let spec = *specs
                .get(
                    usize::try_from(assignment.piece_index)
                        .map_err(|_| VerifyError::new("piece index does not fit usize"))?,
                )
                .ok_or_else(|| VerifyError::new("primary assignment lies outside schedule"))?;
            add_io(
                total,
                simulate_piece_input(image, left, right, spec, false)?,
            )
        })
}

fn simulate_replay_io(
    image: &MachineImage,
    left: &Manifest,
    right: &Manifest,
    output: &Manifest,
    specs: &[PieceSpec],
) -> Result<(IoCounters, IoCounters)> {
    let mut input_total = IoCounters::default();
    let mut output_total = IoCounters::default();
    let columns = right.ty.shape[1];
    let panel = image.resource_certificate.panel_elements;
    for lane in 0..image.resource_certificate.lanes {
        let mut output_io = IoSimulator::new(output);
        for spec in specs
            .iter()
            .filter(|candidate| candidate.lane_index == lane)
        {
            input_total = add_io(
                input_total,
                simulate_piece_input(image, left, right, *spec, true)?,
            )?;
            simulate_panels(
                &mut output_io,
                checked_mul(spec.row_start, columns, "output piece first value")?,
                checked_mul(spec.row_count, columns, "output piece values")?,
                panel,
            )?;
        }
        output_total = add_io(output_total, output_io.counters)?;
    }
    Ok((input_total, output_total))
}

struct ReplayOutcome {
    pieces_checked: u64,
    bytes_checked: u64,
    operations: u128,
    input_io: IoCounters,
    output_io: IoCounters,
}

fn checked_usize_product(values: &[u64], label: &str) -> Result<usize> {
    let product = values
        .iter()
        .try_fold(1_u64, |total, value| total.checked_mul(*value));
    usize::try_from(product.ok_or_else(|| VerifyError::new(format!("{label} overflows u64")))?)
        .map_err(|_| VerifyError::new(format!("{label} does not fit usize")))
}

fn read_left_piece(
    reader: &mut TensorReader<'_>,
    spec: PieceSpec,
    inner: u64,
    panel_elements: usize,
) -> Result<Vec<i32>> {
    let rows = usize::try_from(spec.row_count)
        .map_err(|_| VerifyError::new("piece row count does not fit usize"))?;
    let inner_usize = usize::try_from(inner)
        .map_err(|_| VerifyError::new("contracted dimension does not fit usize"))?;
    let mut values = allocate_i32(
        checked_usize_product(&[spec.row_count, inner], "left piece value count")?,
        "left piece values",
    )?;
    let mut encoded = allocate_bytes(
        panel_elements
            .checked_mul(4)
            .ok_or_else(|| VerifyError::new("left decode panel bytes overflow usize"))?,
        "left decode panel",
    )?;
    for local_row in 0..rows {
        let global_row = checked_add(
            spec.row_start,
            u64::try_from(local_row).map_err(|_| VerifyError::new("local row does not fit u64"))?,
            "left global row",
        )?;
        let row_byte_offset = checked_mul(
            checked_mul(global_row, inner, "left row first value")?,
            4,
            "left row byte offset",
        )?;
        let mut column_start = 0_usize;
        while column_start < inner_usize {
            let count = (inner_usize - column_start).min(panel_elements);
            let byte_count = count
                .checked_mul(4)
                .ok_or_else(|| VerifyError::new("left panel byte count overflows usize"))?;
            let byte_offset = checked_add(
                row_byte_offset,
                u64::try_from(column_start)
                    .map_err(|_| VerifyError::new("left column does not fit u64"))?
                    .checked_mul(4)
                    .ok_or_else(|| VerifyError::new("left column byte offset overflows u64"))?,
                "left panel byte offset",
            )?;
            reader.read_exact_at(byte_offset, &mut encoded[..byte_count])?;
            for local_column in (0..count).rev() {
                let source = local_column * 4;
                let decoded = i32::from_le_bytes(
                    encoded[source..source + 4]
                        .try_into()
                        .expect("four-byte i32 decode"),
                );
                values[local_row * inner_usize + column_start + local_column] = decoded;
            }
            column_start += count;
        }
    }
    Ok(values)
}

fn compute_piece(
    root: &Path,
    image: &MachineImage,
    left: &Manifest,
    right: &Manifest,
    spec: PieceSpec,
    max_io: usize,
) -> Result<(Vec<i32>, IoCounters)> {
    let inner = left.ty.shape[1];
    let columns = right.ty.shape[1];
    let rows = usize::try_from(spec.row_count)
        .map_err(|_| VerifyError::new("piece rows do not fit usize"))?;
    let inner_usize = usize::try_from(inner)
        .map_err(|_| VerifyError::new("contracted dimension does not fit usize"))?;
    let columns_usize = usize::try_from(columns)
        .map_err(|_| VerifyError::new("column count does not fit usize"))?;
    let panel_elements = usize::try_from(image.resource_certificate.panel_elements)
        .map_err(|_| VerifyError::new("panel elements do not fit usize"))?;
    if panel_elements == 0
        || panel_elements
            .checked_mul(4)
            .is_none_or(|bytes| bytes > max_io)
    {
        return Err(VerifyError::new(
            "resource certificate has an invalid panel extent",
        ));
    }
    let mut left_reader = TensorReader::new(root, left, max_io);
    let left_values = read_left_piece(&mut left_reader, spec, inner, panel_elements)?;
    let mut right_reader = TensorReader::new(root, right, max_io);
    let mut output = allocate_i32(
        checked_usize_product(&[spec.row_count, columns], "output piece value count")?,
        "reference output piece",
    )?;
    let panel_bytes = panel_elements * 4;
    let mut encoded_right = allocate_bytes(panel_bytes, "encoded right panel")?;
    let mut decoded_right = allocate_i32(panel_elements, "decoded right panel")?;

    // This implementation owns all address derivation and arithmetic. Global k
    // is the outer loop, so every output accumulator observes strict ascending-k
    // wrapping arithmetic regardless of row/column traversal order.
    for contracted in 0..inner_usize {
        let right_row = checked_mul(
            u64::try_from(contracted)
                .map_err(|_| VerifyError::new("contracted index does not fit u64"))?,
            columns,
            "right row first value",
        )?;
        let mut column_start = 0_usize;
        while column_start < columns_usize {
            let count = (columns_usize - column_start).min(panel_elements);
            let byte_count = count
                .checked_mul(4)
                .ok_or_else(|| VerifyError::new("right panel byte count overflows usize"))?;
            let first_value = checked_add(
                right_row,
                u64::try_from(column_start)
                    .map_err(|_| VerifyError::new("right column does not fit u64"))?,
                "right panel first value",
            )?;
            right_reader.read_exact_at(
                checked_mul(first_value, 4, "right panel byte offset")?,
                &mut encoded_right[..byte_count],
            )?;
            for local_column in (0..count).rev() {
                let source = local_column * 4;
                decoded_right[local_column] = i32::from_le_bytes(
                    encoded_right[source..source + 4]
                        .try_into()
                        .expect("four-byte i32 decode"),
                );
            }
            for local_row in (0..rows).rev() {
                let lhs = left_values[local_row * inner_usize + contracted];
                for local_column in (0..count).rev() {
                    let index = local_row * columns_usize + column_start + local_column;
                    output[index] =
                        output[index].wrapping_add(lhs.wrapping_mul(decoded_right[local_column]));
                }
            }
            column_start += count;
        }
    }
    Ok((output, add_io(left_reader.counters, right_reader.counters)?))
}

fn compare_piece(
    output_reader: &mut TensorReader<'_>,
    expected: &[i32],
    spec: PieceSpec,
    columns: u64,
    panel_elements: usize,
) -> Result<()> {
    let panel_bytes = panel_elements
        .checked_mul(4)
        .ok_or_else(|| VerifyError::new("output panel bytes overflow usize"))?;
    let mut actual = allocate_bytes(panel_bytes, "output comparison panel")?;
    let global_first = checked_mul(spec.row_start, columns, "output piece first value")?;
    for (panel_index, expected_panel) in expected.chunks(panel_elements).enumerate() {
        let piece_offset = panel_index
            .checked_mul(panel_elements)
            .ok_or_else(|| VerifyError::new("output panel value offset overflows usize"))?;
        let byte_count = expected_panel
            .len()
            .checked_mul(4)
            .ok_or_else(|| VerifyError::new("output comparison byte count overflows usize"))?;
        let first_value = checked_add(
            global_first,
            u64::try_from(piece_offset)
                .map_err(|_| VerifyError::new("output panel offset does not fit u64"))?,
            "output panel first value",
        )?;
        output_reader.read_exact_at(
            checked_mul(first_value, 4, "output panel byte offset")?,
            &mut actual[..byte_count],
        )?;
        for index in (0..expected_panel.len()).rev() {
            let byte = index * 4;
            let observed = i32::from_le_bytes(
                actual[byte..byte + 4]
                    .try_into()
                    .expect("four-byte output i32 decode"),
            );
            if observed != expected_panel[index] {
                let piece_value = u64::try_from(panel_index * panel_elements + index)
                    .map_err(|_| VerifyError::new("piece mismatch index does not fit u64"))?;
                let global_value = checked_add(global_first, piece_value, "global mismatch index")?;
                return Err(VerifyError::new(format!(
                    "exact GEMM mismatch at output value {global_value}: expected {}, observed {observed}",
                    expected_panel[index]
                )));
            }
        }
    }
    Ok(())
}

fn exact_replay(
    root: &Path,
    image: &MachineImage,
    left: &LoadedTensor,
    right: &LoadedTensor,
    output: &LoadedTensor,
    specs: &[PieceSpec],
    max_io: usize,
) -> Result<ReplayOutcome> {
    let columns = right.manifest.ty.shape[1];
    let inner = left.manifest.ty.shape[1];
    let panel = usize::try_from(image.resource_certificate.panel_elements)
        .map_err(|_| VerifyError::new("panel extent does not fit usize"))?;
    let mut pieces_checked = 0_u64;
    let mut bytes_checked = 0_u64;
    let mut operations = 0_u128;
    let mut input_io = IoCounters::default();
    let mut output_io = IoCounters::default();

    // Keep one authenticated output reader per logical lane, matching the
    // frozen one-cache counter model while executing sequentially in this
    // standalone process.
    for lane in 0..image.resource_certificate.lanes {
        let mut output_reader = TensorReader::new(root, &output.manifest, max_io);
        for spec in specs
            .iter()
            .filter(|candidate| candidate.lane_index == lane)
        {
            let (expected, piece_input_io) =
                compute_piece(root, image, &left.manifest, &right.manifest, *spec, max_io)?;
            input_io = add_io(input_io, piece_input_io)?;
            compare_piece(&mut output_reader, &expected, *spec, columns, panel)?;
            pieces_checked = checked_add(pieces_checked, 1, "replay pieces checked")?;
            bytes_checked = checked_add(
                bytes_checked,
                checked_mul(
                    checked_mul(spec.row_count, columns, "piece output values")?,
                    4,
                    "piece output bytes",
                )?,
                "replay bytes checked",
            )?;
            operations = operations
                .checked_add(operation_count(spec.row_count, inner, columns)?)
                .ok_or_else(|| VerifyError::new("replay operation count overflows u128"))?;
        }
        output_io = add_io(output_io, output_reader.counters)?;
    }
    if pieces_checked != specs.len() as u64
        || bytes_checked != output.manifest.byte_length
        || operations != operation_count(left.manifest.ty.shape[0], inner, columns)?
    {
        return Err(VerifyError::new(
            "fresh replay did not cover the complete output geometry",
        ));
    }
    Ok(ReplayOutcome {
        pieces_checked,
        bytes_checked,
        operations,
        input_io,
        output_io,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
        image_path: PathBuf,
        result_path: PathBuf,
        store: PathBuf,
        left: StoredTensor,
        output: StoredTensor,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    impl Fixture {
        fn create() -> Self {
            Self::create_with_values(
                &[1, 2, 3, 4, 5, 6],
                &[7, 8, 9, 10, 11, 12],
                &[58, 64, 139, 154],
            )
        }

        #[allow(clippy::too_many_lines)]
        fn create_with_values(
            left_values: &[i32],
            right_values: &[i32],
            output_values: &[i32],
        ) -> Self {
            let unique = FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "mfenx-contract-v1-verifier-test-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir(&root).expect("create fixture root");
            let store = root.join("store");
            fs::create_dir(&store).expect("create fixture store");
            let left_type = TensorType {
                element: ElementType::I32,
                shape: vec![2, 3],
            };
            let right_type = TensorType {
                element: ElementType::I32,
                shape: vec![3, 2],
            };
            let output_type = TensorType {
                element: ElementType::I32,
                shape: vec![2, 2],
            };
            let left = publish_tensor(&store, left_type.clone(), left_values, 16);
            let right = publish_tensor(&store, right_type.clone(), right_values, 16);
            let output = publish_tensor(&store, output_type.clone(), output_values, 16);
            let max_io = 16_u64;
            let max_managed = 16 * 1024 * 1024;
            let left_loaded = load_tensor(&store, &left, max_io).expect("load fixture left");
            let right_loaded = load_tensor(&store, &right, max_io).expect("load fixture right");
            let certificate =
                derive_resource_certificate(&left_loaded, &right_loaded, max_managed, max_io, 2)
                    .expect("derive fixture certificate");
            let program = expected_program(&left_type, &right_type, &output_type);
            let program_digest = typed_digest(&program, "fixture program").expect("program digest");
            let image = MachineImage {
                schema_version: IMAGE_SCHEMA,
                isa_version: ISA_REVISION,
                machine_class: MachineClass::SoftwareDefinedLocalSupercomputerV2,
                backend: "rarecomp_mfenx_local_cpu_lane_engine".into(),
                execution_dataflow: ExecutionDataflow::ContiguousRightPanelsV2,
                program_digest,
                program,
                inputs: Inputs {
                    left: left.clone(),
                    right: right.clone(),
                },
                instructions: vec![MachineInstruction::StreamedI32Gemm {
                    left: left.clone(),
                    right: right.clone(),
                    output_type: output_type.clone(),
                }],
                schedule: LaneSchedule {
                    schema_version: SCHEDULE_SCHEMA,
                    instruction_index: 0,
                    policy: LanePolicy::DeterministicStripedV1,
                    lanes: certificate.lanes,
                    piece_count: certificate.piece_count,
                },
                resource_certificate: certificate,
                verification: VerificationPolicy::ExactReplay,
            };
            let image_digest = typed_digest(&image, "fixture image").expect("image digest");
            let certificate_digest =
                typed_digest(&image.resource_certificate, "fixture certificate")
                    .expect("certificate digest");
            let output_loaded = load_tensor(&store, &output, max_io).expect("load fixture output");
            let specs = piece_specs(&image, &output_type).expect("fixture piece specs");
            let assignments = specs
                .iter()
                .map(|spec| PieceAssignment {
                    piece_index: spec.index,
                    lane_index: spec.lane_index,
                })
                .collect::<Vec<_>>();
            let primary_io = simulate_primary_io(
                &image,
                &left_loaded.manifest,
                &right_loaded.manifest,
                &specs,
                &assignments,
            )
            .expect("fixture primary I/O");
            let (verification_input, verification_output) = simulate_replay_io(
                &image,
                &left_loaded.manifest,
                &right_loaded.manifest,
                &output_loaded.manifest,
                &specs,
            )
            .expect("fixture verification I/O");
            let admission = IoCounters {
                requested_bytes: 0,
                authenticated_chunk_bytes: left.byte_length + right.byte_length,
                chunk_loads: (left_loaded.manifest.chunks.len()
                    + right_loaded.manifest.chunks.len()) as u64,
                cache_hits: 0,
            };
            let useful = operation_count(2, 3, 2).expect("fixture operations");
            let retained_storage = image.resource_certificate.retained_input_storage_bytes
                + image.resource_certificate.retained_output_storage_bytes
                + image
                    .resource_certificate
                    .retained_checkpoint_storage_upper_bound_bytes;
            let result = ExecutionResult {
                schema_version: RESULT_SCHEMA,
                image_digest,
                machine_class: MachineClass::SoftwareDefinedLocalSupercomputerV2,
                backend: "rarecomp_mfenx_local_cpu_lane_engine".into(),
                resource_certificate_digest: certificate_digest,
                outputs: vec![Output {
                    index: 0,
                    tensor: output.clone(),
                }],
                metrics: ExecutionMetrics {
                    end_to_end_ns: 3,
                    execution_ns: 1,
                    finalization_ns: 1,
                    useful_integer_operations: useful,
                    executed_primary_integer_operations: useful,
                    physical_integer_operations: useful * 2,
                    primary_input_io: primary_io,
                    total_pieces: specs.len() as u64,
                    reused_pieces: 0,
                    executed_pieces: specs.len() as u64,
                    reused_assignments: vec![],
                    executed_assignments: assignments,
                    lanes: image.resource_certificate.lanes,
                    certified_managed_peak_bytes: image
                        .resource_certificate
                        .certified_managed_peak_bytes,
                    retained_storage_bytes: retained_storage,
                    gpu_devices_required: 0,
                    network_transports_required: 0,
                    runtime_telemetry_attestation: AttestationStatus::SelfReported,
                    recovery_partition_attestation: AttestationStatus::SelfReported,
                    directory_metadata_sync: if cfg!(unix) {
                        DirectoryMetadataSync::Available
                    } else {
                        DirectoryMetadataSync::Unavailable
                    },
                    process_crash_recovery: ProcessCrashRecovery::Supported,
                },
                verification: ClaimedVerification {
                    verified: true,
                    method: "parallel_independently_addressed_exact_replay_v2".into(),
                    pieces_checked: specs.len() as u64,
                    bytes_checked: output.byte_length,
                    verification_integer_operations: useful,
                    admission_input_io: admission,
                    input_io: verification_input,
                    output_io: verification_output,
                    verification_ns: 1,
                },
            };
            let image_path = root.join("image.json");
            let result_path = root.join("result.json");
            write_pretty(&image_path, &image);
            write_pretty(&result_path, &result);
            Self {
                root,
                image_path,
                result_path,
                store,
                left,
                output,
            }
        }

        fn verify(&self) -> Result<VerificationReport> {
            super::verify(&self.image_path, &self.result_path, &self.store)
        }
    }

    fn encode_i32(values: &[i32]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect()
    }

    fn publish_tensor(
        root: &Path,
        ty: TensorType,
        values: &[i32],
        chunk_bytes: u32,
    ) -> StoredTensor {
        let payload = encode_i32(values);
        assert_eq!(
            payload.len() as u64,
            tensor_byte_length(&ty).expect("tensor length")
        );
        let mut chunks = Vec::new();
        for (index, bytes) in payload.chunks(chunk_bytes as usize).enumerate() {
            let digest = chunk_digest(bytes);
            let path = object_path(root, "chunks", &digest, "bin").expect("chunk path");
            fs::create_dir_all(path.parent().expect("chunk parent")).expect("chunk directory");
            fs::write(path, bytes).expect("write chunk");
            chunks.push(ChunkRef {
                offset: index as u64 * u64::from(chunk_bytes),
                length: u32::try_from(bytes.len()).expect("fixture chunk length fits u32"),
                digest,
            });
        }
        let mut manifest = Manifest {
            schema_version: MANIFEST_SCHEMA,
            ty: ty.clone(),
            encoding: TensorEncoding::CanonicalLeRowMajor,
            byte_length: payload.len() as u64,
            chunk_bytes,
            chunks,
            content_root: String::new(),
        };
        manifest.content_root = manifest_root(&manifest).expect("manifest root");
        let path =
            object_path(root, "manifests", &manifest.content_root, "json").expect("manifest path");
        fs::create_dir_all(path.parent().expect("manifest parent")).expect("manifest directory");
        fs::write(
            &path,
            serde_json::to_vec(&manifest).expect("canonical manifest"),
        )
        .expect("write manifest");
        StoredTensor {
            manifest_digest: manifest.content_root,
            ty,
            byte_length: payload.len() as u64,
        }
    }

    fn write_pretty<T: Serialize>(path: &Path, value: &T) {
        fs::write(path, serde_json::to_vec_pretty(value).expect("pretty JSON"))
            .expect("write JSON");
    }

    #[test]
    fn accepts_independent_small_fixture() {
        let fixture = Fixture::create();
        let report = fixture.verify().expect("fixture must verify");
        assert!(report.accepted);
        assert_eq!(
            report.identities.output_manifest_root,
            fixture.output.manifest_digest
        );
        assert_eq!(report.arithmetic.integer_operations, 24);
        assert_eq!(report.arithmetic.values_checked, 4);
    }

    #[test]
    fn accepts_wrapping_i32_overflow_fixture() {
        let fixture = Fixture::create_with_values(
            &[i32::MAX, i32::MAX, i32::MAX, i32::MIN, -1, 2],
            &[2, -3, i32::MAX, i32::MIN, -1, 5],
            &[i32::MIN, 2_147_483_646, i32::MAX, 10],
        );
        let report = fixture.verify().expect("overflow fixture must verify");
        assert!(report.checks.exact_output_replay);
        assert_eq!(report.arithmetic.integer_operations, 24);
    }

    #[test]
    fn accepts_value_canonical_control_reencoding() {
        let fixture = Fixture::create();
        for path in [&fixture.image_path, &fixture.result_path] {
            let value: serde_json::Value =
                serde_json::from_slice(&fs::read(path).expect("read pretty control JSON"))
                    .expect("parse control JSON value");
            fs::write(
                path,
                serde_json::to_vec(&value).expect("compact control JSON"),
            )
            .expect("write compact control JSON");
        }
        fixture
            .verify()
            .expect("value-equivalent compact control JSON must verify");
    }

    #[test]
    fn accepts_fully_recovered_result() {
        let fixture = Fixture::create();
        let mut result: ExecutionResult =
            read_control(&fixture.result_path, "test result").expect("parse result");
        result.metrics.reused_assignments = result.metrics.executed_assignments.clone();
        result.metrics.executed_assignments.clear();
        result.metrics.reused_pieces = result.metrics.total_pieces;
        result.metrics.executed_pieces = 0;
        result.metrics.executed_primary_integer_operations = 0;
        result.metrics.physical_integer_operations = result.metrics.useful_integer_operations;
        result.metrics.primary_input_io = IoCounters::default();
        write_pretty(&fixture.result_path, &result);
        fixture
            .verify()
            .expect("complete recovery partition with exact output must verify");
    }

    #[test]
    fn accepts_producer_directory_sync_capability_independent_of_verifier_host() {
        let fixture = Fixture::create();
        let mut result: ExecutionResult =
            read_control(&fixture.result_path, "test result").expect("parse result");
        result.metrics.directory_metadata_sync = match result.metrics.directory_metadata_sync {
            DirectoryMetadataSync::Available => DirectoryMetadataSync::Unavailable,
            DirectoryMetadataSync::Unavailable => DirectoryMetadataSync::Available,
        };
        write_pretty(&fixture.result_path, &result);
        fixture
            .verify()
            .expect("producer capability must not be inferred from verifier host");
    }

    #[test]
    fn rejects_empty_and_extreme_shapes_without_panicking() {
        let empty = TensorType {
            element: ElementType::I32,
            shape: vec![0, 1],
        };
        let one = TensorType {
            element: ElementType::I32,
            shape: vec![1, 1],
        };
        assert!(gemm_output_type(&empty, &one).is_err());

        let extreme = TensorType {
            element: ElementType::I32,
            shape: vec![u64::MAX, u64::MAX],
        };
        assert!(tensor_byte_length(&extreme).is_err());
    }

    #[test]
    fn matches_standalone_conformance_domains() {
        let payload = encode_i32(&[1, 2, 3, 4]);
        let expected_chunk = "fdeae937cf9d2fc823b0207cdf8ea4f24679bf24985da2cdba52af75059d4db9";
        assert_eq!(chunk_digest(&payload), expected_chunk);
        let manifest = Manifest {
            schema_version: 1,
            ty: TensorType {
                element: ElementType::I32,
                shape: vec![2, 2],
            },
            encoding: TensorEncoding::CanonicalLeRowMajor,
            byte_length: 16,
            chunk_bytes: 16,
            chunks: vec![ChunkRef {
                offset: 0,
                length: 16,
                digest: expected_chunk.into(),
            }],
            content_root: "e4044cd7014ba71d3a0fd7750572a5057b044a399807b801ee47f11a4630e366".into(),
        };
        assert_eq!(
            manifest_root(&manifest).expect("manifest root"),
            manifest.content_root
        );
        assert_eq!(
            serde_json::to_vec(&manifest).expect("manifest JSON").len(),
            321
        );
    }

    #[test]
    fn rejects_noncanonical_manifest_bytes() {
        let fixture = Fixture::create();
        let path = object_path(
            &fixture.store,
            "manifests",
            &fixture.left.manifest_digest,
            "json",
        )
        .expect("left manifest path");
        let mut bytes = fs::read(&path).expect("read manifest");
        bytes.push(b'\n');
        fs::write(path, bytes).expect("mutate manifest");
        assert!(
            fixture
                .verify()
                .expect_err("must reject")
                .to_string()
                .contains("not exact compact")
        );
    }

    #[test]
    fn rejects_reordered_manifest_members() {
        let fixture = Fixture::create();
        let loaded = load_tensor(&fixture.store, &fixture.left, 16).expect("load left");
        let manifest = &loaded.manifest;
        let reordered = format!(
            "{{\"ty\":{},\"schema_version\":{},\"encoding\":{},\"byte_length\":{},\"chunk_bytes\":{},\"chunks\":{},\"content_root\":{}}}",
            serde_json::to_string(&manifest.ty).expect("type JSON"),
            manifest.schema_version,
            serde_json::to_string(&manifest.encoding).expect("encoding JSON"),
            manifest.byte_length,
            manifest.chunk_bytes,
            serde_json::to_string(&manifest.chunks).expect("chunks JSON"),
            serde_json::to_string(&manifest.content_root).expect("root JSON"),
        );
        let path = object_path(
            &fixture.store,
            "manifests",
            &fixture.left.manifest_digest,
            "json",
        )
        .expect("left manifest path");
        fs::write(path, reordered).expect("write reordered manifest");
        assert!(
            fixture
                .verify()
                .expect_err("must reject")
                .to_string()
                .contains("not exact compact")
        );
    }

    #[test]
    fn rejects_noncanonical_manifest_chunk_geometry() {
        let fixture = Fixture::create();
        let mut loaded = load_tensor(&fixture.store, &fixture.left, 16).expect("load left");
        loaded.manifest.chunks[0].offset = 1;
        let path = object_path(
            &fixture.store,
            "manifests",
            &fixture.left.manifest_digest,
            "json",
        )
        .expect("left manifest path");
        fs::write(
            path,
            serde_json::to_vec(&loaded.manifest).expect("canonical manifest JSON"),
        )
        .expect("write changed manifest");
        assert!(
            fixture
                .verify()
                .expect_err("must reject")
                .to_string()
                .contains("noncanonical geometry")
        );
    }

    #[test]
    fn rejects_corrupt_authenticated_chunk() {
        let fixture = Fixture::create();
        let loaded = load_tensor(&fixture.store, &fixture.left, 16).expect("load left");
        let digest = &loaded.manifest.chunks[0].digest;
        let path = object_path(&fixture.store, "chunks", digest, "bin").expect("chunk path");
        let mut bytes = fs::read(&path).expect("read chunk");
        bytes[0] ^= 1;
        fs::write(path, bytes).expect("mutate chunk");
        assert!(
            fixture
                .verify()
                .expect_err("must reject")
                .to_string()
                .contains("chunk digest mismatch")
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_manifest_base_directory() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::create();
        let base = fixture.store.join("manifests");
        let moved = fixture.root.join("real-manifests");
        fs::rename(&base, &moved).expect("move manifest base");
        symlink(&moved, &base).expect("symlink manifest base");
        assert!(
            fixture
                .verify()
                .expect_err("must reject")
                .to_string()
                .contains("manifests base")
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_store_root() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::create();
        let linked_store = fixture.root.join("linked-store");
        symlink(&fixture.store, &linked_store).expect("symlink store root");
        assert!(
            super::verify(&fixture.image_path, &fixture.result_path, &linked_store)
                .expect_err("must reject")
                .to_string()
                .contains("tensor store is not a real non-symlink directory")
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_manifest_file() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::create();
        let path = object_path(
            &fixture.store,
            "manifests",
            &fixture.left.manifest_digest,
            "json",
        )
        .expect("manifest path");
        let moved = fixture.root.join("real-left-manifest.json");
        fs::rename(&path, &moved).expect("move manifest");
        symlink(&moved, &path).expect("symlink manifest");
        assert!(
            fixture
                .verify()
                .expect_err("must reject")
                .to_string()
                .contains("tensor manifest is not a regular non-symlink file")
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_chunk_prefix_directory() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::create();
        let left = load_tensor(&fixture.store, &fixture.left, 16).expect("load left");
        let prefix = &left.manifest.chunks[0].digest[..2];
        let directory = fixture.store.join("chunks").join(prefix);
        let moved = fixture.root.join("real-chunk-prefix");
        fs::rename(&directory, &moved).expect("move chunk prefix");
        symlink(&moved, &directory).expect("symlink chunk prefix");
        assert!(
            fixture
                .verify()
                .expect_err("must reject")
                .to_string()
                .contains("chunks prefix")
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_chunk_file() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::create();
        let left = load_tensor(&fixture.store, &fixture.left, 16).expect("load left");
        let digest = &left.manifest.chunks[0].digest;
        let path = object_path(&fixture.store, "chunks", digest, "bin").expect("chunk path");
        let moved = fixture.root.join("real-left-chunk.bin");
        fs::rename(&path, &moved).expect("move chunk");
        symlink(&moved, &path).expect("symlink chunk");
        assert!(
            fixture
                .verify()
                .expect_err("must reject")
                .to_string()
                .contains("tensor chunk is not a regular non-symlink file")
        );
    }

    #[test]
    fn rejects_coherently_readdressed_wrong_output() {
        let fixture = Fixture::create();
        let wrong = publish_tensor(
            &fixture.store,
            TensorType {
                element: ElementType::I32,
                shape: vec![2, 2],
            },
            &[59, 64, 139, 154],
            16,
        );
        let mut result: ExecutionResult =
            read_control(&fixture.result_path, "test result").expect("parse result");
        result.outputs[0].tensor = wrong;
        write_pretty(&fixture.result_path, &result);
        assert!(
            fixture
                .verify()
                .expect_err("must reject")
                .to_string()
                .contains("exact GEMM mismatch")
        );
    }

    #[test]
    fn rejects_coherently_rechunked_correct_output() {
        let fixture = Fixture::create();
        let rechunked = publish_tensor(
            &fixture.store,
            TensorType {
                element: ElementType::I32,
                shape: vec![2, 2],
            },
            &[58, 64, 139, 154],
            8,
        );
        let image: MachineImage =
            read_control(&fixture.image_path, "test image").expect("parse image");
        let left = load_tensor(&fixture.store, &image.inputs.left, 16).expect("load left");
        let right = load_tensor(&fixture.store, &image.inputs.right, 16).expect("load right");
        let output = load_tensor(&fixture.store, &rechunked, 16).expect("load rechunked output");
        let specs = piece_specs(&image, &rechunked.ty).expect("piece specs");
        let (_, output_io) = simulate_replay_io(
            &image,
            &left.manifest,
            &right.manifest,
            &output.manifest,
            &specs,
        )
        .expect("rechunked output I/O");
        let mut result: ExecutionResult =
            read_control(&fixture.result_path, "test result").expect("parse result");
        result.outputs[0].tensor = rechunked;
        result.verification.output_io = output_io;
        write_pretty(&fixture.result_path, &result);
        assert!(
            fixture
                .verify()
                .expect_err("must reject coherently rechunked output")
                .to_string()
                .contains("output manifest chunk extent")
        );
    }

    #[test]
    fn rejects_unknown_control_member() {
        let fixture = Fixture::create();
        let mut image: serde_json::Value =
            serde_json::from_slice(&fs::read(&fixture.image_path).expect("read image"))
                .expect("parse image value");
        image
            .as_object_mut()
            .expect("image object")
            .insert("unexpected".into(), serde_json::json!(true));
        write_pretty(&fixture.image_path, &image);
        assert!(
            fixture
                .verify()
                .expect_err("must reject")
                .to_string()
                .contains("strict machine image schema")
        );
    }

    #[test]
    fn rejects_duplicate_frozen_input_member() {
        let fixture = Fixture::create();
        let image: MachineImage =
            read_control(&fixture.image_path, "test image").expect("parse image");
        let duplicate = format!(
            "\"inputs\": {{\n    \"left\": {},",
            serde_json::to_string(&image.inputs.left).expect("serialize duplicate input")
        );
        let text = fs::read_to_string(&fixture.image_path).expect("read image");
        let changed = text.replacen("\"inputs\": {", &duplicate, 1);
        assert_ne!(text, changed);
        fs::write(&fixture.image_path, changed).expect("write duplicate input image");
        assert!(
            fixture
                .verify()
                .expect_err("must reject duplicate input member")
                .to_string()
                .contains("duplicate field `left`")
        );
    }

    #[test]
    fn rejects_duplicate_struct_member() {
        let fixture = Fixture::create();
        let text = fs::read_to_string(&fixture.result_path).expect("read result");
        let changed = text.replacen('{', "{\n  \"schema_version\": 3,", 1);
        fs::write(&fixture.result_path, changed).expect("write duplicate result member");
        assert!(
            fixture
                .verify()
                .expect_err("must reject duplicate struct member")
                .to_string()
                .contains("duplicate field `schema_version`")
        );
    }

    #[test]
    fn rejects_result_counter_forgery() {
        let fixture = Fixture::create();
        let mut result: ExecutionResult =
            read_control(&fixture.result_path, "test result").expect("parse result");
        result.metrics.primary_input_io.cache_hits += 1;
        write_pretty(&fixture.result_path, &result);
        assert!(
            fixture
                .verify()
                .expect_err("must reject")
                .to_string()
                .contains("deterministic claims")
        );
    }

    #[test]
    fn rejects_piece_partition_forgery() {
        let fixture = Fixture::create();
        let mut result: ExecutionResult =
            read_control(&fixture.result_path, "test result").expect("parse result");
        result.metrics.executed_assignments[0].lane_index = 63;
        write_pretty(&fixture.result_path, &result);
        assert!(
            fixture
                .verify()
                .expect_err("must reject")
                .to_string()
                .contains("wrong lane")
        );
    }

    #[test]
    fn rejects_retained_storage_forgery() {
        let fixture = Fixture::create();
        let mut result: ExecutionResult =
            read_control(&fixture.result_path, "test result").expect("parse result");
        result.metrics.retained_storage_bytes += 1;
        write_pretty(&fixture.result_path, &result);
        assert!(
            fixture
                .verify()
                .expect_err("must reject")
                .to_string()
                .contains("deterministic claims")
        );
    }

    #[test]
    fn rejects_result_image_digest_forgery() {
        let fixture = Fixture::create();
        let mut result: ExecutionResult =
            read_control(&fixture.result_path, "test result").expect("parse result");
        result.image_digest = "0".repeat(64);
        write_pretty(&fixture.result_path, &result);
        assert!(
            fixture
                .verify()
                .expect_err("must reject")
                .to_string()
                .contains("frozen result envelope")
        );
    }

    #[test]
    fn rejects_resource_certificate_mutation() {
        let fixture = Fixture::create();
        let mut image: MachineImage =
            read_control(&fixture.image_path, "test image").expect("parse image");
        image.resource_certificate.panel_elements -= 1;
        write_pretty(&fixture.image_path, &image);
        assert!(
            fixture
                .verify()
                .expect_err("must reject")
                .to_string()
                .contains("resource certificate")
        );
    }

    #[test]
    fn rejects_noninteger_control_number_lexeme() {
        let fixture = Fixture::create();
        let text = fs::read_to_string(&fixture.image_path).expect("read image");
        let changed = text.replacen("\"schema_version\": 3", "\"schema_version\": 3e0", 1);
        assert_ne!(text, changed);
        fs::write(&fixture.image_path, changed).expect("mutate image number");
        assert!(
            fixture
                .verify()
                .expect_err("must reject")
                .to_string()
                .contains("noncanonical integer token")
        );
    }
}

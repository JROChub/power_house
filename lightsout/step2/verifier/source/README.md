# MFENX contract-v1 reference verifier

This crate is a standalone verifier for
`mfenx-replay-gated-execution-contract/v1`. It has no dependency on the MFENX
executor, tensor-store crate, or product binary. It owns its wire types,
canonicalization checks, digest derivations, bounded content-addressed reader,
resource-certificate derivation, result validation, and wrapping-`i32` GEMM.

The release binary is built from the self-contained source directory distributed
with the verifier, not from the larger MFENX workspace. That distinction keeps
Cargo feature resolution and root-crate metadata outside the executor workspace.
The included `rust-toolchain.toml` pins Rust 1.93.1. From that standalone
directory, build and run it with:

```text
env -u RUSTFLAGS -u CARGO_ENCODED_RUSTFLAGS \
  CARGO_INCREMENTAL=0 CARGO_NET_OFFLINE=true SOURCE_DATE_EPOCH=0 \
  TZ=UTC LC_ALL=C \
  cargo build --release --locked --offline --jobs 1
target/release/mfenx-contract-v1-verifier \
  --image artifacts/gemm.mfx.json \
  --result artifacts/uninterrupted.result.json \
  --store store
```

Two clean builds from different standalone source directories produced identical
release bytes. A build selected through the enclosing MFENX workspace is not the
published verifier build boundary and may have a different binary digest even
when it is functionally equivalent.

The verifier never executes `mfenx-local`. A zero exit status means the image,
result, manifests, chunks, deterministic claims, and independently recomputed
output all passed. The JSON report distinguishes the result's claimed replay
from the fresh replay performed by this program.

This is a separate process and binary with verifier-owned wire structs, digest
derivations, resource formulas, range accounting, and arithmetic loops. It does
not import, depend on, or execute the MFENX executor or tensor-store crates. It
is still maintained in the same repository and uses the same Rust toolchain and
contract specification, so it is not an independently developed implementation,
a diverse-toolchain check, or a separate security authority.

`directory_metadata_sync` records a capability of the runtime that produced the
result. The verifier validates it as a strict typed value but does not infer it
from the verifier host; a result produced on Unix remains verifiable elsewhere.

The store is required to be a private, caller-controlled real directory. Store
roots, `manifests`/`chunks` bases, digest-prefix directories, manifests, and
chunks are rejected when they are symlinks. As frozen by the threat model, the
verifier does not defend against a privileged or same-user writer racing those
checks and reads; concurrent hostile mutation and general filesystem TOCTOU are
outside this local contract boundary.

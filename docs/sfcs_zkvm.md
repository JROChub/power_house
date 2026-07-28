# SFCS Whole-Program Private VM

Status: implemented release surface for Power House v0.4.0.

Power House v0.4.0 adds an opt-in whole-program private execution backend
through the `sfcs-risc0` feature. It accepts a RISC Zero program binary,
executes it with private input, verifies a real cryptographic receipt, binds
the deterministic public statement into SFCS and `.pha`, creates Rootprint
identity, emits an SLBIT-compatible semantic packet, and packages the receipt
for offline Memory Capsule verification.

This document distinguishes that authoritative whole-program path from the
older scoped `sfcs-zk` compatibility profiles.

## Whole-Program Proof Boundary

The `sfcs-risc0` path proves the execution relation enforced by the RISC Zero
RISC-V zkVM. A successful receipt authenticates:

- the exact guest image ID;
- successful guest termination;
- the public journal committed by the guest;
- one cryptographically verified execution of that image;
- privacy of data supplied through the guest input channel, subject to the
  RISC Zero proof-system assumptions and the guest's own disclosure behavior.

The guest decides what becomes public by committing bytes to its journal.
Power House never treats an unverified execution trace, a coverage counter, or
a collection of unrelated commitments as a whole-program proof.

The backend uses RISC Zero 3.0.6 with `disable-dev-mode`. It also inspects the
receipt variant and rejects `InnerReceipt::Fake` before verification. Setting
`RISC0_DEV_MODE` cannot make a fake receipt valid in this build.

## Deterministic SFCS Identity

RISC Zero receipts may contain proof transport details that are not suitable
as deterministic Power House identity. The integration therefore separates
the deterministic statement from the receipt:

```text
RISC Zero program binary
  -> image ID
  -> complete program digest
  -> deterministic SFCS graph projection
  -> public journal digest
  -> successful receipt-claim digest
  -> deterministic .pha core

verified receipt + complete program binary
  -> mandatory external proof attachment
```

The SFCS graph projects both the user and kernel executable text carried by
the RISC Zero `R0BF` program binary. The complete binary is also bound by its
image ID and a domain-separated SHA-256 digest.

The `.pha` core contains only the deterministic statement. The complete
program and receipt are carried in one integrity-checked external attachment.
Because Power House v1 deliberately excludes external attachments from
`phx_fingerprint`, two valid receipts for the same program and public journal
retain the same `.pha` and Rootprint identity.

Core verification alone does not silently promote an attachment into a
Power House proof. Use the protocol-specific verifier:

```rust
use power_house::verify_sfcs_risc0_private_vm_embedding;

let proof = verify_sfcs_risc0_private_vm_embedding(&artifact)?;
```

That verifier requires exactly one correctly identified receipt attachment,
checks its transport digest, parses bounded canonical base64, reconstructs the
program binding, rejects fake receipts, verifies the cryptographic receipt,
checks the journal and successful claim, and compares the attachment statement
with the deterministic `.pha` core.

## Memory Capsule And Rootprint

The whole-program proof can be carried by Rootprint and a Memory Capsule
without changing Rootprint v1 rules:

```rust
use power_house::{
    provenance::Rootprint,
    verify_sfcs_risc0_private_vm_capsule,
    MemoryCapsuleBuilder,
    MemoryVerificationPolicy,
};

let rootprint = Rootprint::new("private-program", artifact.clone())?;
let capsule = MemoryCapsuleBuilder::new("private-program")
    .with_pha(artifact)
    .with_rootprint(rootprint)
    .with_replay_required()
    .build()?;

verify_sfcs_risc0_private_vm_capsule(
    &capsule,
    MemoryVerificationPolicy::strict(),
)?;
```

`verify_sfcs_risc0_private_vm_capsule(...)` first verifies the complete
capsule and Rootprint replay, then invokes cryptographic receipt verification.
Generic Memory Capsule verification remains backward-compatible and does not
pretend to understand every external proof system.

## Rust To Private Proof

RISC Zero supplies the compiler and guest runtime for the whole-program path.
A guest is ordinary Rust compiled to the pinned RISC-V zkVM target. It may use
the guest-compatible Rust ecosystem and can implement arithmetic, branches,
loops, memory, functions, and application-specific logic.

The checked-in conformance guest reads two private `u32` values and publishes
only their wrapping sum:

```rust
#![no_main]
#![no_std]

use risc0_zkvm::guest::env;

risc0_zkvm::guest::entry!(main);

fn main() {
    let mut private_values = [0_u32; 2];
    env::read_slice(&mut private_values);
    env::commit(&private_values[0].wrapping_add(private_values[1]));
}
```

The reproducible guest workspace is in `risc0-methods/`. CI rebuilds the
program and byte-compares it with
`conformance/sfcs-risc0/private-sum-v1.bin` before running proof gates.

This is the broad compiler path for private general-purpose programs. The
existing direct Rust, LLVM-style SSA, and WASM-style stack frontends remain
deterministic scoped source-to-fractal interfaces; they are not described as
complete implementations of those languages.

## CLI Pipeline

Install the whole-program backend:

```bash
cargo install power_house --features sfcs-risc0
```

Build a RISC Zero guest with the pinned toolchain, prepare raw private input,
then run:

```bash
julian sfcs risc0-prove guest.bin \
  --input private-input.bin \
  --artifact-output execution.pha \
  --rootprint-output execution.rootprint.json \
  --capsule-output execution.phm \
  --semantic-output execution.slbit.json \
  --sidecar-output execution.observatory.json \
  --report execution.report.json \
  --label private-program

julian sfcs verify-risc0-pha execution.pha
julian sfcs verify-risc0-capsule execution.phm
```

The command does not place the private input bytes in its report, semantic
packet, `.pha` public statement, or Rootprint identity. A guest can still
deliberately publish private data in its journal, so guest review remains part
of the privacy boundary.

## SLBIT Separation

The CLI emits an SLBIT-compatible semantic packet and Observatory sidecar
bound to the verified Rootprint branch and replay fingerprint. The semantic
packet contains only the public statement.

SLBIT remains non-core:

- changing semantic text does not change `phx_fingerprint`;
- changing a sidecar does not change Rootprint identity;
- semantic validity never substitutes for receipt verification;
- generated explanation is marked non-authoritative.

## Scoped Compatibility Profiles

The `sfcs-zk` feature remains available for compatibility:

- `power-house/sfcs-zk-private-add/v1-draft` proves the admitted private
  no-overflow add relation;
- `power-house/sfcs-zk-private-vm/v1-draft` carries commitments and individual
  proofs for selected linear, range, memory, bitwise, comparison, and branch
  relations.

The private-VM draft does **not** cryptographically link every individual
relation proof, global trace digest, final state, and public output into one
authoritative hidden execution. It must not be used as the whole-program
arbitrary private VM security boundary. Use `sfcs-risc0` for that requirement.

The public deterministic VM and constraint profiles remain useful for visible
replay, debugging, conformance, and exact transition inspection.

## Origin

The `sfcs` feature also provides `Origin`, a transactional deterministic
creation API:

```rust
let mut origin = Origin::manifest(spec, policy)?;
let receipt = origin.derive(next_spec)?;
origin.verify()?;
```

`Origin` prepares, verifies, and commits deterministic native SFCS creation as
one in-memory transition. Failed derivation leaves identity, lineage, receipt,
and creative-capacity accounting unchanged. Creative capacity is an
identity-bound software resource budget; it is not electricity, currency, or
cross-process authorization.

`Origin` and `sfcs-risc0` have separate proof boundaries. Origin does not turn
a public deterministic SFCS replay into a zero-knowledge proof.

## Release Gates

The whole-program release gate includes:

- reproducible guest compilation and byte comparison;
- a real receipt generated outside development mode;
- fake-receipt rejection;
- wrong-program rejection;
- program and graph content binding;
- receipt and statement mutation rejection;
- deterministic `.pha` and Rootprint identity with receipt transport removed;
- exact private-input byte absence from serialized receipt transport;
- Rootprint replay and protocol-specific offline Memory Capsule verification;
- complete CLI prove, package, and reverify coverage;
- all-feature Clippy and rustdoc with warnings denied.

Run the local gates with:

```bash
PATH="$HOME/.risc0/bin:$PATH" \
  cargo test --locked --features sfcs-risc0 \
  --test sfcs_risc0 \
  --test sfcs_risc0_cli
```

## Security Boundary

Power House verifies deterministic identity, transport integrity, Rootprint
lineage, capsule replay, and the binding between those objects and the RISC
Zero public statement. RISC Zero verifies the hidden RISC-V execution.

Neither layer proves that the source code expresses the user's intent, that a
public output is factually true outside the program, or that a guest did not
publish sensitive data. Those remain source-review and application-policy
responsibilities.

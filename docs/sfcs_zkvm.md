# SFCS Provenance-First Private VM

Status: implemented release surface for Power House v0.3.24.

This document describes the shipped SFCS private VM, compiler, provenance, and
observability surface. It is written as an implementation reference for the
crate, CLI, `.pha` artifacts, Rootprint lineage, Memory Capsules, and semantic
sidecars that are produced by the current release.

## Objective

SFCS makes the Power House computation graph the native representation for
program structure, execution evidence, proof binding, identity, and provenance.
The implementation maps source and VM execution into deterministic
computational-fractal graphs, records proof evidence as `.pha` payloads, anchors
identity through Rootprint, and packages the result as Memory Capsules that can
be verified offline.

The core objective is direct source-to-fractal execution: Power House workloads
should preserve structure instead of losing it through an external static
circuit layer. The SFCS surface therefore prioritizes deterministic fractal
execution, Sovereign Fast Path routing where structure is available, and one
unified identity path for proof, replay, provenance, and observability.

## Implemented VM Core

The `sfcs::vm` module provides a deterministic RV32I execution foundation.

Implemented capabilities:

- deterministic RV32I interpreter;
- full instruction transition trace;
- register and memory digests before and after each step;
- public register and memory outputs;
- VM execution projected into committed SFCS graph nodes;
- `.pha` embedding protocol `power-house/sfcs-vm-execution/v1-draft`;
- verifier `verify_sfcs_vm_execution_embedding(...)`;
- CLI commands `julian sfcs vm-run` and `julian sfcs verify-vm-pha`;
- unit, CLI, mutation, and property-based reproducibility coverage.

The VM core preserves existing `.pha` fingerprint rules, Rootprint v1 branch
semantics, Memory Capsule verification order, and offline replay behavior.

## Public VM Constraint Proofs

The public VM constraint layer verifies supported RV32I executions by
deterministic replay and explicit constraint coverage.

Implemented capabilities:

- protocol `power-house/sfcs-vm-constraints/v1-draft`;
- proof object `SfcsVmConstraintProof`;
- verifier `verify_sfcs_vm_constraint_embedding(...)`;
- CLI commands `julian sfcs vm-constraints` and
  `julian sfcs verify-vm-constraints-pha`;
- transition checks for every executed instruction;
- register range coverage counts for every step;
- memory range and consistency coverage for every memory access;
- trace digest, final state digest, final memory digest, and
  execution-fractal digest binding;
- `.pha` embedding and Memory Capsule compatibility;
- mutation coverage for transition commitments and public-input proof digest
  substitution.

This transparent profile is useful when public replay and exact constraint
visibility are desired.

## Private VM Proof Profiles

The `sfcs-zk` feature provides privacy-preserving SFCS proof profiles that hide
private witness inputs and private trace data while publishing public outputs,
commitments, and verifier-side proof evidence.

Implemented protocols:

- `power-house/sfcs-zk-private-add/v1-draft`;
- `power-house/sfcs-zk-private-vm/v1-draft`.

Implemented private proof capabilities:

- private no-overflow RV32I add statement;
- Pedersen commitments to hidden input registers;
- Fiat-Shamir Schnorr proof that committed private inputs sum to the public
  output;
- private VM witness file support with hidden initial registers, memory, trace
  digest, execution-fractal digest, final state digest, final memory digest,
  and private constraint-proof digest;
- Fiat-Shamir Schnorr opening proofs for committed private VM execution
  digests;
- verifier-side homomorphic private transition proofs for no-overflow and
  no-underflow linear VM relations, including `add`, `addi`, `sub`, and `subi`;
- public-scale relation checks such as no-overflow `slli`;
- zero-knowledge 32-bit range proofs for committed private VM values, built
  from bit commitments, finite OR proofs that each bit is zero or one, and
  homomorphic recomposition to the original value commitment;
- zero-knowledge read-after-write memory consistency proofs for private memory
  events where a read is backed by a prior hidden write to the same hidden
  address and width;
- private `lw` and `sw` address-calculation proofs where the address relation
  is eligible for the linear proof layer;
- equality proofs binding memory access values to source or destination
  register values;
- byte-level memory semantics proofs for `sb`, `sh`, `sw`, `lb`, `lh`, `lw`,
  `lbu`, and `lhu`;
- low-byte store extraction, byte read-after-write consistency, and sign/zero
  extension binding;
- finite-relation bitwise proofs for `and`, `or`, `xor`, `andi`, `ori`, and
  `xori`, tied to the same bit commitments used by the range layer;
- unsigned and signed comparison proofs for `slt`, `sltu`, `slti`, and
  `sltiu` using 32-bit slack commitments and finite-relation OR proofs;
- branch condition proofs for equality, non-equality, signed order, and
  unsigned order branches: `beq`, `bne`, `blt`, `bge`, `bltu`, and `bgeu`;
- public output, transition coverage, register range coverage, memory range
  coverage, memory consistency, and branch coverage binding for private VM
  proof statements;
- `.pha` embedding and replay verification;
- CLI commands `julian sfcs zk-private-add`, `julian sfcs zk-private-vm`, and
  `julian sfcs verify-zk-pha`;
- mutation coverage for public outputs, proof bodies, challenges, overflow,
  program-shape rejection, private VM commitments, private public fields,
  opening responses, linear relation proofs, range-proof bit responses, memory
  equality proofs, bitwise proofs, comparison proofs, branch proofs, and
  partial-width memory semantics.

The verifier learns the declared public outputs, public commitments, and
declared coverage metadata. Hidden witness values and hidden trace data stay
inside the private proof/witness boundary.

## Source-To-Fractal Compiler Surface

SFCS compiler paths lower source into deterministic computational-fractal
graphs instead of making an external circuit compiler the identity authority.

Implemented compiler APIs:

- `compile_private_add_source(...)`;
- `compile_public_rust_source(...)`;
- `compile_llvm_ir_source(...)`;
- `compile_wasm_stack_source(...)`.

Implemented schemas:

- `power-house/sfcs-rust-private-add/v1-draft`;
- `power-house/sfcs-rust-public/v1-draft`;
- `power-house/sfcs-llvm-ir/v1-draft`;
- `power-house/sfcs-wasm-stack/v1-draft`.

Implemented frontend coverage:

- private Rust-subset shape: one `u32 + u32 -> u32` function lowered into
  private RV32I add proof memory;
- public Rust-subset expression functions with multiple `u32` parameters,
  arithmetic, comparisons, and `if { } else { }` expressions;
- LLVM-style SSA i32 functions with arithmetic, bitwise operations, unsigned
  comparisons, `select`, constants, labels, and explicit returns;
- WASM-style i32 stack IR with params, locals, constants, arithmetic, bitwise
  operations, comparisons, `select`, and `return`;
- deterministic lowering to SFCS graphs and VM proof paths;
- deterministic source digest and semantic packet digest;
- slbit-style semantic packet generation with non-authoritative explanation
  constraints;
- compiler acceptance, compiler rejection, and CLI end-to-end coverage.

Every frontend path preserves the rule that `.pha` and Rootprint are the core
identity authorities. Semantic packets explain the result after binding and do
not mutate proof identity.

## End-To-End Proof Memory Commands

Private add source-to-proof-to-Memory-Capsule:

```bash
julian sfcs rust-private-add private_add.rs \
  --lhs-value 144 \
  --rhs-value 233 \
  --lhs-blinding 1111111111111111111111111111111111111111111111111111111111111111 \
  --rhs-blinding 2222222222222222222222222222222222222222222222222222222222222222 \
  --artifact-output private-add.pha \
  --rootprint-output private-add.rootprint.json \
  --sidecar-output private-add.observatory.json \
  --capsule-output private-add.phm \
  --report private-add.report.json
julian sfcs verify-zk-pha private-add.pha
julian memory verify private-add.phm
```

Private VM proof-memory pipeline:

```bash
julian sfcs zk-private-vm private-vm.program.json \
  --witness private-vm.witness.json \
  --artifact-output private-vm.pha \
  --rootprint-output private-vm.rootprint.json \
  --semantic-output private-vm.semantic.json \
  --sidecar-output private-vm.observatory.json \
  --capsule-output private-vm.phm \
  --report private-vm.report.json
julian sfcs verify-zk-pha private-vm.pha
julian memory verify private-vm.phm
```

The private VM command verifies the proof, embeds it into `.pha`, initializes a
Rootprint, binds a non-core semantic packet through an Observatory sidecar,
packages the result as a Memory Capsule, and verifies that capsule before
writing reports.

## Verification Output

Successful `julian sfcs verify-zk-pha` output reports the protocol, public
output binding, proof digest, and embedded artifact identity.

Successful `julian memory verify` output reports:

```text
CORE        VALID
ROOTPRINT   VALID
REPLAY      VALID
SIDECAR     VALID
SEMANTIC    VALID
```

The Memory Capsule report keeps the layer boundary explicit. A semantic packet
can be valid, invalid, accepted, or rejected without silently changing the
core `.pha` fingerprint or Rootprint branch identity.

## Rejection Behavior

The implementation rejects tampering at the layer where it occurs:

- changed public output;
- changed proof body;
- changed Fiat-Shamir challenge;
- overflow in no-overflow private add;
- unsupported private program shape;
- changed private VM commitment;
- changed private VM public field;
- changed opening response;
- changed linear transition proof;
- changed range-proof bit response;
- changed memory equality proof;
- changed bitwise finite-relation proof;
- changed signed or unsigned comparison proof;
- changed equality, non-equality, or order branch proof;
- changed partial-width memory extraction or sign/zero-extension binding;
- changed Rootprint binding;
- changed semantic packet digest;
- changed sidecar replay fingerprint.

When a semantic mutation is rejected, the report records that the core proof
identity remains unchanged. When a core proof mutation is rejected, the report
records the core layer failure directly.

## Truth Boundary

Power House verifies proof, provenance, `.pha` fingerprint, Rootprint lineage,
Memory Capsule binding, and replay state.

SFCS represents computation as deterministic fractal execution and proof
evidence inside that Power House identity model.

slbit-style semantic packets and Observatory sidecars explain verified proof
memory. They are non-core presentation and inspection layers. They can bind to
verified branch IDs, replay fingerprints, and packet digests, but they do not
change core proof identity.

## Expansion Model

Additional Rust, LLVM, WASM, and VM surfaces are admitted through the same
deterministic criteria used here:

- concrete lowering into SFCS or VM execution;
- concrete proof and verification relation;
- `.pha` embedding;
- Rootprint binding;
- Memory Capsule packaging;
- semantic sidecar binding;
- mutation coverage;
- offline verification.

This keeps the SFCS direction clear: computation enters the Power House
identity system as deterministic fractal execution and proof memory, not as an
external circuit artifact that becomes authoritative outside Rootprint.

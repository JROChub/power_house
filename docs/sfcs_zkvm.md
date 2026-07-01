# SFCS Provenance-First zkVM Gate

Status: implementation gate.

This document records the release rule for building a general-purpose zkVM on
top of Power House + SFCS.

## Objective

The target system is a provenance-first zkVM where:

- normal programs execute through a real VM;
- private inputs can be proven without disclosure;
- public outputs, Rootprint lineage, and `.pha` fingerprints remain stable and
  offline-verifiable;
- slbit explains execution meaning without changing proof identity;
- Memory Capsules package the complete proof-memory object.

The long-term objective is to make external circuit compilers and standalone
zkVM workflows unnecessary as the default path for workloads Power House
targets. That objective must not be achieved by weakening deterministic replay,
`.pha` identity, Rootprint v1 compatibility, Memory Capsule integrity, or slbit
separation.

## Phase Gate 1: VM Core

Implemented foundation:

- `sfcs::vm`;
- deterministic RV32I interpreter;
- full instruction transition trace;
- register and memory digests before and after each step;
- public register and memory outputs;
- VM execution projected into committed SFCS graph nodes;
- `.pha` embedding protocol `power-house/sfcs-vm-execution/v1-draft`;
- verifier `verify_sfcs_vm_execution_embedding(...)`;
- CLI commands `julian sfcs vm-run` and `julian sfcs verify-vm-pha`;
- unit, CLI, mutation, and property-based reproducibility tests.

The VM core is not allowed to change `.pha` fingerprint rules or Rootprint v1
branch semantics.

## Phase Gate 1B: Public VM Constraint Proofs

Implemented for arbitrary public RV32I executions supported by the VM core:

- protocol `power-house/sfcs-vm-constraints/v1-draft`;
- proof object `SfcsVmConstraintProof`;
- verifier `verify_sfcs_vm_constraint_embedding(...)`;
- CLI commands `julian sfcs vm-constraints` and
  `julian sfcs verify-vm-constraints-pha`;
- transition checks for every executed instruction;
- register range coverage counts for every step;
- memory range and consistency coverage for every memory access;
- trace digest, final state digest, final memory digest, and execution-fractal
  digest binding;
- `.pha` embedding and Memory Capsule compatibility;
- mutation tests for transition commitments and public input proof digest
  substitution.

This gate is transparent and public. It proves the supported VM transition and
memory rules by deterministic replay. It does not hide private VM state and is
not a substitute for the arbitrary private VM zero-knowledge gate.

## Phase Gate 2: Real Zero-Knowledge Privacy

Partially implemented for one constrained profile.

Implemented groundwork:

- feature flag `sfcs-zk`;
- protocol `power-house/sfcs-zk-private-add/v1-draft`;
- protocol `power-house/sfcs-zk-private-vm/v1-draft`;
- private no-overflow RV32I add statement;
- Pedersen commitments to hidden input registers;
- Fiat-Shamir Schnorr proof that committed private inputs sum to the public
  output;
- private VM witness file support with hidden initial registers, memory, trace
  digest, execution-fractal digest, final state digest, final memory digest,
  and private constraint-proof digest;
- Fiat-Shamir Schnorr opening proofs for committed private VM execution
  digests;
- verifier-side homomorphic private transition proofs for no-overflow/no-
  underflow linear VM relations (`add`, `addi`, `sub`, `subi`) and public-scale
  relations such as no-overflow `slli`;
- zero-knowledge 32-bit range proofs for committed private VM values used by
  those transition proofs, built from bit commitments, OR proofs that each bit
  is zero or one, and homomorphic recomposition to the original value
  commitment;
- zero-knowledge read-after-write memory consistency proofs for private memory
  events where a read is backed by a prior hidden write to the same hidden
  address and width;
- private `lw/sw` address-calculation proofs where the address relation is
  eligible for the linear proof layer, plus equality proofs binding memory
  access values to the source or destination register values;
- byte-level memory semantics proofs for `sb`, `sh`, `sw`, `lb`, `lh`, `lw`,
  `lbu`, and `lhu`, including low-byte store extraction, byte read-after-write
  consistency, and sign/zero extension binding;
- finite-relation bitwise proofs for `and`, `or`, `xor`, `andi`, `ori`, and
  `xori`, tied to the same bit commitments used by the range layer;
- unsigned and signed comparison proofs for `slt`, `sltu`, `slti`, and
  `sltiu` using 32-bit slack commitments and finite-relation OR proofs;
- branch condition proofs for equality, non-equality, signed order, and
  unsigned order branches (`beq`, `bne`, `blt`, `bge`, `bltu`, `bgeu`);
- public output, transition coverage, register range coverage, memory range
  coverage, memory consistency, and branch coverage binding for private VM
  proof statements;
- `.pha` embedding and replay verification;
- CLI commands `julian sfcs zk-private-add`, `julian sfcs zk-private-vm`, and
  `julian sfcs verify-zk-pha`;
- mutation tests for public output, proof body, challenge, overflow, wrong
  program shape, private VM commitments, private VM public fields, opening
  responses, linear relation proofs, range proof bit responses, and memory
  equality proofs.

These profiles are accepted as privacy milestones. The private-VM profile
hides supported VM witnesses and trace data while proving verifier-side
evidence for linear arithmetic, range/bitness, bitwise operations, comparisons,
byte-addressed memory semantics, memory consistency, memory/register binding,
and branch conditions for the supported RV32I execution subset. The full
production arbitrary-private-zkVM gate remains a larger security and compiler
target: it requires independent review, broader conformance vectors,
performance hardening, and unrestricted Rust/LLVM/binary-WASM compatibility.

Required before promotion:

- a concrete proof system that verifies VM transition semantics;
- private input commitments;
- public output binding;
- verifier learns no private input or private trace values beyond the declared
  public outputs and commitments;
- invalid private witnesses fail;
- malformed commitments fail;
- `.pha` and Rootprint integration remains offline-verifiable;
- security review of soundness and zero-knowledge assumptions.

Commitment-only hiding is not enough for the final arbitrary private zkVM
claim. The current private-VM proof layer now carries real verifier-side ZK
evidence for the supported RV32I linear, range, bitwise, comparison, byte
memory, memory-binding, and branch relations it admits, but production
promotion still requires reviewed coverage for every admitted VM class.

For the complete gate, the proof system must continue expanding from the
current supported RV32I subset into a reviewed production relation that covers
instruction decoding, register transition, range constraints, byte memory,
branch behavior, halting, and public output binding for every admitted VM
class inside the zero-knowledge relation.

## Phase Gate 3: Compiler Frontend

Partially implemented for scoped Rust, LLVM-style SSA, and WASM-style subsets.

Implemented groundwork:

- compiler API `compile_private_add_source(...)`;
- compiler API `compile_public_rust_source(...)`;
- compiler API `compile_llvm_ir_source(...)`;
- compiler API `compile_wasm_stack_source(...)`;
- schema `power-house/sfcs-rust-private-add/v1-draft`;
- schema `power-house/sfcs-rust-public/v1-draft`;
- schema `power-house/sfcs-llvm-ir/v1-draft`;
- schema `power-house/sfcs-wasm-stack/v1-draft`;
- accepted source shape: one `u32 + u32 -> u32` function;
- public Rust-subset expression functions with multiple `u32` parameters,
  arithmetic, comparisons, and `if { } else { }` expressions;
- LLVM-style SSA i32 functions with arithmetic, bitwise operations, unsigned
  comparisons, `select`, constants, labels, and explicit returns;
- WASM-style i32 stack IR with params, locals, constants, arithmetic, bitwise
  operations, comparisons, `select`, and `return`;
- deterministic lowering to the private-add RV32I `add; ecall` program;
- deterministic lowering to SFCS graphs for public compiler paths;
- deterministic source digest and semantic packet digest;
- slbit-style semantic packet generation with non-authoritative explanation
  constraints;
- one-command CLI pipeline `julian sfcs rust-private-add` that creates a
  `.pha` artifact, Rootprint graph, Observatory sidecar, Memory Capsule, and
  machine-readable report;
- one-command private VM proof-memory pipeline `julian sfcs zk-private-vm`
  that creates a `.pha` artifact, Rootprint graph, slbit-style semantic packet,
  Observatory sidecar, Memory Capsule, and machine-readable report for the
  supported private RV32I VM profile;
- public CLI compiler commands `julian sfcs rust-public`,
  `julian sfcs llvm-ir`, and
  `julian sfcs wasm-stack`;
- compiler acceptance/rejection tests and CLI end-to-end tests.

This satisfies scoped source-to-fractal compiler milestones, the first
source-to-proof-to-memory-capsule milestone for the private-add profile, and
the direct private-VM proof-to-memory-capsule milestone for the supported
private RV32I profile. It does not satisfy the full compiler gate for normal
Rust crates, unrestricted LLVM IR, or binary WebAssembly modules.

Required before promotion:

- a defined Rust subset or LLVM/WASM path;
- deterministic lowering into the VM/SFCS execution path;
- reproducible build inputs and compiler version metadata;
- generated Rootprint provenance hooks;
- slbit semantic packet generation;
- mutation tests for source, compiler IR, VM program, trace, proof, and
  semantic sidecar.

The compiler must not silently use an external circuit as the authoritative
identity layer.

## Phase Gate 4: Full Pipeline

Implemented for the constrained private-add source path and the supported
private RV32I VM proof path.

Implemented private-add source command shape:

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

Implemented private VM proof-memory command shape:

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

Required unrestricted compiler command shape remains gated until the broader
Rust/LLVM/binary-WASM compiler paths lower into the private VM proof relation:

Required command shape:

```bash
power-house-zkvm build program.rs --output program.sfcs-vm.json
power-house-zkvm prove program.sfcs-vm.json --private input.json --output proof.pha
julian rootprint init proof.pha --label main --output proof.rootprint.json
julian memory create --pha proof.pha --rootprint proof.rootprint.json --output proof.phm
julian memory verify proof.phm
```

Required verification result:

```text
VM          VALID
ZK          VALID
PHA         VALID
ROOTPRINT   VALID
REPLAY      VALID
MEMORY      VALID
SLBIT       VALID
```

## Rejection Rules

The release must reject:

- changed instruction word;
- changed register output;
- changed memory output;
- changed private witness;
- changed proof bytes;
- changed Rootprint branch ID;
- changed `.pha` public inputs;
- changed VM execution-fractal node;
- changed slbit semantic packet digest;
- unsupported critical extension;
- malformed canonical JSON;
- duplicate JSON keys.

Every rejection must name the layer where the falsehood failed.

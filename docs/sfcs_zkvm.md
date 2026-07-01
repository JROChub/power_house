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
- private no-overflow RV32I add statement;
- Pedersen commitments to hidden input registers;
- Fiat-Shamir Schnorr proof that committed private inputs sum to the public
  output;
- `.pha` embedding and replay verification;
- CLI commands `julian sfcs zk-private-add` and `julian sfcs verify-zk-pha`;
- mutation tests for public output, proof body, challenge, overflow, and wrong
  program shape.

This profile is accepted only as the first privacy layer. It does not satisfy
the full arbitrary VM privacy gate.

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

Commitment-only hiding is not enough. A release must include a verifier that
checks a real proof of execution.

For the complete gate, the proof system must cover more than the current
private-add profile. It must verify instruction decoding, register transition,
range constraints, memory consistency, branch behavior, halting, and public
output binding for the supported VM class.

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
- public CLI compiler commands `julian sfcs rust-public`,
  `julian sfcs llvm-ir`, and
  `julian sfcs wasm-stack`;
- compiler acceptance/rejection tests and CLI end-to-end tests.

This satisfies scoped source-to-fractal compiler milestones and the first
source-to-proof-to-memory-capsule milestone for the private-add profile. It
does not satisfy the full compiler gate for normal Rust crates, unrestricted
LLVM IR, or binary WebAssembly modules.

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

Partially implemented for the constrained private-add path.

Implemented command shape:

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

Required full general-purpose command shape remains blocked until all earlier
gates pass:

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

## Public Claim Rule

Until Phase Gates 2, 3, and 4 are complete for the private arbitrary
VM/compiler class, documentation must say:

```text
SFCS has a deterministic VM execution foundation, public VM transition and
memory constraint proofs, scoped Rust/LLVM/WASM-style source-to-fractal
compilers, and a constrained end-to-end private-add
source-to-proof-to-Memory-Capsule path.
```

It must not say:

```text
Power House ships a complete general-purpose zkVM.
```

The complete zkVM claim is allowed only after the full compile -> prove ->
verify -> provenance -> observability pipeline works and passes the release
gates above.

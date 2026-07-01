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

## Phase Gate 2: Real Zero-Knowledge Privacy

Blocked until implemented.

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

## Phase Gate 3: Compiler Frontend

Blocked until implemented.

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

Blocked until all earlier gates pass.

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

Until Phase Gates 2, 3, and 4 are complete, documentation must say:

```text
SFCS has a deterministic VM execution foundation.
```

It must not say:

```text
Power House ships a complete general-purpose zkVM.
```

The complete zkVM claim is allowed only after the full compile -> prove ->
verify -> provenance -> observability pipeline works and passes the release
gates above.

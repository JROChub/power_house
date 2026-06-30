# Sovereign Fractal Computation Substrate

Status: experimental design gate for Power House v0.3.14.

SFCS is a proposed opt-in computational-fractal layer. It must not change
Power House v0.3.14 core guarantees unless a future schema is separately
specified, implemented, tested, and released.

## Compatibility Decision

The implementation guide's core direction is valuable, but one correction is
mandatory: SFCS must not overload Rootprint v1 nodes as executable program
nodes. Rootprint v1 identity is already defined over branch label, sorted
parents, and `.pha` `phx_fingerprint`. Changing that would break existing
branch IDs, replay fingerprints, conformance vectors, Memory Capsules, and
slbit sidecar bindings.

The safe integration path is:

1. represent computation in an SFCS graph;
2. compute a deterministic SFCS graph digest;
3. commit the SFCS graph as ordinary `.pha` core proof payload;
4. anchor that `.pha` artifact through Rootprint v1;
5. bind Memory Capsule and slbit semantic packets after Rootprint replay.

This keeps every existing invariant intact while creating a path for a future
Rootprint v2 or dedicated SFCS capsule schema.

## Non-Negotiable Invariants

- `.pha` `phx_fingerprint` remains calculated exactly by the `.pha` schema in
  force for that artifact.
- Rootprint v1 branch IDs remain calculated from label, parents, and carried
  artifact `phx_fingerprint`.
- Rootprint replay remains graph-structural and does not execute SFCS payloads.
- Memory Capsule v1 verification continues to run core, Rootprint, replay,
  sidecar, semantic, and witness checks in the existing order.
- slbit remains non-core and may explain SFCS results only after binding to
  verified branch IDs and replay fingerprints.
- Structure discovery must be deterministic. It cannot depend on CPU timing,
  network state, thread scheduling, random exploration, or cache state.

## Current Repository Surface

The first implementation step is intentionally small and safe:

- feature flag: `sfcs`
- module: `src/sfcs/mod.rs`
- schema: `power-house/sfcs-fractal/v1-draft`
- bridge: `SfcsGraph::to_pha_artifact(...)`

The feature is not in the default feature set. Enabling it adds draft types for
computational fractal nodes, deterministic graph digestion, basic structure
discovery, strict duplicate-key JSON parsing, a simple arithmetic evaluator,
a fast-path workload descriptor, and `.pha` embedding verification. It does
not change existing public behavior.

## Corrected Architecture

```text
Program or producer
        |
        v
SFCS graph (draft computational fractal)
        |
        | deterministic digest
        v
.pha core payload with protocol power-house/sfcs/v1-draft
        |
        v
Rootprint v1 branch
        |
        v
Memory Capsule v1 replay and challenge checks
        |
        v
slbit semantic packet binding
```

Rootprint can carry SFCS computation by carrying the `.pha` artifact. It does
not become an executable VM by mutation of the v1 schema.

## Structure Discovery Rules

The draft discovery engine classifies:

- fast-path eligible: `input`, `const`, `add`, `mul`, `fast_path_claim`;
- dense/general: `branch`, `dense_step`, `memory_read`, `memory_write`.

Future versions may discover larger algebraic regions, but every rewrite must
be recorded as deterministic data and must produce the same output from the
same input across operating systems, architectures, compiler versions, and
network conditions.

## SFCS `.pha` Embedding Verification

`SfcsGraph::to_pha_artifact(...)` commits the graph into a normal `.pha`
artifact. `verify_sfcs_pha_embedding(...)` then checks the additional SFCS
binding:

- the `.pha` core fingerprint is valid;
- the embedded protocol is `power-house/sfcs/v1-draft`;
- the embedded proof payload decodes as a valid SFCS graph;
- the graph digest matches `provenance.fractal_digest`;
- public node counters match deterministic structure discovery.

This matters because a `.pha` can be core-valid while still carrying stale or
inconsistent SFCS metadata. SFCS-specific validation is explicit and separate,
matching the existing Power House boundary pattern.

## What Is Not Implemented Yet

The draft module does not claim:

- complete arbitrary program execution;
- zkVM replacement;
- cryptographic proof generation for dense control flow;
- semantic equivalence between optimized and unoptimized programs;
- Rootprint v2 executable-node semantics.

Those require separate conformance vectors, mutation tests, independent review,
and a release gate.

## Required Release Gates Before Promotion

Before SFCS can move beyond draft status:

1. Legacy `.pha`, Rootprint, identity, Memory Capsule, slbit, and sparse
   conformance vectors must pass unchanged.
2. SFCS canonical digests must match across Rust, Python, and browser/WASM
   vectors.
3. SFCS graph parsing must reject duplicate keys, floats, cycles, missing
   references, unsupported critical extensions, and unknown executable ops.
4. Structure-discovery rewrites must be deterministic and mutation-tested.
5. Any future executable Rootprint schema must be versioned separately from
   Rootprint v1.
6. Public docs must describe SFCS as experimental until dense soundness,
   replay, and equivalence proofs are independently validated.

## Validation

Run the current draft tests with:

```bash
cargo test --features sfcs --test sfcs
```

Run legacy gates to prove isolation:

```bash
cargo test --locked
cargo test --features sfcs --locked
```

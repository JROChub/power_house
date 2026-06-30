# Sovereign Fractal Computation Substrate

Status: SFCS implementation contract and experimental release gate for Power
House v0.3.18.

SFCS is the path for making the Rootprint-fractal graph the native
representation, execution environment, and proof substrate for Power House
computation. The end-state objective is explicit: for the workloads Power
House targets, direct source-to-fractal execution, deterministic structure
discovery, and Sovereign Fast Path routing must make traditional circuit
compilers and zkVM workflows unnecessary and unwise as the default path.

The v0.3.18 implementation is not the completed end-state. It is a guarded
milestone that moves the repository toward that objective while preserving the
guarantees that cannot be compromised: deterministic replay, `.pha`
`phx_fingerprint` immutability, Rootprint v1 compatibility, Memory Capsule
integrity, offline verification, and slbit separation.

## Development Contract

The SFCS work is governed by these non-negotiable requirements:

1. The primary objective is to make external circuit compilers and zkVM
   approaches unnecessary and inferior as the default path for the general
   dense arbitrary circuit workloads Power House targets.
2. The fractal graph is the primary computational representation, not an
   after-the-fact visualization or optional metadata side feature.
3. Direct source-to-fractal mapping is prioritized over traditional
   source-to-circuit compilation.
4. Structure discovery must maximize deterministic routing into the Sovereign
   Fast Path wherever algebraic structure exists.
5. The final SFCS system must deliver unified identity and provenance across
   structured and general workloads under the Power House identity model.
6. No SFCS step may weaken or alter `.pha` `phx_fingerprint` rules, break
   Rootprint v1 verification, compromise Memory Capsule integrity, require
   network access for local verification, or introduce nondeterministic
   behavior.
7. Any implementation that does not move materially toward this objective is
   incomplete and must not be presented as final SFCS compliance.

This document therefore separates two statements that must both remain true:

- **Objective:** SFCS is being built so external circuit compilers and zkVM
  workflows become unnecessary and unwise for targeted Power House workloads.
- **Current release boundary:** v0.3.18 is a tested foundation step, not the
  full dense arbitrary computation end-state.

## Compatibility Decision

The implementation guide's core direction is mandatory, but one compatibility
constraint must be respected while Rootprint v1 remains the public identity
schema: SFCS must not mutate Rootprint v1 node semantics in place. Rootprint
v1 identity is already defined over branch label, sorted parents, and `.pha`
`phx_fingerprint`. Changing that would break existing branch IDs, replay
fingerprints, conformance vectors, Memory Capsules, and slbit sidecar
bindings.

The safe integration path is:

1. represent computation in an SFCS graph;
2. compute a deterministic SFCS graph digest;
3. commit the SFCS graph as ordinary `.pha` core proof payload;
4. anchor that `.pha` artifact through Rootprint v1;
5. bind Memory Capsule and slbit semantic packets after Rootprint replay.

This is not a retreat into external compilation. It is the safe bridge that
lets SFCS computation be represented directly as deterministic fractal payload
today, while creating the path for a future Rootprint v2 or dedicated SFCS
capsule schema where executable fractal nodes become first-class identity
objects.

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

The v0.3.18 implementation is still intentionally isolated, but it now covers
the first executable SFCS workflow:

- feature flag: `sfcs`
- module: `src/sfcs/mod.rs`
- schema: `power-house/sfcs-fractal/v1-draft`
- graph bridge: `SfcsGraph::to_pha_artifact(...)`
- execution bridge: `SfcsGraph::to_execution_pha_artifact(...)`
- native source bridge: `SfcsGraph::from_source(...)`
- execution protocol: `power-house/sfcs-execution/v1-draft`

The feature is not in the default feature set. Enabling it adds draft types for
computational fractal nodes, deterministic graph digestion, basic structure
discovery, strict duplicate-key JSON parsing, direct textual SFCS program
parsing, arithmetic execution traces, synthesis plans, a fast-path workload
descriptor, and `.pha` embedding verification. It does not change existing
public behavior.

v0.3.18 strengthens this draft with committed source metadata, additional
control-oriented source operations, deterministic connected structure regions,
region digests, synthesis operations bound back to the exact region that
created them, and the higher-level native expression frontend
`SfcsGraph::from_source(...)`. The frontend accepts `input`, `let`, and
`output` statements and lowers expressions directly into committed fractal
nodes. This is the preferred SFCS direction because developers express
computation as source that becomes the graph itself, not as an external
circuit artifact.

## Corrected Architecture

```text
Program or producer
        |
        v
SFCS graph (draft computational fractal)
        |
        | deterministic digest + optional execution trace + synthesis plan
        v
.pha core payload with protocol power-house/sfcs/v1-draft
or power-house/sfcs-execution/v1-draft
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

## Direct SFCS Program Parsing

`SfcsGraph::from_program(...)` parses a small deterministic source form
directly into fractal nodes. This is not a traditional circuit compiler: each
source line becomes one first-class node or output declaration.

Supported lines:

```text
input a
input b
const c 7
add sum a b
sub delta sum c
eq same sum c
not changed same
branch selected changed delta sum
mul z sum c
label z Final structured output
meta z source direct-fractal
output z
```

The parsed graph is verified using the same structural checks as JSON-loaded
graphs: valid IDs, no duplicate nodes, no duplicate inputs, known references,
declared outputs, acyclic topology, committed metadata limits, and no control
characters in labels or metadata.

## Native Expression Source

`SfcsGraph::from_source(...)` is the higher-level source-to-fractal frontend.
It maps source statements into the graph directly and creates deterministic
intermediate nodes for nested expressions.

Supported statements:

```text
input a
input b
let delta = a-b
let same = a == b
let doubled = delta * 2
let fallback = a + a
let out = if !same then doubled else fallback
output out
```

Supported expression features:

- integer constants;
- identifiers;
- `+`, `-`, and `*`;
- `==`, `&&`, `||`, and `!`;
- parentheses;
- `if <expr> then <expr> else <expr>`;
- `output <expr> as <id>` for output expressions with a stable node ID.

Repeated source values such as `a + a` are represented by explicit
deterministic `alias` nodes. The graph therefore remains strict about duplicate
operation inputs while still supporting normal source expressions.

Expression lowering commits generated nodes, source-operation metadata, and
the resulting graph digest. Mutating generated nodes, aliases, constants,
operation kinds, public outputs, traces, or synthesis plans changes the
replayed embedding and is rejected by SFCS-specific verification.

## Structure Discovery Rules

The draft discovery engine classifies:

- fast-path eligible: `input`, `alias`, `const`, `add`, `sub`, `mul`,
  `fast_path_claim`;
- dense/general: `eq`, `and`, `or`, `not`, `branch`, `dense_step`,
  `memory_read`, `memory_write`.

It then groups same-kind connected nodes into deterministic structure regions.
Each region records:

- stable region ID;
- region kind;
- topologically ordered nodes;
- entry nodes with dependencies outside the region;
- output nodes consumed outside the region or exported by the graph;
- source graph digest;
- domain-separated region digest.

Region ordering follows dependency completion, not the earliest independent
node in a region. This prevents a downstream fast-path region from being
recorded before the dense/control boundary it depends on.

Future versions may discover larger algebraic regions, but every rewrite must
be recorded as deterministic data and must produce the same output from the
same input across operating systems, architectures, compiler versions, and
network conditions.

## Execution Trace

`SfcsGraph::execution_trace(...)` executes the draft arithmetic subset and
emits a digest-bound trace:

- graph digest;
- input digest;
- output digest;
- per-step node digests;
- final trace digest;
- public outputs.

The draft evaluator supports `input`, `alias`, `const`, `add`, `sub`, `mul`,
`eq`, `and`, `or`, `not`, and `branch`. Dense and memory placeholders are
rejected by the evaluator until a future profile defines their proof semantics.

## Synthesis Plan

`SfcsGraph::synthesis_plan(...)` deterministically records where the graph can
be routed to the Sovereign fast path and where dense/general boundaries remain.
Each operation has its own operation digest and is bound to the exact structure
region digest that produced it. The plan also emits an embedding invariant
digest that binds the graph digest to the synthesis digest.

This is the first repository step toward the intended SFCS model:

```text
program -> fractal graph -> execution trace -> synthesis plan -> .pha identity
```

No hidden rewrite or optimization is accepted. Rewrites must become explicit,
digest-bound data.

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

`SfcsGraph::to_execution_pha_artifact(...)` commits graph, execution trace, and
synthesis plan into a normal `.pha` artifact. `verify_sfcs_execution_embedding`
then replays the graph from public inputs, regenerates the synthesis plan,
checks public outputs, verifies public region counters, and verifies provenance
digests.

## Completion Gap

The v0.3.18 module is not final SFCS compliance yet. The remaining gaps are:

- complete arbitrary program execution beyond the arithmetic subset;
- full replacement of external circuit compiler and zkVM workflows for targeted
  general dense workloads;
- cryptographic proof generation for dense control flow;
- semantic equivalence between optimized and unoptimized programs;
- Rootprint v2 executable-node semantics.

These gaps do not change the objective. They define the remaining work needed
before SFCS can honestly be described as having made those traditional paths
unnecessary for the workloads Power House targets. Each gap requires separate
conformance vectors, mutation tests, independent review, and a release gate.

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
6. Public docs must describe current releases as incomplete SFCS milestones
   until dense soundness, replay, equivalence, and source-to-fractal execution
   have been independently validated at the target scope.

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

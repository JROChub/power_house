# Sovereign Fractal Computation Substrate

Status: SFCS implementation contract and experimental release gate.

SFCS is the path for making the Rootprint-fractal graph the native
representation, execution environment, and proof substrate for Power House
computation. The end-state objective is explicit: for the workloads Power
House targets, direct source-to-fractal execution, deterministic structure
discovery, and Sovereign Fast Path routing must make traditional circuit
compilers and zkVM workflows unnecessary and unwise as the default path.

The implementation is guarded by the release gates below. Every expansion must
move computation into the Rootprint-fractal model while preserving guarantees
that cannot be compromised: deterministic replay, `.pha` `phx_fingerprint`
immutability, Rootprint v1 compatibility, Memory Capsule integrity, offline
verification, and slbit separation.

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
- **Current release boundary:** the public SFCS surface is promoted only when
  the implementation, tests, and conformance vectors satisfy the gates in this
  document.

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

The implementation is intentionally isolated behind a feature flag, but it now
covers a reproducible executable SFCS workflow:

- feature flag: `sfcs`
- module: `src/sfcs/mod.rs`
- schema: `power-house/sfcs-fractal/v1-draft`
- graph bridge: `SfcsGraph::to_pha_artifact(...)`
- execution bridge: `SfcsGraph::to_execution_pha_artifact(...)`
- native source bridge: `SfcsGraph::from_source(...)`
- execution protocol: `power-house/sfcs-execution/v1-draft`
- CLI: `julian sfcs source|eval|inspect|verify-pha` when built with
  `--features sfcs`

The feature is not in the default feature set. Enabling it adds draft types for
computational fractal nodes, deterministic graph digestion, strict duplicate-key
JSON parsing, direct textual SFCS program parsing, source-to-fractal expression
lowering, deterministic execution traces, memory-state digests, synthesis
plans, a fast-path workload descriptor, `.pha` embedding verification, and an
offline CLI. It does not change existing public behavior.

The current source frontend accepts `input`, `let`, and `output` statements and
lowers expressions directly into committed fractal nodes. It covers arithmetic,
comparisons, boolean control, bitwise integer operations, deterministic shifts,
branching, and ordered `load(...)` / `store(...)` memory operations. This is the
preferred SFCS direction because developers express computation as source that
becomes the graph itself, not as an external circuit artifact.

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
div half sum c
mod rem sum c
eq same sum c
lt below rem c
bit_xor mask rem c
not changed same
branch selected changed delta sum
mul z sum c
memory_write write_addr c z
memory_read read_addr c write_addr
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
let shifted = doubled << 1
let remainder = shifted % 7
let stored = store(a, remainder)
let loaded = load(a)
let fallback = a + a
let out = if !same then loaded ^ fallback else fallback
output out
```

Supported expression features:

- integer constants;
- identifiers;
- `+`, `-`, `*`, `/`, and `%`;
- `<`, `<=`, `>`, `>=`, `==`, `&&`, `||`, and `!`;
- `&`, `|`, `^`, `<<`, and `>>`;
- parentheses;
- `load(<address>)` and `store(<address>, <value>)`;
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
- dense/general: `div`, `mod`, comparisons, boolean control, bitwise ops,
  shifts, `branch`, `dense_step`, `memory_read`, and `memory_write`.

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

`SfcsGraph::execution_trace(...)` executes the source-to-fractal subset and
emits a digest-bound trace:

- graph digest;
- input digest;
- output digest;
- deterministic memory digest before and after each step;
- per-step node digests;
- final trace digest;
- public outputs.

The draft evaluator supports `input`, `alias`, `const`, `add`, `sub`, `mul`,
`div`, `mod`, comparisons, boolean control, bitwise ops, shifts, `branch`,
`memory_read`, and `memory_write`. Division or remainder by zero rejects
execution. Negative or oversized shift amounts reject execution. Explicit
opaque `dense_step` nodes are rejected until a proof profile defines their
semantics.

Memory is deterministic and local to the trace. `store(address, value)` writes
the value into an ordered integer-addressed memory map. `load(address)` returns
the stored value or `0` when the address has not been written. Source lowering
adds memory dependencies so reads and writes replay in source order, and every
trace step commits the memory state before and after execution.

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

## SFCS CLI

Build with the feature enabled:

```bash
cargo build --features sfcs --bin julian
```

Parse native source directly into a canonical graph:

```bash
julian sfcs source dense.sfcs --output dense.graph.json
```

Execute source locally and emit a replayable report plus an SFCS execution
`.pha` artifact:

```bash
julian sfcs eval dense.sfcs \
  --input addr=7 \
  --input value=29 \
  --report dense.report.json \
  --artifact-output dense.execution.pha \
  --label dense-memory
```

Inspect a graph:

```bash
julian sfcs inspect dense.graph.json
```

Verify the `.pha` embedding:

```bash
julian sfcs verify-pha dense.execution.pha
```

These commands are offline. They do not mutate Rootprint, do not require
network access, and do not alter existing `.pha` fingerprint rules.

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

## Promotion Gates

SFCS can only be promoted as complete for the targeted workload class when
these gates are satisfied:

- source-to-fractal execution covers the supported general dense workload
  profile without requiring an external circuit compiler as the default path;
- structure discovery and rewrite recording are deterministic and
  mutation-tested;
- dense/control proof profiles have explicit soundness statements;
- optimized and unoptimized fractals have deterministic equivalence checks;
- Memory Capsule and slbit bindings verify unchanged after SFCS embedding;
- any executable Rootprint schema is versioned separately from Rootprint v1 or
  carried as `.pha` core payload without mutating Rootprint v1 semantics.

These gates do not change the objective. They define what must be true before
SFCS is publicly described as making traditional paths unnecessary for the
workloads Power House targets.

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
6. Public docs must tie SFCS claims to the exact release gates and workload
   profile that passed.

## Validation

Run the current draft tests with:

```bash
cargo test --features sfcs --test sfcs
cargo test --features sfcs --test sfcs_cli
```

Run legacy gates to prove isolation:

```bash
cargo test --locked
cargo test --features sfcs --locked
```

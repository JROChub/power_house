# Sovereign Fractal Computation Substrate

Release: **v0.3.19**

SFCS is the Power House source-to-fractal execution path. It makes the
Rootprint-fractal graph the native representation, execution record, and proof
routing substrate for Power House computation.

For the workload class Power House targets, the intended default is direct
Power House source -> deterministic fractal graph -> digest-bound execution
trace -> synthesis plan -> `.pha` identity. External zkVM and circuit-compiler
flows are bypassed by the native SFCS path.

## What v0.3.19 Ships

Power House v0.3.19 includes an opt-in SFCS implementation behind
`--features sfcs`:

- module: `src/sfcs/mod.rs`
- feature flag: `sfcs`
- graph schema: `power-house/sfcs-fractal/v1-draft`
- execution schema: `power-house/sfcs-execution/v1-draft`
- source bridge: `SfcsGraph::from_source(...)`
- line-oriented bridge: `SfcsGraph::from_program(...)`
- graph bridge: `SfcsGraph::to_pha_artifact(...)`
- execution bridge: `SfcsGraph::to_execution_pha_artifact(...)`
- embedding verifier: `verify_sfcs_pha_embedding(...)`
- execution verifier: `verify_sfcs_execution_embedding(...)`
- CLI: `julian sfcs source|eval|inspect|verify-pha`

The public surface covers direct source parsing, deterministic graph digestion,
strict duplicate-key JSON parsing, source-to-fractal expression lowering,
deterministic dense integer and memory execution traces, deterministic structure
regions, synthesis plans, Sovereign fast-path workload descriptors, `.pha`
embedding verification, and an offline CLI.

## Architecture

```text
Power House source
        |
        v
SFCS graph
        |
        | deterministic graph digest
        | deterministic execution trace
        | deterministic synthesis plan
        v
.pha execution artifact
        |
        v
Rootprint v1 branch
        |
        v
Memory Capsule replay and challenge checks
        |
        v
Optional slbit semantic packet binding
```

Rootprint v1 branch identity remains calculated from label, parents, and the
carried `.pha` `phx_fingerprint`. SFCS does not mutate Rootprint v1 semantics.
Instead, SFCS computation is committed as ordinary `.pha` core payload and
checked by explicit SFCS verification.

## Native Expression Source

`SfcsGraph::from_source(...)` maps source statements directly into committed
fractal nodes. Assignments create stable graph nodes, and nested expressions
create deterministic intermediate nodes.

Supported statements:

```text
input <id>
let <id> = <expr>
output <id> [id...]
output <expr> as <id>
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

Example:

```text
input addr
input value
let masked = value ^ 255
let stored = store(addr, masked)
let loaded = load(addr)
let doubled = loaded * 2
let out = if doubled > value then doubled else value
output out
```

Repeated source values such as `a + a` are represented by explicit
deterministic `alias` nodes. The graph remains strict about duplicate operation
inputs while still supporting normal source expressions.

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

The evaluator supports `input`, `alias`, `const`, `add`, `sub`, `mul`, `div`,
`mod`, comparisons, boolean control, bitwise operations, shifts, `branch`,
`memory_read`, and `memory_write`.

Division or remainder by zero rejects execution. Negative or oversized shift
amounts reject execution. Explicit opaque `dense_step` nodes are rejected until a
proof profile defines their semantics.

Memory is deterministic and local to the trace. `store(address, value)` writes
the value into an ordered integer-addressed memory map. `load(address)` returns
the stored value or `0` when the address has not been written. Source lowering
adds memory dependencies so reads and writes replay in source order, and every
trace step commits the memory state before and after execution.

## Structure Discovery And Synthesis

The discovery engine classifies fast-path eligible nodes and dense/general
nodes, then groups same-kind connected nodes into deterministic structure
regions. Each region records a stable ID, region kind, topologically ordered
nodes, entry nodes, output nodes, source graph digest, and domain-separated
region digest.

`SfcsGraph::synthesis_plan(...)` records where the graph can be routed to the
Sovereign fast path and where dense/general boundaries remain. Each operation
has its own operation digest and is bound to the exact structure-region digest
that produced it. The plan also emits an embedding invariant digest that binds
the graph digest to the synthesis digest.

No hidden rewrite or optimization is accepted. Rewrites must become explicit,
digest-bound data.

## CLI

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

These commands are offline. They do not mutate Rootprint, do not require network
access, and do not alter existing `.pha` fingerprint rules.

## `.pha` Embedding Verification

`SfcsGraph::to_pha_artifact(...)` commits the graph into a normal `.pha`
artifact. `verify_sfcs_pha_embedding(...)` checks the additional SFCS binding:

- the `.pha` core fingerprint is valid;
- the embedded protocol is `power-house/sfcs/v1-draft`;
- the embedded proof payload decodes as a valid SFCS graph;
- the graph digest matches `provenance.fractal_digest`;
- public node counters match deterministic structure discovery.

`SfcsGraph::to_execution_pha_artifact(...)` commits graph, execution trace, and
synthesis plan into a normal `.pha` artifact. `verify_sfcs_execution_embedding`
then replays the graph from public inputs, regenerates the synthesis plan,
checks public outputs, verifies public region counters, and verifies provenance
digests.

## Validation

Run the SFCS tests with:

```bash
cargo test --features sfcs --test sfcs
cargo test --features sfcs --test sfcs_cli
```

Run legacy gates to prove isolation:

```bash
cargo test --locked
cargo test --features sfcs --locked
```

Run the release consistency gate before tagging:

```bash
python3 scripts/check_release_consistency.py --expected-tag v0.3.19
```

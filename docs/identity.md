# Identity Layer

Status: normative for Power House v0.3.14.

The identity layer is an immutable abstraction over existing `.pha` and
Rootprint primitives. It does not replace Rootprint and does not require the
`net` feature.

## Rust API

```rust
use power_house::{identity::Identity, provenance::PhaArtifact};
use serde_json::json;

let artifact = PhaArtifact::new(
    json!({"producer": "example"}),
    "power-house/example/v1",
    json!({"claim": 7}),
    json!({"accepted": true}),
)?;
let (identity, graph) = Identity::create("main", artifact)?;
identity.verify(&graph)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`Identity` stores a `PhaArtifact` and `RootprintId`. Its fields are private and
exposed through `pha()` and `rootprint_id()` so a created identity cannot be
altered in place. `fork` and `merge` return new identities.

| Identity API | Rootprint primitive |
| --- | --- |
| `Identity::create` | `Rootprint::new` |
| `Identity::fork` | `Rootprint::fork` |
| `Identity::merge` | `Rootprint::merge` |
| `Identity::verify` | artifact verification, graph verification, and node resolution |
| `Identity::replay` | `Rootprint::replay` |
| `Identity::equivalent` | `Rootprint::equivalent` |

## JULIAN CLI

```bash
julian identity create main.pha \
  --label main \
  --identity-output main.identity.json \
  --rootprint-output identity.rootprint.json \
  --artifact-output main-bound.pha

julian identity fork main.identity.json identity.rootprint.json candidate.pha \
  --label candidate \
  --identity-output candidate.identity.json

julian identity merge candidate.identity.json audit.identity.json \
  identity.rootprint.json accepted.pha \
  --label accepted \
  --identity-output accepted.identity.json

julian identity verify accepted.identity.json identity.rootprint.json
julian identity replay accepted.identity.json identity.rootprint.json \
  --output accepted.replay.json
julian identity equivalent candidate.identity.json audit.identity.json \
  identity.rootprint.json
```

CLI operations read existing identities and emit new identity files. They never
rewrite a source identity.

## Identity-Aware `.pha`

The v1 schema gains one optional top-level field:

```json
{
  "schema": "power-house/pha/v1",
  "provenance": {},
  "embedded_proof": {
    "protocol": "power-house/example/v1",
    "public_inputs": {},
    "proof": {}
  },
  "identity_root": "sha256:<64 lowercase hex characters>",
  "phx_fingerprint": "sha256:<64 lowercase hex characters>"
}
```

The Rust declaration is:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub identity_root: Option<RootprintId>
```

Legacy artifacts omit the field and retain exactly the same fingerprint.
`identity_root` is intentionally outside the established v1 fingerprint. A
Rootprint branch ID already commits to the artifact fingerprint, so including
the branch ID in that same fingerprint would create an unsatisfiable circular
hash. Identity verification binds the two values in graph context instead.

## Verification Workflow

`Identity::verify` performs all of the following without network access:

1. validates the `.pha` schema and core fingerprint;
2. verifies the complete Rootprint DAG;
3. requires the artifact `identity_root` to equal the envelope `rootprint_id`;
4. resolves that exact node in the graph;
5. requires the graph node to bind back to the same identifier;
6. requires the graph node and identity artifact to share the same core
   fingerprint.

Missing, malformed, unresolved, or mismatched identity roots fail
verification.

## Replay Workflow

Rootprint replay verifies the graph, derives canonical depth from parent links,
sorts nodes by `(canonical_depth, branch_id)`, and reconstructs a state
containing:

- root branch;
- ordered branch projections;
- sorted graph tips;
- a domain-separated SHA-256 state fingerprint.

The projection contains only graph structure and `.pha` core fingerprints.
External proof attachments and `identity_root` transport pointers cannot alter
the replay result. Rust and Python produce byte-compatible JSON values and the
same replay fingerprint.

## Python API

```python
from power_house import (
    create_identity,
    equivalent_identity,
    fork_identity,
    merge_identity,
    replay_identity,
    verify_identity,
)
```

The shared vectors in `conformance/identity-v1` define the required identity,
Rootprint, and replay outputs for both SDKs.

## Network Isolation

Identity, `.pha`, Rootprint, replay, merge, and equivalence compile with default
features and remain fully available offline. Networking, peer discovery,
distributed synchronization, and quorum reconciliation remain gated behind
`--features net`. Network state is not an input to any local verification
result.

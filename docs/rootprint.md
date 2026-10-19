# Rootprint v1

Status: normative for Power House v0.4.1.

For repeated inspections, `Rootprint::verified()` returns an immutable borrowed
view containing the verified graph and canonical replay state. Use
`view.state()` for replay identity and `sidecar.verify_verified(&view)` for
multiple Observatory packets. The graph cannot be mutated while the view is
used, and the view has no public unchecked constructor or deserializer.
Artifact and graph checks occur when the view is created; sidecar checks still
validate every binding, branch reference and sidecar digest. External proof
soundness remains a separate verification boundary.

```rust
use power_house::provenance::{PhaArtifact, Rootprint};
use power_house::ObservatorySidecar;
use serde_json::json;
use std::collections::BTreeMap;
let artifact = PhaArtifact::new(json!({}), "example/v1", json!({}), json!({}))?;
let graph = Rootprint::new("root", artifact)?;
let sidecar = ObservatorySidecar::new(&graph, BTreeMap::new())?;
let verified = graph.verified()?;
sidecar.verify_verified(&verified)?;
println!("{}", verified.state().state_fingerprint);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Rootprint is the primary Power House provenance workflow. It is a deterministic
directed acyclic graph whose nodes carry `.pha` artifacts and whose edges record
fork and merge ancestry.

## Core rule

Rootprint identity and validity depend only on Power House data:

- parent branch IDs
- branch label
- the artifact `phx_fingerprint`

External proof attachments are never inputs to branch IDs, graph verification,
navigation, equivalence, forking, or merging. A branch can carry EPA because
its `.pha` artifact can carry EPA, but Rootprint does not require or interpret
it.

Identity-aware `identity_root` pointers are also excluded from branch IDs and
logical replay. They are resolved by the identity layer.

Observatory sidecars and `slbit` visualization packets are also excluded from
Rootprint documents and every core identity calculation. A sidecar may bind
semantic packets to exact branch IDs and a replay fingerprint after the graph
has been verified.

## Document model

A Rootprint document contains:

| Field | Meaning |
| --- | --- |
| `schema` | Must equal `power-house/rootprint/v1`. |
| `root_branch` | Deterministic ID of the unique root branch. |
| `branches` | Object keyed by deterministic branch ID. |

Each branch contains:

| Field | Meaning |
| --- | --- |
| `id` | Domain-separated deterministic branch ID. |
| `label` | Human-readable selector, limited to 128 characters. |
| `sequence` | Parent-before-child ordering value. |
| `parents` | Zero IDs for root, one for fork, two sorted IDs for merge. |
| `artifact` | A core-valid `.pha` artifact. |

## Rust interface

```rust
use power_house::{prove_with_rootprint, provenance::PhaArtifact};
use serde_json::json;

let main_artifact = PhaArtifact::new(
    json!({"source": "experiment-7"}),
    "power-house/sumcheck/v1",
    json!({"claim": 42}),
    json!({"rounds": []}),
)?;

let mut rootprint =
    prove_with_rootprint!(label: "main", artifact: main_artifact)?;

let branch_artifact = PhaArtifact::new(
    json!({"source": "experiment-7"}),
    "power-house/sumcheck/v1",
    json!({"claim": 43}),
    json!({"rounds": []}),
)?;

let branch_id = prove_with_rootprint!(
    rootprint: &mut rootprint,
    fork: "main",
    label: "candidate",
    artifact: branch_artifact,
)?;

rootprint.verify()?;
# let _ = branch_id;
# Ok::<(), Box<dyn std::error::Error>>(())
```

The macro also supports merges:

```rust,ignore
prove_with_rootprint!(
    rootprint: &mut rootprint,
    merge: [left_id, right_id],
    label: "accepted",
    artifact: merged_artifact,
)?;
```

`Rootprint::replay()` reconstructs canonical logical state. The module-level
`merge(left, right)` deterministically unions valid graphs with the same root,
and `equivalent(left, right)` compares replay outcomes.

## CLI workflow

```bash
julian rootprint init main.pha \
  --label main \
  --output experiment.rootprint.json

julian rootprint fork experiment.rootprint.json main candidate.pha \
  --label candidate

julian rootprint merge experiment.rootprint.json candidate audit audit.pha \
  --label accepted

julian rootprint navigate experiment.rootprint.json accepted
julian rootprint equivalent experiment.rootprint.json candidate audit
julian rootprint verify experiment.rootprint.json
```

Selectors resolve exact branch IDs, unique ID prefixes, or unique labels.
`rootprint verify` is always a Power House core operation.

`julian attach-external-proof` is a separate optional operation. It may add EPA
transport data to a `.pha` artifact without changing its `phx_fingerprint`.

`julian observatory verify` is another separate optional operation. It verifies
the Rootprint first, then checks a semantic sidecar without interpreting packet
meaning or changing core validity.

## Deterministic branch ID

Every branch ID is domain-separated SHA-256 over canonical JSON with
lexicographically sorted object keys:

```text
sha256(
  "power-house:rootprint:v1:branch-id\0" ||
  compact_json({
    "artifact_phx_fingerprint": artifact.phx_fingerprint,
    "label": label,
    "parents": sorted_parent_ids
  })
)
```

The root has no parents, forks have one parent, and merges have two sorted,
unique parents. Sequence numbers prove parent-before-child ordering but are not
part of branch identity.

## Optional EPA

EPA integrity can be checked explicitly through
`Rootprint::verify_external_proof_attachments()`. It is not called by
`Rootprint::verify()` and is not part of the standard CLI workflow.

## Verification invariants

Core verification requires:

1. the Rootprint schema is supported;
2. the root exists, has sequence `0`, and has no parents;
3. every branch map key equals its stored branch ID;
4. every carried `.pha` artifact passes core verification;
5. every branch ID recalculates exactly;
6. parent lists are sorted, unique, and contain at most two IDs;
7. every parent has a lower sequence than its child;
8. every branch is reachable from the root.

Replay derives canonical depth from parent links, so older valid graphs with
larger monotonic sequence gaps reconstruct the same logical state.

Canonical vectors are published under `conformance/pha-v1` and
`conformance/identity-v1`. Optional semantic binding vectors are published
under `conformance/slbit-v1`.

Security assumptions and mutation behavior are defined in the
[Provenance Security Model](provenance_security.md).

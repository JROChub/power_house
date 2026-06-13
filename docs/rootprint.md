# Rootprint v1

Status: normative for Power House v0.3.4.

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

Canonical vectors are published under `conformance/pha-v1`.

Security assumptions and mutation behavior are defined in the
[Provenance Security Model](provenance_security.md).

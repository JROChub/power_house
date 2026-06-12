# Rootprint v1

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

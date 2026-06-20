# SDKs

Power House v0.3.12 ships matching Rust and zero-dependency Python interfaces
for identities, `.pha` v1, Rootprint v1, and deterministic replay.

## Rust

```rust
use power_house::{prove_with_rootprint, provenance::PhaArtifact};
use serde_json::json;

let artifact = PhaArtifact::new(
    json!({"source": "rust"}),
    "power-house/example/v1",
    json!({"claim": 7}),
    json!({"accepted": true}),
)?;
let graph = prove_with_rootprint!(label: "main", artifact: artifact)?;
graph.verify()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Python

The Python package is distributed in this repository and in the crate source
archive. Install it from a checkout or extracted crate:

```bash
python3 -m pip install ./sdk/python
```

```python
from power_house import create_artifact, new_rootprint, verify_rootprint

artifact = create_artifact(
    {"source": "python"},
    "power-house/example/v1",
    {"claim": 7},
    {"accepted": True},
)
graph = new_rootprint("main", artifact)
verify_rootprint(graph)
```

The default Python namespace contains no EPA functions. Optional attachment
transport requires an explicit import from `power_house.external`.

Both implementations consume the vectors in `conformance/pha-v1`.
Identity and replay outputs are cross-checked through
`conformance/identity-v1`.

## Core API mapping

| Operation | Rust | Python |
| --- | --- | --- |
| Create `.pha` | `PhaArtifact::new` | `create_artifact` |
| Verify `.pha` core | `PhaArtifact::verify` | `verify_artifact` |
| Create Rootprint | `Rootprint::new` or `prove_with_rootprint!` | `new_rootprint` |
| Navigate | `Rootprint::navigate` | `navigate` |
| Fork | `Rootprint::fork` | `fork` |
| Merge | `Rootprint::merge` | `merge` |
| Compare core identity | `Rootprint::equivalent` | `equivalent` |
| Verify graph | `Rootprint::verify` | `verify_rootprint` |
| Replay graph | `Rootprint::replay` | `replay_rootprint` |
| Merge graphs | `provenance::merge_rootprints` | `merge_rootprints` |
| Create identity | `Identity::create` | `create_identity` |
| Fork identity | `Identity::fork` | `fork_identity` |
| Merge identity | `Identity::merge` | `merge_identity` |
| Verify identity | `Identity::verify` | `verify_identity` |
| Replay identity | `Identity::replay` | `replay_identity` |
| Compare identities | `Identity::equivalent` | `equivalent_identity` |

## Conformance

```bash
cargo run --example pha_conformance_vectors
cargo run --example identity_conformance_vectors
PYTHONPATH=sdk/python python3 -m unittest discover -s sdk/python/tests -v
```

The suite verifies matching fingerprints, branch IDs, replay state
fingerprints, and identity outcomes; rejects core and binding mutations; and
confirms that EPA mutation remains outside core validity.

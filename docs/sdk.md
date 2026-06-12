# SDKs

Power House v0.3.0 ships matching Rust and zero-dependency Python interfaces
for `.pha` and Rootprint v1.

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

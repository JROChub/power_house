# Power House Python SDK

The default `power_house` namespace implements `.pha` v1 and Rootprint v1
without requiring or interpreting external proof attachments.

```python
from power_house import create_artifact, new_rootprint, verify_rootprint

artifact = create_artifact(
    provenance={"source": "python"},
    protocol="power-house/example/v1",
    public_inputs={"claim": 7},
    proof={"accepted": True},
)
graph = new_rootprint("main", artifact)
verify_rootprint(graph)
```

EPA transport helpers are deliberately separated:

```python
from power_house.external import attach_external_proof, verify_external_attachments
```

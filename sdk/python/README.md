# Power House Python SDK

Version: 0.3.13

The default `power_house` namespace implements immutable identity operations,
`.pha` v1, and Rootprint v1 without requiring network access or interpreting
external proof attachments.

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

Identity operations use the same canonical format and replay digest as Rust:

```python
from power_house import create_identity, replay_identity, verify_identity

identity, graph = create_identity(artifact, "main")
verify_identity(identity, graph)
state = replay_identity(identity, graph)
```

EPA transport helpers are deliberately separated:

```python
from power_house.external import attach_external_proof, verify_external_attachments
```

Memory Capsules can be loaded and checked offline. The Python SDK verifies
capsule/core digests, `.pha`, Rootprint replay, and semantic packet transport
bindings; unsupported proof profiles are reported instead of silently accepted
as fully proven:

```python
from power_house import load_memory_capsule, verify_memory_capsule

capsule = load_memory_capsule("earth-001.phm")
report = verify_memory_capsule(capsule, policy="strict")
print(report.core_valid, report.replay_valid)
```

Run the shared conformance suite from the repository root:

```bash
PYTHONPATH=sdk/python python3 -m unittest discover -s sdk/python/tests -v
```

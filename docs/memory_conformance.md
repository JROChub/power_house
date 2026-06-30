# Memory Conformance

Status: active conformance guide for Power House v0.3.16.

The Memory Capsule conformance surface is currently covered by:

```bash
cargo test --test memory_capsule --locked
cargo test --test memory_cli --locked
```

The tests verify:

- strict JSON duplicate-key rejection,
- floating-point rejection,
- valid capsule verification,
- deterministic replay,
- semantic packet binding,
- sidecar verification,
- semantic mutation rejection with core validity preserved,
- CLI create, verify, replay, challenge, and export.

Invalid artifacts must fail with stable rejection traces rather than ambiguous
errors.

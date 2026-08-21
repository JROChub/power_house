# Execution-contract v1 conformance vectors

These vectors accompany
[`docs/EXECUTION_CONTRACT_V1.md`](../../docs/EXECUTION_CONTRACT_V1.md) and
[`docs/CANONICAL_FORMATS_V1.md`](../../docs/CANONICAL_FORMATS_V1.md).

`vectors.json` has two layers:

- `standalone_tensor` is small and self-contained. It is sufficient to test payload byte
  order, chunk domain separation, manifest-root binary encoding, and exact manifest JSON
  bytes without running the MFENX executor.
- `frozen_release` points into the sealed v2 evidence tree. It tests typed program/image/
  certificate/result identities, checkpoint-piece identity, and complete replay against a
  real accepted execution.

Paths inside `frozen_release` are relative to
`evidence/mfenx-software-supercomputer-v2-20260821-a1/`. Verify that evidence root's own
`SHA256SUMS` before using it.

## Required behavior

A conforming implementation must:

1. verify this directory's `SHA256SUMS`;
2. derive every `positive_conformance` expected value without trusting the stored digest;
3. accept the exact standalone manifest string only when written with no extra byte;
4. validate the frozen image/result through typed compact serialization, not by hashing
   their pretty source files;
5. perform a fresh exact replay for `P-006`; and
6. reject every `negative_conformance` transformation for the stated reason class.

The mutation descriptions are deterministic transformations, not pre-mutated files. A
harness should copy inputs to a private temporary directory, apply exactly one mutation,
and retain both stdout/stderr and exit status. It must never alter the frozen evidence
tree.

Passing these vectors demonstrates format and contract compatibility. It is not a
security audit, scaling benchmark, or reproducible-build result.

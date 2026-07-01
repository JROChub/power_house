# Externally Committed Sparse Workload

Status: active format guide for Power House v0.3.22.

Power-House can now bind a sum-check certificate to a separately supplied
sparse polynomial file.

The workflow uses two independent artifacts:

- `PHSMv1`: canonical external sparse multilinear polynomial
- `PHCPv1`: sum-check certificate containing the polynomial commitment

The certificate does not contain the polynomial terms. Verification requires
both files and rejects any change to either artifact.

## Reproduce

Generate the external workload:

```bash
cargo run --release --example committed_workload -- generate
```

Produce a commitment-bound proof in a separate command:

```bash
cargo run --release --example committed_workload -- prove
```

Verify from the two stored files:

```bash
cargo run --release --example committed_workload -- verify
```

Independently replay the files using Python:

```bash
python3 scripts/verify_sparse_certificate.py \
  target/external_interaction_model.phcp \
  --polynomial target/external_interaction_model.phsm
```

## Reference Artifact

```text
domain variables:       1,000,000
sparse terms:           8,192
maximum degree:         12
term incidences:        57,546
workload bytes:         591,464
certificate bytes:      16,000,128
final evaluation:       802,396,925

workload commitment:
33bfa6068acdd615c9eb5e2990f0aaed5928f6be260d9b8571268292f4f8dc2c

transcript digest:
7008e3fd94878b34fab60fec7446433d69aa74ec34473bfdac9e52029f7f921e

PHSMv1 SHA-256:
c8376831f47a50a7423be6412776382bc23618b037e9fdd163594d389d68864d

PHCPv1 SHA-256:
82045e6eb851991e08d9c4cd782abff3bb06cb8ec5f149e7c2d4287113e6a54a
```

Reference performance on the manifest hardware:

```text
Rust prove:   1,606.105 ms
Rust verify:  1,588.329 ms
Python verify: 11.42 seconds
```

## Security Boundary

This release adds binding to external public data through a domain-separated
BLAKE2b-256 commitment. It protects against substituting a different workload
after the certificate is produced.

It is not yet a succinct polynomial commitment:

- the verifier reads the entire sparse workload,
- the workload is public,
- there is no hiding property,
- no opening proof binds a secret witness,
- verifier work remains `O(n + I log n)`.

The next cryptographic milestone is replacing full workload replay with a
proven multilinear polynomial commitment and opening proof.

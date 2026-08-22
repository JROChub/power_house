# MFENX Local v2 Step 2 addendum

This directory is a separate Step 2 release addendum. It does not change the
frozen Step 1 release at
`release/mfenx-local-v2-20260821-a1/`.

Release status is fail-closed: this addendum is valid only when
`ADDENDUM-MANIFEST.canonical.json` exists, its detached signature verifies in
the `mfenx-step2-addendum` namespace under the pinned project key, and
`verify-addendum.sh` passes. Missing, altered, or unsigned files are not a
release.

The detached signature binds:

- the existing signed Step 1 manifest and signature;
- the unchanged accepted `mfenx-local` executable;
- the sealed Step 1 evidence inventory and accepted output root;
- the standalone contract-v1 verifier source, binary, build record, audit, and
  retained W0 report;
- a revision-distinct distribution archive and its validation record;
- the checksum-closed same-host reproducible-build capture;
- the sealed v1/v2 records supporting the single-run same-workload wall-time
  comparison;
- source-only scaling and adversarial plan harnesses; and
- an addendum-scoped CycloneDX SBOM and in-toto/SLSA-shaped provenance
  statement.

The addendum does not convert plans into results. The scaling design contains
100 planned attempts and zero executed attempts. The adversarial design contains
214 enumerated cases and zero executed cases. Neither source harness currently
implements the evidence parsers required to establish a result.

The standalone verifier is scoped to final image, result, and tensor-store
verification, including a fresh exact arithmetic replay. It does not attest the
historical checkpoint or kill event that produced a result, and it is not an
independently authored or separately governed implementation.

The reproducible-build record establishes only byte-identical remapped builds
in independent directories on one physical host. It does not establish
cross-host or cross-distribution reproducibility.

The sealed performance observation is also narrow: on the same accepted
workload and output root, one fresh v2 run took 5.608764486 seconds externally
and one v1 run took 276.103268901 seconds, for a ratio of 49.2271104608x rounded
to 10 decimal places. This is a single-run cross-release comparison, not a
statistical distribution, lane-scaling result, or universal speedup claim.

## Contents

- `CLAIM_LEDGER.md` defines the exact signed claims and their limits.
- `ADDENDUM-MANIFEST.schema.json` defines the final manifest shape.
- `FINALIZATION-INPUT.template.json` routes every artifact that must be frozen.
- `generate-addendum-manifest.sh` creates the final bytes deterministically
  under the MFENX jq-cS integer profile and refuses placeholders or mismatches.
- `verify-addendum.sh` verifies the signature, the frozen Step 1 chain, every
  addendum artifact, claim counters, SBOM, and provenance.
- `SIGNING.md` defines the separate signing domain and release gates.
- `SIGNING-AUDIT.md` records the exact gate and claim review used to authorize
  the detached signature.
- `SUPPLY_CHAIN_PLAN.md` defines SBOM and provenance generation and validation.

## Finalization and verification

1. Freeze and independently audit the final standalone verifier identity.
2. Freeze and validate the revisioned distribution archive.
3. Generate and validate the Step 2 SBOM and provenance according to
   `SUPPLY_CHAIN_PLAN.md`.
4. Copy the template to `FINALIZATION-INPUT.canonical.json`, replace every
   `PENDING_*` value, set `draft` to `false`, refresh every size and SHA-256,
   and serialize it with `jq -cS .` plus one LF.
5. Run `bash generate-addendum-manifest.sh`, review the exact manifest, and
   run `bash verify-addendum.sh --unsigned-candidate`. This validates all
   content while explicitly leaving authenticity unestablished.
6. After every gate in `SIGNING.md` passes, sign only those reviewed bytes
   under namespace `mfenx-step2-addendum` and run `bash verify-addendum.sh`.

The release-key fingerprint must be pinned through an independent trusted
channel before relying on either the Step 1 or Step 2 signature:

```text
SHA256:Uhj/Ci2+3KA2JN/H8+Sl6nhAiTeD76zvajqvxLOYTTc
```

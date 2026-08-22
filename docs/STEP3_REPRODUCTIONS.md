# Step 3 hosted-VM reproduction protocol

## Status

The workflow and evidence policy are prepared for manual dispatch.
`packaging/step3-candidate-identity.json` is enabled and pins the complete
signed validation-candidate tuple. The four exact assets are deployed at their
checked-in `mfenx.com` URLs; fresh downloads matched every pinned SHA-256 and
passed the namespace-restricted SSH signature check.

No three-host reproduction claim exists yet.

## Frozen identity boundary

The workflow has no caller-supplied release URLs. It reads one checked-in JSON
identity containing the release ID, exact HTTPS asset URLs, archive layout,
SHA-256 digests, signing principal and namespace, key fingerprint, executor and
verifier digests, and canonical output root. The validator rejects partial
placeholder replacement, non-`mfenx.com` URLs, unsafe names or paths, malformed
digests, the wrong signature namespace, and an identity whose gate is disabled.

The installed identity was populated from the signed candidate release record,
not from an older release. It binds:

- the candidate archive and its SHA-256;
- the signed canonical manifest and its SHA-256;
- the detached SSH signature and its SHA-256;
- the namespace-restricted `allowed_signers` policy, SHA-256, and trusted key
  fingerprint;
- the archive name, root, manifest artifact path, and complete source subpath;
- the executor and standalone verifier SHA-256 values; and
- the canonical workload output root.

The signature namespace is fixed to `mfenx-validation-candidate` and the
principal is fixed to `mfenx-release`.

The stable asset mapping is:

```text
https://mfenx.com/lightsout/candidate/downloads/mfenx-local-v2-validation-candidate-20260822-a1-signed-x86_64-unknown-linux-gnu.tar.zst
https://mfenx.com/lightsout/candidate/release/VALIDATION-CANDIDATE-MANIFEST.canonical.json
https://mfenx.com/lightsout/candidate/release/VALIDATION-CANDIDATE-MANIFEST.canonical.json.sig
https://mfenx.com/lightsout/candidate/release/allowed_signers
```

## What each host does

Each of three `ubuntu-24.04` GitHub-hosted matrix jobs:

1. downloads the exact checked-in assets;
2. verifies every pinned hash, the namespace-restricted OpenSSH signature, the
   trusted key fingerprint, and the manifest-to-archive binding;
3. rejects unsafe, duplicate, symlink, or special archive members and runs the
   archive's own checksum verifier;
4. installs the archive into an isolated destination;
5. creates and executes a fresh canonical 4-by-4 wrapping-i32 GEMM;
6. requires executor exact replay and standalone reference-verifier replay;
7. requires the output root to equal the frozen oracle; and
8. checksum-closes and attests the complete success or failure capture.

The workflow resolves jobs through the attempt-scoped REST endpoint:

```text
/repos/{owner}/{repo}/actions/runs/{run_id}/attempts/{attempt_number}/jobs
```

This prevents a re-run from being mixed with jobs from an earlier attempt.

## Claim construction

The provisional aggregate cannot contain a public claim. It first requires:

- exactly `host-1`, `host-2`, and `host-3`;
- three distinct positive runner IDs and Actions job IDs;
- three distinct hashes of ephemeral VM components;
- one repository, commit, run, attempt, candidate identity, and executor;
- three checksum-closed successful captures; and
- three host-archive attestations verified against this exact workflow, source
  commit, repository, and GitHub-hosted runner policy.

The provisional aggregate is then attested and that attestation is verified.
Only after that verification may the policy create a separate claim record.
The claim record is itself attested and verified before the workflow can pass.
Failed and incomplete aggregates are still archived and attested when possible,
but cannot produce a claim record.

## Scope

This is release/workload reproduction on three distinct ephemeral virtual
machine jobs from one GitHub-hosted Ubuntu image family. It is not proof of
three administrative domains, physical-machine identity, hardware attestation,
source-to-binary reproducibility, or performance scaling. Host fingerprints
are reuse checks, not trusted hardware identities.

## Deployment set

The default branch must contain these files before manual dispatch:

```text
.github/workflows/step3-three-host-reproduction.yml
packaging/step3-candidate-identity.json
packaging/step3-candidate-identity.py
packaging/step3-verify-candidate.sh
packaging/step3-hosted-release-run.sh
packaging/step3-reproduction-evidence.py
docs/STEP3_REPRODUCTIONS.md
```

GitHub only accepts `workflow_dispatch` for a workflow present on the default
branch. The repository must permit artifact attestations and the declared OIDC,
attestation, Actions-read, and contents-read permissions.

Merging the workflow and dispatching it are separate operator actions. No claim
may be published until the workflow has retained and verified all three host
outcomes and both aggregate attestations.

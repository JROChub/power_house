# Step 2 addendum signing operations

## Signing gates

Do not sign until all of the following are true:

- the final standalone verifier source-to-binary identity has an independently
  checked build record;
- a frozen W0 verifier report and final verifier audit report exist;
- the revision-distinct distribution archive passes extracted validation;
- the Step 2 SBOM and provenance pass the checks in `SUPPLY_CHAIN_PLAN.md`;
- `FINALIZATION-INPUT.canonical.json` contains no placeholder and every routed
  path, size, and digest matches the repository;
- the generated manifest has been manually reviewed; and
- `verify-addendum.sh --unsigned-candidate` passes while explicitly reporting
  that authenticity remains unestablished; and
- the release owner gives an explicit `SIGN GO`.

No private key may be copied into or referenced by this repository, manifest,
evidence, or command log.

## Domain separation

The addendum uses the same public project-continuity key as Step 1, but a
different signing namespace:

```text
principal:   mfenx-release
namespace:   mfenx-step2-addendum
algorithm:   ssh-ed25519
fingerprint: SHA256:Uhj/Ci2+3KA2JN/H8+Sl6nhAiTeD76zvajqvxLOYTTc
```

`allowed_signers` restricts this bundle to that principal, namespace, and key.
A Step 1 `mfenx-release` signature must not verify as a Step 2 addendum
signature, and a Step 2 signature must not verify in the Step 1 namespace.

## Verification command

After final signing, direct signature verification from this directory is:

```sh
ssh-keygen -Y verify \
  -f allowed_signers \
  -I mfenx-release \
  -n mfenx-step2-addendum \
  -s ADDENDUM-MANIFEST.canonical.json.sig \
  < ADDENDUM-MANIFEST.canonical.json
```

The complete verification command is:

```sh
bash release/mfenx-local-v2-20260821-a1-step2-addendum-a1/verify-addendum.sh
```

## Signing command after all gates and explicit approval

The operator supplies the private-key path out of band. It must never be placed
in a script, manifest, log, shell history example, or repository file. After the
hold is cleared, sign the already reviewed bytes with:

```sh
ssh-keygen -Y sign \
  -f /operator-controlled/path/to/the-approved-key \
  -n mfenx-step2-addendum \
  ADDENDUM-MANIFEST.canonical.json
```

Re-run `bash verify-addendum.sh` immediately. A public key bundled beside a
signature does not establish controller identity; independently pin the
fingerprint.

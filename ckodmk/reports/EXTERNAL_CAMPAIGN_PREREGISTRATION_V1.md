# External hostile-campaign preregistration v1

Schema: `mfenx/ckodmk-external-campaign-preregistration/v1`

This document is the fail-closed entry gate for an evaluator-controlled CKODMK
hostile campaign. It is not evidence that a campaign ran. The evaluator must
author the generator and oracle, choose the distribution, control the hidden
seed, sign the preregistration, execute on evaluator-controlled systems, and
return every selected attempt.

The profile requires at least 1,000 effective-fault targets and 100 valid-control
targets. It retains invalid, unsupported, excluded, crashed, and timed-out
attempts in explicit denominators. Each fault and control group fixes its target,
attempt ceiling, and subtype allocation before execution. Subtypes must sum
exactly to their group target.

The three mandatory baselines are hash-only provenance, ordinary differential
testing, and the strongest applicable existing validator selected by the
evaluator. They must receive the identical selected fault set and the same
content-addressed resource budget. A baseline may finish early, but unused
budget cannot be transferred to another case or tool.

The execution reserve is derived from bounded snapshots and four bounded tool
outputs per selected attempt. This avoids the historical V3 campaign's
unnecessarily pessimistic multiplication of multiple 64 MiB streams. The
preregistration validator checks the arithmetic lower bound; the evaluator may
declare a larger reserve.

## Authentication

Obtain the document SHA-256 independently and verify structure before signing:

```bash
python scripts/verify_external_campaign_preregistration.py \
  preregistration.json \
  --sha256 sha256:<digest>
```

Sign the exact bytes with the evaluator's independently trusted OpenSSH key:

```bash
ssh-keygen -Y sign \
  -f /secure/evaluator_ed25519 \
  -n ckodmk-external-campaign-preregistration \
  preregistration.json
```

MFENX verifies the signature using an evaluator identity, document digest, and
`allowed_signers` file received through separate trusted channels:

```bash
python scripts/verify_external_campaign_preregistration.py \
  preregistration.json \
  --sha256 sha256:<digest> \
  --signature preregistration.json.sig \
  --allowed-signers /secure/ckodmk-evaluators.allowed_signers \
  --evaluator evaluator@example.org
```

Structural success does not establish evaluator identity unless
`authentication_established` is true. Authentication does not establish that
the campaign later followed the plan; the signed return package must bind this
preregistration digest, raw cases, terminal result manifest, seed reveal, and
all outcome denominators.

The golden document at
`specs/golden/external-campaign-preregistration-v1.json` contains placeholders.
It must never be presented as an executed or signed result.

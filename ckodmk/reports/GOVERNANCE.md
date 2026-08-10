# CKODMK governance and release policy

Policy version: 1.0
Applies to: CKODMK 0.2.x research releases and later schema revisions

This file is normative for project maintenance. It does not turn a research
artifact into a production-approved release, authenticate an unsigned bundle,
or replace a customer's own change-control process.

## Authority and separation of duties

Candidate generators are never semantic authorities. A producer may create a
candidate, certificate, report, or optimization, but only the checker named by
the applicable versioned specification can authorize the corresponding
technical verdict. A project maintainer may authorize publication but cannot
upgrade `INCONCLUSIVE`, advisory, observational, or integrity evidence into a
stronger semantic result.

A future production release requires two distinct recorded approvals:

1. a technical approval bound to the complete release digest and its passing
   mandatory gates; and
2. an owner release authorization bound to the same digest.

Neither approval may be inferred from a branch name, HTTP status, successful
upload, mutable tag, or artifact filename. CKODMK 0.2 has no owner signing key
and therefore remains explicitly `NONE_HASH_IDENTITY_ONLY`.

## Version and schema evolution

Schema identifiers contain an explicit terminal version, for example
`mfenx/ckodmk-certificate/v2`. Consumers MUST reject an unknown identifier,
unknown field, duplicate key, or incompatible semantic profile. They MUST NOT
guess, coerce, or silently fall back to an older version.

- Patch releases may fix implementation defects without changing accepted
  syntax or claim meaning. Affected evidence is regenerated.
- A change to syntax, canonicalization, arithmetic, semantics, evidence
  meaning, decision policy, or trust boundary requires a new schema/profile
  identifier and conformance corpus.
- Old and new schemas are separate protocols. An adapter is an untrusted
  transformation and needs its own evidence; it cannot inherit a verdict.
- A supported schema receives security fixes for at least 180 days after its
  successor is tagged. Early retirement requires a published critical-risk
  notice and a fail-closed consumer release.
- No evidence score or qualification status transfers to a new version. The
  rubric is re-evaluated against the new frozen artifact.

## Support policy

The latest tagged 0.2.x release is the supported research line. Support means
reproducible defect triage and security fixes for the declared profiles; it
does not mean universal model coverage, device support, uptime, regulatory
approval, or a zero-defect warranty.

The browser application and private release gate are monitored, but a monitor
result is operational evidence only. A semantic change requires the complete
release gate and new affected evidence. Unsupported models, operators,
devices, or claims return a documented refusal or `INCONCLUSIVE`; they are not
handled by best-effort execution.

## Vulnerability and defect handling

Security reports use the private-advisory channel described in
`SECURITY.md`. Customer or confidential model data must not be attached to a
public issue. Triage targets are:

| Severity | Initial acknowledgement | Containment or decision target |
|---|---:|---:|
| Critical false accept, secret exposure, or release-authentication bypass | 1 business day | 3 business days |
| High false accept, parser ambiguity, or practical resource bypass | 2 business days | 7 business days |
| Medium/low correctness or availability defect | 5 business days | next supported patch or documented rejection |

These are response targets, not guarantees. A critical or high false accept
immediately returns the affected profile and qualification rows to `PENDING`.
Resolution requires a minimal regression, fail-closed behavior, impact review,
new artifact identity, and disclosure appropriate to the affected users.

## Release signing and key policy

Hashes without an authenticated key prove byte identity only. Until the
following controls are implemented and exercised, releases MUST remain marked
unsigned and MUST NOT be called production-authorized:

- an offline owner root key or threshold root with independently stored
  recovery material;
- short-lived CI signing identities bound to the private repository, immutable
  workflow revision, source commit, and release digest;
- consumer verification against an out-of-band trusted root and exact expected
  repository/workflow identity;
- a public or auditor-accessible append-only record of issued and revoked
  release identities;
- documented expiry, emergency revocation, rotation, compromise recovery, and
  dual-signature rollover;
- a rule that revoked, expired, unknown, or ambiguously signed artifacts fail
  closed before semantic verification.

Key material must never be committed, placed in browser assets, copied into a
container build context, or printed in CI logs. Routine signing uses no
long-lived repository secret when a supported workload-identity mechanism is
available. Root rotation requires an overlap period in which old and new roots
sign the same transition statement. Emergency revocation requires an owner
statement identifying the affected key, earliest suspect time, artifact
digests, replacement key, and required consumer action.

Repository-level immutable releases were enabled on 2026-08-10. GitHub applies
that setting only to releases published after enablement: after publication,
their tag and attached assets cannot be moved, replaced, or deleted while the
release exists, and GitHub creates a release attestation. Release publication
therefore uses a draft, uploads and independently verifies every asset, and
publishes only after the asset set is complete. This control does not make an
artifact semantically correct and does not retroactively protect older
releases.

The private repository's current GitHub plan still does not provide
protected-branch or private workflow artifact-attestation enforcement. The
immutable-release attestation is a transport/provenance control, not the
out-of-band owner-root authorization described above. Those remaining
infrastructure limitations cannot be truthfully claimed around; an upgrade or
an independently administered signing/release system is required before G2 or
C5 can receive full production-level credit.

## Dependency, license, and data governance

- Project code is Apache-2.0 unless a file states otherwise.
- Every distributed third-party component must appear in a lockfile and SBOM,
  retain required notices, and pass license review for its deployment model.
- A vulnerability scan is advisory evidence, not proof of safety. Its database
  date, tool version, inputs, failures, and exceptions are retained.
- Model, dataset, calibration, customer, compiler-SDK, and device-runtime terms
  are reviewed independently; an artifact hash is not a license grant.
- Customer artifacts stay in the agreed local, on-premises, or isolated
  environment by default. Publication requires explicit permission and a
  separate privacy review.
- Public browser reports collect no device identity automatically. Any
  evaluator-entered target description remains opt-in and locally generated.

## Change control and evidence retention

Every behavior-affecting change records the affected artifact, semantics,
scope, boundary, evidence kind, threat, tests, and evidence that must be
regenerated. Required CI may not be deleted, bypassed, or converted to
`continue-on-error` without an explicit policy revision and owner review.

Release manifests, raw failures, exclusions, unsupported cases, environment
records, lockfiles, SBOMs, and external return packages are retained for the
period promised to the evaluator or customer and never silently rewritten.
Corrections create a new content identity and preserve the superseded record.

## External evaluation and consequential use

Only a qualified non-author can satisfy rubric gates that require independent
evidence. The evaluator controls its environment, hostile corpus, exclusions,
and unblinding. MFENX may validate the returned package but may not edit its raw
outcomes or signature.

Customer evidence counts only when a qualified external team uses CKODMK for
an actual release decision and records integration effort, false alarms,
interpretation, and retention or payment intent. Demonstrations, downloads,
internal agents, free-form praise, and unsigned summaries do not count.

## Policy amendments

Changing this policy requires a version increment, review of every cited
rubric row, and a record of why the prior policy was insufficient. An amendment
cannot retroactively authenticate an artifact, erase a negative result, or
upgrade evidence collected under weaker rules.

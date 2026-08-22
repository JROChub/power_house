# Step 3 security-review package

## Status and boundary

The automated evidence workflow and public human-review request template are
prepared for manual use. The enabled identity pins the signed validation
candidate and its complete source path. Fresh downloads from every pinned
`mfenx.com` URL matched the checked-in digests and signature policy.

Automated analysis is not an independent code or security review. An
independent result requires an unrelated qualified human, conflict disclosure,
a commit- and digest-bound report, and a reviewer-controlled signature.

## Candidate source boundary

The website repository is not assumed to be the Rust product workspace. The
workflow downloads the exact signed candidate archive, verifies the same hashes,
SSH namespace, key fingerprint, manifest binding, archive paths, and internal
inventory used by the reproduction pipeline, then copies the archive's declared
complete source tree to `candidate-source`.

Before adding any review harness, it records a SHA-256 inventory of every signed
source file. The commit-bound fuzz harness and deny policy are then injected as
review inputs and inventoried separately. Missing `Cargo.toml`, `Cargo.lock`,
critical crates, or the declared source path fails closed.

## Automated evidence

The manual workflow retains unfiltered outcomes for:

- pinned operating-system, Rust, Cargo analyzer, and GitHub CLI prerequisites;
- candidate input and signed-source verification;
- exact Rust, Cargo, audit, deny, fuzz, SSH, kernel, and architecture versions;
- GitHub CodeQL Rust `security-extended` analysis and retained SARIF;
- `cargo audit` against the locked graph;
- the checked-in `cargo deny` advisory, license, source, and ban policy;
- `cargo fmt --check`, strict workspace Clippy, and all workspace tests;
- bounded libFuzzer campaigns for contract control and tensor manifests;
- generated corpus, crash, timeout, and OOM artifacts; and
- a bounded source policy for unsafe-code forbids, verifier dependency
  isolation, verifier process spawning, and workflow Python shell use.

Every wrapped command records arguments, working directory, exit status,
timing, stdout, and stderr. Missing tools, skipped actions, malformed outcomes,
timeouts, missing SARIF, CodeQL results, and fuzz crashes fail the automated
gate. The entire evidence directory is checksum-closed, archived, attested,
and its attestation is verified before the workflow may pass.

The summary always contains:

```json
{
  "independent_human_review": {"status": "not_performed"},
  "independent_code_or_security_review_complete": false
}
```

No workflow input or successful analyzer can change those fields.

## Human review request

`.github/ISSUE_TEMPLATE/independent-security-review.yml` provides a public way
for a prospective reviewer to disclose identity, qualifications, conflicts,
scope, exclusions, and compensation terms. Creating the template does not
engage a reviewer. Opening an issue does not mean work began or completed.

The deliverable must follow `docs/STEP3_HUMAN_REVIEW_BRIEF.md` and validate
against `packaging/review/human-review-report.schema.json`. Active unpublished
vulnerabilities should use the repository's private security-reporting channel,
not a public issue.

## Deployment set

```text
.github/workflows/step3-security-review.yml
.github/ISSUE_TEMPLATE/independent-security-review.yml
packaging/step3-candidate-identity.json
packaging/step3-candidate-identity.py
packaging/step3-verify-candidate.sh
packaging/step3-security-review-evidence.py
packaging/review/deny.toml
packaging/review/fuzz/Cargo.toml
packaging/review/fuzz/Cargo.lock
packaging/review/fuzz/fuzz_targets/contract_control.rs
packaging/review/fuzz/fuzz_targets/tensor_manifest.rs
packaging/review/report-tool-versions.sh
packaging/review/human-review-report.schema.json
docs/STEP3_SECURITY_REVIEW.md
docs/STEP3_HUMAN_REVIEW_BRIEF.md
```

GitHub Code Security/CodeQL and artifact attestations must be available. The
workflow needs the declared `security-events`, OIDC, attestation, and contents
permissions. Repository administrators must resolve any conflict between
CodeQL default setup and this advanced manual-build workflow before dispatch.
Activation and reviewer engagement remain separate external work.

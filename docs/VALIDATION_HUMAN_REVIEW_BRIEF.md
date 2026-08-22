# Independent human code/security review brief

## Assignment

Prospective reviewers may use the public
`.github/ISSUE_TEMPLATE/independent-security-review.yml` form to disclose
qualifications, conflicts, proposed scope, and compensation terms. A proposal
does not mean a review has started or completed.

Review one immutable commit implementing
`mfenx-replay-gated-execution-contract/v1`. The primary question is whether an
untrusted image, result, checkpoint, or private-store object can make the
executor or verifier accept a false exact-i32 result, escape documented
resource bounds, publish partial state as complete, or silently reuse invalid
work after restart.

The reviewer must not be an author or maintainer of the reviewed implementation
and must disclose financial, employment, and close personal conflicts. Payment
must not depend on a passing disposition. Automated reports are inputs, not the
review itself.

## Required scope

At minimum, inspect:

1. canonical JSON projection and integer lexeme enforcement;
2. program/image/result/certificate identity derivation and domain separation;
3. tensor manifest/chunk authentication, range validation, and cache behavior;
4. exact ascending-k wrapping-i32 arithmetic and output comparison;
5. lane partition completeness, overlap, and result counter derivation;
6. checkpoint receipt validation, stale/corrupt reuse, publication ordering,
   temporary-file behavior, directory synchronization, and hostile symlinks;
7. all checked shape, byte-length, offset, allocation, and operation-count
   arithmetic;
8. verifier independence from executor code and process invocation;
9. dependency/advisory/SBOM findings relevant to the deployed binaries; and
10. documented non-goals, especially concurrent hostile writers, sudden power
    loss, telemetry attestation, and whole-process memory bounds.

Reproduce at least the canonical conformance vectors and independently choose
additional malformed inputs. Record every attempted attack and every failed
setup; do not omit negative or inconclusive results.

## Deliverable

Produce JSON conforming to
`packaging/review/human-review-report.schema.json` plus a prose report. Bind both
to the exact commit and evidence digests. Give each finding a stable ID,
severity, byte/path evidence, exploit preconditions, affected claim, and
disposition. Sign the final JSON under a reviewer-controlled key and publish
verification instructions and signer identity.

A `pass_with_findings` disposition does not close open critical/high findings.
Project maintainers may respond, but must not rewrite the independent report.
Publish the original report, maintainer response, fixes, and reviewer retest as
separate immutable artifacts.

## Completion gate

The ledger may mark independent review complete only after:

- reviewer independence is checked and disclosed;
- the report validates against the schema and its signature verifies;
- the reviewed commit equals the release candidate or every later change is
  separately scoped and reviewed;
- all critical/high findings are fixed and retested or remain publicly open;
- successes, failures, limitations, and disputes are published together; and
- the ledger names the immutable report and digests.

No independent reviewer has been engaged or completed this brief in the
current local work. That external coordination is an explicit open blocker.

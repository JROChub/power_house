#!/usr/bin/env python3
"""Static consistency checks for the deployable Validation pipeline package."""

import json
import pathlib
import re
import unittest

import yaml


REPOSITORY = pathlib.Path(__file__).resolve().parents[2]
WORKFLOWS = (
    ".github/workflows/validation-three-host-reproduction.yml",
    ".github/workflows/validation-security-review.yml",
)


class ValidationFoundationAssetTests(unittest.TestCase):
    def test_workflows_parse_and_pin_every_external_action(self):
        for relative in WORKFLOWS:
            with self.subTest(workflow=relative):
                path = REPOSITORY / relative
                document = yaml.safe_load(path.read_text(encoding="utf-8"))
                self.assertIsInstance(document, dict)
                uses = re.findall(r"^\s*uses:\s*([^\s#]+)", path.read_text(), re.MULTILINE)
                self.assertTrue(uses)
                self.assertTrue(
                    all(re.fullmatch(r"[^@\s]+@[0-9a-f]{40}", action) for action in uses), uses
                )

    def test_candidate_identity_is_complete_enabled_and_pinned(self):
        identity = json.loads(
            (REPOSITORY / "packaging/validation-candidate-identity.json").read_text()
        )
        self.assertTrue(identity["enabled"])
        serialized = json.dumps(identity)
        placeholders = re.findall(r"__FINAL_CANDIDATE_[A-Z0-9_]+__", serialized)
        self.assertEqual(placeholders, [])
        self.assertEqual(
            identity["archive"]["sha256"],
            "9485bba9d5bb7a10e911db6070fd081f8f6ade2b8894460eea2729fbe868405f",
        )
        self.assertEqual(
            identity["manifest"]["sha256"],
            "fb4023a172927ba7555376f0217f84c3dd2bcb057ce11d59e7b2697d02ab6229",
        )
        self.assertEqual(
            identity["signature"]["sha256"],
            "f4ccf040df310dea9820e6ddab2c4c7b3ca0db6caabffc38463859b25a106458",
        )
        self.assertEqual(
            identity["archive"]["url"],
            "https://mfenx.com/lightsout/candidate/downloads/"
            "mfenx-local-v2-validation-candidate-20260822-a1-signed-"
            "x86_64-unknown-linux-gnu.tar.zst",
        )

    def test_reproduction_workflow_uses_attempt_scoped_api_and_attested_claim_gate(self):
        workflow = (REPOSITORY / WORKFLOWS[0]).read_text()
        policy = (REPOSITORY / "packaging/validation-reproduction-evidence.py").read_text()
        self.assertIn("slot: [host-1, host-2, host-3]", workflow)
        self.assertIn("/attempts/${GITHUB_RUN_ATTEMPT}/jobs", workflow)
        self.assertIn("--deny-self-hosted-runners", workflow)
        self.assertIn("finalize-claim", workflow)
        self.assertIn("verify_claim_attestation", workflow)
        self.assertIn("unsigned aggregate must not contain a public claim", policy)

    def test_security_workflow_analyzes_recorded_overlay_of_signed_sources(self):
        workflow = (REPOSITORY / WORKFLOWS[1]).read_text()
        self.assertIn("source-root: candidate-review-source", workflow)
        self.assertIn("build-mode: none", workflow)
        self.assertNotIn("build-mode: manual", workflow)
        self.assertNotIn("cargo build --workspace", workflow)
        self.assertIn("validation-verify-candidate.sh", workflow)
        self.assertIn("source-inventory", workflow)
        self.assertIn("validation-review-overlay.py create", workflow)
        self.assertIn("candidate-verifier-source", workflow)
        self.assertNotIn("push:\n", workflow)
        self.assertIn(
            "release_source_mutated",
            (REPOSITORY / "packaging/validation-review-overlay.py").read_text(),
        )
        self.assertIn(
            "independent_code_or_security_review_complete",
            (REPOSITORY / "packaging/validation-security-review-evidence.py").read_text(),
        )

    def test_issue_form_and_review_schema_are_valid(self):
        issue = yaml.safe_load(
            (REPOSITORY / ".github/ISSUE_TEMPLATE/independent-security-review.yml").read_text()
        )
        self.assertEqual(issue["name"], "Independent security review request")
        ids = [item.get("id") for item in issue["body"] if isinstance(item, dict)]
        self.assertIn("conflicts", ids)
        self.assertIn("independence", ids)

        schema = json.loads(
            (REPOSITORY / "packaging/review/human-review-report.schema.json").read_text()
        )
        try:
            import jsonschema
        except ImportError as error:
            self.skipTest(str(error))
        jsonschema.Draft202012Validator.check_schema(schema)
        self.assertIn("evidence_digests", schema["required"])


if __name__ == "__main__":
    unittest.main()

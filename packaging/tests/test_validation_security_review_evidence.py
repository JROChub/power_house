#!/usr/bin/env python3
"""Tests for automated security evidence and the human-review boundary."""

import argparse
import importlib.util
import json
import pathlib
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "validation-security-review-evidence.py"
SPEC = importlib.util.spec_from_file_location("validation_security_review_evidence", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, sort_keys=True) + "\n", encoding="ascii")


class ValidationSecurityReviewEvidenceTests(unittest.TestCase):
    def make_identity(self, root):
        identity = root / "identity.json"
        write_json(
            identity,
            {
                "schema": MODULE.IDENTITY_SCHEMA,
                "enabled": True,
                "release_id": "candidate-a1",
                "archive": {"sha256": "1" * 64, "source_subdir": "source"},
                "signature": {
                    "principal": "mfenx-release",
                    "namespace": "mfenx-validation-candidate",
                },
                "workload": {
                    "executor_sha256": "2" * 64,
                    "verifier_sha256": "3" * 64,
                },
            },
        )
        return identity

    def initialized(self, root):
        evidence = root / "evidence"
        MODULE.init(
            argparse.Namespace(
                identity=self.make_identity(root),
                output=evidence,
                repository="owner/repository",
                commit="a" * 40,
                run_id="800",
                run_attempt="1",
            )
        )
        return evidence

    def populate(self, root, failed_tool=None, sarif_results=None):
        for tool in MODULE.REQUIRED_AUTOMATED:
            write_json(
                root / "outcomes" / f"{tool}.json",
                {
                    "schema": MODULE.SCHEMA,
                    "kind": "automated_tool_outcome",
                    "tool": tool,
                    "status": "failure" if tool == failed_tool else "success",
                    "human_review_performed": False,
                },
            )
        write_json(
            root / "codeql-results" / "rust.sarif",
            {"version": "2.1.0", "runs": [{"results": sarif_results or []}]},
        )

    def make_static_source(self, root):
        for relative in (
            "crates/rarecomp-mfenx-local/src/lib.rs",
            "crates/rarecomp-mfenx-local/src/main.rs",
            "crates/rarecomp-mfenx-tensor-store/src/lib.rs",
            "crates/mfenx-contract-v1-verifier/src/lib.rs",
            "crates/mfenx-contract-v1-verifier/src/main.rs",
        ):
            path = root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("#![forbid(unsafe_code)]\n", encoding="utf-8")
        (root / "crates/mfenx-contract-v1-verifier/Cargo.toml").write_text(
            "[package]\nname='verifier'\nversion='0.0.0'\n[dependencies]\nblake3='1'\nserde='1'\nserde_json='1'\n",
            encoding="utf-8",
        )
        policy = root / "policy/packaging"
        policy.mkdir(parents=True)
        (policy / "validation-policy.py").write_text("print('bounded')\n", encoding="utf-8")
        return root / "policy"

    def test_zero_result_automation_passes_but_never_claims_human_review(self):
        with tempfile.TemporaryDirectory() as temporary:
            evidence = self.initialized(pathlib.Path(temporary))
            self.populate(evidence)
            self.assertEqual(MODULE.finalize(argparse.Namespace(root=evidence)), 0)
            summary = json.loads((evidence / "summary.json").read_text())
            self.assertEqual(summary["automated_evidence"]["status"], "pass")
            self.assertEqual(summary["candidate"]["release_id"], "candidate-a1")
            self.assertEqual(summary["independent_human_review"]["status"], "not_performed")
            self.assertFalse(summary["independent_code_or_security_review_complete"])
            MODULE.check_automated(argparse.Namespace(summary=evidence / "summary.json"))

    def test_failed_tool_and_sarif_result_fail_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            evidence = self.initialized(pathlib.Path(temporary))
            self.populate(evidence, failed_tool="cargo-audit", sarif_results=[{"ruleId": "x"}])
            MODULE.finalize(argparse.Namespace(root=evidence))
            summary = json.loads((evidence / "summary.json").read_text())
            self.assertEqual(summary["automated_evidence"]["status"], "fail")
            self.assertEqual(summary["automated_evidence"]["unwaived_codeql_result_count"], 1)
            with self.assertRaises(MODULE.ReviewError):
                MODULE.check_automated(argparse.Namespace(summary=evidence / "summary.json"))

    def test_source_inventory_binds_files_and_rejects_symlink(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            evidence = self.initialized(root)
            source = root / "source"
            source.mkdir()
            (source / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
            self.assertEqual(
                MODULE.source_inventory(argparse.Namespace(root=evidence, source=source)), 0
            )
            report = json.loads((evidence / "candidate-source-inventory.json").read_text())
            self.assertEqual(report["file_count"], 1)

        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            evidence = self.initialized(root)
            source = root / "source"
            source.mkdir()
            (source / "target").write_text("bytes", encoding="utf-8")
            (source / "link").symlink_to("target")
            self.assertEqual(
                MODULE.source_inventory(argparse.Namespace(root=evidence, source=source)), 1
            )

    def test_bounded_static_policy_separates_candidate_and_workflow_roots(self):
        with tempfile.TemporaryDirectory() as temporary:
            source = pathlib.Path(temporary) / "source"
            source.mkdir()
            policy = self.make_static_source(source)
            evidence = pathlib.Path(temporary) / "evidence"
            (evidence / "outcomes").mkdir(parents=True)
            status = MODULE.static_policy(
                argparse.Namespace(root=evidence, repository=source, policy_repository=policy)
            )
            self.assertEqual(status, 0)
            report = json.loads((evidence / "static-policy.json").read_text())
            self.assertTrue(report["checks"]["reference_verifier_does_not_spawn_executor_or_shell"])
            self.assertTrue(report["passed"])

    def test_external_skipped_step_is_recorded_as_failure(self):
        with tempfile.TemporaryDirectory() as temporary:
            evidence = self.initialized(pathlib.Path(temporary))
            status = MODULE.record_external(
                argparse.Namespace(
                    root=evidence,
                    tool="codeql-init",
                    status="skipped",
                    artifacts=None,
                    note="not available",
                )
            )
            self.assertEqual(status, 1)
            outcome = json.loads((evidence / "outcomes/codeql-init.json").read_text())
            self.assertEqual(outcome["status"], "failure")


if __name__ == "__main__":
    unittest.main()

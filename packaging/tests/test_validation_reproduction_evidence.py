#!/usr/bin/env python3
"""Fail-closed tests for hosted-VM reproduction and attested claim policy."""

import argparse
import importlib.util
import json
import pathlib
import tarfile
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "validation-reproduction-evidence.py"
SPEC = importlib.util.spec_from_file_location("validation_reproduction_evidence", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, sort_keys=True) + "\n", encoding="ascii")


def checksum_close(root):
    (root / "SHA256SUMS").write_text("".join(MODULE.manifest_lines(root)), encoding="ascii")


class ValidationReproductionEvidenceTests(unittest.TestCase):
    def make_identity(self, root, enabled=True):
        identity = root / "identity.json"
        document = {
            "schema": MODULE.IDENTITY_SCHEMA,
            "enabled": enabled,
            "release_id": "candidate-a1" if enabled else "__FINAL_CANDIDATE_RELEASE_ID__",
            "archive": {"sha256": "1" * 64},
            "manifest": {"sha256": "2" * 64},
            "signature": {
                "sha256": "3" * 64,
                "principal": "mfenx-release",
                "namespace": "mfenx-validation-candidate",
            },
            "allowed_signers": {
                "sha256": "4" * 64,
                "key_fingerprint": "SHA256:" + "A" * 43,
            },
            "workload": {
                "executor_sha256": "5" * 64,
                "verifier_sha256": "6" * 64,
                "canonical_output_root": "7" * 64,
            },
        }
        write_json(identity, document)
        return identity

    def make_capture(self, root, identity):
        expected = MODULE.load_expected_identity(identity)
        capture = root / "capture"
        capture.mkdir()
        record = {
            "schema": "mfenx.step3-hosted-release-run.v1",
            "status": "PASS",
            "signed_input": {
                "release_id": expected["release_id"],
                "identity_sha256": expected["identity_sha256"],
                "archive_sha256": expected["archive_sha256"],
                "candidate_manifest_sha256": expected["manifest_sha256"],
                "candidate_signature_sha256": expected["signature_sha256"],
                "allowed_signers_sha256": expected["allowed_signers_sha256"],
                "release_key_fingerprint": expected["key_fingerprint"],
                "candidate_signature_verified": True,
                "archive_inventory_verified": True,
            },
            "execution": {
                "executor_sha256": expected["executor_sha256"],
                "verifier_sha256": expected["verifier_sha256"],
                "output_root": expected["output_root"],
                "executor_exact_replay_accepted": True,
                "standalone_reference_verifier_accepted": True,
            },
            "claim_scope": {
                "release_workload_reproduction": True,
                "reproducible_binary_build": False,
            },
        }
        write_json(capture / "record.json", record)
        (capture / "raw.log").write_text("retained\n", encoding="ascii")
        checksum_close(capture)
        return capture

    def make_archive(self, root, slot, runner_id, job_id, fingerprint, expected):
        artifact = root / f"validation-{slot}"
        artifact.mkdir(parents=True)
        tree = root / f"tree-{slot}" / "host-evidence"
        tree.mkdir(parents=True)
        record = {
            "schema": MODULE.SCHEMA,
            "kind": "host_reproduction",
            "slot": slot,
            "status": "succeeded",
            "failures": [],
            "github": {
                "repository": "owner/repository",
                "commit": "a" * 40,
                "run_id": "700",
                "run_attempt": "1",
                "runner_id": runner_id,
                "actions_job_id": job_id,
            },
            "host": {"fingerprint": fingerprint},
            "capture": {
                "accepted_binary_sha256": expected["executor_sha256"],
                "candidate_release_id": expected["release_id"],
                "candidate_identity_sha256": expected["identity_sha256"],
            },
        }
        write_json(tree / "host-record.json", record)
        checksum_close(tree)
        archive = artifact / f"{slot}.tar.gz"
        with tarfile.open(archive, "w:gz") as stream:
            stream.add(tree, arcname="host-evidence")
        (artifact / f"{slot}.tar.gz.sha256").write_text(
            f"{MODULE.sha256_file(archive)}  {archive.name}\n", encoding="ascii"
        )
        write_json(artifact / f"{slot}.sigstore.json", {"bundle": slot})
        return artifact

    def test_capture_and_host_record_require_exact_closed_success(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            identity = self.make_identity(root)
            expected = MODULE.load_expected_identity(identity)
            capture = self.make_capture(root, identity)
            ok, failures, _ = MODULE.capture_success(capture, expected)
            self.assertTrue(ok, failures)

            jobs = root / "jobs.json"
            write_json(
                jobs,
                {
                    "jobs": [
                        {
                            "name": "Reproduce host-1",
                            "runner_id": 101,
                            "id": 201,
                            "runner_name": "GitHub Actions 101",
                            "labels": ["ubuntu-24.04"],
                        }
                    ]
                },
            )
            output = root / "host-record.json"
            status = MODULE.host_record(
                argparse.Namespace(
                    identity=identity,
                    capture=capture,
                    output=output,
                    slot="host-1",
                    repository="owner/repository",
                    commit="a" * 40,
                    run_id="700",
                    run_attempt="1",
                    job_name="Reproduce host-1",
                    job_api_json=jobs,
                )
            )
            self.assertEqual(status, 0)
            self.assertEqual(json.loads(output.read_text())["status"], "succeeded")

            (capture / "unlisted").write_text("injected", encoding="ascii")
            ok, failures, _ = MODULE.capture_success(capture, expected)
            self.assertFalse(ok)
            self.assertIn("capture checksum inventory is not the exact file set", failures)

    def test_aggregate_is_provisional_until_attestation_is_verified(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            identity = self.make_identity(root)
            expected = MODULE.load_expected_identity(identity)
            inputs = root / "inputs"
            verifications = root / "verifications"
            verifications.mkdir()
            for index, slot in enumerate(MODULE.EXPECTED_SLOTS, 1):
                self.make_archive(inputs, slot, index, 100 + index, f"{index:064x}", expected)
                write_json(
                    verifications / f"{slot}.json",
                    [{"attestation": {"slot": slot}, "verificationResult": {"verified": True}}],
                )
            aggregate = root / "aggregate"
            MODULE.aggregate(
                argparse.Namespace(
                    identity=identity,
                    inputs=inputs,
                    verifications=verifications,
                    output=aggregate,
                    repository="owner/repository",
                    commit="a" * 40,
                    run_id="700",
                    run_attempt="1",
                )
            )
            summary = aggregate / "summary.json"
            document = json.loads(summary.read_text())
            self.assertEqual(document["status"], "host_gate_complete")
            self.assertTrue(document["host_gate_eligible"])
            self.assertFalse(document["claim_eligible"])
            self.assertIsNone(document["claim"])
            MODULE.check_host_gate(argparse.Namespace(summary=summary))

            archive = root / "aggregate.tar.gz"
            with tarfile.open(archive, "w:gz") as stream:
                stream.add(aggregate, arcname="aggregate-evidence")
            archive_digest = root / "aggregate.tar.gz.sha256"
            archive_digest.write_text(
                f"{MODULE.sha256_file(archive)}  {archive.name}\n", encoding="ascii"
            )
            bundle = root / "aggregate.sigstore.json"
            write_json(bundle, {"mediaType": "bundle"})
            verification = root / "aggregate.verification.json"
            write_json(
                verification,
                [{"attestation": {"subject": "aggregate"}, "verificationResult": {"verified": True}}],
            )
            claim = root / "claim.json"
            MODULE.finalize_claim(
                argparse.Namespace(
                    identity=identity,
                    summary=summary,
                    archive=archive,
                    archive_digest=archive_digest,
                    bundle=bundle,
                    verification=verification,
                    output=claim,
                    repository="owner/repository",
                    commit="a" * 40,
                    run_id="700",
                    run_attempt="1",
                )
            )
            MODULE.check_claim(argparse.Namespace(claim=claim))

    def test_aggregate_rejects_reused_host_identity(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            identity = self.make_identity(root)
            expected = MODULE.load_expected_identity(identity)
            inputs = root / "inputs"
            verifications = root / "verifications"
            verifications.mkdir()
            for index, slot in enumerate(MODULE.EXPECTED_SLOTS, 1):
                self.make_archive(inputs, slot, 1, 101, f"{1:064x}", expected)
                write_json(
                    verifications / f"{slot}.json",
                    [{"attestation": {"slot": slot}, "verificationResult": {"verified": True}}],
                )
            output = root / "aggregate"
            MODULE.aggregate(
                argparse.Namespace(
                    identity=identity,
                    inputs=inputs,
                    verifications=verifications,
                    output=output,
                    repository="owner/repository",
                    commit="a" * 40,
                    run_id="700",
                    run_attempt="1",
                )
            )
            summary = json.loads((output / "summary.json").read_text())
            self.assertEqual(summary["status"], "failed")
            with self.assertRaises(MODULE.EvidenceError):
                MODULE.check_host_gate(argparse.Namespace(summary=output / "summary.json"))

    def test_disabled_identity_cannot_enter_evidence_policy(self):
        with tempfile.TemporaryDirectory() as temporary:
            identity = self.make_identity(pathlib.Path(temporary), enabled=False)
            with self.assertRaises(MODULE.EvidenceError):
                MODULE.load_expected_identity(identity)

    def test_seal_and_archive_reject_special_members(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            (root / "file").write_text("bytes", encoding="ascii")
            (root / "link").symlink_to("file")
            with self.assertRaises(MODULE.EvidenceError):
                MODULE.seal_directory(root)

        with tempfile.TemporaryDirectory() as temporary:
            archive = pathlib.Path(temporary) / "host.tar.gz"
            with tarfile.open(archive, "w:gz") as stream:
                member = tarfile.TarInfo("host-evidence/fifo")
                member.type = tarfile.FIFOTYPE
                stream.addfile(member)
            _, failures = MODULE.inspect_host_archive(archive)
            self.assertTrue(any("special member" in failure for failure in failures), failures)


if __name__ == "__main__":
    unittest.main()

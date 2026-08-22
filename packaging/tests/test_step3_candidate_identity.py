#!/usr/bin/env python3
"""Fail-closed tests for the Step 3 candidate identity and signature verifier."""

import argparse
import contextlib
import copy
import hashlib
import importlib.util
import io
import json
import pathlib
import subprocess
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "step3-candidate-identity.py"
SPEC = importlib.util.spec_from_file_location("step3_candidate_identity", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)
TEMPLATE = SCRIPT.with_name("step3-candidate-identity.json")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class Step3CandidateIdentityTests(unittest.TestCase):
    def test_checked_in_identity_is_complete_enabled_and_frozen(self):
        document = MODULE.load_identity(TEMPLATE, require_enabled=True)
        self.assertTrue(document["enabled"])
        self.assertEqual(
            document["archive"]["sha256"],
            "9485bba9d5bb7a10e911db6070fd081f8f6ade2b8894460eea2729fbe868405f",
        )
        self.assertEqual(
            document["workload"]["canonical_output_root"],
            "c4620971a11a6873de7f45f79a93a277fd2cf3f58a89ed4600e6167afed40606",
        )

    def test_placeholder_reintroduced_into_enabled_identity_is_rejected(self):
        document = json.loads(TEMPLATE.read_text())
        document["archive"]["sha256"] = "__FINAL_CANDIDATE_ARCHIVE_SHA256__"
        with self.assertRaises(MODULE.IdentityError):
            MODULE.validate_identity(document, require_enabled=False)

    def test_member_policy_rejects_traversal_and_duplicates(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            identity = json.loads(TEMPLATE.read_text())
            identity["enabled"] = True
            identity["release_id"] = "candidate-a1"
            for section in ("archive", "manifest", "signature", "allowed_signers"):
                name = {
                    "archive": "candidate.tar.zst",
                    "manifest": "MANIFEST.json",
                    "signature": "MANIFEST.json.sig",
                    "allowed_signers": "allowed_signers",
                }[section]
                identity[section]["name"] = name
                identity[section]["url"] = f"https://mfenx.com/releases/a1/{name}"
            identity["archive"].update(
                root="candidate-root",
                source_subdir="source",
                candidate_manifest_path="MANIFEST.json",
                candidate_signature_path="MANIFEST.json.sig",
                allowed_signers_path="policy/allowed_signers",
                executor_path="bin/mfenx-local",
                verifier_path="bin/mfenx-contract-v1-verifier",
                sha256="a" * 64,
            )
            identity["manifest"]["sha256"] = "b" * 64
            identity["signature"]["sha256"] = "c" * 64
            identity["allowed_signers"].update(
                sha256="d" * 64,
                key_fingerprint="SHA256:" + "A" * 43,
            )
            identity["workload"] = {
                "executor_sha256": "e" * 64,
                "verifier_sha256": "f" * 64,
                "canonical_output_root": "0" * 64,
            }
            identity_path = root / "identity.json"
            identity_path.write_text(json.dumps(identity), encoding="ascii")

            members = root / "members.txt"
            members.write_text("candidate-root/\ncandidate-root/file\n", encoding="ascii")
            self.assertEqual(
                MODULE.check_members(argparse.Namespace(identity=identity_path, members=members)), 0
            )
            members.write_text("candidate-root/\ncandidate-root/../escape\n", encoding="ascii")
            with self.assertRaises(MODULE.IdentityError):
                MODULE.check_members(argparse.Namespace(identity=identity_path, members=members))

    def test_signed_manifest_and_namespace_policy_are_verified(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            private_key = root / "release-key"
            subprocess.run(
                ["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", str(private_key)],
                check=True,
            )
            payload = root / "candidate-root"
            public_fields = private_key.with_suffix(".pub").read_text().split()
            allowed = root / "allowed_signers"
            allowed.write_text(
                f'mfenx-release namespaces="mfenx-validation-candidate" {public_fields[0]} {public_fields[1]}\n',
                encoding="ascii",
            )
            fingerprint = subprocess.run(
                ["ssh-keygen", "-lf", str(private_key.with_suffix(".pub"))],
                check=True,
                stdout=subprocess.PIPE,
                text=True,
            ).stdout.split()[1]

            (payload / "bin").mkdir(parents=True)
            (payload / "source").mkdir()
            (payload / "policy").mkdir()
            executor = payload / "bin/mfenx-local"
            verifier = payload / "bin/mfenx-contract-v1-verifier"
            executor.write_text("#!/usr/bin/env bash\nexit 0\n", encoding="ascii")
            verifier.write_text("#!/usr/bin/env bash\nexit 0\n", encoding="ascii")
            cargo_manifest = payload / "source/Cargo.toml"
            cargo_manifest.write_text("[workspace]\n", encoding="ascii")
            embedded_allowed = payload / "policy/allowed_signers"
            embedded_allowed.write_bytes(allowed.read_bytes())
            for path in (executor, verifier):
                path.chmod(0o555)
            for path in (cargo_manifest, embedded_allowed):
                path.chmod(0o444)

            def artifact(path, relative, role, mode):
                return {
                    "media_type": "text/plain",
                    "mode": mode,
                    "path": relative,
                    "role": role,
                    "sha256": digest(path),
                    "size_bytes": path.stat().st_size,
                }

            manifest_document = {
                "schema": "mfenx.validation-candidate-manifest.v1",
                "release_id": "candidate-a1",
                "release_class": "validation_candidate",
                "release_status": "signed_validation_candidate",
                "signature": {
                    "signer_identity": "mfenx-release",
                    "namespace": "mfenx-validation-candidate",
                    "public_key_fingerprint": fingerprint,
                    "signature_present": True,
                },
                "subject": {
                    "executor": {"path": "bin/mfenx-local", "sha256": digest(executor)},
                    "reference_verifier": {
                        "path": "bin/mfenx-contract-v1-verifier",
                        "sha256": digest(verifier),
                    },
                },
                "artifacts": [
                    artifact(executor, "bin/mfenx-local", "candidate_executor_binary", "0555"),
                    artifact(
                        verifier,
                        "bin/mfenx-contract-v1-verifier",
                        "reference_verifier_binary",
                        "0555",
                    ),
                    artifact(cargo_manifest, "source/Cargo.toml", "candidate_source", "0444"),
                    artifact(
                        embedded_allowed,
                        "policy/allowed_signers",
                        "signature_verification_policy",
                        "0444",
                    ),
                ],
            }
            manifest = root / "MANIFEST.json"
            manifest.write_text(
                json.dumps(manifest_document, sort_keys=True, separators=(",", ":")) + "\n",
                encoding="ascii",
            )
            subprocess.run(
                [
                    "ssh-keygen",
                    "-Y",
                    "sign",
                    "-f",
                    str(private_key),
                    "-n",
                    "mfenx-validation-candidate",
                    str(manifest),
                ],
                check=True,
                stdout=subprocess.DEVNULL,
            )
            signature = root / "MANIFEST.json.sig"
            embedded_manifest = payload / "MANIFEST.json"
            embedded_signature = payload / "MANIFEST.json.sig"
            embedded_manifest.write_bytes(manifest.read_bytes())
            embedded_signature.write_bytes(signature.read_bytes())
            embedded_manifest.chmod(0o444)
            embedded_signature.chmod(0o444)

            uncompressed = root / "candidate.tar"
            subprocess.run(
                [
                    "tar",
                    "--sort=name",
                    "--owner=0",
                    "--group=0",
                    "--numeric-owner",
                    "-cf",
                    str(uncompressed),
                    "-C",
                    str(root),
                    payload.name,
                ],
                check=True,
            )
            archive = root / "candidate.tar.zst"
            subprocess.run(
                ["zstd", "-q", "-f", str(uncompressed), "-o", str(archive)], check=True
            )

            identity = json.loads(TEMPLATE.read_text())
            identity.update(enabled=True, release_id="candidate-a1")
            identity["archive"].update(
                url="https://mfenx.com/releases/a1/candidate.tar.zst",
                name=archive.name,
                root="candidate-root",
                sha256=digest(archive),
                source_subdir="source",
                candidate_manifest_path="MANIFEST.json",
                candidate_signature_path="MANIFEST.json.sig",
                allowed_signers_path="policy/allowed_signers",
                executor_path="bin/mfenx-local",
                verifier_path="bin/mfenx-contract-v1-verifier",
            )
            identity["manifest"].update(
                url="https://mfenx.com/releases/a1/MANIFEST.json",
                name=manifest.name,
                sha256=digest(manifest),
            )
            identity["signature"].update(
                url="https://mfenx.com/releases/a1/MANIFEST.json.sig",
                name=signature.name,
                sha256=digest(signature),
            )
            identity["allowed_signers"].update(
                url="https://mfenx.com/releases/a1/allowed_signers",
                sha256=digest(allowed),
                key_fingerprint=fingerprint,
            )
            identity["workload"] = {
                "executor_sha256": digest(executor),
                "verifier_sha256": digest(verifier),
                "canonical_output_root": "3" * 64,
            }
            identity_path = root / "identity.json"
            identity_path.write_text(json.dumps(identity), encoding="ascii")
            args = argparse.Namespace(
                identity=identity_path,
                archive=archive,
                manifest=manifest,
                signature=signature,
                allowed_signers=allowed,
            )
            with contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(MODULE.verify_files(args), 0)

            extract = root / "extracted"
            subprocess.run(
                [
                    "bash",
                    str(SCRIPT.with_name("step3-verify-candidate.sh")),
                    "--identity",
                    str(identity_path),
                    "--archive",
                    str(archive),
                    "--manifest",
                    str(manifest),
                    "--signature",
                    str(signature),
                    "--allowed-signers",
                    str(allowed),
                    "--extract-dir",
                    str(extract),
                ],
                check=True,
                env={"PATH": "/usr/local/bin:/usr/bin:/bin", "TMPDIR": "/var/tmp"},
                stdout=subprocess.DEVNULL,
            )
            self.assertTrue((extract / "candidate-root/source/Cargo.toml").is_file())

            wrong = copy.deepcopy(identity)
            wrong["signature"]["namespace"] = "mfenx-wrong-namespace"
            wrong_path = root / "wrong.json"
            wrong_path.write_text(json.dumps(wrong), encoding="ascii")
            with self.assertRaises(MODULE.IdentityError):
                MODULE.load_identity(wrong_path)


if __name__ == "__main__":
    unittest.main()

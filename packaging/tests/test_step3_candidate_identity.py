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
    def test_checked_in_template_is_disabled_and_cannot_run(self):
        document = MODULE.load_identity(TEMPLATE, require_enabled=False)
        self.assertFalse(document["enabled"])
        with self.assertRaises(MODULE.IdentityError):
            MODULE.load_identity(TEMPLATE, require_enabled=True)

    def test_partial_placeholder_replacement_is_rejected(self):
        document = json.loads(TEMPLATE.read_text())
        document["release_id"] = "candidate-a1"
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
                manifest_path="dist/candidate.tar.zst",
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
            (payload / "source").mkdir(parents=True)
            (payload / "source/Cargo.toml").write_text("[workspace]\n", encoding="ascii")
            verify_script = payload / "verify-archive.sh"
            verify_script.write_text(
                "#!/usr/bin/env bash\nset -Eeuo pipefail\nsha256sum -c SHA256SUMS >/dev/null\n",
                encoding="ascii",
            )
            (payload / "SHA256SUMS").write_text(
                f"{digest(verify_script)}  verify-archive.sh\n"
                f"{digest(payload / 'source/Cargo.toml')}  source/Cargo.toml\n",
                encoding="ascii",
            )
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
            manifest = root / "MANIFEST.json"
            manifest.write_text(
                json.dumps(
                    {
                        "artifacts": [
                            {
                                "path": "dist/candidate.tar.zst",
                                "role": "revisioned_distribution_archive",
                                "sha256": digest(archive),
                            }
                        ]
                    },
                    sort_keys=True,
                    separators=(",", ":"),
                ),
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

            identity = json.loads(TEMPLATE.read_text())
            identity.update(enabled=True, release_id="candidate-a1")
            identity["archive"].update(
                url="https://mfenx.com/releases/a1/candidate.tar.zst",
                name=archive.name,
                root="candidate-root",
                sha256=digest(archive),
                source_subdir="source",
                manifest_path="dist/candidate.tar.zst",
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
                "executor_sha256": "1" * 64,
                "verifier_sha256": "2" * 64,
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

#!/usr/bin/env python3
"""Fail-closed tests for the Step 3 analysis-only workspace overlay."""

import argparse
import hashlib
import importlib.util
import json
import pathlib
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "step3-review-overlay.py"
SPEC = importlib.util.spec_from_file_location("step3_review_overlay", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


def write(path, content):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def tree_digest(root):
    digest = hashlib.sha256()
    for path in sorted(root.rglob("*")):
        if path.is_file():
            digest.update(path.relative_to(root).as_posix().encode("utf-8"))
            digest.update(path.read_bytes())
    return digest.hexdigest()


class Step3ReviewOverlayTests(unittest.TestCase):
    def fixture(self, root):
        candidate = root / "candidate"
        write(
            candidate / "Cargo.toml",
            """[workspace]
members = [
    "crates/not-shipped",
]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2024"
license = "Apache-2.0"
rust-version = "1.85"
""",
        )
        write(candidate / "Cargo.lock", "version = 4\n")
        write(candidate / "LICENSE", "test license\n")
        write(candidate / "rust-toolchain.toml", "[toolchain]\nchannel = 'stable'\n")
        for crate in MODULE.LOCAL_CRATES:
            write(
                candidate / "crates" / crate / "Cargo.toml",
                f'[package]\nname = "{crate}"\nversion = "0.1.0"\n',
            )
            write(candidate / "crates" / crate / "src/lib.rs", "pub fn present() {}\n")

        verifier = root / "verifier"
        write(
            verifier / "Cargo.toml",
            '[package]\nname = "mfenx-contract-v1-verifier"\nversion = "0.1.0"\n',
        )
        write(verifier / "src/lib.rs", "pub fn verify() {}\n")

        harness = root / "review"
        write(harness / "deny.toml", "[advisories]\nversion = 2\n")
        write(harness / "fuzz/Cargo.toml", "[workspace]\n")
        write(harness / "workspace-adapter/Cargo.lock", "version = 4\n")

        identity = root / "identity.json"
        identity.write_text(
            json.dumps(
                {
                    "schema": MODULE.IDENTITY_SCHEMA,
                    "enabled": True,
                    "release_id": "candidate-a1",
                    "archive": {"sha256": "a" * 64},
                },
                sort_keys=True,
            )
            + "\n",
            encoding="ascii",
        )
        return candidate, verifier, harness, identity

    def create_args(self, root, candidate, verifier, harness, identity):
        return argparse.Namespace(
            identity=identity,
            candidate_source=candidate,
            verifier_source=verifier,
            review_harness=harness,
            output=root / "overlay",
            record=root / "evidence/overlay.json",
        )

    def test_overlay_records_every_file_without_mutating_signed_inputs(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            candidate, verifier, harness, identity = self.fixture(root)
            before = (tree_digest(candidate), tree_digest(verifier))
            args = self.create_args(root, candidate, verifier, harness, identity)

            self.assertEqual(MODULE.create(args), 0)
            self.assertEqual(before, (tree_digest(candidate), tree_digest(verifier)))
            record = json.loads(args.record.read_text(encoding="ascii"))
            self.assertFalse(record["release_source_mutated"])
            self.assertFalse(record["may_be_represented_as_release_source"])
            self.assertEqual(
                record["workspace_adapter"]["members"], list(MODULE.WORKSPACE_MEMBERS)
            )
            self.assertEqual(
                record["overlay"]["file_count"], len(record["overlay"]["files"])
            )
            self.assertEqual(
                {item["provenance"] for item in record["overlay"]["files"]},
                {
                    "generated_workspace_adapter",
                    "commit_bound_workspace_adapter",
                    "signed_local_source",
                    "signed_verifier_source",
                    "commit_bound_review_harness",
                },
            )
            self.assertEqual(
                MODULE.check(argparse.Namespace(output=args.output, record=args.record)), 0
            )

    def test_check_rejects_changed_overlay_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            fixture = self.fixture(root)
            args = self.create_args(root, *fixture)
            MODULE.create(args)
            (args.output / "crates/rarecomp-mfenx-ir/src/lib.rs").write_text(
                "pub fn changed() {}\n", encoding="utf-8"
            )
            with self.assertRaises(MODULE.OverlayError):
                MODULE.check(argparse.Namespace(output=args.output, record=args.record))

    def test_create_rejects_unexpected_candidate_crate(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            candidate, verifier, harness, identity = self.fixture(root)
            write(candidate / "crates/unexpected/Cargo.toml", "[package]\nname='x'\n")
            with self.assertRaises(MODULE.OverlayError):
                MODULE.create(
                    self.create_args(root, candidate, verifier, harness, identity)
                )

    def test_create_rejects_symlinked_harness_input(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            candidate, verifier, harness, identity = self.fixture(root)
            (harness / "link").symlink_to("deny.toml")
            with self.assertRaises(MODULE.OverlayError):
                MODULE.create(
                    self.create_args(root, candidate, verifier, harness, identity)
                )


if __name__ == "__main__":
    unittest.main()

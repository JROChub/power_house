#!/usr/bin/env python3
"""Conformance and mutation tests for the Python sparse verifier."""

from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VERIFIER_PATH = ROOT / "scripts" / "verify_sparse_certificate.py"
SPEC = importlib.util.spec_from_file_location("verify_sparse_certificate", VERIFIER_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("could not load sparse verifier")
verifier = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = verifier
SPEC.loader.exec_module(verifier)


class SparseVerifierTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.vector_dir = ROOT / "conformance" / "v1"
        cls.seeded_path = cls.vector_dir / "seeded-valid.phsp"
        cls.polynomial_path = cls.vector_dir / "committed-valid.phsm"
        cls.committed_path = cls.vector_dir / "committed-valid.phcp"
        cls.manifest_path = cls.vector_dir / "manifest.json"
        for path in (cls.seeded_path, cls.polynomial_path, cls.committed_path):
            if not path.exists():
                raise RuntimeError(f"missing conformance vector: {path}")

        cls.seeded = cls.seeded_path.read_bytes()
        cls.polynomial = cls.polynomial_path.read_bytes()
        cls.committed = cls.committed_path.read_bytes()
        cls.manifest = json.loads(cls.manifest_path.read_text(encoding="utf-8"))

    def assert_rejected(self, operation) -> None:
        with self.assertRaises(verifier.CertificateError):
            operation()

    def test_conformance_vectors_verify(self) -> None:
        seeded_report = verifier.verify_seeded(self.seeded)
        committed_report = verifier.verify_committed(
            self.committed, self.polynomial_path
        )
        self.assertEqual(
            seeded_report["rounds_verified"],
            self.manifest["seeded"]["variables"],
        )
        self.assertEqual(
            committed_report["rounds_verified"],
            self.manifest["committed"]["variables"],
        )
        self.assertEqual(
            committed_report["polynomial_digest"],
            self.manifest["committed"]["polynomial_commitment"],
        )

    def test_seeded_certificate_mutations_are_rejected(self) -> None:
        for offset in range(len(self.seeded)):
            with self.subTest(offset=offset):
                mutated = bytearray(self.seeded)
                mutated[offset] ^= 1
                self.assert_rejected(lambda: verifier.verify_seeded(bytes(mutated)))

    def test_committed_certificate_mutations_are_rejected(self) -> None:
        for offset in range(len(self.committed)):
            with self.subTest(offset=offset):
                mutated = bytearray(self.committed)
                mutated[offset] ^= 1
                self.assert_rejected(
                    lambda: verifier.verify_committed(
                        bytes(mutated), self.polynomial_path
                    )
                )

    def test_committed_polynomial_mutations_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "mutated.phsm"
            for offset in range(len(self.polynomial)):
                with self.subTest(offset=offset):
                    mutated = bytearray(self.polynomial)
                    mutated[offset] ^= 1
                    path.write_bytes(mutated)
                    self.assert_rejected(
                        lambda: verifier.verify_committed(self.committed, path)
                    )

    def test_published_million_round_artifacts_verify_when_present(self) -> None:
        seeded_path = ROOT / "target" / "power_house_sparse_record.phsp"
        polynomial_path = ROOT / "target" / "external_interaction_model.phsm"
        committed_path = ROOT / "target" / "external_interaction_model.phcp"
        if not all(path.exists() for path in (seeded_path, polynomial_path, committed_path)):
            self.skipTest("published-scale artifacts have not been generated")

        seeded_report = verifier.verify_seeded(seeded_path.read_bytes())
        committed_report = verifier.verify_committed(
            committed_path.read_bytes(), polynomial_path
        )
        self.assertEqual(seeded_report["rounds_verified"], 1_000_000)
        self.assertEqual(committed_report["rounds_verified"], 1_000_000)

    def test_truncation_is_rejected(self) -> None:
        for data, operation in (
            (self.seeded, verifier.verify_seeded),
            (
                self.committed,
                lambda value: verifier.verify_committed(value, self.polynomial_path),
            ),
        ):
            for amount in (1, 8, 32, len(data) // 2):
                with self.subTest(size=len(data), amount=amount):
                    self.assert_rejected(lambda: operation(data[:-amount]))

    def test_primality_gate_rejects_pseudoprimes(self) -> None:
        for composite in (9, 341, 561, 1_105, 1_729, 3_215_031_751):
            with self.subTest(composite=composite):
                self.assertFalse(verifier.is_prime_u64(composite))
        for prime in (3, 101, 1_000_000_007, 18_446_744_073_709_551_557):
            with self.subTest(prime=prime):
                self.assertTrue(verifier.is_prime_u64(prime))


if __name__ == "__main__":
    unittest.main(verbosity=2)

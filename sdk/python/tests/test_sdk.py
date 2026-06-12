import copy
import json
import pathlib
import unittest

from power_house import (
    PowerHouseError,
    calculate_phx_fingerprint,
    create_artifact,
    equivalent,
    fork,
    merge,
    new_rootprint,
    verify_artifact,
    verify_rootprint,
)
from power_house.external import attach_external_proof, verify_external_attachments


ROOT = pathlib.Path(__file__).resolve().parents[3]
VECTORS = ROOT / "conformance" / "pha-v1"


class PowerHouseSdkTests(unittest.TestCase):
    def test_core_defaults_and_optional_epa_are_separate(self):
        artifact = create_artifact(
            {"source": "python"},
            "power-house/test/v1",
            {"claim": 7},
            {"accepted": True},
        )
        self.assertNotIn(
            "external_proof_attachments", artifact["embedded_proof"]
        )
        attached = attach_external_proof(
            artifact, "epa-1", "external/test/v1", {"proof": "opaque"}
        )
        self.assertEqual(
            artifact["phx_fingerprint"], attached["phx_fingerprint"]
        )
        verify_artifact(attached)
        verify_external_attachments(attached)

        attached["embedded_proof"]["external_proof_attachments"][0][
            "payload"
        ] = {"proof": "mutated"}
        verify_artifact(attached)
        with self.assertRaises(PowerHouseError):
            verify_external_attachments(attached)

    def test_rootprint_branching_ignores_epa(self):
        base = create_artifact({}, "power-house/test/v1", {}, {"value": 1})
        graph = new_rootprint("main", base)
        attached = attach_external_proof(
            base, "epa-1", "external/test/v1", {"proof": "opaque"}
        )
        left = fork(graph, "main", "left", attached)
        right = fork(graph, "main", "right", base)
        merge(graph, left, right, "merged", base)
        self.assertTrue(equivalent(graph, left, right))
        verify_rootprint(graph)

    def test_non_integer_numbers_are_rejected(self):
        with self.assertRaises(PowerHouseError):
            create_artifact({}, "power-house/test/v1", {}, {"value": 1.5})

    def test_rust_conformance_vectors(self):
        core = json.loads((VECTORS / "core-valid.pha").read_text())
        attached = json.loads((VECTORS / "core-with-epa.pha").read_text())
        graph = json.loads((VECTORS / "rootprint-valid.json").read_text())
        verify_artifact(core)
        verify_artifact(attached)
        verify_external_attachments(attached)
        verify_rootprint(graph)
        self.assertEqual(
            core["phx_fingerprint"], calculate_phx_fingerprint(core)
        )
        self.assertEqual(
            core["phx_fingerprint"], attached["phx_fingerprint"]
        )

        broken = copy.deepcopy(core)
        broken["embedded_proof"]["proof"]["accepted"] = False
        with self.assertRaises(PowerHouseError):
            verify_artifact(broken)


if __name__ == "__main__":
    unittest.main()

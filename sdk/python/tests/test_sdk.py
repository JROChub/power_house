import copy
import json
import pathlib
import unittest

from power_house import (
    PowerHouseError,
    calculate_phx_fingerprint,
    create_artifact,
    create_identity,
    equivalent,
    equivalent_identity,
    equivalent_rootprints,
    fork,
    fork_identity,
    merge,
    merge_identity,
    merge_rootprints,
    new_rootprint,
    replay_identity,
    replay_rootprint,
    verify_artifact,
    verify_identity,
    verify_rootprint,
)
from power_house.external import attach_external_proof, verify_external_attachments


ROOT = pathlib.Path(__file__).resolve().parents[3]
VECTORS = ROOT / "conformance" / "pha-v1"
IDENTITY_VECTORS = ROOT / "conformance" / "identity-v1"


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

    def test_identity_operations_and_replay_are_deterministic(self):
        root_artifact = create_artifact(
            {"source": "python-identity"},
            "power-house/identity-test/v1",
            {"claim": 1},
            {"accepted": True},
        )
        root, graph = create_identity(root_artifact, "main")
        left = fork_identity(
            root,
            graph,
            "left",
            create_artifact({}, "power-house/test/v1", {}, {"value": 2}),
        )
        right = fork_identity(
            root,
            graph,
            "right",
            create_artifact({}, "power-house/test/v1", {}, {"value": 2}),
        )
        self.assertTrue(equivalent_identity(left, right, graph))
        merged = merge_identity(
            left,
            right,
            graph,
            "merged",
            create_artifact({}, "power-house/test/v1", {}, {"value": 3}),
        )
        verify_identity(merged, graph)
        self.assertEqual(
            replay_identity(merged, graph),
            replay_identity(merged, graph),
        )

    def test_graph_merge_is_commutative(self):
        artifact = create_artifact(
            {}, "power-house/test/v1", {}, {"value": 1}
        )
        left = new_rootprint("main", artifact)
        right = new_rootprint("main", artifact)
        fork(left, "main", "left", artifact)
        fork(right, "main", "right", artifact)
        left_first = merge_rootprints(left, right)
        right_first = merge_rootprints(right, left)
        self.assertTrue(equivalent_rootprints(left_first, right_first))
        self.assertEqual(
            replay_rootprint(left_first),
            replay_rootprint(right_first),
        )
        elevated = copy.deepcopy(left_first)
        child = next(
            branch
            for branch in elevated["branches"].values()
            if branch["parents"]
        )
        child["sequence"] = 9
        verify_rootprint(elevated)
        self.assertEqual(
            replay_rootprint(elevated),
            replay_rootprint(left_first),
        )

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

    def test_rust_identity_conformance_vectors(self):
        identity = json.loads(
            (IDENTITY_VECTORS / "identity-valid.json").read_text()
        )
        graph = json.loads(
            (IDENTITY_VECTORS / "rootprint-valid.json").read_text()
        )
        expected = json.loads(
            (IDENTITY_VECTORS / "replay-valid.json").read_text()
        )
        verify_identity(identity, graph)
        self.assertEqual(replay_identity(identity, graph), expected)

        def vector_artifact(stage):
            return create_artifact(
                {
                    "producer": "power-house-identity-conformance",
                    "stage": stage,
                },
                "power-house/identity-conformance/v1",
                {"claim": 36, "stage": stage},
                {"accepted": True},
            )

        generated_root, generated_graph = create_identity(
            vector_artifact("main"), "main"
        )
        shared = vector_artifact("candidate")
        generated_left = fork_identity(
            generated_root, generated_graph, "left", shared
        )
        generated_right = fork_identity(
            generated_root, generated_graph, "right", shared
        )
        generated_identity = merge_identity(
            generated_left,
            generated_right,
            generated_graph,
            "accepted",
            vector_artifact("accepted"),
        )
        self.assertEqual(generated_identity, identity)
        self.assertEqual(generated_graph, graph)
        self.assertEqual(
            replay_identity(generated_identity, generated_graph), expected
        )


if __name__ == "__main__":
    unittest.main()

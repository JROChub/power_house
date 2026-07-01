import copy
import json
import pathlib
import tempfile
import unittest

from power_house import (
    MEMORY_CAPSULE_SCHEMA_V1,
    PowerHouseError,
    ROOTPRINT_SCHEMA_V1,
    calculate_memory_capsule_digest,
    calculate_memory_core_digest,
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
    load_memory_capsule,
    replay_identity,
    replay_rootprint,
    semantic_packet_digest,
    verify_artifact,
    verify_identity,
    verify_memory_capsule,
    verify_rootprint,
)
from power_house.external import attach_external_proof, verify_external_attachments


ROOT = pathlib.Path(__file__).resolve().parents[3]
VECTORS = ROOT / "conformance" / "pha-v1"
IDENTITY_VECTORS = ROOT / "conformance" / "identity-v1"


def memory_capsule_fixture(include_semantics=False):
    artifact = create_artifact(
        {"source": "python-memory"},
        "power-house/python-memory/v1",
        {"claim": 13},
        {"accepted": True},
    )
    graph = new_rootprint("main", artifact)
    replay = replay_rootprint(graph)
    core = {
        "pha": artifact,
        "proofs": [
            {
                "kind": "rootprint",
                "schema": ROOTPRINT_SCHEMA_V1,
                "digest": replay["state_fingerprint"],
                "bytes_ref": None,
                "public_statement": "python memory fixture",
                "verification_profile": "rootprint-replay",
            }
        ],
        "core_digest": "",
        "core_verification_policy": {
            "require_rootprint": True,
            "require_replay": True,
            "allow_external_attachments": True,
            "fail_on_unknown_critical": True,
        },
    }
    core["core_digest"] = calculate_memory_core_digest(core)
    capsule = {
        "header": {
            "schema": MEMORY_CAPSULE_SCHEMA_V1,
            "capsule_id": "phm_python_memory",
            "capsule_digest": None,
            "created_at_unix_ms": 0,
            "producer": {
                "name": "python-sdk",
                "tool": "power_house",
                "power_house_version": "0.3.22",
                "slbit_version": None,
                "rustc": None,
                "platform": "python",
            },
            "critical_extensions": [],
            "noncritical_extensions": [],
        },
        "core": core,
        "lineage": {
            "rootprint": graph,
            "branches": [
                {
                    "branch_id": graph["root_branch"],
                    "label": "main",
                    "parent_ids": [],
                    "artifact_digest": artifact["phx_fingerprint"],
                    "state_fingerprint": replay["state_fingerprint"],
                    "operation": "create",
                }
            ],
            "equivalence": [],
        },
        "replay": {
            "replay": {
                "engine": "power_house",
                "version": "0.3.22",
                "commands": ["python verify_memory_capsule"],
                "expected": {
                    "core_valid": True,
                    "rootprint_valid": True,
                    "replay_fingerprint": replay["state_fingerprint"],
                    "sidecar_valid": None,
                },
                "resource_bounds": {
                    "max_memory_mb": 512,
                    "max_disk_mb": 1024,
                    "max_wall_seconds_reference": 600,
                },
                "network_required": False,
            }
        },
        "witnesses": [],
        "receipts": [],
    }
    if include_semantics:
        packet = {
            "schema": "slbit/viz-packet/v3",
            "packet_id": "slp_python_memory",
            "packet_digest": "",
            "claim": {
                "label": "python-memory",
                "bound_core": {
                    "capsule_id": capsule["header"]["capsule_id"],
                    "branch_id": graph["root_branch"],
                    "replay_fingerprint": replay["state_fingerprint"],
                },
            },
        }
        packet["packet_digest"] = semantic_packet_digest(packet)
        capsule["semantics"] = {
            "sidecar_schema": "power-house/observatory-sidecar/v2",
            "packets": [
                {
                    "packet_schema": "slbit/viz-packet/v3",
                    "packet_id": packet["packet_id"],
                    "packet_digest": packet["packet_digest"],
                    "bound_branch_id": graph["root_branch"],
                    "bound_replay_fingerprint": replay["state_fingerprint"],
                    "role": "claim_view",
                    "packet": packet,
                }
            ],
            "semantic_policy": {
                "semantic_changes_affect_core": False,
                "llm_text_is_non_authoritative": True,
                "require_packet_digest": True,
                "require_branch_binding": True,
            },
        }
    capsule["header"]["capsule_digest"] = calculate_memory_capsule_digest(capsule)
    return capsule


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

    def test_memory_capsule_verification_and_loading(self):
        capsule = memory_capsule_fixture()
        report = verify_memory_capsule(capsule)
        self.assertTrue(report.core_valid)
        self.assertTrue(report.rootprint_valid)
        self.assertTrue(report.replay_valid)
        self.assertEqual(report.unsupported_profiles, ())

        with tempfile.TemporaryDirectory() as temp:
            path = pathlib.Path(temp) / "capsule.phm"
            path.write_text(json.dumps(capsule, separators=(",", ":"), sort_keys=True))
            self.assertEqual(load_memory_capsule(path), capsule)
            path.write_text('{"a":1,"a":2}')
            with self.assertRaises(PowerHouseError):
                load_memory_capsule(path)

    def test_memory_semantic_mutation_is_non_core(self):
        capsule = memory_capsule_fixture(include_semantics=True)
        report = verify_memory_capsule(capsule, policy="inspect")
        self.assertTrue(report.core_valid)
        self.assertTrue(report.semantic_valid)
        self.assertFalse(report.sidecar_valid)

        original_core_digest = capsule["core"]["core_digest"]
        mutated = copy.deepcopy(capsule)
        mutated["semantics"]["packets"][0]["packet"]["claim"]["label"] = "tampered"
        mutated["header"]["capsule_digest"] = calculate_memory_capsule_digest(mutated)
        with self.assertRaises(PowerHouseError):
            verify_memory_capsule(mutated, policy="inspect")
        self.assertEqual(mutated["core"]["core_digest"], original_core_digest)


if __name__ == "__main__":
    unittest.main()

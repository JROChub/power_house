"""Pure Power House `.pha` and Rootprint operations."""

from __future__ import annotations

import copy
import hashlib
import json
import unicodedata
from typing import Any, Dict, Iterable, List, Tuple

PHA_SCHEMA_V1 = "power-house/pha/v1"
ROOTPRINT_SCHEMA_V1 = "power-house/rootprint/v1"

_PHX_DOMAIN = b"power-house:pha:v1:phx-fingerprint\x00"
_BRANCH_DOMAIN = b"power-house:rootprint:v1:branch-id\x00"
_REPLAY_DOMAIN = b"power-house:rootprint:v1:replay-state\x00"


class PowerHouseError(ValueError):
    """Raised when a Power House artifact or Rootprint graph is invalid."""


def _canonical_json(value: Any) -> bytes:
    _validate_json_numbers(value)
    return json.dumps(
        value,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")


def _validate_json_numbers(value: Any) -> None:
    if isinstance(value, bool) or value is None or isinstance(value, str):
        return
    if isinstance(value, int):
        if value < -(2**63) or value > 2**64 - 1:
            raise PowerHouseError("JSON integer is outside the Rust i64/u64 range")
        return
    if isinstance(value, float):
        raise PowerHouseError("non-integer JSON numbers are not canonical")
    if isinstance(value, list):
        for item in value:
            _validate_json_numbers(item)
        return
    if isinstance(value, dict):
        for item in value.values():
            _validate_json_numbers(item)
        return
    raise PowerHouseError(f"value is not JSON-compatible: {type(value).__name__}")


def _sha256(domain: bytes, value: Any) -> str:
    digest = hashlib.sha256(domain + _canonical_json(value)).hexdigest()
    return f"sha256:{digest}"


def _core_projection(artifact: Dict[str, Any]) -> Dict[str, Any]:
    try:
        embedded = artifact["embedded_proof"]
        return {
            "embedded_proof": {
                "proof": embedded["proof"],
                "protocol": embedded["protocol"],
                "public_inputs": embedded["public_inputs"],
            },
            "provenance": artifact["provenance"],
            "schema": artifact["schema"],
        }
    except (KeyError, TypeError) as error:
        raise PowerHouseError(f"invalid PHA structure: {error}") from error


def calculate_phx_fingerprint(artifact: Dict[str, Any]) -> str:
    """Calculate core identity while ignoring EPA and stored fingerprint."""
    return _sha256(_PHX_DOMAIN, _core_projection(artifact))


def create_artifact(
    provenance: Any,
    protocol: str,
    public_inputs: Any,
    proof: Any,
) -> Dict[str, Any]:
    """Create a pure Power House `.pha` v1 artifact."""
    if not isinstance(protocol, str) or not protocol.strip():
        raise PowerHouseError("embedded proof protocol must not be empty")
    artifact = {
        "schema": PHA_SCHEMA_V1,
        "provenance": copy.deepcopy(provenance),
        "embedded_proof": {
            "protocol": protocol,
            "public_inputs": copy.deepcopy(public_inputs),
            "proof": copy.deepcopy(proof),
        },
        "phx_fingerprint": "",
    }
    artifact["phx_fingerprint"] = calculate_phx_fingerprint(artifact)
    return artifact


def verify_artifact(artifact: Dict[str, Any]) -> None:
    """Verify only Power House core data."""
    if artifact.get("schema") != PHA_SCHEMA_V1:
        raise PowerHouseError(f"unsupported PHA schema: {artifact.get('schema')}")
    protocol = artifact.get("embedded_proof", {}).get("protocol")
    if not isinstance(protocol, str) or not protocol.strip():
        raise PowerHouseError("embedded proof protocol must not be empty")
    identity_root = artifact.get("identity_root")
    if identity_root is not None:
        _validate_rootprint_id(identity_root)
    found = artifact.get("phx_fingerprint")
    expected = calculate_phx_fingerprint(artifact)
    if found != expected:
        raise PowerHouseError(
            f"PHA core fingerprint mismatch: expected {expected}, found {found}"
        )


def _branch_id(label: str, parents: Iterable[str], artifact: Dict[str, Any]) -> str:
    verify_artifact(artifact)
    return _sha256(
        _BRANCH_DOMAIN,
        {
            "artifact_phx_fingerprint": artifact["phx_fingerprint"],
            "label": label,
            "parents": list(parents),
        },
    )


def _normalize_label(label: str) -> str:
    if (
        not isinstance(label, str)
        or not label.strip()
        or len(label.strip()) > 128
        or any(unicodedata.category(character) == "Cc" for character in label)
    ):
        raise PowerHouseError(f"invalid branch label: {label!r}")
    return label.strip()


def _validate_rootprint_id(value: Any) -> None:
    if (
        not isinstance(value, str)
        or not value.startswith("sha256:")
        or len(value) != 71
        or any(character not in "0123456789abcdef" for character in value[7:])
    ):
        raise PowerHouseError(f"invalid Rootprint identifier: {value}")


def new_rootprint(label: str, artifact: Dict[str, Any]) -> Dict[str, Any]:
    """Create a Rootprint graph from a verified artifact."""
    label = _normalize_label(label)
    artifact = copy.deepcopy(artifact)
    branch_id = _branch_id(label, [], artifact)
    return {
        "schema": ROOTPRINT_SCHEMA_V1,
        "root_branch": branch_id,
        "branches": {
            branch_id: {
                "id": branch_id,
                "label": label,
                "sequence": 0,
                "parents": [],
                "artifact": artifact,
            }
        },
    }


def navigate(graph: Dict[str, Any], selector: str) -> Dict[str, Any]:
    """Resolve an exact ID, unique ID prefix, or unique label."""
    branches = graph.get("branches", {})
    if selector in branches:
        return branches[selector]
    matches = [
        branch
        for branch in branches.values()
        if branch.get("id", "").startswith(selector) or branch.get("label") == selector
    ]
    if not matches:
        raise PowerHouseError(f"branch not found: {selector}")
    if len(matches) != 1:
        raise PowerHouseError(f"ambiguous branch selector: {selector}")
    return matches[0]


def fork(
    graph: Dict[str, Any],
    parent: str,
    label: str,
    artifact: Dict[str, Any],
) -> str:
    """Append a one-parent branch and return its deterministic ID."""
    parent_branch = navigate(graph, parent)
    label = _normalize_label(label)
    artifact = copy.deepcopy(artifact)
    parents = [parent_branch["id"]]
    branch_id = _branch_id(label, parents, artifact)
    if branch_id in graph["branches"]:
        raise PowerHouseError(f"branch already exists: {branch_id}")
    graph["branches"][branch_id] = {
        "id": branch_id,
        "label": label,
        "sequence": parent_branch["sequence"] + 1,
        "parents": parents,
        "artifact": artifact,
    }
    return branch_id


def merge(
    graph: Dict[str, Any],
    left: str,
    right: str,
    label: str,
    artifact: Dict[str, Any],
) -> str:
    """Append a two-parent merge branch and return its deterministic ID."""
    left_branch = navigate(graph, left)
    right_branch = navigate(graph, right)
    if left_branch["id"] == right_branch["id"]:
        raise PowerHouseError("merge parents resolve to the same branch")
    label = _normalize_label(label)
    artifact = copy.deepcopy(artifact)
    parents = sorted([left_branch["id"], right_branch["id"]])
    branch_id = _branch_id(label, parents, artifact)
    if branch_id in graph["branches"]:
        raise PowerHouseError(f"branch already exists: {branch_id}")
    graph["branches"][branch_id] = {
        "id": branch_id,
        "label": label,
        "sequence": max(left_branch["sequence"], right_branch["sequence"]) + 1,
        "parents": parents,
        "artifact": artifact,
    }
    return branch_id


def equivalent(graph: Dict[str, Any], left: str, right: str) -> bool:
    """Compare branch core identities while ignoring EPA."""
    return (
        navigate(graph, left)["artifact"]["phx_fingerprint"]
        == navigate(graph, right)["artifact"]["phx_fingerprint"]
    )


def verify_rootprint(graph: Dict[str, Any]) -> None:
    """Verify Rootprint graph invariants using Power House core data only."""
    if graph.get("schema") != ROOTPRINT_SCHEMA_V1:
        raise PowerHouseError(f"unsupported Rootprint schema: {graph.get('schema')}")
    branches = graph.get("branches")
    root_id = graph.get("root_branch")
    if not isinstance(branches, dict) or root_id not in branches:
        raise PowerHouseError("Rootprint root branch is missing")
    root = branches[root_id]
    if root.get("sequence") != 0 or root.get("parents") != []:
        raise PowerHouseError("root branch must have sequence 0 and no parents")

    for key, branch in branches.items():
        if key != branch.get("id"):
            raise PowerHouseError("branch map key does not match branch id")
        verify_artifact(branch["artifact"])
        parents: List[str] = branch.get("parents", [])
        if branch["id"] != _branch_id(branch["label"], parents, branch["artifact"]):
            raise PowerHouseError("Rootprint branch ID mismatch")
        if len(parents) > 2 or parents != sorted(set(parents)):
            raise PowerHouseError("branch parents must be sorted and unique")
        if key != root_id and not parents:
            raise PowerHouseError("non-root branch has no parent")
        for parent_id in parents:
            parent = branches.get(parent_id)
            if parent is None or parent["sequence"] >= branch["sequence"]:
                raise PowerHouseError("branch does not follow its parent")
    reachable = {root_id}
    while True:
        expanded = reachable | {
            branch_id
            for branch_id, branch in branches.items()
            if any(parent in reachable for parent in branch["parents"])
        }
        if expanded == reachable:
            break
        reachable = expanded
    if reachable != set(branches):
        raise PowerHouseError("graph contains a branch unreachable from the root")


def replay_rootprint(graph: Dict[str, Any]) -> Dict[str, Any]:
    """Reconstruct the canonical logical state of a Rootprint graph."""
    verify_rootprint(graph)
    topological = sorted(
        graph["branches"].values(),
        key=lambda branch: (branch["sequence"], branch["id"]),
    )
    canonical_sequences: Dict[str, int] = {}
    for branch in topological:
        canonical_sequences[branch["id"]] = (
            0
            if not branch["parents"]
            else max(
                canonical_sequences[parent] for parent in branch["parents"]
            )
            + 1
        )
    ordered = sorted(
        topological,
        key=lambda branch: (
            canonical_sequences[branch["id"]],
            branch["id"],
        ),
    )
    branches = [
        {
            "id": branch["id"],
            "label": branch["label"],
            "sequence": canonical_sequences[branch["id"]],
            "parents": list(branch["parents"]),
            "artifact_phx_fingerprint": branch["artifact"]["phx_fingerprint"],
        }
        for branch in ordered
    ]
    parent_ids = {
        parent
        for branch in graph["branches"].values()
        for parent in branch["parents"]
    }
    tips = sorted(
        branch_id
        for branch_id in graph["branches"]
        if branch_id not in parent_ids
    )
    projection = {
        "branches": branches,
        "root_branch": graph["root_branch"],
        "tips": tips,
    }
    return {
        "root_branch": graph["root_branch"],
        "branches": branches,
        "tips": tips,
        "state_fingerprint": _sha256(_REPLAY_DOMAIN, projection),
    }


def equivalent_rootprints(left: Dict[str, Any], right: Dict[str, Any]) -> bool:
    """Return whether two valid graphs replay to identical logical state."""
    return replay_rootprint(left) == replay_rootprint(right)


def merge_rootprints(
    left: Dict[str, Any], right: Dict[str, Any]
) -> Dict[str, Any]:
    """Deterministically union two valid Rootprint graphs with one root."""
    verify_rootprint(left)
    verify_rootprint(right)
    if left["root_branch"] != right["root_branch"]:
        raise PowerHouseError(
            "cannot merge Rootprint graphs with different roots"
        )
    merged = copy.deepcopy(left)
    for branch_id, candidate in right["branches"].items():
        existing = merged["branches"].get(branch_id)
        if existing is None:
            merged["branches"][branch_id] = copy.deepcopy(candidate)
            continue
        existing_core = {
            "artifact_phx_fingerprint": existing["artifact"]["phx_fingerprint"],
            "id": existing["id"],
            "label": existing["label"],
            "parents": existing["parents"],
        }
        candidate_core = {
            "artifact_phx_fingerprint": candidate["artifact"]["phx_fingerprint"],
            "id": candidate["id"],
            "label": candidate["label"],
            "parents": candidate["parents"],
        }
        if existing_core != candidate_core:
            raise PowerHouseError(
                f"conflicting Rootprint branch data for {branch_id}"
            )
        if _canonical_json(candidate) < _canonical_json(existing):
            merged["branches"][branch_id] = copy.deepcopy(candidate)
    verify_rootprint(merged)
    return merged


def create_identity(
    artifact: Dict[str, Any], label: str
) -> Tuple[Dict[str, Any], Dict[str, Any]]:
    """Create an immutable identity envelope and Rootprint graph."""
    artifact = copy.deepcopy(artifact)
    artifact.pop("identity_root", None)
    graph = new_rootprint(label, artifact)
    rootprint_id = graph["root_branch"]
    return _bind_identity(graph, rootprint_id), graph


def fork_identity(
    identity: Dict[str, Any],
    graph: Dict[str, Any],
    label: str,
    artifact: Dict[str, Any],
) -> Dict[str, Any]:
    """Create a child identity without mutating the parent identity."""
    verify_identity(identity, graph)
    artifact = copy.deepcopy(artifact)
    artifact.pop("identity_root", None)
    rootprint_id = fork(
        graph, identity["rootprint_id"], label, artifact
    )
    return _bind_identity(graph, rootprint_id)


def merge_identity(
    left: Dict[str, Any],
    right: Dict[str, Any],
    graph: Dict[str, Any],
    label: str,
    artifact: Dict[str, Any],
) -> Dict[str, Any]:
    """Merge two identities into a deterministic reconciliation identity."""
    verify_identity(left, graph)
    verify_identity(right, graph)
    artifact = copy.deepcopy(artifact)
    artifact.pop("identity_root", None)
    rootprint_id = merge(
        graph,
        left["rootprint_id"],
        right["rootprint_id"],
        label,
        artifact,
    )
    return _bind_identity(graph, rootprint_id)


def verify_identity(identity: Dict[str, Any], graph: Dict[str, Any]) -> None:
    """Verify artifact, graph, node resolution, and identity binding."""
    if not isinstance(identity, dict):
        raise PowerHouseError("identity must be a JSON object")
    artifact = identity.get("pha")
    rootprint_id = identity.get("rootprint_id")
    if not isinstance(artifact, dict):
        raise PowerHouseError("identity pha is missing")
    _validate_rootprint_id(rootprint_id)
    verify_artifact(artifact)
    verify_rootprint(graph)
    if artifact.get("identity_root") != rootprint_id:
        raise PowerHouseError("identity_root does not match the identity")
    branch = navigate(graph, rootprint_id)
    if branch["id"] != rootprint_id:
        raise PowerHouseError("identity_root cannot be resolved")
    if branch["artifact"].get("identity_root") != rootprint_id:
        raise PowerHouseError("Rootprint node does not bind back to identity")
    if (
        branch["artifact"]["phx_fingerprint"]
        != artifact["phx_fingerprint"]
    ):
        raise PowerHouseError("identity artifact does not match Rootprint node")


def replay_identity(
    identity: Dict[str, Any], graph: Dict[str, Any]
) -> Dict[str, Any]:
    """Replay a graph and resolve one identity deterministically."""
    verify_identity(identity, graph)
    state = replay_rootprint(graph)
    if identity["rootprint_id"] not in {
        branch["id"] for branch in state["branches"]
    }:
        raise PowerHouseError("identity_root cannot be resolved during replay")
    return {
        "rootprint_id": identity["rootprint_id"],
        "artifact_phx_fingerprint": identity["pha"]["phx_fingerprint"],
        "graph": state,
    }


def equivalent_identity(
    left: Dict[str, Any],
    right: Dict[str, Any],
    graph: Dict[str, Any],
) -> bool:
    """Compare two verified identity core artifacts."""
    verify_identity(left, graph)
    verify_identity(right, graph)
    return equivalent(
        graph, left["rootprint_id"], right["rootprint_id"]
    )


def _bind_identity(
    graph: Dict[str, Any], rootprint_id: str
) -> Dict[str, Any]:
    _validate_rootprint_id(rootprint_id)
    branch = graph["branches"].get(rootprint_id)
    if branch is None:
        raise PowerHouseError(f"identity_root cannot be resolved: {rootprint_id}")
    branch["artifact"]["identity_root"] = rootprint_id
    return {
        "pha": copy.deepcopy(branch["artifact"]),
        "rootprint_id": rootprint_id,
    }

"""Pure Power House `.pha` and Rootprint operations."""

from __future__ import annotations

import copy
import hashlib
import json
import unicodedata
from typing import Any, Dict, Iterable, List

PHA_SCHEMA_V1 = "power-house/pha/v1"
ROOTPRINT_SCHEMA_V1 = "power-house/rootprint/v1"

_PHX_DOMAIN = b"power-house:pha:v1:phx-fingerprint\x00"
_BRANCH_DOMAIN = b"power-house:rootprint:v1:branch-id\x00"


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

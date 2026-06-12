"""Explicit, optional external proof attachment transport helpers."""

from __future__ import annotations

import copy
import hashlib
import json
from typing import Any, Dict, Optional

from .core import PowerHouseError, _validate_json_numbers, verify_artifact


def _payload_digest(payload: Any) -> str:
    _validate_json_numbers(payload)
    encoded = json.dumps(
        payload,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    return f"sha256:{hashlib.sha256(encoded).hexdigest()}"


def attach_external_proof(
    artifact: Dict[str, Any],
    attachment_id: str,
    proof_system: str,
    payload: Any,
    *,
    verifier_hint: Optional[str] = None,
    metadata: Any = None,
) -> Dict[str, Any]:
    """Return a copy with EPA data; core identity remains unchanged."""
    verify_artifact(artifact)
    if not attachment_id.strip() or not proof_system.strip():
        raise PowerHouseError("attachment id and proof system must not be empty")
    updated = copy.deepcopy(artifact)
    attachment = {
        "id": attachment_id,
        "proof_system": proof_system,
        "payload": copy.deepcopy(payload),
        "payload_sha256": _payload_digest(payload),
    }
    if verifier_hint is not None:
        attachment["verifier_hint"] = verifier_hint
    if metadata is not None:
        attachment["metadata"] = copy.deepcopy(metadata)
    embedded = updated["embedded_proof"]
    embedded.setdefault("external_proof_attachments", []).append(attachment)
    return updated


def verify_external_attachments(artifact: Dict[str, Any]) -> None:
    """Verify EPA transport integrity after verifying Power House core data."""
    verify_artifact(artifact)
    attachments = artifact["embedded_proof"].get("external_proof_attachments", [])
    for attachment in attachments:
        if not attachment.get("id", "").strip():
            raise PowerHouseError("attachment id must not be empty")
        if not attachment.get("proof_system", "").strip():
            raise PowerHouseError("attachment proof system must not be empty")
        expected = _payload_digest(attachment.get("payload"))
        if attachment.get("payload_sha256") != expected:
            raise PowerHouseError(
                f"external proof attachment {attachment.get('id')} digest mismatch"
            )

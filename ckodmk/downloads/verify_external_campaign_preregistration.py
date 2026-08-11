#!/usr/bin/env python3
"""Validate and optionally authenticate a CKODMK external campaign preregistration."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import subprocess
import tempfile
from pathlib import Path
from typing import Any

SCHEMA = "mfenx/ckodmk-external-campaign-preregistration/v1"
NAMESPACE = "ckodmk-external-campaign-preregistration"
SSH_KEYGEN = Path("/usr/bin/ssh-keygen")
MAX_DOCUMENT_BYTES = 2_000_000
MAX_SIGNATURE_BYTES = 32_768
MAX_SIGNERS_BYTES = 131_072
MAX_TEXT = 512
MAX_GROUPS = 256
DIGEST = re.compile(r"sha256:[0-9a-f]{64}\Z")
COMMIT = re.compile(r"[0-9a-f]{40}\Z")
IDENTIFIER = re.compile(r"[A-Za-z0-9][A-Za-z0-9_.:@/+\-]{0,255}\Z")
RFC3339 = re.compile(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z\Z")
REQUIRED_BASELINES = {
    "hash-only",
    "differential-testing",
    "strongest-applicable-validator",
}


class VerificationError(ValueError):
    """A structural, arithmetic, binding, or authentication failure."""


def _no_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            raise VerificationError(f"duplicate JSON key: {key}")
        value[key] = item
    return value


def _reject_float(value: str) -> Any:
    raise VerificationError(f"JSON floating-point token is forbidden: {value}")


def _reject_constant(value: str) -> Any:
    raise VerificationError(f"JSON non-finite constant is forbidden: {value}")


def _open_regular(path: Path, limit: int, label: str) -> bytes:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    flags |= getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0)
    descriptor = os.open(path, flags)
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            raise VerificationError(f"{label} is not a regular file")
        if before.st_size > limit:
            raise VerificationError(f"{label} exceeds {limit} bytes")
        payload = bytearray()
        remaining = before.st_size
        while remaining:
            chunk = os.read(descriptor, min(remaining, 1_048_576))
            if not chunk:
                raise VerificationError(f"{label} changed while reading")
            payload.extend(chunk)
            remaining -= len(chunk)
        if os.read(descriptor, 1):
            raise VerificationError(f"{label} grew while reading")
        after = os.fstat(descriptor)
        identity = lambda item: (item.st_dev, item.st_ino, item.st_size, item.st_mtime_ns)
        if identity(before) != identity(after):
            raise VerificationError(f"{label} changed while reading")
        return bytes(payload)
    finally:
        os.close(descriptor)


def _object(value: Any, fields: set[str], label: str) -> dict[str, Any]:
    if type(value) is not dict:
        raise VerificationError(f"{label} must be an object")
    actual = set(value)
    if actual != fields:
        raise VerificationError(
            f"{label} fields differ: missing={sorted(fields-actual)}, "
            f"unknown={sorted(actual-fields)}"
        )
    return value


def _text(value: Any, label: str, *, identifier: bool = False) -> str:
    if type(value) is not str or not 1 <= len(value) <= MAX_TEXT or not value.isascii():
        raise VerificationError(f"{label} must be bounded nonempty ASCII")
    if identifier and not IDENTIFIER.fullmatch(value):
        raise VerificationError(f"{label} is not a canonical identifier")
    return value


def _bool(value: Any, label: str) -> bool:
    if type(value) is not bool:
        raise VerificationError(f"{label} must be boolean")
    return value


def _integer(value: Any, label: str, *, minimum: int = 0, maximum: int = 10**12) -> int:
    if type(value) is not int or not minimum <= value <= maximum:
        raise VerificationError(f"{label} must be an integer in [{minimum}, {maximum}]")
    return value


def _digest(value: Any, label: str) -> str:
    if type(value) is not str or not DIGEST.fullmatch(value):
        raise VerificationError(f"{label} must be a canonical SHA-256 digest")
    return value


def _groups(value: Any, label: str) -> tuple[list[dict[str, Any]], int, int]:
    if type(value) is not list or not 1 <= len(value) <= MAX_GROUPS:
        raise VerificationError(f"{label} must be a bounded nonempty array")
    result: list[dict[str, Any]] = []
    names: set[str] = set()
    total_target = 0
    total_ceiling = 0
    fields = {"id", "effective_target", "attempt_ceiling", "subtype_targets"}
    for index, raw in enumerate(value):
        item = _object(raw, fields, f"{label}[{index}]")
        name = _text(item["id"], f"{label}[{index}].id", identifier=True)
        if name in names:
            raise VerificationError(f"{label} identifiers must be unique")
        names.add(name)
        target = _integer(
            item["effective_target"], f"{label}[{index}].effective_target", minimum=1
        )
        ceiling = _integer(
            item["attempt_ceiling"], f"{label}[{index}].attempt_ceiling", minimum=target
        )
        subtypes = item["subtype_targets"]
        if type(subtypes) is not dict or not subtypes or len(subtypes) > MAX_GROUPS:
            raise VerificationError(f"{label}[{index}].subtype_targets is invalid")
        subtotal = 0
        for subtype, count in subtypes.items():
            _text(subtype, f"{label}[{index}].subtype", identifier=True)
            subtotal += _integer(
                count,
                f"{label}[{index}].subtype_targets[{subtype}]",
                minimum=1,
            )
        if subtotal != target:
            raise VerificationError(f"{label}[{index}] subtype targets do not sum to target")
        total_target += target
        total_ceiling += ceiling
        result.append(item)
    return result, total_target, total_ceiling


def validate(document: Any, expected_evaluator: str | None = None) -> dict[str, Any]:
    root = _object(
        document,
        {
            "schema",
            "campaign_id",
            "created_at_utc",
            "evaluator",
            "release",
            "generator",
            "seed",
            "fault_plan",
            "control_plan",
            "denominator_policy",
            "oracle_policy",
            "baselines",
            "execution_budget",
            "unblinding",
            "limitations",
        },
        "document",
    )
    if root["schema"] != SCHEMA:
        raise VerificationError("unsupported preregistration schema")
    _text(root["campaign_id"], "campaign_id", identifier=True)
    if type(root["created_at_utc"]) is not str or not RFC3339.fullmatch(
        root["created_at_utc"]
    ):
        raise VerificationError("created_at_utc must use canonical UTC seconds")

    evaluator = _object(
        root["evaluator"],
        {
            "identity",
            "organization",
            "non_author",
            "controls_generator",
            "controls_oracle",
            "controls_seed",
            "mfenx_pre_execution_access",
        },
        "evaluator",
    )
    identity = _text(evaluator["identity"], "evaluator.identity", identifier=True)
    if expected_evaluator is not None and identity != expected_evaluator:
        raise VerificationError("evaluator identity differs from caller expectation")
    _text(evaluator["organization"], "evaluator.organization")
    for field in (
        "non_author",
        "controls_generator",
        "controls_oracle",
        "controls_seed",
    ):
        if not _bool(evaluator[field], f"evaluator.{field}"):
            raise VerificationError(f"evaluator.{field} must be true")
    if _bool(evaluator["mfenx_pre_execution_access"], "evaluator.mfenx_pre_execution_access"):
        raise VerificationError("MFENX must not receive private campaign state before completion")

    release = _object(
        root["release"],
        {"repository", "commit", "archive_sha256", "manifest_sha256", "ci_run_url"},
        "release",
    )
    _text(release["repository"], "release.repository", identifier=True)
    if type(release["commit"]) is not str or not COMMIT.fullmatch(release["commit"]):
        raise VerificationError("release.commit must be 40 lowercase hexadecimal characters")
    _digest(release["archive_sha256"], "release.archive_sha256")
    _digest(release["manifest_sha256"], "release.manifest_sha256")
    if not _text(release["ci_run_url"], "release.ci_run_url").startswith(
        "https://github.com/"
    ):
        raise VerificationError("release.ci_run_url must be an HTTPS GitHub URL")

    generator = _object(
        root["generator"],
        {
            "implementation_sha256",
            "specification_sha256",
            "environment_sha256",
            "authored_by_mfenx",
        },
        "generator",
    )
    for field in ("implementation_sha256", "specification_sha256", "environment_sha256"):
        _digest(generator[field], f"generator.{field}")
    if _bool(generator["authored_by_mfenx"], "generator.authored_by_mfenx"):
        raise VerificationError("the qualification generator cannot be authored by MFENX")

    seed = _object(
        root["seed"],
        {"commitment", "commitment_scheme", "source", "revealed"},
        "seed",
    )
    _digest(seed["commitment"], "seed.commitment")
    if seed["commitment_scheme"] != "sha256-domain-separated-v1":
        raise VerificationError("unsupported seed commitment scheme")
    _text(seed["source"], "seed.source")
    if _bool(seed["revealed"], "seed.revealed"):
        raise VerificationError("the preregistration must not disclose the seed")

    _, effective_faults, attempt_ceiling = _groups(root["fault_plan"], "fault_plan")
    if effective_faults < 1_000:
        raise VerificationError("fault plan must target at least 1,000 effective faults")
    _, controls, control_attempt_ceiling = _groups(root["control_plan"], "control_plan")
    if controls < 100:
        raise VerificationError("control plan must target at least 100 valid controls")

    denominator = _object(
        root["denominator_policy"],
        {
            "selected_attempts",
            "invalid_mutations",
            "unsupported",
            "exclusions",
            "crashes",
            "timeouts",
        },
        "denominator_policy",
    )
    required_denominator = {
        "selected_attempts": "RETAIN_ALL",
        "invalid_mutations": "COUNT_SELECTED_NOT_EFFECTIVE",
        "unsupported": "COUNT_SELECTED_NOT_EFFECTIVE",
        "exclusions": "COUNT_SELECTED_NOT_EFFECTIVE",
        "crashes": "COUNT_FAILURE",
        "timeouts": "COUNT_FAILURE",
    }
    if denominator != required_denominator:
        raise VerificationError("denominator policy permits case disappearance or relabeling")

    oracle = _object(
        root["oracle_policy"],
        {"owner", "frozen_before_execution", "raw_case_records", "counterexamples_retained"},
        "oracle_policy",
    )
    if oracle["owner"] != "EVALUATOR_ONLY":
        raise VerificationError("qualification oracle must be evaluator-owned")
    for field in ("frozen_before_execution", "raw_case_records", "counterexamples_retained"):
        if not _bool(oracle[field], f"oracle_policy.{field}"):
            raise VerificationError(f"oracle_policy.{field} must be true")

    baselines = root["baselines"]
    if type(baselines) is not list or not 3 <= len(baselines) <= 16:
        raise VerificationError("baselines must be a bounded array")
    baseline_ids: set[str] = set()
    budgets: set[str] = set()
    for index, raw in enumerate(baselines):
        baseline = _object(
            raw,
            {"id", "implementation_sha256", "common_fault_set", "budget_profile_sha256"},
            f"baselines[{index}]",
        )
        baseline_id = _text(baseline["id"], f"baselines[{index}].id", identifier=True)
        if baseline_id in baseline_ids:
            raise VerificationError("baseline identifiers must be unique")
        baseline_ids.add(baseline_id)
        _digest(baseline["implementation_sha256"], f"baselines[{index}].implementation_sha256")
        if not _bool(baseline["common_fault_set"], f"baselines[{index}].common_fault_set"):
            raise VerificationError("every baseline must use the common selected fault set")
        budgets.add(
            _digest(
                baseline["budget_profile_sha256"],
                f"baselines[{index}].budget_profile_sha256",
            )
        )
    if not REQUIRED_BASELINES <= baseline_ids:
        raise VerificationError("required matched baselines are absent")
    if len(budgets) != 1:
        raise VerificationError("baselines do not share one matched budget profile")

    budget = _object(
        root["execution_budget"],
        {
            "profile_sha256",
            "per_tool_wall_seconds",
            "per_tool_cpu_seconds",
            "per_tool_address_space_bytes",
            "per_tool_output_bytes",
            "per_case_snapshot_bytes",
            "selected_attempt_capacity",
            "control_attempt_capacity",
            "aggregate_output_reserve_bytes",
        },
        "execution_budget",
    )
    profile_digest = _digest(budget["profile_sha256"], "execution_budget.profile_sha256")
    if budgets != {profile_digest}:
        raise VerificationError("baseline budget digest differs from execution budget")
    for field in (
        "per_tool_wall_seconds",
        "per_tool_cpu_seconds",
        "per_tool_address_space_bytes",
        "per_tool_output_bytes",
        "per_case_snapshot_bytes",
    ):
        _integer(budget[field], f"execution_budget.{field}", minimum=1)
    if _integer(
        budget["selected_attempt_capacity"],
        "execution_budget.selected_attempt_capacity",
        minimum=attempt_ceiling,
    ) < attempt_ceiling:
        raise VerificationError("selected-attempt capacity is below the fault attempt ceiling")
    if _integer(
        budget["control_attempt_capacity"],
        "execution_budget.control_attempt_capacity",
        minimum=control_attempt_ceiling,
    ) < control_attempt_ceiling:
        raise VerificationError("control-attempt capacity is below the control attempt ceiling")
    minimum_reserve = (
        budget["selected_attempt_capacity"] + budget["control_attempt_capacity"]
    ) * (budget["per_case_snapshot_bytes"] + 4 * budget["per_tool_output_bytes"])
    if _integer(
        budget["aggregate_output_reserve_bytes"],
        "execution_budget.aggregate_output_reserve_bytes",
        minimum=minimum_reserve,
    ) < minimum_reserve:
        raise VerificationError("aggregate output reserve is below the declared bounded formula")

    unblinding = _object(
        root["unblinding"],
        {
            "results_manifest_pinned_first",
            "seed_reveal_after_terminal_results",
            "reruns_allowed",
            "signed_return_required",
        },
        "unblinding",
    )
    for field in (
        "results_manifest_pinned_first",
        "seed_reveal_after_terminal_results",
        "signed_return_required",
    ):
        if not _bool(unblinding[field], f"unblinding.{field}"):
            raise VerificationError(f"unblinding.{field} must be true")
    if _bool(unblinding["reruns_allowed"], "unblinding.reruns_allowed"):
        raise VerificationError("qualification campaign reruns must require a new preregistration")

    limitations = root["limitations"]
    if type(limitations) is not list or len(limitations) > 256:
        raise VerificationError("limitations must be a bounded array")
    for index, item in enumerate(limitations):
        _text(item, f"limitations[{index}]")
    return {
        "campaign_id": root["campaign_id"],
        "control_target": controls,
        "evaluator": identity,
        "fault_attempt_ceiling": attempt_ceiling,
        "fault_target": effective_faults,
        "minimum_profile_satisfied": True,
        "schema": "mfenx/ckodmk-external-campaign-preregistration-verification/v1",
    }


def verify_signature(payload: bytes, signature: bytes, allowed: bytes, identity: str) -> None:
    if not SSH_KEYGEN.is_file():
        raise VerificationError("OpenSSH signature verifier is unavailable")
    with tempfile.TemporaryDirectory(prefix="ckodmk-prereg-verify-") as temporary:
        root = Path(temporary)
        signature_path = root / "document.sig"
        allowed_path = root / "allowed_signers"
        signature_path.write_bytes(signature)
        allowed_path.write_bytes(allowed)
        completed = subprocess.run(
            [
                str(SSH_KEYGEN),
                "-Y",
                "verify",
                "-f",
                str(allowed_path),
                "-I",
                identity,
                "-n",
                NAMESPACE,
                "-s",
                str(signature_path),
            ],
            input=payload,
            capture_output=True,
            timeout=15,
            check=False,
        )
        if completed.returncode != 0:
            detail = completed.stderr.decode("utf-8", "replace").strip()[:300]
            raise VerificationError(f"OpenSSH signature verification failed: {detail}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("document", type=Path)
    parser.add_argument("--sha256", required=True)
    parser.add_argument("--signature", type=Path)
    parser.add_argument("--allowed-signers", type=Path)
    parser.add_argument("--evaluator")
    arguments = parser.parse_args()
    try:
        expected = _digest(arguments.sha256, "caller document digest")
        payload = _open_regular(arguments.document, MAX_DOCUMENT_BYTES, "document")
        observed = "sha256:" + hashlib.sha256(payload).hexdigest()
        if observed != expected:
            raise VerificationError("document digest differs from caller pin")
        try:
            document = json.loads(
                payload.decode("utf-8", "strict"),
                object_pairs_hook=_no_duplicates,
                parse_float=_reject_float,
                parse_constant=_reject_constant,
            )
        except VerificationError:
            raise
        except (UnicodeError, json.JSONDecodeError, RecursionError, ValueError) as exc:
            raise VerificationError(f"invalid preregistration JSON: {exc}") from exc
        authentication = any(
            value is not None
            for value in (arguments.signature, arguments.allowed_signers, arguments.evaluator)
        )
        if authentication and not all(
            value is not None
            for value in (arguments.signature, arguments.allowed_signers, arguments.evaluator)
        ):
            raise VerificationError(
                "signature, allowed signers, and evaluator are all required together"
            )
        result = validate(document, arguments.evaluator)
        result["document_sha256"] = observed
        result["authentication_established"] = False
        if authentication:
            signature = _open_regular(arguments.signature, MAX_SIGNATURE_BYTES, "signature")
            allowed = _open_regular(
                arguments.allowed_signers, MAX_SIGNERS_BYTES, "allowed signers"
            )
            verify_signature(payload, signature, allowed, arguments.evaluator)
            result["authentication_established"] = True
        print(json.dumps(result, sort_keys=True, separators=(",", ":")))
        return 0
    except (OSError, subprocess.SubprocessError, VerificationError) as exc:
        parser.exit(2, f"REJECTED: {exc}\n")


if __name__ == "__main__":
    raise SystemExit(main())

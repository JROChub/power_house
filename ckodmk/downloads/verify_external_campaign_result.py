#!/usr/bin/env python3
"""Recompute CKODMK external campaign outcomes from bounded per-case records."""

from __future__ import annotations

import argparse
import hashlib
import json
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

try:
    from verify_external_campaign_preregistration import (
        MAX_DOCUMENT_BYTES,
        VerificationError,
        _digest,
        _no_duplicates,
        _object,
        _open_regular,
        _reject_constant,
        _reject_float,
        _text,
        validate as validate_preregistration,
    )
except ModuleNotFoundError:  # imported as scripts.verify_external_campaign_result
    from scripts.verify_external_campaign_preregistration import (
        MAX_DOCUMENT_BYTES,
        VerificationError,
        _digest,
        _no_duplicates,
        _object,
        _open_regular,
        _reject_constant,
        _reject_float,
        _text,
        validate as validate_preregistration,
    )

SCHEMA = "mfenx/ckodmk-external-campaign-result/v1"
MAX_RESULT_BYTES = 32_000_000
MAX_CASES = 20_000
ZERO_DIGEST = "sha256:" + "0" * 64
SEED_DOMAIN = b"mfenx/ckodmk-external-campaign-seed/v1\0"
DECISIONS = {"PASS", "BLOCK", "INCONCLUSIVE", "CRASH", "TIMEOUT"}
FAULT_CLASSES = {"EFFECTIVE", "INVALID_MUTATION", "UNSUPPORTED", "EXCLUDED"}


def _integer(value: Any, label: str, minimum: int = 0) -> int:
    if type(value) is not int or not minimum <= value <= 10**18:
        raise VerificationError(f"{label} must be an in-range integer")
    return value


def _nonzero_digest(value: Any, label: str) -> str:
    digest = _digest(value, label)
    if digest == ZERO_DIGEST:
        raise VerificationError(f"{label} cannot be a placeholder")
    return digest


def _parse_json(payload: bytes, label: str) -> Any:
    try:
        return json.loads(
            payload.decode("utf-8", "strict"),
            object_pairs_hook=_no_duplicates,
            parse_float=_reject_float,
            parse_constant=_reject_constant,
        )
    except VerificationError:
        raise
    except (UnicodeError, json.JSONDecodeError, RecursionError, ValueError) as exc:
        raise VerificationError(f"invalid {label} JSON: {exc}") from exc


def _tool_result(
    value: Any, label: str, expected_budget: str, maximum_duration_ns: int
) -> dict[str, Any]:
    value = _object(
        value,
        {"decision", "duration_ns", "budget_profile_sha256", "evidence_sha256"},
        label,
    )
    decision = _text(value["decision"], f"{label}.decision", identifier=True)
    if decision not in DECISIONS:
        raise VerificationError(f"{label}.decision is unsupported")
    duration = _integer(value["duration_ns"], f"{label}.duration_ns", minimum=1)
    if duration > maximum_duration_ns:
        raise VerificationError(f"{label}.duration_ns exceeds the matched wall budget")
    if _digest(value["budget_profile_sha256"], f"{label}.budget_profile_sha256") != expected_budget:
        raise VerificationError(f"{label} uses a different budget profile")
    _nonzero_digest(value["evidence_sha256"], f"{label}.evidence_sha256")
    return value


def validate_result(
    result: Any,
    preregistration: dict[str, Any],
    preregistration_sha256: str,
) -> dict[str, Any]:
    result = _object(
        result,
        {
            "schema",
            "campaign_id",
            "preregistration_sha256",
            "terminal",
            "results_manifest_sha256",
            "seed_reveal",
            "cases",
            "limitations",
        },
        "result",
    )
    if result["schema"] != SCHEMA:
        raise VerificationError("unsupported campaign-result schema")
    if result["campaign_id"] != preregistration["campaign_id"]:
        raise VerificationError("result campaign identifier differs from preregistration")
    if _digest(result["preregistration_sha256"], "result.preregistration_sha256") != preregistration_sha256:
        raise VerificationError("result does not bind the caller-pinned preregistration")
    if result["terminal"] is not True:
        raise VerificationError("campaign result must be terminal")
    _nonzero_digest(result["results_manifest_sha256"], "result.results_manifest_sha256")

    reveal = _object(
        result["seed_reveal"],
        {"seed_hex", "revealed_after_results_manifest", "results_manifest_sha256"},
        "seed_reveal",
    )
    if reveal["revealed_after_results_manifest"] is not True:
        raise VerificationError("seed was not revealed after terminal result pinning")
    if reveal["results_manifest_sha256"] != result["results_manifest_sha256"]:
        raise VerificationError("seed reveal binds a different result manifest")
    if type(reveal["seed_hex"]) is not str or len(reveal["seed_hex"]) != 64:
        raise VerificationError("seed reveal must contain 32 lowercase hexadecimal bytes")
    try:
        seed = bytes.fromhex(reveal["seed_hex"])
    except ValueError as exc:
        raise VerificationError("seed reveal is not hexadecimal") from exc
    if reveal["seed_hex"] != seed.hex():
        raise VerificationError("seed reveal is not canonical lowercase hexadecimal")
    opened = "sha256:" + hashlib.sha256(SEED_DOMAIN + seed).hexdigest()
    if opened != preregistration["seed"]["commitment"]:
        raise VerificationError("seed reveal does not open the preregistered commitment")

    cases = result["cases"]
    if type(cases) is not list or not 1 <= len(cases) <= MAX_CASES:
        raise VerificationError("cases must be a bounded nonempty array")
    expected_budget = preregistration["execution_budget"]["profile_sha256"]
    maximum_duration_ns = (
        preregistration["execution_budget"]["per_tool_wall_seconds"] * 1_000_000_000
    )
    baseline_ids = {item["id"] for item in preregistration["baselines"]}
    fault_groups = {item["id"]: item for item in preregistration["fault_plan"]}
    control_groups = {item["id"]: item for item in preregistration["control_plan"]}
    seen: set[str] = set()
    fault_selected: Counter[str] = Counter()
    fault_effective: Counter[str] = Counter()
    fault_subtypes: dict[str, Counter[str]] = defaultdict(Counter)
    control_selected: Counter[str] = Counter()
    control_subtypes: dict[str, Counter[str]] = defaultdict(Counter)
    counts: Counter[str] = Counter()
    baseline_counts: dict[str, Counter[str]] = {name: Counter() for name in baseline_ids}

    fields = {
        "case_id",
        "selected_index",
        "kind",
        "group",
        "subtype",
        "classification",
        "ckodmk",
        "baselines",
        "case_evidence_sha256",
    }
    for index, raw in enumerate(cases):
        case = _object(raw, fields, f"cases[{index}]")
        case_id = _text(case["case_id"], f"cases[{index}].case_id", identifier=True)
        if case_id in seen:
            raise VerificationError("case identifiers must be unique")
        seen.add(case_id)
        if _integer(case["selected_index"], f"cases[{index}].selected_index") != index:
            raise VerificationError("selected case indices must be contiguous and ordered")
        kind = case["kind"]
        group = case["group"]
        subtype = case["subtype"]
        _text(group, f"cases[{index}].group", identifier=True)
        _text(subtype, f"cases[{index}].subtype", identifier=True)
        _nonzero_digest(case["case_evidence_sha256"], f"cases[{index}].case_evidence_sha256")
        ckodmk = _tool_result(
            case["ckodmk"],
            f"cases[{index}].ckodmk",
            expected_budget,
            maximum_duration_ns,
        )
        tools = case["baselines"]
        if type(tools) is not list or len(tools) != len(baseline_ids):
            raise VerificationError("each case must retain every preregistered baseline")
        observed_baselines: set[str] = set()
        for tool_index, tool_raw in enumerate(tools):
            tool = _object(tool_raw, {"id", "result"}, f"cases[{index}].baselines[{tool_index}]")
            tool_id = _text(tool["id"], f"cases[{index}].baselines[{tool_index}].id", identifier=True)
            if tool_id not in baseline_ids or tool_id in observed_baselines:
                raise VerificationError("case baseline set is missing, duplicated, or unknown")
            observed_baselines.add(tool_id)
            tool_result = _tool_result(
                tool["result"],
                f"cases[{index}].baselines[{tool_index}].result",
                expected_budget,
                maximum_duration_ns,
            )
            baseline_counts[tool_id][tool_result["decision"]] += 1
        if observed_baselines != baseline_ids:
            raise VerificationError("case baseline set differs from preregistration")

        decision = ckodmk["decision"]
        counts[f"all_decision:{decision}"] += 1
        if kind == "FAULT":
            if group not in fault_groups or subtype not in fault_groups[group]["subtype_targets"]:
                raise VerificationError("fault group or subtype was not preregistered")
            classification = case["classification"]
            if classification not in FAULT_CLASSES:
                raise VerificationError("fault classification is unsupported")
            fault_selected[group] += 1
            counts[f"fault_class:{classification}"] += 1
            if classification == "EFFECTIVE":
                fault_effective[group] += 1
                fault_subtypes[group][subtype] += 1
                if decision == "PASS":
                    counts["false_passes"] += 1
                elif decision == "BLOCK":
                    counts["safe_non_passes"] += 1
                else:
                    counts[decision.lower()] += 1
        elif kind == "CONTROL":
            if group not in control_groups or subtype not in control_groups[group]["subtype_targets"]:
                raise VerificationError("control group or subtype was not preregistered")
            if case["classification"] != "VALID_CONTROL":
                raise VerificationError("control case must be classified VALID_CONTROL")
            control_selected[group] += 1
            control_subtypes[group][subtype] += 1
            if decision == "PASS":
                counts["control_passes"] += 1
            elif decision == "BLOCK":
                counts["control_false_blocks"] += 1
            else:
                counts[f"control_{decision.lower()}"] += 1
        else:
            raise VerificationError("case kind must be FAULT or CONTROL")

    for group, plan in fault_groups.items():
        if fault_selected[group] > plan["attempt_ceiling"]:
            raise VerificationError(f"fault group {group} exceeded its attempt ceiling")
        if fault_effective[group] != plan["effective_target"]:
            raise VerificationError(f"fault group {group} did not reach its effective target")
        if dict(fault_subtypes[group]) != plan["subtype_targets"]:
            raise VerificationError(f"fault group {group} differs from subtype targets")
    for group, plan in control_groups.items():
        if control_selected[group] > plan["attempt_ceiling"]:
            raise VerificationError(f"control group {group} exceeded its attempt ceiling")
        if control_selected[group] != plan["effective_target"]:
            raise VerificationError(f"control group {group} did not reach its target")
        if dict(control_subtypes[group]) != plan["subtype_targets"]:
            raise VerificationError(f"control group {group} differs from subtype targets")

    effective = sum(fault_effective.values())
    controls = sum(control_selected.values())
    campaign_pass = all(
        (
            effective >= 1_000,
            controls >= 100,
            counts["false_passes"] == 0,
            counts["all_decision:CRASH"] == 0,
            counts["all_decision:TIMEOUT"] == 0,
            counts["control_passes"] == controls,
        )
    )
    limitations = result["limitations"]
    if type(limitations) is not list or len(limitations) > 256:
        raise VerificationError("limitations must be a bounded array")
    for index, item in enumerate(limitations):
        _text(item, f"limitations[{index}]")
    return {
        "baseline_decisions": {name: dict(sorted(value.items())) for name, value in sorted(baseline_counts.items())},
        "campaign_id": result["campaign_id"],
        "campaign_pass": campaign_pass,
        "control_cases": controls,
        "control_false_blocks": counts["control_false_blocks"],
        "control_passes": counts["control_passes"],
        "crashes": counts["all_decision:CRASH"],
        "effective_faults": effective,
        "excluded_cases": counts["fault_class:EXCLUDED"],
        "false_passes": counts["false_passes"],
        "inconclusive_effective_faults": counts["inconclusive"],
        "invalid_cases": counts["fault_class:INVALID_MUTATION"],
        "safe_non_passes": counts["safe_non_passes"],
        "selected_fault_attempts": sum(fault_selected.values()),
        "schema": "mfenx/ckodmk-external-campaign-result-verification/v1",
        "timeouts": counts["all_decision:TIMEOUT"],
        "unsupported_cases": counts["fault_class:UNSUPPORTED"],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("result", type=Path)
    parser.add_argument("--sha256", required=True)
    parser.add_argument("--preregistration", required=True, type=Path)
    parser.add_argument("--preregistration-sha256", required=True)
    arguments = parser.parse_args()
    try:
        expected_result = _digest(arguments.sha256, "caller result digest")
        expected_prereg = _digest(
            arguments.preregistration_sha256, "caller preregistration digest"
        )
        result_bytes = _open_regular(arguments.result, MAX_RESULT_BYTES, "result")
        prereg_bytes = _open_regular(
            arguments.preregistration, MAX_DOCUMENT_BYTES, "preregistration"
        )
        if "sha256:" + hashlib.sha256(result_bytes).hexdigest() != expected_result:
            raise VerificationError("result differs from caller digest")
        if "sha256:" + hashlib.sha256(prereg_bytes).hexdigest() != expected_prereg:
            raise VerificationError("preregistration differs from caller digest")
        preregistration = _parse_json(prereg_bytes, "preregistration")
        validate_preregistration(preregistration)
        result = _parse_json(result_bytes, "result")
        summary = validate_result(result, preregistration, expected_prereg)
        summary["result_sha256"] = expected_result
        summary["preregistration_sha256"] = expected_prereg
        print(json.dumps(summary, sort_keys=True, separators=(",", ":")))
        return 0
    except (OSError, VerificationError) as exc:
        parser.exit(2, f"REJECTED: {exc}\n")


if __name__ == "__main__":
    raise SystemExit(main())

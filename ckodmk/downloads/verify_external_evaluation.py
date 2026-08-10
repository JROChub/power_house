#!/usr/bin/env python3
"""Strict consumer for signed CKODMK external-evaluation assertions."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import subprocess
import tempfile
from decimal import Decimal, ROUND_CEILING, localcontext
from pathlib import Path
from typing import Any

SCHEMA = "mfenx/ckodmk-external-evaluation-return/v1"
NAMESPACE = "ckodmk-external-evaluation"
SSH_KEYGEN = Path("/usr/bin/ssh-keygen")
MAX_REPORT_BYTES = 2_000_000
MAX_SIGNATURE_BYTES = 32_768
MAX_SIGNERS_BYTES = 131_072
MAX_TEXT = 512
MAX_ITEMS = 10_000
DIGEST = re.compile(r"sha256:[0-9a-f]{64}\Z")
COMMIT = re.compile(r"[0-9a-f]{40}\Z")
IDENTIFIER = re.compile(r"[A-Za-z0-9][A-Za-z0-9_.:@/+\-]{0,255}\Z")
RFC3339 = re.compile(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z\Z")


class VerificationError(ValueError):
    pass


def _no_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise VerificationError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def _reject_float(value: str) -> Any:
    raise VerificationError(f"JSON floating-point token is forbidden: {value}")


def _reject_constant(value: str) -> Any:
    raise VerificationError(f"JSON constant is forbidden: {value}")


def _open_regular(path: Path, limit: int, label: str) -> bytes:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    flags |= getattr(os, "O_NONBLOCK", 0)
    descriptor = os.open(path, flags)
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            raise VerificationError(f"{label} is not a regular file")
        if before.st_size > limit:
            raise VerificationError(f"{label} exceeds {limit} bytes")
        data = bytearray()
        remaining = before.st_size
        while remaining:
            chunk = os.read(descriptor, min(remaining, 1_048_576))
            if not chunk:
                raise VerificationError(f"{label} changed while reading")
            data.extend(chunk)
            remaining -= len(chunk)
        if os.read(descriptor, 1):
            raise VerificationError(f"{label} grew while reading")
        after = os.fstat(descriptor)
        identity = lambda item: (item.st_dev, item.st_ino, item.st_size, item.st_mtime_ns)
        if identity(before) != identity(after):
            raise VerificationError(f"{label} changed while reading")
        return bytes(data)
    finally:
        os.close(descriptor)


def _exact_keys(value: Any, keys: set[str], label: str) -> dict[str, Any]:
    if type(value) is not dict:
        raise VerificationError(f"{label} must be an object")
    actual = set(value)
    if actual != keys:
        raise VerificationError(
            f"{label} fields differ: missing={sorted(keys-actual)}, unknown={sorted(actual-keys)}"
        )
    return value


def _text(value: Any, label: str, *, identifier: bool = False) -> str:
    if type(value) is not str or not 1 <= len(value) <= MAX_TEXT or not value.isascii():
        raise VerificationError(f"{label} must be bounded nonempty ASCII")
    if identifier and not IDENTIFIER.fullmatch(value):
        raise VerificationError(f"{label} is not a canonical identifier")
    return value


def _status(value: Any, label: str) -> str:
    value = _text(value, label, identifier=True)
    if value not in {"PASS", "FAIL", "NOT_EVALUATED"}:
        raise VerificationError(f"{label} has an unsupported status")
    return value


def _bool(value: Any, label: str) -> bool:
    if type(value) is not bool:
        raise VerificationError(f"{label} must be boolean")
    return value


def _count(value: Any, label: str, maximum: int = 1_000_000_000) -> int:
    if type(value) is not int or not 0 <= value <= maximum:
        raise VerificationError(f"{label} must be an in-range nonnegative integer")
    return value


def _digest(value: Any, label: str) -> str:
    if type(value) is not str or not DIGEST.fullmatch(value):
        raise VerificationError(f"{label} must be a canonical SHA-256 digest")
    return value


def _strings(value: Any, label: str, *, maximum: int = 256) -> list[str]:
    if type(value) is not list or len(value) > maximum:
        raise VerificationError(f"{label} must be a bounded array")
    result = [_text(item, f"{label}[{index}]") for index, item in enumerate(value)]
    if len(result) != len(set(result)):
        raise VerificationError(f"{label} contains duplicates")
    return result


def _validate_evaluator(value: Any, expected_identity: str) -> dict[str, Any]:
    value = _exact_keys(
        value,
        {
            "identity",
            "organization",
            "qualifications",
            "conflicts",
            "non_author",
            "controlled_execution",
        },
        "evaluator",
    )
    identity = _text(value["identity"], "evaluator.identity", identifier=True)
    if identity != expected_identity:
        raise VerificationError("report evaluator identity differs from caller expectation")
    _text(value["organization"], "evaluator.organization")
    if not _strings(value["qualifications"], "evaluator.qualifications"):
        raise VerificationError("evaluator qualifications cannot be empty")
    _strings(value["conflicts"], "evaluator.conflicts")
    _bool(value["non_author"], "evaluator.non_author")
    _bool(value["controlled_execution"], "evaluator.controlled_execution")
    return value


def _validate_release(value: Any) -> dict[str, Any]:
    value = _exact_keys(
        value,
        {"repository", "commit", "archive_sha256", "archive_manifest_sha256", "ci_run_url"},
        "release",
    )
    _text(value["repository"], "release.repository", identifier=True)
    if type(value["commit"]) is not str or not COMMIT.fullmatch(value["commit"]):
        raise VerificationError("release.commit must be 40 lowercase hexadecimal characters")
    _digest(value["archive_sha256"], "release.archive_sha256")
    _digest(value["archive_manifest_sha256"], "release.archive_manifest_sha256")
    url = _text(value["ci_run_url"], "release.ci_run_url")
    if not url.startswith("https://github.com/"):
        raise VerificationError("release.ci_run_url must be an HTTPS GitHub URL")
    return value


def _validate_execution(value: Any) -> dict[str, Any]:
    value = _exact_keys(
        value,
        {
            "started_at_utc",
            "ended_at_utc",
            "clean_environment",
            "private_guidance_used",
            "environment_sha256",
            "commands_sha256",
            "logs_sha256",
        },
        "execution",
    )
    for field in ("started_at_utc", "ended_at_utc"):
        if type(value[field]) is not str or not RFC3339.fullmatch(value[field]):
            raise VerificationError(f"execution.{field} must be canonical UTC seconds")
    if value["ended_at_utc"] < value["started_at_utc"]:
        raise VerificationError("execution end precedes start")
    _bool(value["clean_environment"], "execution.clean_environment")
    _bool(value["private_guidance_used"], "execution.private_guidance_used")
    for field in ("environment_sha256", "commands_sha256", "logs_sha256"):
        _digest(value[field], f"execution.{field}")
    return value


def _validate_audit(value: Any) -> dict[str, Any]:
    value = _exact_keys(
        value,
        {
            "status",
            "specification_agreement",
            "shared_implementation_authority_found",
            "open_critical_findings",
            "open_high_findings",
            "report_sha256",
        },
        "checker_audit",
    )
    _status(value["status"], "checker_audit.status")
    _status(value["specification_agreement"], "checker_audit.specification_agreement")
    _bool(
        value["shared_implementation_authority_found"],
        "checker_audit.shared_implementation_authority_found",
    )
    _count(value["open_critical_findings"], "checker_audit.open_critical_findings")
    _count(value["open_high_findings"], "checker_audit.open_high_findings")
    _digest(value["report_sha256"], "checker_audit.report_sha256")
    return value


def _false_pass_upper_ppm(effective: int, false_passes: int) -> int | None:
    if effective == 0 or false_passes != 0:
        return None
    with localcontext() as context:
        context.prec = 80
        n = Decimal(effective)
        bound = Decimal(1) - (Decimal("0.05").ln() / n).exp()
        return int((bound * Decimal(1_000_000)).to_integral_value(rounding=ROUND_CEILING))


def _validate_campaign(value: Any) -> dict[str, Any]:
    value = _exact_keys(
        value,
        {
            "status",
            "preregistered",
            "non_author_controlled",
            "selected_cases",
            "effective_cases",
            "invalid_cases",
            "excluded_cases",
            "safe_non_passes",
            "false_passes",
            "inconclusive",
            "crashes",
            "timeouts",
            "control_cases",
            "control_passes",
            "control_false_blocks",
            "control_inconclusive",
            "control_crashes",
            "control_timeouts",
            "zero_false_pass_upper_95_ppm",
            "raw_evidence_sha256",
        },
        "hostile_campaign",
    )
    status = _status(value["status"], "hostile_campaign.status")
    _bool(value["preregistered"], "hostile_campaign.preregistered")
    _bool(value["non_author_controlled"], "hostile_campaign.non_author_controlled")
    fields = (
        "selected_cases",
        "effective_cases",
        "invalid_cases",
        "excluded_cases",
        "safe_non_passes",
        "false_passes",
        "inconclusive",
        "crashes",
        "timeouts",
        "control_cases",
        "control_passes",
        "control_false_blocks",
        "control_inconclusive",
        "control_crashes",
        "control_timeouts",
    )
    for field in fields:
        _count(value[field], f"hostile_campaign.{field}")
    accounted = (
        value["effective_cases"] + value["invalid_cases"] + value["excluded_cases"]
    )
    if accounted != value["selected_cases"]:
        raise VerificationError("hostile campaign denominator accounting is inconsistent")
    target_outcomes = sum(
        value[field]
        for field in (
            "safe_non_passes",
            "false_passes",
            "inconclusive",
            "crashes",
            "timeouts",
        )
    )
    if target_outcomes != value["effective_cases"]:
        raise VerificationError("hostile campaign target outcomes do not exhaust effective cases")
    control_outcomes = sum(
        value[field]
        for field in (
            "control_passes",
            "control_false_blocks",
            "control_inconclusive",
            "control_crashes",
            "control_timeouts",
        )
    )
    if control_outcomes != value["control_cases"]:
        raise VerificationError("hostile campaign control outcomes do not exhaust controls")
    expected_bound = _false_pass_upper_ppm(value["effective_cases"], value["false_passes"])
    bound = value["zero_false_pass_upper_95_ppm"]
    if expected_bound is None:
        if bound is not None:
            raise VerificationError("zero-false-pass bound must be null for this campaign")
    elif type(bound) is not int or bound != expected_bound:
        raise VerificationError("zero-false-pass confidence bound is incorrect")
    _digest(value["raw_evidence_sha256"], "hostile_campaign.raw_evidence_sha256")
    if status == "NOT_EVALUATED" and value["selected_cases"] != 0:
        raise VerificationError("a not-evaluated campaign must have an empty denominator")
    return value


def _validate_targets(value: Any) -> list[dict[str, Any]]:
    if type(value) is not list or len(value) > 32:
        raise VerificationError("physical_targets must be a bounded array")
    result = []
    seen: set[str] = set()
    keys = {
        "target_id",
        "retail_model",
        "os_version",
        "runtime_version",
        "evaluator_controlled",
        "status",
        "profile_sha256",
        "measurements_sha256",
        "report_sha256",
    }
    for index, item in enumerate(value):
        item = _exact_keys(item, keys, f"physical_targets[{index}]")
        target_id = _text(
            item["target_id"],
            f"physical_targets[{index}].target_id",
            identifier=True,
        )
        if target_id in seen:
            raise VerificationError("physical target identifiers must be unique")
        seen.add(target_id)
        for field in ("retail_model", "os_version", "runtime_version"):
            _text(item[field], f"physical_targets[{index}].{field}")
        _bool(item["evaluator_controlled"], f"physical_targets[{index}].evaluator_controlled")
        _status(item["status"], f"physical_targets[{index}].status")
        for field in ("profile_sha256", "measurements_sha256", "report_sha256"):
            _digest(item[field], f"physical_targets[{index}].{field}")
        result.append(item)
    return result


def _validate_named_records(value: Any, label: str, *, use: bool = False) -> list[dict[str, Any]]:
    if type(value) is not list or len(value) > 256:
        raise VerificationError(f"{label} must be a bounded array")
    base = {"id", "status", "evidence_sha256"}
    extra = (
        {"matched_budget", "common_fault_set"}
        if not use
        else {
            "real_release_decision",
            "integration_hours",
            "false_alarms",
            "retention_intent",
        }
    )
    seen: set[str] = set()
    result = []
    for index, item in enumerate(value):
        item = _exact_keys(item, base | extra, f"{label}[{index}]")
        identifier = _text(item["id"], f"{label}[{index}].id", identifier=True)
        if identifier in seen:
            raise VerificationError(f"{label} identifiers must be unique")
        seen.add(identifier)
        _status(item["status"], f"{label}[{index}].status")
        _digest(item["evidence_sha256"], f"{label}[{index}].evidence_sha256")
        if use:
            _bool(item["real_release_decision"], f"{label}[{index}].real_release_decision")
            _count(item["integration_hours"], f"{label}[{index}].integration_hours", 1_000_000)
            _count(item["false_alarms"], f"{label}[{index}].false_alarms", 1_000_000)
            _text(item["retention_intent"], f"{label}[{index}].retention_intent")
        else:
            _bool(item["matched_budget"], f"{label}[{index}].matched_budget")
            _bool(item["common_fault_set"], f"{label}[{index}].common_fault_set")
        result.append(item)
    return result


def _validate_findings(value: Any) -> list[dict[str, Any]]:
    if type(value) is not list or len(value) > MAX_ITEMS:
        raise VerificationError("findings must be a bounded array")
    result = []
    seen: set[str] = set()
    keys = {"id", "severity", "status", "summary", "evidence_sha256"}
    for index, item in enumerate(value):
        item = _exact_keys(item, keys, f"findings[{index}]")
        identifier = _text(item["id"], f"findings[{index}].id", identifier=True)
        if identifier in seen:
            raise VerificationError("finding identifiers must be unique")
        seen.add(identifier)
        severity = _text(item["severity"], f"findings[{index}].severity", identifier=True)
        if severity not in {"CRITICAL", "HIGH", "MEDIUM", "LOW", "INFORMATIONAL"}:
            raise VerificationError("finding severity is unsupported")
        finding_status = _text(item["status"], f"findings[{index}].status", identifier=True)
        if finding_status not in {"OPEN", "RESOLVED", "ACCEPTED_LIMITATION"}:
            raise VerificationError("finding status is unsupported")
        _text(item["summary"], f"findings[{index}].summary")
        _digest(item["evidence_sha256"], f"findings[{index}].evidence_sha256")
        result.append(item)
    return result


def _validate_assessment(value: Any) -> dict[str, Any]:
    value = _exact_keys(
        value,
        {
            "assertion",
            "score",
            "hard_gates_passed",
            "category_minimums_satisfied",
            "worksheet_sha256",
        },
        "evaluator_assessment",
    )
    assertion = _text(value["assertion"], "evaluator_assessment.assertion", identifier=True)
    if assertion not in {"QUALIFIES_ABOVE_9", "DOES_NOT_QUALIFY", "NOT_EVALUATED"}:
        raise VerificationError("unsupported evaluator qualification assertion")
    score = value["score"]
    if score is not None and (type(score) is not int or not 0 <= score <= 100):
        raise VerificationError("evaluator score must be null or an integer from 0 to 100")
    _count(value["hard_gates_passed"], "evaluator_assessment.hard_gates_passed", 10)
    _bool(value["category_minimums_satisfied"], "evaluator_assessment.category_minimums_satisfied")
    _digest(value["worksheet_sha256"], "evaluator_assessment.worksheet_sha256")
    if assertion == "QUALIFIES_ABOVE_9" and score is None:
        raise VerificationError("a qualification assertion requires a score")
    return value


def validate_report(report: Any, expected_identity: str) -> dict[str, Any]:
    report = _exact_keys(
        report,
        {
            "schema",
            "evaluator",
            "release",
            "execution",
            "checker_audit",
            "hostile_campaign",
            "physical_targets",
            "baselines",
            "reproduction",
            "consequential_uses",
            "findings",
            "evaluator_assessment",
            "limitations",
        },
        "report",
    )
    if report["schema"] != SCHEMA:
        raise VerificationError("unsupported external-evaluation schema")
    evaluator = _validate_evaluator(report["evaluator"], expected_identity)
    _validate_release(report["release"])
    execution = _validate_execution(report["execution"])
    audit = _validate_audit(report["checker_audit"])
    campaign = _validate_campaign(report["hostile_campaign"])
    targets = _validate_targets(report["physical_targets"])
    baselines = _validate_named_records(report["baselines"], "baselines")
    reproduction = _status(report["reproduction"], "reproduction")
    uses = _validate_named_records(report["consequential_uses"], "consequential_uses", use=True)
    findings = _validate_findings(report["findings"])
    assessment = _validate_assessment(report["evaluator_assessment"])
    _strings(report["limitations"], "limitations", maximum=512)

    required_baselines = {"hash-only", "differential-testing", "strongest-applicable-validator"}
    baseline_pass = required_baselines.issubset(
        {item["id"] for item in baselines}
    ) and all(
        item["status"] == "PASS"
        and item["matched_budget"]
        and item["common_fault_set"]
        for item in baselines
    )
    target_pass = len(targets) >= 2 and all(
        item["status"] == "PASS" and item["evaluator_controlled"]
        for item in targets
    )
    use_pass = len(uses) >= 2 and all(
        item["status"] == "PASS" and item["real_release_decision"] for item in uses
    )
    open_severe = any(
        item["severity"] in {"CRITICAL", "HIGH"} and item["status"] == "OPEN"
        for item in findings
    )
    minimum = all(
        (
            evaluator["non_author"],
            evaluator["controlled_execution"],
            execution["clean_environment"],
            not execution["private_guidance_used"],
            audit["status"] == "PASS",
            audit["specification_agreement"] == "PASS",
            not audit["shared_implementation_authority_found"],
            audit["open_critical_findings"] == 0,
            audit["open_high_findings"] == 0,
            campaign["status"] == "PASS",
            campaign["preregistered"],
            campaign["non_author_controlled"],
            campaign["effective_cases"] >= 1_000,
            campaign["false_passes"] == 0,
            campaign["crashes"] == 0,
            campaign["timeouts"] == 0,
            campaign["control_cases"] >= 100,
            campaign["control_passes"] == campaign["control_cases"],
            campaign["control_false_blocks"] == 0,
            campaign["control_inconclusive"] == 0,
            campaign["control_crashes"] == 0,
            campaign["control_timeouts"] == 0,
            target_pass,
            baseline_pass,
            reproduction == "PASS",
            use_pass,
            not open_severe,
            assessment["score"] is not None and assessment["score"] >= 91,
            assessment["hard_gates_passed"] == 10,
            assessment["category_minimums_satisfied"],
            assessment["assertion"] == "QUALIFIES_ABOVE_9",
        )
    )
    return {"minimum_profile_satisfied": minimum, "report": report}


def verify_signature(report: bytes, signature: bytes, allowed: bytes, identity: str) -> None:
    if not SSH_KEYGEN.is_file():
        raise VerificationError(
            "trusted OpenSSH verifier is unavailable at /usr/bin/ssh-keygen"
        )
    with tempfile.TemporaryDirectory(prefix="ckodmk-external-verify-") as temporary:
        directory = Path(temporary)
        signature_path = directory / "report.sig"
        allowed_path = directory / "allowed_signers"
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
            input=report,
            capture_output=True,
            timeout=15,
            check=False,
        )
        if completed.returncode != 0:
            detail = completed.stderr.decode("utf-8", "replace").strip()[:300]
            raise VerificationError(f"OpenSSH signature verification failed: {detail}")


def verify(arguments: argparse.Namespace) -> dict[str, Any]:
    expected = _digest(arguments.sha256, "caller report digest")
    identity = _text(arguments.evaluator, "caller evaluator identity", identifier=True)
    report_bytes = _open_regular(arguments.report, MAX_REPORT_BYTES, "report")
    actual = "sha256:" + hashlib.sha256(report_bytes).hexdigest()
    if actual != expected:
        raise VerificationError("report digest does not match caller pin")
    signature = _open_regular(arguments.signature, MAX_SIGNATURE_BYTES, "signature")
    allowed = _open_regular(arguments.allowed_signers, MAX_SIGNERS_BYTES, "allowed signers")
    try:
        report = json.loads(
            report_bytes.decode("utf-8", "strict"),
            object_pairs_hook=_no_duplicates,
            parse_float=_reject_float,
            parse_constant=_reject_constant,
        )
    except VerificationError:
        raise
    except (UnicodeError, json.JSONDecodeError, RecursionError, ValueError) as exc:
        raise VerificationError(f"invalid report JSON: {exc}") from exc
    validated = validate_report(report, identity)
    verify_signature(report_bytes, signature, allowed, identity)
    return {
        "authentication_established": True,
        "decision": "SIGNED_EVALUATOR_ASSERTION_VERIFIED",
        "evaluator": identity,
        "minimum_profile_satisfied": validated["minimum_profile_satisfied"],
        "qualification_assertion": report["evaluator_assessment"]["assertion"],
        "qualification_evidence_reexecuted": False,
        "report_sha256": actual,
        "schema": "mfenx/ckodmk-external-evaluation-verification/v1",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    parser.add_argument("--sha256", required=True)
    parser.add_argument("--signature", required=True, type=Path)
    parser.add_argument("--allowed-signers", required=True, type=Path)
    parser.add_argument("--evaluator", required=True)
    arguments = parser.parse_args()
    try:
        result = verify(arguments)
    except (OSError, subprocess.SubprocessError, VerificationError) as exc:
        parser.exit(2, f"REJECTED: {exc}\n")
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

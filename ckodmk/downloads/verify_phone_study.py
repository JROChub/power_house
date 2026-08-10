#!/usr/bin/env python3
"""Strict structural and arithmetic verifier for a CKODMK phone-study report."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import stat
import unicodedata
from datetime import datetime
from pathlib import Path
from typing import Any


MAX_REPORT_BYTES = 1_000_000
SHA256 = re.compile(r"sha256:[0-9a-f]{64}\Z")
NONCE = re.compile(r"[0-9a-f]{64}\Z")
UTC_TIMESTAMP = re.compile(r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]{3}Z\Z")
RELATIONSHIPS = {"independent-external", "mfenx-author", "other"}


class PhoneReportError(ValueError):
    """The report is outside the strict phone-study v2 profile."""


def _duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise PhoneReportError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def _reject_float(_: str) -> None:
    raise PhoneReportError("floating point JSON values are forbidden")


def _reject_constant(_: str) -> None:
    raise PhoneReportError("nonfinite JSON values are forbidden")


def _parse_int(value: str) -> int:
    if len(value) > 20:
        raise PhoneReportError("JSON integer exceeds the report profile")
    return int(value)


def _read_regular(path: Path) -> bytes:
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0)
    flags |= getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0)
    descriptor = os.open(path, flags)
    try:
        before = os.fstat(descriptor)
        if not stat.S_ISREG(before.st_mode):
            raise PhoneReportError("report must be a regular non-symlink file")
        if not 1 <= before.st_size <= MAX_REPORT_BYTES:
            raise PhoneReportError("report size is outside the accepted range")
        payload = b""
        while len(payload) < before.st_size:
            chunk = os.read(descriptor, min(65_536, before.st_size - len(payload)))
            if not chunk:
                raise PhoneReportError("report changed while reading")
            payload += chunk
        if os.read(descriptor, 1):
            raise PhoneReportError("report grew while reading")
        after = os.fstat(descriptor)
        if (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns) != (
            after.st_dev,
            after.st_ino,
            after.st_size,
            after.st_mtime_ns,
        ):
            raise PhoneReportError("report changed while reading")
        return payload
    finally:
        os.close(descriptor)


def _load(path: Path) -> tuple[dict[str, Any], bytes]:
    payload = _read_regular(path)
    try:
        value = json.loads(
            payload.decode("utf-8", errors="strict"),
            object_pairs_hook=_duplicates,
            parse_int=_parse_int,
            parse_float=_reject_float,
            parse_constant=_reject_constant,
        )
    except PhoneReportError:
        raise
    except (UnicodeError, json.JSONDecodeError, RecursionError, ValueError) as error:
        raise PhoneReportError("report is not strict UTF-8 JSON") from error
    if type(value) is not dict:
        raise PhoneReportError("report root must be an object")
    return value, payload


def _keys(value: Any, expected: set[str], label: str) -> dict[str, Any]:
    if type(value) is not dict or set(value) != expected:
        raise PhoneReportError(f"{label} fields do not match the profile")
    return value


def _integer(value: Any, minimum: int, maximum: int, label: str) -> int:
    if type(value) is not int or not minimum <= value <= maximum:
        raise PhoneReportError(f"{label} is outside the accepted range")
    return value


def _digest(value: Any, label: str) -> str:
    if type(value) is not str or SHA256.fullmatch(value) is None:
        raise PhoneReportError(f"{label} is not canonical SHA-256")
    return value


def _text(value: Any, label: str) -> str:
    if type(value) is not str or value != value.strip() or value != unicodedata.normalize("NFC", value):
        raise PhoneReportError(f"{label} is not normalized text")
    if not 2 <= len(value) <= 96 or any(ord(character) < 32 or ord(character) == 127 for character in value):
        raise PhoneReportError(f"{label} violates the text profile")
    return value


def _percentile(values: list[int], numerator: int, denominator: int) -> int:
    ordered = sorted(values)
    return ordered[(len(ordered) * numerator + denominator - 1) // denominator - 1]


def verify_report(value: dict[str, Any], payload: bytes = b"") -> dict[str, Any]:
    report = _keys(
        value,
        {
            "artifacts",
            "authentication",
            "contract",
            "created_at",
            "execution",
            "interpretation",
            "observations",
            "privacy",
            "schema",
            "session_nonce",
            "target_declaration",
        },
        "report",
    )
    if report["schema"] != "mfenx/ckodmk-browser-phone-study/v2" or report["authentication"] != "none":
        raise PhoneReportError("report schema or authentication is unsupported")
    if type(report["created_at"]) is not str or UTC_TIMESTAMP.fullmatch(report["created_at"]) is None:
        raise PhoneReportError("creation time is not canonical UTC")
    try:
        datetime.fromisoformat(report["created_at"].removesuffix("Z") + "+00:00")
    except ValueError as error:
        raise PhoneReportError("creation time is invalid") from error
    if type(report["session_nonce"]) is not str or NONCE.fullmatch(report["session_nonce"]) is None:
        raise PhoneReportError("session nonce is not 256-bit lowercase hexadecimal")

    privacy = _keys(report["privacy"], {"automatic_device_identifiers_collected", "files_uploaded", "network_result_submission"}, "privacy")
    if privacy["automatic_device_identifiers_collected"] != [] or privacy["files_uploaded"] is not False or privacy["network_result_submission"] is not False:
        raise PhoneReportError("privacy record disagrees with the profile")

    declaration = report["target_declaration"]
    target_status = "ABSENT"
    relationship = None
    if declaration is not None:
        declaration = _keys(declaration, {"basis", "browser", "consent", "evaluator_relationship", "operating_system", "target_model"}, "target declaration")
        if declaration["basis"] != "evaluator-entered; not automatically detected or attested" or declaration["consent"] != "include in this local report only":
            raise PhoneReportError("target declaration basis or consent is invalid")
        _text(declaration["target_model"], "target model")
        _text(declaration["operating_system"], "operating system")
        _text(declaration["browser"], "browser")
        relationship = declaration["evaluator_relationship"]
        if type(relationship) is not str or relationship not in RELATIONSHIPS:
            raise PhoneReportError("evaluator relationship is invalid")
        target_status = "SELF_DECLARED_UNATTESTED"

    execution = _keys(report["execution"], {"backend", "clock", "graph_optimization", "inner_repetitions", "measured_pairs", "order", "profile", "runtime", "runtime_version", "threads", "warmup_pairs"}, "execution")
    expected_execution = {
        "profile": "onnxruntime-web-wasm-f32-batch1-paired-timing/v1",
        "runtime": "onnxruntime-web",
        "runtime_version": "1.27.0",
        "backend": "wasm",
        "threads": 1,
        "graph_optimization": "disabled",
        "warmup_pairs": 12,
        "measured_pairs": 60,
        "inner_repetitions": 128,
        "order": "alternating",
        "clock": "performance.now group duration divided by inner repetitions and converted to integer nanoseconds",
    }
    for name, expected in expected_execution.items():
        observed = execution[name]
        if type(observed) is not type(expected) or observed != expected:
            raise PhoneReportError("execution record disagrees with the profile")

    artifacts = _keys(report["artifacts"], {"candidate_sha256", "contract_sha256", "dataset_sha256", "source_sha256"}, "artifacts")
    for name, digest in artifacts.items():
        _digest(digest, name)
    contract = _keys(report["contract"], {"class_count", "id", "samples"}, "contract")
    if type(contract["id"]) is not str or not 1 <= len(contract["id"]) <= 128:
        raise PhoneReportError("contract identifier is invalid")
    samples = _integer(contract["samples"], 1, 25_000, "sample count")
    _integer(contract["class_count"], 2, 1_024, "class count")

    observations = _keys(report["observations"], {"candidate_p50_ns", "candidate_p95_ns", "pairs", "source_p50_ns", "source_p95_ns"}, "observations")
    pairs = observations["pairs"]
    if type(pairs) is not list or len(pairs) != 60:
        raise PhoneReportError("timing report must contain exactly 60 pairs")
    source_values: list[int] = []
    candidate_values: list[int] = []
    for index, raw_pair in enumerate(pairs):
        pair = _keys(raw_pair, {"candidate_ns", "order", "sample_index", "source_ns"}, f"pair {index}")
        expected_order = "source-candidate" if index % 2 == 0 else "candidate-source"
        sample_index = _integer(pair["sample_index"], 0, samples - 1, f"pair {index} sample index")
        if type(pair["order"]) is not str or pair["order"] != expected_order or sample_index != (index * 977) % samples:
            raise PhoneReportError(f"pair {index} schedule disagrees with the profile")
        source_values.append(_integer(pair["source_ns"], 1, 60_000_000_000, f"pair {index} source timing"))
        candidate_values.append(_integer(pair["candidate_ns"], 1, 60_000_000_000, f"pair {index} candidate timing"))
    expected_summaries = {
        "source_p50_ns": _percentile(source_values, 1, 2),
        "source_p95_ns": _percentile(source_values, 95, 100),
        "candidate_p50_ns": _percentile(candidate_values, 1, 2),
        "candidate_p95_ns": _percentile(candidate_values, 95, 100),
    }
    for name, expected in expected_summaries.items():
        observed = _integer(observations[name], 1, 60_000_000_000, name)
        if observed != expected:
            raise PhoneReportError(f"{name} does not match the raw timing series")
    if report["interpretation"] != "finite browser timing observations only":
        raise PhoneReportError("interpretation disagrees with the profile")
    return {
        "authentication_established": False,
        "decision": "STRUCTURE_AND_ARITHMETIC_VERIFIED",
        "evaluator_relationship": relationship,
        "physical_target_attested": False,
        "report_sha256": "sha256:" + hashlib.sha256(payload).hexdigest() if payload else None,
        "target_record_status": target_status,
        "timing_authenticity_established": False,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    parser.add_argument("--sha256", required=True)
    arguments = parser.parse_args()
    if SHA256.fullmatch(arguments.sha256) is None:
        raise PhoneReportError("caller report digest must be canonical SHA-256")
    value, payload = _load(arguments.report)
    observed = "sha256:" + hashlib.sha256(payload).hexdigest()
    if observed != arguments.sha256:
        raise PhoneReportError("report digest does not match the caller pin")
    print(json.dumps(verify_report(value, payload), sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Create and validate fail-closed cross-host reproduction evidence."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import re
import socket
import sys
import tarfile
from datetime import datetime, timezone
from typing import Any, Iterable


SCHEMA = "mfenx.step3-reproduction.v1"
EXPECTED_SLOTS = ("host-1", "host-2", "host-3")
IDENTITY_SCHEMA = "mfenx.step3-candidate-identity.v1"
HEX64 = re.compile(r"^[0-9a-f]{64}$")


class EvidenceError(RuntimeError):
    """An evidence invariant was not satisfied."""


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode("ascii")


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def load_json(path: pathlib.Path) -> Any:
    with path.open("r", encoding="utf-8") as stream:
        return json.load(stream)


def load_expected_identity(path: pathlib.Path) -> dict[str, str]:
    if path.is_symlink() or not path.is_file():
        raise EvidenceError("candidate identity is missing, non-regular, or symlinked")
    document = load_json(path)
    if not isinstance(document, dict) or document.get("schema") != IDENTITY_SCHEMA:
        raise EvidenceError("candidate identity schema is not supported")
    if document.get("enabled") is not True:
        raise EvidenceError("candidate identity claim gate is disabled")
    try:
        expected = {
            "release_id": document["release_id"],
            "identity_sha256": sha256_file(path),
            "archive_sha256": document["archive"]["sha256"],
            "manifest_sha256": document["manifest"]["sha256"],
            "signature_sha256": document["signature"]["sha256"],
            "allowed_signers_sha256": document["allowed_signers"]["sha256"],
            "key_fingerprint": document["allowed_signers"]["key_fingerprint"],
            "executor_sha256": document["workload"]["executor_sha256"],
            "verifier_sha256": document["workload"]["verifier_sha256"],
            "output_root": document["workload"]["canonical_output_root"],
        }
    except (KeyError, TypeError) as error:
        raise EvidenceError(f"candidate identity lacks a required field: {error}") from error
    if document.get("signature", {}).get("principal") != "mfenx-release":
        raise EvidenceError("candidate identity uses the wrong signing principal")
    if document.get("signature", {}).get("namespace") != "mfenx-validation-candidate":
        raise EvidenceError("candidate identity uses the wrong signing namespace")
    for key in (
        "archive_sha256",
        "manifest_sha256",
        "signature_sha256",
        "allowed_signers_sha256",
        "executor_sha256",
        "verifier_sha256",
        "output_root",
    ):
        if not isinstance(expected[key], str) or not HEX64.fullmatch(expected[key]):
            raise EvidenceError(f"candidate identity {key} is not a lowercase SHA-256")
    if not isinstance(expected["release_id"], str) or expected["release_id"].startswith("__"):
        raise EvidenceError("candidate release_id is not frozen")
    if not isinstance(expected["key_fingerprint"], str) or not expected["key_fingerprint"].startswith("SHA256:"):
        raise EvidenceError("candidate release key fingerprint is not frozen")
    return expected


def write_new_json(path: pathlib.Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    encoded = json.dumps(value, indent=2, sort_keys=True, ensure_ascii=True) + "\n"
    try:
        with path.open("x", encoding="ascii", newline="\n") as stream:
            stream.write(encoded)
    except FileExistsError as error:
        raise EvidenceError(f"refusing to overwrite {path}") from error


def safe_relative(path: pathlib.PurePosixPath) -> bool:
    return bool(path.parts) and not path.is_absolute() and all(part not in ("", ".", "..") for part in path.parts)


def manifest_lines(root: pathlib.Path, excluded: Iterable[str] = ()) -> list[str]:
    excluded_set = set(excluded)
    lines: list[str] = []
    for path in sorted(root.rglob("*")):
        if path.is_symlink():
            raise EvidenceError(f"symlink is not allowed in evidence: {path}")
        if not path.is_file() and not path.is_dir():
            raise EvidenceError(f"special filesystem object is not allowed in evidence: {path}")
        if not path.is_file():
            continue
        relative = path.relative_to(root).as_posix()
        if relative in excluded_set:
            continue
        if not safe_relative(pathlib.PurePosixPath(relative)) or "\n" in relative or "  " in relative:
            raise EvidenceError(f"unsafe evidence path: {relative!r}")
        lines.append(f"{sha256_file(path)}  {relative}\n")
    return lines


def seal_directory(root: pathlib.Path) -> str:
    if root.is_symlink() or not root.is_dir():
        raise EvidenceError(f"evidence root is not a real directory: {root}")
    manifest = root / "SHA256SUMS"
    if manifest.exists() or manifest.is_symlink():
        raise EvidenceError(f"checksum manifest already exists: {manifest}")
    lines = manifest_lines(root)
    with manifest.open("x", encoding="ascii", newline="\n") as stream:
        stream.writelines(lines)
    return sha256_file(manifest)


def read_machine_component(path: pathlib.Path, fallback: str) -> str:
    try:
        value = path.read_text(encoding="ascii").strip()
    except (OSError, UnicodeError):
        value = fallback
    return value


def find_job(job_api: Any, expected_name: str) -> dict[str, Any]:
    jobs = job_api.get("jobs") if isinstance(job_api, dict) else None
    if not isinstance(jobs, list):
        raise EvidenceError("workflow-jobs response has no jobs array")
    matches = [job for job in jobs if isinstance(job, dict) and job.get("name") == expected_name]
    if len(matches) != 1:
        raise EvidenceError(f"expected one workflow job named {expected_name!r}, found {len(matches)}")
    return matches[0]


def capture_success(
    capture: pathlib.Path, expected_identity: dict[str, str]
) -> tuple[bool, list[str], dict[str, Any] | None]:
    failures: list[str] = []
    record_path = capture / "record.json"
    manifest_path = capture / "SHA256SUMS"
    if capture.is_symlink() or not capture.is_dir():
        return False, ["reproduction capture directory is missing"], None
    if not record_path.is_file() or record_path.is_symlink():
        failures.append("capture record.json is missing or symlinked")
    if not manifest_path.is_file() or manifest_path.is_symlink():
        failures.append("capture SHA256SUMS is missing or symlinked")
    if failures:
        return False, failures, None
    record = load_json(record_path)
    if not isinstance(record, dict):
        return False, failures + ["capture record.json is not a JSON object"], None
    if record.get("schema") != "mfenx.step3-hosted-release-run.v1" or record.get("status") != "PASS":
        failures.append("hosted release/workload reproduction did not pass")
    signed_input = record.get("signed_input", {})
    for record_key, expected_key in (
        ("release_id", "release_id"),
        ("identity_sha256", "identity_sha256"),
        ("archive_sha256", "archive_sha256"),
        ("candidate_manifest_sha256", "manifest_sha256"),
        ("candidate_signature_sha256", "signature_sha256"),
        ("allowed_signers_sha256", "allowed_signers_sha256"),
        ("release_key_fingerprint", "key_fingerprint"),
    ):
        if signed_input.get(record_key) != expected_identity[expected_key]:
            failures.append(f"signed input {record_key} differs from the frozen identity")
    if signed_input.get("candidate_signature_verified") is not True:
        failures.append("candidate signature was not verified")
    if signed_input.get("archive_inventory_verified") is not True:
        failures.append("candidate archive inventory was not verified")
    execution = record.get("execution", {})
    if execution.get("executor_sha256") != expected_identity["executor_sha256"]:
        failures.append("executor identity differs from the frozen signed archive")
    if execution.get("verifier_sha256") != expected_identity["verifier_sha256"]:
        failures.append("reference-verifier identity differs from the frozen signed archive")
    if execution.get("output_root") != expected_identity["output_root"]:
        failures.append("canonical workload output root differs from the frozen oracle")
    if execution.get("executor_exact_replay_accepted") is not True:
        failures.append("executor exact replay did not accept")
    if execution.get("standalone_reference_verifier_accepted") is not True:
        failures.append("standalone reference verifier did not accept")
    scope = record.get("claim_scope", {})
    if scope.get("release_workload_reproduction") is not True:
        failures.append("capture does not establish its bounded release/workload reproduction")
    if scope.get("reproducible_binary_build") is not False:
        failures.append("capture improperly claims a reproducible binary build")
    expected: dict[str, str] = {}
    for line_number, line in enumerate(manifest_path.read_text(encoding="ascii").splitlines(), 1):
        parts = line.split("  ", 1)
        if len(parts) != 2 or not HEX64.fullmatch(parts[0]):
            failures.append(f"invalid capture checksum line {line_number}")
            continue
        relative = pathlib.PurePosixPath(parts[1])
        if not safe_relative(relative) or parts[1] in expected:
            failures.append(f"unsafe or duplicate capture checksum path at line {line_number}")
            continue
        expected[parts[1]] = parts[0]
    actual_files = {
        path.relative_to(capture).as_posix(): path
        for path in capture.rglob("*")
        if path.is_file() and not path.is_symlink() and path != manifest_path
    }
    symlinks = [path for path in capture.rglob("*") if path.is_symlink()]
    if symlinks:
        failures.append("capture contains symlinks")
    if set(expected) != set(actual_files):
        failures.append("capture checksum inventory is not the exact file set")
    else:
        for relative, digest in expected.items():
            if sha256_file(actual_files[relative]) != digest:
                failures.append(f"capture checksum mismatch: {relative}")
                break
    return not failures, failures, record


def host_record(args: argparse.Namespace) -> int:
    output = args.output
    if output.exists() or output.is_symlink():
        raise EvidenceError(f"refusing to overwrite {output}")

    expected_identity = load_expected_identity(args.identity)
    failures: list[str] = []
    capture_ok, capture_failures, capture = capture_success(args.capture, expected_identity)
    failures.extend(capture_failures)
    try:
        job = find_job(load_json(args.job_api_json), args.job_name)
    except (EvidenceError, OSError, ValueError, json.JSONDecodeError) as error:
        job = {}
        failures.append(f"workflow job identity unavailable: {error}")

    runner_id = job.get("runner_id")
    job_id = job.get("id")
    runner_name = job.get("runner_name")
    labels = job.get("labels")
    if not isinstance(runner_id, int) or isinstance(runner_id, bool) or runner_id <= 0:
        failures.append("GitHub workflow job has no positive runner_id")
    if not isinstance(job_id, int) or isinstance(job_id, bool) or job_id <= 0:
        failures.append("GitHub workflow job has no positive job id")
    if not isinstance(runner_name, str) or not runner_name:
        failures.append("GitHub workflow job has no runner_name")
    if not isinstance(labels, list) or "ubuntu-24.04" not in labels:
        failures.append("workflow job is not bound to the required ubuntu-24.04 hosted image")
    if isinstance(labels, list) and "self-hosted" in labels:
        failures.append("self-hosted runner evidence is not eligible")

    raw_host_components = {
        "hostname": socket.gethostname(),
        "machine_id": read_machine_component(pathlib.Path("/etc/machine-id"), "unavailable"),
        "boot_id": read_machine_component(pathlib.Path("/proc/sys/kernel/random/boot_id"), "unavailable"),
        "runner_name": runner_name,
        "runner_id": runner_id,
        "actions_job_id": job_id,
    }
    component_hashes = {
        key: sha256_bytes(f"mfenx-step3-host-component-v1\0{value}".encode("utf-8"))
        for key, value in raw_host_components.items()
    }
    host_fingerprint = sha256_bytes(
        b"mfenx-step3-host-fingerprint-v1\0" + canonical_bytes(component_hashes)
    )

    record = {
        "schema": SCHEMA,
        "kind": "host_reproduction",
        "created_at": utc_now(),
        "slot": args.slot,
        "status": "succeeded" if capture_ok and not failures else "failed",
        "failures": failures,
        "claim_policy": {
            "individual_record_establishes_three_host_reproduction": False,
            "requires_three_distinct_runner_ids": True,
            "requires_three_distinct_host_fingerprints": True,
            "requires_signed_aggregate": True,
        },
        "github": {
            "repository": args.repository,
            "commit": args.commit,
            "run_id": args.run_id,
            "run_attempt": args.run_attempt,
            "job_name": args.job_name,
            "actions_job_id": job_id,
            "runner_id": runner_id,
            "runner_name": runner_name,
            "labels": labels,
            "runner_environment": os.environ.get("RUNNER_ENVIRONMENT"),
        },
        "host": {
            "fingerprint": host_fingerprint,
            "component_hashes": component_hashes,
            "raw_host_identifiers_withheld": True,
        },
        "capture": {
            "path": args.capture.name,
            "checksum_manifest_sha256": sha256_file(args.capture / "SHA256SUMS") if capture_ok else None,
            "record_schema": capture.get("schema") if capture else None,
            "accepted_binary_sha256": (
                capture.get("execution", {}).get("executor_sha256") if capture else None
            ),
            "candidate_release_id": expected_identity["release_id"],
            "candidate_identity_sha256": expected_identity["identity_sha256"],
            "hosted_release_workload_gate_passed": capture_ok,
        },
    }
    write_new_json(output, record)
    return 0 if record["status"] == "succeeded" else 1


def parse_manifest_bytes(payload: bytes) -> dict[str, str]:
    result: dict[str, str] = {}
    try:
        text = payload.decode("ascii")
    except UnicodeDecodeError as error:
        raise EvidenceError("checksum manifest is not ASCII") from error
    for number, line in enumerate(text.splitlines(), 1):
        parts = line.split("  ", 1)
        if len(parts) != 2 or not HEX64.fullmatch(parts[0]):
            raise EvidenceError(f"invalid checksum line {number}")
        relative = pathlib.PurePosixPath(parts[1])
        if not safe_relative(relative) or parts[1] in result:
            raise EvidenceError(f"unsafe or duplicate checksum path at line {number}")
        result[parts[1]] = parts[0]
    return result


def inspect_host_archive(path: pathlib.Path) -> tuple[dict[str, Any], list[str]]:
    failures: list[str] = []
    file_payloads: dict[str, bytes] = {}
    try:
        with tarfile.open(path, "r:*") as archive:
            for member in archive.getmembers():
                relative = pathlib.PurePosixPath(member.name)
                if not safe_relative(relative):
                    failures.append(f"archive has unsafe member {member.name!r}")
                    continue
                if relative.parts[0] != "host-evidence":
                    failures.append(f"archive member is outside host-evidence: {member.name!r}")
                    continue
                if not member.isfile() and not member.isdir():
                    failures.append(f"archive has non-regular special member {member.name!r}")
                    continue
                if member.isfile():
                    if member.name in file_payloads:
                        failures.append(f"archive has duplicate member {member.name!r}")
                        continue
                    stream = archive.extractfile(member)
                    if stream is None:
                        failures.append(f"archive member cannot be read: {member.name!r}")
                        continue
                    file_payloads[member.name] = stream.read()
    except (OSError, tarfile.TarError) as error:
        return {}, [f"cannot inspect host archive: {error}"]

    prefix_candidates = {
        pathlib.PurePosixPath(name).parts[0]
        for name in file_payloads
        if pathlib.PurePosixPath(name).parts
    }
    if prefix_candidates != {"host-evidence"}:
        failures.append("host archive must have exactly the host-evidence root")
    record_bytes = file_payloads.get("host-evidence/host-record.json")
    manifest_bytes = file_payloads.get("host-evidence/SHA256SUMS")
    if record_bytes is None or manifest_bytes is None:
        return {}, failures + ["host archive lacks host-record.json or SHA256SUMS"]
    try:
        record = json.loads(record_bytes)
        expected = parse_manifest_bytes(manifest_bytes)
    except (json.JSONDecodeError, EvidenceError) as error:
        return {}, failures + [str(error)]
    actual = {
        name.removeprefix("host-evidence/"): sha256_bytes(payload)
        for name, payload in file_payloads.items()
        if name != "host-evidence/SHA256SUMS"
    }
    if expected != actual:
        failures.append("host archive is not checksum-closed")
    return record, failures


def aggregate(args: argparse.Namespace) -> int:
    output = args.output
    if output.exists() or output.is_symlink():
        raise EvidenceError(f"refusing to overwrite {output}")
    output.mkdir(parents=True)
    expected_identity = load_expected_identity(args.identity)
    records: list[dict[str, Any]] = []
    failures: list[str] = []
    artifact_rows: list[dict[str, Any]] = []
    for slot in EXPECTED_SLOTS:
        directory = args.inputs / f"validation-{slot}"
        archive = directory / f"{slot}.tar.gz"
        digest_file = directory / f"{slot}.tar.gz.sha256"
        bundle = directory / f"{slot}.sigstore.json"
        verification = args.verifications / f"{slot}.json"
        if not all(path.is_file() and not path.is_symlink() for path in (archive, digest_file, bundle, verification)):
            failures.append(f"{slot}: archive, digest, attestation bundle, or verification result missing")
            continue
        digest_text = digest_file.read_text(encoding="ascii").strip().split()
        observed = sha256_file(archive)
        if len(digest_text) != 2 or digest_text[0] != observed or digest_text[1] != archive.name:
            failures.append(f"{slot}: detached SHA-256 does not bind the host archive")
        try:
            verified_attestation(verification)
        except (EvidenceError, OSError, ValueError, json.JSONDecodeError) as error:
            failures.append(f"{slot}: invalid attestation verification result: {error}")
        record, archive_failures = inspect_host_archive(archive)
        failures.extend(f"{slot}: {failure}" for failure in archive_failures)
        if record:
            records.append(record)
        artifact_rows.append(
            {
                "slot": slot,
                "archive": archive.relative_to(args.inputs).as_posix(),
                "archive_sha256": observed,
                "attestation_bundle_sha256": sha256_file(bundle),
                "attestation_verification_sha256": sha256_file(verification),
            }
        )

    if len(records) != 3:
        failures.append(f"expected three parseable host records, found {len(records)}")
    else:
        slots = [record.get("slot") for record in records]
        runner_ids = [record.get("github", {}).get("runner_id") for record in records]
        job_ids = [record.get("github", {}).get("actions_job_id") for record in records]
        fingerprints = [record.get("host", {}).get("fingerprint") for record in records]
        repositories = {record.get("github", {}).get("repository") for record in records}
        commits = {record.get("github", {}).get("commit") for record in records}
        run_ids = {str(record.get("github", {}).get("run_id")) for record in records}
        attempts = {str(record.get("github", {}).get("run_attempt")) for record in records}
        accepted = {record.get("capture", {}).get("accepted_binary_sha256") for record in records}
        candidate_release_ids = {record.get("capture", {}).get("candidate_release_id") for record in records}
        candidate_identity_digests = {
            record.get("capture", {}).get("candidate_identity_sha256") for record in records
        }
        if sorted(slots) != sorted(EXPECTED_SLOTS):
            failures.append("host slots are not exactly host-1, host-2, and host-3")
        if any(record.get("status") != "succeeded" for record in records):
            failures.append("at least one host reproduction did not succeed")
        if len(set(runner_ids)) != 3 or any(not isinstance(value, int) or value <= 0 for value in runner_ids):
            failures.append("three distinct positive GitHub runner IDs were not observed")
        if len(set(job_ids)) != 3 or any(not isinstance(value, int) or value <= 0 for value in job_ids):
            failures.append("three distinct positive GitHub Actions job IDs were not observed")
        if len(set(fingerprints)) != 3 or any(not isinstance(value, str) or not HEX64.fullmatch(value) for value in fingerprints):
            failures.append("three distinct well-formed hosted-machine fingerprints were not observed")
        if repositories != {args.repository}:
            failures.append("host records do not bind one expected repository")
        if commits != {args.commit}:
            failures.append("host records do not bind one expected source commit")
        if run_ids != {str(args.run_id)} or attempts != {str(args.run_attempt)}:
            failures.append("host records do not bind this workflow run and attempt")
        if accepted != {expected_identity["executor_sha256"]}:
            failures.append("host records do not reproduce the one frozen accepted binary")
        if candidate_release_ids != {expected_identity["release_id"]}:
            failures.append("host records do not bind the one frozen candidate release ID")
        if candidate_identity_digests != {expected_identity["identity_sha256"]}:
            failures.append("host records do not bind the checked-in candidate identity bytes")

    summary = {
        "schema": SCHEMA,
        "kind": "three_host_aggregate",
        "created_at": utc_now(),
        "status": "host_gate_complete" if not failures else "failed",
        "claim": None,
        "claim_eligible": False,
        "host_gate_eligible": not failures,
        "failures": failures,
        "required_gates": {
            "host_count": 3,
            "distinct_positive_runner_ids": True,
            "distinct_actions_job_ids": True,
            "distinct_host_fingerprints": True,
            "same_repository_commit_run_attempt": True,
            "all_host_captures_checksum_closed": True,
            "all_host_archives_sigstore_verified": True,
            "self_hosted_runners_forbidden": True,
            "candidate_release_id": expected_identity["release_id"],
            "candidate_identity_sha256": expected_identity["identity_sha256"],
            "accepted_binary_sha256": expected_identity["executor_sha256"],
            "signed_aggregate_required_before_public_claim": True,
        },
        "source": {
            "repository": args.repository,
            "commit": args.commit,
            "run_id": args.run_id,
            "run_attempt": args.run_attempt,
        },
        "hosts": records,
        "artifacts": artifact_rows,
        "limitations": [
            "GitHub OIDC/Sigstore authenticates workflow provenance, not a third-party audit conclusion.",
            "Host fingerprints are hashes of ephemeral runner components and are not hardware attestation.",
            "All three reproductions use ephemeral virtual machines from the same GitHub-hosted Ubuntu image family.",
            "This is release/workload reproduction, not source-to-binary reproducible-build evidence.",
            "A complete aggregate must itself receive and retain its workflow attestation before publication.",
        ],
    }
    write_new_json(output / "summary.json", summary)
    return 0


def check_host_gate(args: argparse.Namespace) -> int:
    summary = load_json(args.summary)
    if summary.get("schema") != SCHEMA or summary.get("kind") != "three_host_aggregate":
        raise EvidenceError("not a validation aggregate summary")
    if summary.get("status") != "host_gate_complete" or summary.get("host_gate_eligible") is not True:
        raise EvidenceError("three-host reproduction host gate is not complete")
    if summary.get("failures") != []:
        raise EvidenceError("complete host aggregate contains failures")
    if summary.get("claim") is not None or summary.get("claim_eligible") is not False:
        raise EvidenceError("unsigned aggregate must not contain a public claim")
    return 0


def verified_attestation(path: pathlib.Path) -> Any:
    if path.is_symlink() or not path.is_file():
        raise EvidenceError(f"attestation input is missing, non-regular, or symlinked: {path}")
    value = load_json(path)
    if (
        not isinstance(value, list)
        or not value
        or any(
            not isinstance(item, dict)
            or not isinstance(item.get("attestation"), dict)
            or not isinstance(item.get("verificationResult"), dict)
            or not item["verificationResult"]
            for item in value
        )
    ):
        raise EvidenceError("attestation verification returned no verified statement")
    return value


def finalize_claim(args: argparse.Namespace) -> int:
    if args.output.exists() or args.output.is_symlink():
        raise EvidenceError(f"refusing to overwrite {args.output}")
    check_host_gate(argparse.Namespace(summary=args.summary))
    summary = load_json(args.summary)
    expected_identity = load_expected_identity(args.identity)
    required_gates = summary.get("required_gates", {})
    if required_gates.get("candidate_release_id") != expected_identity["release_id"]:
        raise EvidenceError("aggregate summary differs from the frozen candidate release ID")
    if required_gates.get("candidate_identity_sha256") != expected_identity["identity_sha256"]:
        raise EvidenceError("aggregate summary differs from the frozen candidate identity bytes")
    source = summary.get("source", {})
    expected_source = {
        "repository": args.repository,
        "commit": args.commit,
        "run_id": args.run_id,
        "run_attempt": args.run_attempt,
    }
    if source != expected_source:
        raise EvidenceError("aggregate source does not bind this workflow run and attempt")
    for path in (args.archive, args.archive_digest, args.bundle):
        if path.is_symlink() or not path.is_file():
            raise EvidenceError(f"claim input is missing, non-regular, or symlinked: {path}")
    digest_fields = args.archive_digest.read_text(encoding="ascii").strip().split()
    observed_archive_sha256 = sha256_file(args.archive)
    if (
        len(digest_fields) != 2
        or digest_fields[0] != observed_archive_sha256
        or digest_fields[1] != args.archive.name
    ):
        raise EvidenceError("detached aggregate digest does not bind the aggregate archive")
    try:
        with tarfile.open(args.archive, "r:gz") as aggregate_archive:
            summary_members = [
                member
                for member in aggregate_archive.getmembers()
                if member.name == "aggregate-evidence/summary.json" and member.isfile()
            ]
            if len(summary_members) != 1:
                raise EvidenceError("aggregate archive does not contain exactly one regular summary")
            stream = aggregate_archive.extractfile(summary_members[0])
            if stream is None or sha256_bytes(stream.read()) != sha256_file(args.summary):
                raise EvidenceError("aggregate archive does not bind the supplied summary bytes")
    except (OSError, tarfile.TarError) as error:
        raise EvidenceError(f"cannot inspect aggregate archive: {error}") from error
    bundle = load_json(args.bundle)
    if not isinstance(bundle, dict) or not bundle:
        raise EvidenceError("aggregate attestation bundle is not a nonempty JSON object")
    verified_attestation(args.verification)

    record = {
        "schema": SCHEMA,
        "kind": "attested_three_host_claim",
        "created_at": utc_now(),
        "status": "complete",
        "claim_eligible": True,
        "claim": (
            "Three distinct GitHub-hosted Ubuntu 24.04 VM jobs each reproduced the canonical "
            "workload from the exact signed candidate archive and passed executor plus standalone "
            "reference-verifier replay."
        ),
        "candidate": {
            "release_id": expected_identity["release_id"],
            "identity_sha256": expected_identity["identity_sha256"],
            "executor_sha256": expected_identity["executor_sha256"],
        },
        "source": source,
        "evidence": {
            "aggregate_summary_sha256": sha256_file(args.summary),
            "aggregate_archive_sha256": observed_archive_sha256,
            "aggregate_attestation_bundle_sha256": sha256_file(args.bundle),
            "aggregate_attestation_verification_sha256": sha256_file(args.verification),
        },
        "limitations": [
            "These were ephemeral virtual machines from one GitHub-hosted runner image family, not three administrative domains.",
            "Distinct runner IDs, job IDs, and hashed VM components are reuse checks, not hardware attestation.",
            "This establishes release/workload reproduction, not source-to-binary reproducibility or a performance result.",
            "GitHub OIDC/Sigstore authenticates workflow provenance; it is not an independent human review.",
        ],
    }
    write_new_json(args.output, record)
    return 0


def check_claim(args: argparse.Namespace) -> int:
    record = load_json(args.claim)
    if record.get("schema") != SCHEMA or record.get("kind") != "attested_three_host_claim":
        raise EvidenceError("not a validation attested three-host claim")
    if record.get("status") != "complete" or record.get("claim_eligible") is not True:
        raise EvidenceError("three-host attested claim is not complete")
    if not isinstance(record.get("claim"), str) or not record["claim"]:
        raise EvidenceError("three-host attested claim text is absent")
    return 0


def check_host(args: argparse.Namespace) -> int:
    record = load_json(args.record)
    if record.get("schema") != SCHEMA or record.get("kind") != "host_reproduction":
        raise EvidenceError("not a validation host reproduction record")
    if record.get("status") != "succeeded" or record.get("failures") != []:
        raise EvidenceError("host reproduction gate is not successful")
    return 0


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    commands = result.add_subparsers(dest="command", required=True)
    host = commands.add_parser("host-record")
    host.add_argument("--identity", required=True, type=pathlib.Path)
    host.add_argument("--capture", required=True, type=pathlib.Path)
    host.add_argument("--output", required=True, type=pathlib.Path)
    host.add_argument("--slot", required=True, choices=EXPECTED_SLOTS)
    host.add_argument("--repository", required=True)
    host.add_argument("--commit", required=True)
    host.add_argument("--run-id", required=True)
    host.add_argument("--run-attempt", required=True)
    host.add_argument("--job-name", required=True)
    host.add_argument("--job-api-json", required=True, type=pathlib.Path)
    host.set_defaults(function=host_record)
    seal = commands.add_parser("seal")
    seal.add_argument("--root", required=True, type=pathlib.Path)
    seal.set_defaults(function=lambda arguments: (print(seal_directory(arguments.root)), 0)[1])
    combine = commands.add_parser("aggregate")
    combine.add_argument("--identity", required=True, type=pathlib.Path)
    combine.add_argument("--inputs", required=True, type=pathlib.Path)
    combine.add_argument("--verifications", required=True, type=pathlib.Path)
    combine.add_argument("--output", required=True, type=pathlib.Path)
    combine.add_argument("--repository", required=True)
    combine.add_argument("--commit", required=True)
    combine.add_argument("--run-id", required=True)
    combine.add_argument("--run-attempt", required=True)
    combine.set_defaults(function=aggregate)
    gate = commands.add_parser("check-host-gate")
    gate.add_argument("--summary", required=True, type=pathlib.Path)
    gate.set_defaults(function=check_host_gate)
    finalize = commands.add_parser("finalize-claim")
    finalize.add_argument("--identity", required=True, type=pathlib.Path)
    finalize.add_argument("--summary", required=True, type=pathlib.Path)
    finalize.add_argument("--archive", required=True, type=pathlib.Path)
    finalize.add_argument("--archive-digest", required=True, type=pathlib.Path)
    finalize.add_argument("--bundle", required=True, type=pathlib.Path)
    finalize.add_argument("--verification", required=True, type=pathlib.Path)
    finalize.add_argument("--output", required=True, type=pathlib.Path)
    finalize.add_argument("--repository", required=True)
    finalize.add_argument("--commit", required=True)
    finalize.add_argument("--run-id", required=True)
    finalize.add_argument("--run-attempt", required=True)
    finalize.set_defaults(function=finalize_claim)
    claim_gate = commands.add_parser("check-claim")
    claim_gate.add_argument("--claim", required=True, type=pathlib.Path)
    claim_gate.set_defaults(function=check_claim)
    host_gate = commands.add_parser("check-host")
    host_gate.add_argument("--record", required=True, type=pathlib.Path)
    host_gate.set_defaults(function=check_host)
    return result


def main() -> int:
    try:
        arguments = parser().parse_args()
        return int(arguments.function(arguments))
    except (EvidenceError, OSError, ValueError, json.JSONDecodeError) as error:
        print(f"validation reproduction evidence error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Capture automated security evidence without impersonating a human review."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import re
import subprocess
import sys
import time
from datetime import datetime, timezone
from typing import Any


SCHEMA = "mfenx.step3-security-review.v1"
IDENTITY_SCHEMA = "mfenx.step3-candidate-identity.v1"
HEX64 = re.compile(r"^[0-9a-f]{64}$")
TOOL_NAME = re.compile(r"^[a-z0-9][a-z0-9-]{0,63}$")
REQUIRED_AUTOMATED = (
    "prerequisites",
    "candidate-input-verification",
    "candidate-source-inventory",
    "toolchain-context",
    "codeql-init",
    "codeql-build",
    "codeql-analyze",
    "cargo-audit",
    "cargo-deny",
    "rustfmt",
    "clippy",
    "unit-tests",
    "fuzz-harness-setup",
    "fuzz-contract-control",
    "fuzz-tensor-manifest",
    "fuzz-artifact-collection",
    "static-policy",
)


class ReviewError(RuntimeError):
    """An evidence or policy invariant failed."""


def utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def load_json(path: pathlib.Path) -> Any:
    with path.open("r", encoding="utf-8") as stream:
        return json.load(stream)


def candidate_context(path: pathlib.Path) -> dict[str, Any]:
    if path.is_symlink() or not path.is_file():
        raise ReviewError("candidate identity is missing, non-regular, or symlinked")
    document = load_json(path)
    if (
        not isinstance(document, dict)
        or document.get("schema") != IDENTITY_SCHEMA
        or document.get("enabled") is not True
    ):
        raise ReviewError("candidate identity is not enabled and frozen")
    try:
        context = {
            "release_id": document["release_id"],
            "identity_sha256": sha256_file(path),
            "archive_sha256": document["archive"]["sha256"],
            "source_subdir": document["archive"]["source_subdir"],
            "executor_sha256": document["workload"]["executor_sha256"],
            "verifier_sha256": document["workload"]["verifier_sha256"],
            "signature_principal": document["signature"]["principal"],
            "signature_namespace": document["signature"]["namespace"],
        }
    except (KeyError, TypeError) as error:
        raise ReviewError(f"candidate identity lacks a required field: {error}") from error
    for key in ("identity_sha256", "archive_sha256", "executor_sha256", "verifier_sha256"):
        if not isinstance(context[key], str) or not HEX64.fullmatch(context[key]):
            raise ReviewError(f"candidate identity {key} is not a lowercase SHA-256")
    if not isinstance(context["release_id"], str) or context["release_id"].startswith("__"):
        raise ReviewError("candidate release ID is not frozen")
    if not isinstance(context["source_subdir"], str) or context["source_subdir"].startswith("__"):
        raise ReviewError("candidate source subdirectory is not frozen")
    if context["signature_principal"] != "mfenx-release":
        raise ReviewError("candidate identity uses the wrong signing principal")
    if context["signature_namespace"] != "mfenx-validation-candidate":
        raise ReviewError("candidate identity uses the wrong signing namespace")
    return context


def write_new_json(path: pathlib.Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    try:
        with path.open("x", encoding="ascii", newline="\n") as stream:
            json.dump(value, stream, indent=2, sort_keys=True, ensure_ascii=True)
            stream.write("\n")
    except FileExistsError as error:
        raise ReviewError(f"refusing to overwrite {path}") from error


def validate_tool(tool: str) -> None:
    if not TOOL_NAME.fullmatch(tool):
        raise ReviewError(f"unsafe tool label: {tool!r}")


def artifact_index(root: pathlib.Path) -> list[dict[str, Any]]:
    if not root.exists():
        return []
    rows: list[dict[str, Any]] = []
    for path in sorted(root.rglob("*")):
        if path.is_symlink():
            raise ReviewError(f"symlinked evidence artifact is forbidden: {path}")
        if not path.is_file() and not path.is_dir():
            raise ReviewError(f"special evidence artifact is forbidden: {path}")
        if path.is_file():
            rows.append(
                {
                    "path": path.relative_to(root).as_posix(),
                    "sha256": sha256_file(path),
                    "size_bytes": path.stat().st_size,
                }
            )
    return rows


def init(args: argparse.Namespace) -> int:
    if args.output.exists() or args.output.is_symlink():
        raise ReviewError(f"refusing to overwrite {args.output}")
    (args.output / "outcomes").mkdir(parents=True)
    (args.output / "logs").mkdir()
    candidate = candidate_context(args.identity)
    write_new_json(
        args.output / "run-context.json",
        {
            "schema": SCHEMA,
            "kind": "automated_review_context",
            "created_at": utc_now(),
            "repository": args.repository,
            "commit": args.commit,
            "workflow_run_id": args.run_id,
            "workflow_run_attempt": args.run_attempt,
            "candidate": candidate,
            "automated_evidence_is_independent_human_review": False,
        },
    )
    return 0


def run_tool(args: argparse.Namespace) -> int:
    validate_tool(args.tool)
    if not args.command:
        raise ReviewError("run requires a command after --")
    outcome_path = args.root / "outcomes" / f"{args.tool}.json"
    stdout_path = args.root / "logs" / f"{args.tool}.stdout.log"
    stderr_path = args.root / "logs" / f"{args.tool}.stderr.log"
    for path in (outcome_path, stdout_path, stderr_path):
        if path.exists() or path.is_symlink():
            raise ReviewError(f"refusing to overwrite {path}")
    started_at = utc_now()
    monotonic_start = time.monotonic_ns()
    with stdout_path.open("xb") as stdout, stderr_path.open("xb") as stderr:
        try:
            completed = subprocess.run(
                args.command,
                cwd=args.cwd,
                stdin=subprocess.DEVNULL,
                stdout=stdout,
                stderr=stderr,
                check=False,
                timeout=args.timeout_seconds,
            )
            exit_status = completed.returncode
            timed_out = False
        except subprocess.TimeoutExpired:
            exit_status = 124
            timed_out = True
            stderr.write(b"\nsecurity evidence wrapper: command timed out\n")
    record = {
        "schema": SCHEMA,
        "kind": "automated_tool_outcome",
        "tool": args.tool,
        "status": "success" if exit_status == 0 else "failure",
        "exit_status": exit_status,
        "timed_out": timed_out,
        "started_at": started_at,
        "finished_at": utc_now(),
        "duration_ns": time.monotonic_ns() - monotonic_start,
        "command": args.command,
        "working_directory": str(args.cwd),
        "stdout": {
            "path": stdout_path.relative_to(args.root).as_posix(),
            "sha256": sha256_file(stdout_path),
            "size_bytes": stdout_path.stat().st_size,
        },
        "stderr": {
            "path": stderr_path.relative_to(args.root).as_posix(),
            "sha256": sha256_file(stderr_path),
            "size_bytes": stderr_path.stat().st_size,
        },
        "human_review_performed": False,
    }
    write_new_json(outcome_path, record)
    return exit_status


def record_external(args: argparse.Namespace) -> int:
    validate_tool(args.tool)
    outcome_path = args.root / "outcomes" / f"{args.tool}.json"
    artifacts = artifact_index(args.artifacts) if args.artifacts else []
    status = "success" if args.status == "success" else "failure"
    write_new_json(
        outcome_path,
        {
            "schema": SCHEMA,
            "kind": "automated_tool_outcome",
            "tool": args.tool,
            "status": status,
            "exit_status": 0 if status == "success" else 1,
            "timed_out": False,
            "started_at": None,
            "finished_at": utc_now(),
            "duration_ns": None,
            "command": None,
            "working_directory": None,
            "external_step": True,
            "note": args.note,
            "artifacts": artifacts,
            "human_review_performed": False,
        },
    )
    return 0 if status == "success" else 1


def source_inventory(args: argparse.Namespace) -> int:
    validate_tool("candidate-source-inventory")
    outcome_path = args.root / "outcomes/candidate-source-inventory.json"
    report_path = args.root / "candidate-source-inventory.json"
    if outcome_path.exists() or report_path.exists():
        raise ReviewError("refusing to overwrite candidate source inventory evidence")
    failures: list[str] = []
    rows: list[dict[str, Any]] = []
    if args.source.is_symlink() or not args.source.is_dir():
        failures.append("candidate source directory is missing or symlinked")
    else:
        for path in sorted(args.source.rglob("*")):
            if path.is_symlink():
                failures.append(f"candidate source contains symlink: {path.relative_to(args.source)}")
                continue
            if not path.is_file() and not path.is_dir():
                failures.append(f"candidate source contains special object: {path.relative_to(args.source)}")
                continue
            if path.is_file():
                relative = path.relative_to(args.source).as_posix()
                if "\n" in relative or "\r" in relative or "  " in relative:
                    failures.append(f"candidate source has unsafe path: {relative!r}")
                    continue
                rows.append(
                    {
                        "path": relative,
                        "sha256": sha256_file(path),
                        "size_bytes": path.stat().st_size,
                    }
                )
    context = load_json(args.root / "run-context.json")
    report = {
        "schema": SCHEMA,
        "kind": "candidate_source_inventory",
        "created_at": utc_now(),
        "status": "success" if rows and not failures else "failure",
        "candidate": context.get("candidate"),
        "source_path_from_signed_archive": context.get("candidate", {}).get("source_subdir"),
        "file_count": len(rows),
        "files": rows,
        "failures": failures,
        "inventory_phase": "signed_source_before_review_harness_injection",
        "review_harness_included_by_this_command": False,
        "human_review_performed": False,
    }
    write_new_json(report_path, report)
    write_new_json(
        outcome_path,
        {
            "schema": SCHEMA,
            "kind": "automated_tool_outcome",
            "tool": "candidate-source-inventory",
            "status": report["status"],
            "exit_status": 0 if report["status"] == "success" else 1,
            "timed_out": False,
            "finished_at": utc_now(),
            "report": {
                "path": report_path.relative_to(args.root).as_posix(),
                "sha256": sha256_file(report_path),
                "size_bytes": report_path.stat().st_size,
            },
            "human_review_performed": False,
        },
    )
    return 0 if report["status"] == "success" else 1


def static_policy(args: argparse.Namespace) -> int:
    validate_tool("static-policy")
    repository = args.repository.resolve()
    policy_repository = (args.policy_repository or args.repository).resolve()
    checks: dict[str, bool] = {}
    findings: list[dict[str, Any]] = []
    critical_roots = (
        "crates/rarecomp-mfenx-local/src",
        "crates/rarecomp-mfenx-tensor-store/src",
        "crates/mfenx-contract-v1-verifier/src",
    )
    crate_roots = (
        "crates/rarecomp-mfenx-local/src/lib.rs",
        "crates/rarecomp-mfenx-local/src/main.rs",
        "crates/rarecomp-mfenx-tensor-store/src/lib.rs",
        "crates/mfenx-contract-v1-verifier/src/lib.rs",
        "crates/mfenx-contract-v1-verifier/src/main.rs",
    )
    missing_forbid = []
    for relative in crate_roots:
        text = (repository / relative).read_text(encoding="utf-8")
        if "#![forbid(unsafe_code)]" not in text:
            missing_forbid.append(relative)
    checks["critical_crate_roots_forbid_unsafe_code"] = not missing_forbid
    findings.extend({"kind": "missing_forbid_unsafe", "path": path} for path in missing_forbid)

    unsafe_syntax = re.compile(r"\bunsafe\s*(?:\{|fn\b|impl\b|trait\b|extern\b)")
    unsafe_hits = []
    for root_relative in critical_roots:
        for path in sorted((repository / root_relative).rglob("*.rs")):
            for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
                stripped = line.strip()
                if stripped.startswith("//"):
                    continue
                if unsafe_syntax.search(line):
                    unsafe_hits.append({"path": path.relative_to(repository).as_posix(), "line": number})
    checks["no_unsafe_syntax_in_critical_scope"] = not unsafe_hits
    findings.extend({"kind": "unsafe_syntax", **hit} for hit in unsafe_hits)

    verifier_manifest = load_json_like_toml_dependencies(
        repository / "crates/mfenx-contract-v1-verifier/Cargo.toml"
    )
    allowed_verifier_dependencies = {"blake3", "serde", "serde_json"}
    unexpected = sorted(set(verifier_manifest) - allowed_verifier_dependencies)
    checks["reference_verifier_dependency_allowlist"] = not unexpected
    findings.extend({"kind": "unexpected_verifier_dependency", "dependency": item} for item in unexpected)

    verifier_text = "\n".join(
        path.read_text(encoding="utf-8")
        for path in sorted((repository / "crates/mfenx-contract-v1-verifier/src").rglob("*.rs"))
    )
    process_patterns = {
        "std-process-command": re.compile(r"\bstd::process::Command\b"),
        "tokio-process-command": re.compile(r"\btokio::process::Command\b"),
        "process-command": re.compile(r"\bprocess::Command\b"),
        "command-constructor": re.compile(r"\bCommand::new\s*\("),
    }
    process_hits = [name for name, pattern in process_patterns.items() if pattern.search(verifier_text)]
    checks["reference_verifier_does_not_spawn_executor_or_shell"] = not process_hits
    findings.extend({"kind": "verifier_process_token", "token": item} for item in process_hits)

    python_shell_hits = []
    for path in sorted((policy_repository / "packaging").glob("validation-*.py")):
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if re.search(r"\bshell\s*=\s*True\b", line):
                python_shell_hits.append(
                    {"path": path.relative_to(policy_repository).as_posix(), "line": number}
                )
    checks["step3_python_never_uses_shell_true"] = not python_shell_hits
    findings.extend({"kind": "python_shell_true", **hit} for hit in python_shell_hits)

    report_path = args.root / "static-policy.json"
    report = {
        "schema": SCHEMA,
        "kind": "bounded_static_policy",
        "created_at": utc_now(),
        "scope": {
            "candidate_source": list(critical_roots),
            "workflow_policy": "packaging/validation-*.py",
        },
        "checks": checks,
        "findings": findings,
        "passed": all(checks.values()),
        "limitations": [
            "This bounded source policy is not a sound parser or proof of memory safety.",
            "Rust compiler lints, Clippy, CodeQL, dependency tools, and fuzzing are separate evidence streams.",
        ],
        "human_review_performed": False,
    }
    write_new_json(report_path, report)
    outcome = argparse.Namespace(
        root=args.root,
        tool="static-policy",
        status="success" if report["passed"] else "failure",
        artifacts=report_path.parent,
        note="bounded repository policy scan; not human review",
    )
    # Avoid indexing the whole root recursively while it is still being written.
    write_new_json(
        args.root / "outcomes/static-policy.json",
        {
            "schema": SCHEMA,
            "kind": "automated_tool_outcome",
            "tool": "static-policy",
            "status": outcome.status,
            "exit_status": 0 if report["passed"] else 1,
            "timed_out": False,
            "finished_at": utc_now(),
            "report": {
                "path": "static-policy.json",
                "sha256": sha256_file(report_path),
                "size_bytes": report_path.stat().st_size,
            },
            "human_review_performed": False,
        },
    )
    return 0 if report["passed"] else 1


def load_json_like_toml_dependencies(path: pathlib.Path) -> list[str]:
    dependencies: list[str] = []
    section = ""
    for raw_line in path.read_text(encoding="utf-8").splitlines():
        line = raw_line.split("#", 1)[0].strip()
        if line.startswith("[") and line.endswith("]"):
            section = line[1:-1]
            continue
        if section == "dependencies" and "=" in line:
            name = line.split("=", 1)[0].strip()
            if name:
                dependencies.append(name)
    return dependencies


def sarif_results(root: pathlib.Path) -> tuple[int, list[dict[str, Any]], list[str]]:
    count = 0
    rows: list[dict[str, Any]] = []
    failures: list[str] = []
    for path in sorted(root.rglob("*.sarif")) if root.exists() else []:
        try:
            document = load_json(path)
            runs = document.get("runs")
            if not isinstance(runs, list):
                raise ReviewError("SARIF runs is not an array")
            path_count = 0
            for run in runs:
                results = run.get("results", []) if isinstance(run, dict) else None
                if not isinstance(results, list):
                    raise ReviewError("SARIF results is not an array")
                path_count += len(results)
            count += path_count
            rows.append({"path": path.relative_to(root).as_posix(), "sha256": sha256_file(path), "results": path_count})
        except (OSError, ValueError, json.JSONDecodeError, ReviewError) as error:
            failures.append(f"invalid SARIF {path}: {error}")
    if not rows:
        failures.append("no CodeQL SARIF file was retained")
    return count, rows, failures


def finalize(args: argparse.Namespace) -> int:
    summary_path = args.root / "summary.json"
    if summary_path.exists() or summary_path.is_symlink():
        raise ReviewError(f"refusing to overwrite {summary_path}")
    failures: list[str] = []
    outcomes: dict[str, Any] = {}
    for tool in REQUIRED_AUTOMATED:
        path = args.root / "outcomes" / f"{tool}.json"
        if not path.is_file() or path.is_symlink():
            failures.append(f"missing outcome: {tool}")
            continue
        try:
            outcome = load_json(path)
        except (OSError, ValueError, json.JSONDecodeError) as error:
            failures.append(f"invalid outcome {tool}: {error}")
            continue
        outcomes[tool] = outcome
        if outcome.get("tool") != tool or outcome.get("status") != "success":
            failures.append(f"automated tool did not succeed: {tool}")
    result_count, sarif, sarif_failures = sarif_results(args.root / "codeql-results")
    failures.extend(sarif_failures)
    if result_count:
        failures.append(f"CodeQL retained {result_count} unwaived result(s)")
    context = load_json(args.root / "run-context.json")
    summary = {
        "schema": SCHEMA,
        "kind": "security_review_summary",
        "created_at": utc_now(),
        "source": {
            "repository": context.get("repository"),
            "commit": context.get("commit"),
            "workflow_run_id": context.get("workflow_run_id"),
            "workflow_run_attempt": context.get("workflow_run_attempt"),
        },
        "candidate": context.get("candidate"),
        "automated_evidence": {
            "status": "pass" if not failures else "fail",
            "required_tools": list(REQUIRED_AUTOMATED),
            "outcomes": outcomes,
            "codeql_sarif": sarif,
            "unwaived_codeql_result_count": result_count,
            "failures": failures,
        },
        "independent_human_review": {
            "status": "not_performed",
            "reviewer": None,
            "independence_checked": False,
            "signed_report": None,
        },
        "independent_code_or_security_review_complete": False,
        "claim_policy": {
            "automated_pass_may_be_called_independent_review": False,
            "human_review_requires_external_reviewer_and_signed_report": True,
            "successes_and_failures_must_both_be_retained": True,
        },
    }
    write_new_json(summary_path, summary)
    return 0


def seal(args: argparse.Namespace) -> int:
    manifest = args.root / "SHA256SUMS"
    if manifest.exists() or manifest.is_symlink():
        raise ReviewError(f"refusing to overwrite {manifest}")
    lines = []
    for path in sorted(args.root.rglob("*")):
        if path.is_symlink():
            raise ReviewError(f"symlinked evidence is forbidden: {path}")
        if not path.is_file() and not path.is_dir():
            raise ReviewError(f"special filesystem object is forbidden: {path}")
        if path.is_file():
            relative = path.relative_to(args.root).as_posix()
            if "\n" in relative or "  " in relative:
                raise ReviewError(f"unsafe evidence path: {relative!r}")
            lines.append(f"{sha256_file(path)}  {relative}\n")
    with manifest.open("x", encoding="ascii", newline="\n") as stream:
        stream.writelines(lines)
    print(sha256_file(manifest))
    return 0


def check_automated(args: argparse.Namespace) -> int:
    summary = load_json(args.summary)
    if summary.get("schema") != SCHEMA or summary.get("kind") != "security_review_summary":
        raise ReviewError("not a validation security review summary")
    if summary.get("automated_evidence", {}).get("status") != "pass":
        raise ReviewError("automated security evidence gate did not pass")
    if summary.get("independent_code_or_security_review_complete") is not False:
        raise ReviewError("automation must not claim an independent review")
    return 0


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    commands = result.add_subparsers(dest="command_name", required=True)
    initialize = commands.add_parser("init")
    initialize.add_argument("--identity", required=True, type=pathlib.Path)
    initialize.add_argument("--output", required=True, type=pathlib.Path)
    initialize.add_argument("--repository", required=True)
    initialize.add_argument("--commit", required=True)
    initialize.add_argument("--run-id", required=True)
    initialize.add_argument("--run-attempt", required=True)
    initialize.set_defaults(function=init)
    run = commands.add_parser("run")
    run.add_argument("--root", required=True, type=pathlib.Path)
    run.add_argument("--tool", required=True)
    run.add_argument("--cwd", type=pathlib.Path, default=pathlib.Path.cwd())
    run.add_argument("--timeout-seconds", type=int, default=3600)
    run.add_argument("command", nargs=argparse.REMAINDER)
    run.set_defaults(function=run_tool)
    external = commands.add_parser("record-external")
    external.add_argument("--root", required=True, type=pathlib.Path)
    external.add_argument("--tool", required=True)
    external.add_argument("--status", required=True, choices=("success", "failure", "cancelled", "skipped"))
    external.add_argument("--artifacts", type=pathlib.Path)
    external.add_argument("--note", required=True)
    external.set_defaults(function=record_external)
    inventory = commands.add_parser("source-inventory")
    inventory.add_argument("--root", required=True, type=pathlib.Path)
    inventory.add_argument("--source", required=True, type=pathlib.Path)
    inventory.set_defaults(function=source_inventory)
    static = commands.add_parser("static-policy")
    static.add_argument("--root", required=True, type=pathlib.Path)
    static.add_argument("--repository", required=True, type=pathlib.Path)
    static.add_argument("--policy-repository", type=pathlib.Path)
    static.set_defaults(function=static_policy)
    finish = commands.add_parser("finalize")
    finish.add_argument("--root", required=True, type=pathlib.Path)
    finish.set_defaults(function=finalize)
    close = commands.add_parser("seal")
    close.add_argument("--root", required=True, type=pathlib.Path)
    close.set_defaults(function=seal)
    gate = commands.add_parser("check-automated")
    gate.add_argument("--summary", required=True, type=pathlib.Path)
    gate.set_defaults(function=check_automated)
    return result


def main() -> int:
    try:
        arguments = parser().parse_args()
        # argparse.REMAINDER preserves a conventional separator; it is not part of argv.
        if hasattr(arguments, "command") and arguments.command[:1] == ["--"]:
            arguments.command = arguments.command[1:]
        return int(arguments.function(arguments))
    except (ReviewError, OSError, ValueError, json.JSONDecodeError) as error:
        print(f"validation security evidence error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())

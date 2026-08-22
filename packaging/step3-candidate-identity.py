#!/usr/bin/env python3
"""Validate and consume the one frozen Step 3 release identity."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import re
import shlex
import subprocess
import sys
import urllib.parse
from typing import Any


SCHEMA = "mfenx.step3-candidate-identity.v1"
PLACEHOLDER = re.compile(r"^__FINAL_CANDIDATE_[A-Z0-9_]+__$")
HEX64 = re.compile(r"^[0-9a-f]{64}$")
SAFE_NAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._+-]{0,199}$")
SAFE_TOKEN = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._+-]{0,127}$")
SSH_FINGERPRINT = re.compile(r"^SHA256:[A-Za-z0-9+/]{43}$")
KEY_TYPE = re.compile(r"^(?:ssh-(?:ed25519|rsa)|ecdsa-sha2-[A-Za-z0-9@._+-]+)$")

EXPECTED_KEYS = {
    "": {"schema", "enabled", "release_id", "archive", "manifest", "signature", "allowed_signers", "workload"},
    "archive": {
        "url",
        "name",
        "root",
        "sha256",
        "source_subdir",
        "candidate_manifest_path",
        "candidate_signature_path",
        "allowed_signers_path",
        "executor_path",
        "verifier_path",
    },
    "manifest": {"url", "name", "sha256"},
    "signature": {"url", "name", "sha256", "principal", "namespace"},
    "allowed_signers": {"url", "name", "sha256", "key_fingerprint"},
    "workload": {"executor_sha256", "verifier_sha256", "canonical_output_root"},
}

PLACEHOLDER_FIELDS = (
    "release_id",
    "archive.url",
    "archive.name",
    "archive.root",
    "archive.sha256",
    "archive.source_subdir",
    "archive.candidate_manifest_path",
    "archive.candidate_signature_path",
    "archive.allowed_signers_path",
    "archive.executor_path",
    "archive.verifier_path",
    "manifest.url",
    "manifest.name",
    "manifest.sha256",
    "signature.url",
    "signature.name",
    "signature.sha256",
    "allowed_signers.url",
    "allowed_signers.sha256",
    "allowed_signers.key_fingerprint",
    "workload.executor_sha256",
    "workload.verifier_sha256",
    "workload.canonical_output_root",
)


class IdentityError(RuntimeError):
    """The candidate identity is incomplete or inconsistent."""


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def nested(document: dict[str, Any], field: str) -> Any:
    value: Any = document
    for component in field.split("."):
        if not isinstance(value, dict) or component not in value:
            raise IdentityError(f"candidate identity lacks {field}")
        value = value[component]
    return value


def safe_relative(value: str, *, one_component: bool = False) -> bool:
    if not isinstance(value, str) or not value or "\\" in value or "\n" in value or "\r" in value:
        return False
    path = pathlib.PurePosixPath(value)
    if path.is_absolute() or any(component in ("", ".", "..") for component in value.split("/")):
        return False
    if any(not SAFE_NAME.fullmatch(component) for component in value.split("/")):
        return False
    return not one_component or len(path.parts) == 1


def validate_url(value: str, expected_name: str, label: str) -> None:
    parsed = urllib.parse.urlsplit(value)
    decoded_path = urllib.parse.unquote(parsed.path).removeprefix("/")
    if (
        parsed.scheme != "https"
        or parsed.hostname not in {"mfenx.com", "www.mfenx.com"}
        or parsed.username is not None
        or parsed.password is not None
        or parsed.port is not None
        or parsed.query
        or parsed.fragment
        or not safe_relative(decoded_path)
        or pathlib.PurePosixPath(parsed.path).name != expected_name
    ):
        raise IdentityError(f"{label} must be an exact HTTPS mfenx.com asset URL ending in {expected_name!r}")


def validate_key_sets(document: dict[str, Any]) -> None:
    if set(document) != EXPECTED_KEYS[""]:
        raise IdentityError("candidate identity top-level keys differ from the frozen schema")
    for section in ("archive", "manifest", "signature", "allowed_signers", "workload"):
        value = document.get(section)
        if not isinstance(value, dict) or set(value) != EXPECTED_KEYS[section]:
            raise IdentityError(f"candidate identity {section} keys differ from the frozen schema")


def validate_identity(document: Any, *, require_enabled: bool) -> dict[str, Any]:
    if not isinstance(document, dict):
        raise IdentityError("candidate identity must be a JSON object")
    validate_key_sets(document)
    if document.get("schema") != SCHEMA:
        raise IdentityError("candidate identity schema is not supported")
    if not isinstance(document.get("enabled"), bool):
        raise IdentityError("candidate identity enabled gate must be boolean")
    if document["signature"]["principal"] != "mfenx-release":
        raise IdentityError("signature.principal must be mfenx-release")
    if document["signature"]["namespace"] != "mfenx-validation-candidate":
        raise IdentityError("signature.namespace must be mfenx-validation-candidate")
    if document["allowed_signers"]["name"] != "allowed_signers":
        raise IdentityError("allowed_signers.name must be allowed_signers")

    if document["enabled"] is False:
        partial = [field for field in PLACEHOLDER_FIELDS if not isinstance(nested(document, field), str) or not PLACEHOLDER.fullmatch(nested(document, field))]
        if partial:
            raise IdentityError("disabled candidate identity is partially populated: " + ", ".join(partial))
        if require_enabled:
            raise IdentityError("candidate identity is disabled; final signed tuple has not been installed")
        return document

    if any(PLACEHOLDER.fullmatch(str(nested(document, field))) for field in PLACEHOLDER_FIELDS):
        raise IdentityError("enabled candidate identity still contains a final-candidate placeholder")
    if not SAFE_TOKEN.fullmatch(document["release_id"]):
        raise IdentityError("release_id is not a safe immutable identifier")

    for section in ("archive", "manifest", "signature", "allowed_signers"):
        name = document[section]["name"]
        if not isinstance(name, str) or not SAFE_NAME.fullmatch(name):
            raise IdentityError(f"{section}.name is not a safe basename")
        validate_url(document[section]["url"], name, f"{section}.url")

    archive = document["archive"]
    if not safe_relative(archive["root"], one_component=True):
        raise IdentityError("archive.root must be one safe path component")
    for field in (
        "source_subdir",
        "candidate_manifest_path",
        "candidate_signature_path",
        "allowed_signers_path",
        "executor_path",
        "verifier_path",
    ):
        if not safe_relative(archive[field]):
            raise IdentityError(f"archive.{field} must be a safe relative path")
    if len(
        {
            archive["candidate_manifest_path"],
            archive["candidate_signature_path"],
            archive["allowed_signers_path"],
            archive["executor_path"],
            archive["verifier_path"],
        }
    ) != 5:
        raise IdentityError("candidate archive identity paths must be distinct")

    for field in (
        "archive.sha256",
        "manifest.sha256",
        "signature.sha256",
        "allowed_signers.sha256",
        "workload.executor_sha256",
        "workload.verifier_sha256",
        "workload.canonical_output_root",
    ):
        value = nested(document, field)
        if not isinstance(value, str) or not HEX64.fullmatch(value):
            raise IdentityError(f"{field} must be a lowercase SHA-256 digest")

    if not SSH_FINGERPRINT.fullmatch(document["allowed_signers"]["key_fingerprint"]):
        raise IdentityError("allowed_signers.key_fingerprint is not a SHA-256 SSH fingerprint")
    return document


def load_identity(path: pathlib.Path, *, require_enabled: bool = True) -> dict[str, Any]:
    if path.is_symlink() or not path.is_file():
        raise IdentityError(f"candidate identity is missing, non-regular, or symlinked: {path}")
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise IdentityError(f"cannot parse candidate identity: {error}") from error
    return validate_identity(document, require_enabled=require_enabled)


def signer_keys(path: pathlib.Path, principal: str, namespace: str) -> list[tuple[str, str]]:
    matches: list[tuple[str, str]] = []
    for number, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        stripped = raw.strip()
        if not stripped or stripped.startswith("#"):
            continue
        try:
            tokens = shlex.split(stripped, comments=True, posix=True)
        except ValueError as error:
            raise IdentityError(f"invalid allowed_signers line {number}: {error}") from error
        key_indexes = [index for index, token in enumerate(tokens) if KEY_TYPE.fullmatch(token)]
        if len(key_indexes) != 1 or key_indexes[0] + 1 >= len(tokens):
            raise IdentityError(f"allowed_signers line {number} has no unique supported public key")
        key_index = key_indexes[0]
        principals = tokens[0].split(",")
        options = []
        for token in tokens[1:key_index]:
            options.extend(token.split(","))
        namespaces = [token.removeprefix("namespaces=") for token in options if token.startswith("namespaces=")]
        if principal in principals and namespaces == [namespace]:
            matches.append((tokens[key_index], tokens[key_index + 1]))
    return matches


def signer_fingerprint(key_type: str, key_data: str) -> str:
    completed = subprocess.run(
        ["ssh-keygen", "-lf", "-"],
        input=f"{key_type} {key_data}\n".encode("ascii"),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode != 0:
        raise IdentityError("ssh-keygen could not fingerprint the allowed signer key")
    fields = completed.stdout.decode("utf-8", errors="replace").split()
    if len(fields) < 2:
        raise IdentityError("ssh-keygen returned no allowed signer fingerprint")
    return fields[1]


def validate_signed_manifest(manifest: Any, identity: dict[str, Any]) -> list[dict[str, Any]]:
    if not isinstance(manifest, dict):
        raise IdentityError("candidate manifest must be a JSON object")
    signature = manifest.get("signature")
    subject = manifest.get("subject")
    if (
        manifest.get("schema") != "mfenx.validation-candidate-manifest.v1"
        or manifest.get("release_id") != identity["release_id"]
        or manifest.get("release_class") != "validation_candidate"
        or manifest.get("release_status") != "signed_validation_candidate"
        or not isinstance(signature, dict)
        or signature.get("signer_identity") != identity["signature"]["principal"]
        or signature.get("namespace") != identity["signature"]["namespace"]
        or signature.get("public_key_fingerprint")
        != identity["allowed_signers"]["key_fingerprint"]
        or signature.get("signature_present") is not True
        or not isinstance(subject, dict)
    ):
        raise IdentityError("signed manifest release or signature policy differs from the frozen identity")

    executor = subject.get("executor")
    verifier = subject.get("reference_verifier")
    archive = identity["archive"]
    workload = identity["workload"]
    if (
        not isinstance(executor, dict)
        or executor.get("path") != archive["executor_path"]
        or executor.get("sha256") != workload["executor_sha256"]
        or not isinstance(verifier, dict)
        or verifier.get("path") != archive["verifier_path"]
        or verifier.get("sha256") != workload["verifier_sha256"]
    ):
        raise IdentityError("signed manifest subject does not bind the frozen executor and verifier")

    artifacts = manifest.get("artifacts")
    if not isinstance(artifacts, list) or not artifacts:
        raise IdentityError("candidate manifest has no artifacts array")
    expected_artifacts = (
        {
            "path": archive["executor_path"],
            "role": "candidate_executor_binary",
            "sha256": workload["executor_sha256"],
        },
        {
            "path": archive["verifier_path"],
            "role": "reference_verifier_binary",
            "sha256": workload["verifier_sha256"],
        },
    )
    for expected in expected_artifacts:
        matches = [
            item
            for item in artifacts
            if isinstance(item, dict)
            and all(item.get(field) == value for field, value in expected.items())
        ]
        if len(matches) != 1:
            raise IdentityError(
                f"candidate manifest does not bind exactly one signed {expected['role']}"
            )
    return artifacts


def verify_files(args: argparse.Namespace) -> int:
    identity = load_identity(args.identity)
    paths = {
        "archive": args.archive,
        "manifest": args.manifest,
        "signature": args.signature,
        "allowed_signers": args.allowed_signers,
    }
    for label, path in paths.items():
        if path.is_symlink() or not path.is_file():
            raise IdentityError(f"{label} input is missing, non-regular, or symlinked")
        observed = sha256_file(path)
        if observed != identity[label]["sha256"]:
            raise IdentityError(f"{label} SHA-256 differs from the frozen identity")

    manifest_bytes = args.manifest.read_bytes()
    completed = subprocess.run(
        [
            "ssh-keygen",
            "-Y",
            "verify",
            "-f",
            str(args.allowed_signers),
            "-I",
            identity["signature"]["principal"],
            "-n",
            identity["signature"]["namespace"],
            "-s",
            str(args.signature),
        ],
        input=manifest_bytes,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode != 0:
        raise IdentityError("candidate manifest SSH signature verification failed")

    keys = signer_keys(
        args.allowed_signers,
        identity["signature"]["principal"],
        identity["signature"]["namespace"],
    )
    if len(keys) != 1:
        raise IdentityError("allowed_signers must contain exactly one namespace-restricted candidate key")
    observed_fingerprint = signer_fingerprint(*keys[0])
    if observed_fingerprint != identity["allowed_signers"]["key_fingerprint"]:
        raise IdentityError("release key fingerprint differs from the frozen identity")

    try:
        manifest = json.loads(manifest_bytes)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise IdentityError(f"candidate manifest is not valid JSON: {error}") from error
    validate_signed_manifest(manifest, identity)

    report = {
        "schema": "mfenx.step3-candidate-input-verification.v1",
        "release_id": identity["release_id"],
        "identity_sha256": sha256_file(args.identity),
        "archive_sha256": sha256_file(args.archive),
        "manifest_sha256": sha256_file(args.manifest),
        "signature_sha256": sha256_file(args.signature),
        "allowed_signers_sha256": sha256_file(args.allowed_signers),
        "release_key_fingerprint": observed_fingerprint,
        "signature_principal": identity["signature"]["principal"],
        "signature_namespace": identity["signature"]["namespace"],
        "signature_verified": True,
        "outer_archive_digest_verified": True,
        "signed_executor_and_verifier_artifacts_verified": True,
    }
    print(json.dumps(report, sort_keys=True, separators=(",", ":")))
    return 0


def verify_extracted(args: argparse.Namespace) -> int:
    identity = load_identity(args.identity)
    root = args.root
    if root.is_symlink() or not root.is_dir():
        raise IdentityError("extracted candidate root is missing, non-directory, or symlinked")
    for path in root.rglob("*"):
        if path.is_symlink() or (not path.is_file() and not path.is_dir()):
            raise IdentityError(f"extracted candidate contains a special object: {path}")

    archive = identity["archive"]
    embedded_manifest = root / archive["candidate_manifest_path"]
    embedded_signature = root / archive["candidate_signature_path"]
    embedded_allowed_signers = root / archive["allowed_signers_path"]
    for embedded, supplied, label in (
        (embedded_manifest, args.manifest, "candidate manifest"),
        (embedded_signature, args.signature, "candidate signature"),
        (embedded_allowed_signers, args.allowed_signers, "allowed signers policy"),
    ):
        if embedded.is_symlink() or not embedded.is_file():
            raise IdentityError(f"archive lacks its regular embedded {label}")
        if sha256_file(embedded) != sha256_file(supplied):
            raise IdentityError(f"archive embedded {label} differs from the verified external input")

    manifest = json.loads(embedded_manifest.read_bytes())
    artifacts = validate_signed_manifest(manifest, identity)
    expected: dict[str, dict[str, Any]] = {}
    for index, artifact in enumerate(artifacts):
        if not isinstance(artifact, dict) or set(artifact) != {
            "media_type",
            "mode",
            "path",
            "role",
            "sha256",
            "size_bytes",
        }:
            raise IdentityError(f"signed manifest artifact {index} has an unexpected schema")
        relative = artifact.get("path")
        if not safe_relative(relative) or relative in expected:
            raise IdentityError(f"signed manifest artifact {index} has an unsafe or duplicate path")
        if (
            artifact.get("mode") not in {"0444", "0555"}
            or not HEX64.fullmatch(str(artifact.get("sha256")))
            or not isinstance(artifact.get("size_bytes"), int)
            or isinstance(artifact.get("size_bytes"), bool)
            or artifact["size_bytes"] < 0
        ):
            raise IdentityError(f"signed manifest artifact {index} metadata is invalid")
        expected[relative] = artifact

    manifest_relative = archive["candidate_manifest_path"]
    signature_relative = archive["candidate_signature_path"]
    if manifest_relative in expected or signature_relative in expected:
        raise IdentityError("signed manifest improperly enumerates itself or its detached signature")
    expected_files = set(expected) | {manifest_relative, signature_relative}
    actual_files = {
        path.relative_to(root).as_posix(): path
        for path in root.rglob("*")
        if path.is_file()
    }
    if set(actual_files) != expected_files:
        missing = sorted(expected_files - set(actual_files))
        extra = sorted(set(actual_files) - expected_files)
        raise IdentityError(
            f"archive file closure differs from the signed candidate closure; missing={missing}, extra={extra}"
        )
    for relative, artifact in expected.items():
        path = actual_files[relative]
        observed_mode = f"0{path.stat().st_mode & 0o777:03o}"
        if (
            sha256_file(path) != artifact["sha256"]
            or path.stat().st_size != artifact["size_bytes"]
            or observed_mode != artifact["mode"]
        ):
            raise IdentityError(f"archive artifact differs from the signed manifest: {relative}")

    executor = root / archive["executor_path"]
    verifier = root / archive["verifier_path"]
    source = root / archive["source_subdir"]
    if (
        not os.access(executor, os.X_OK)
        or sha256_file(executor) != identity["workload"]["executor_sha256"]
        or not os.access(verifier, os.X_OK)
        or sha256_file(verifier) != identity["workload"]["verifier_sha256"]
    ):
        raise IdentityError("extracted executor or verifier differs from the frozen signed identity")
    if source.is_symlink() or not source.is_dir() or not (source / "Cargo.toml").is_file():
        raise IdentityError("archive lacks the frozen candidate source directory")
    report = {
        "schema": "mfenx.step3-candidate-archive-closure.v1",
        "release_id": identity["release_id"],
        "file_count": len(actual_files),
        "signed_artifact_count": len(expected),
        "embedded_signed_inputs_match": True,
        "exact_signed_file_closure": True,
        "executor_sha256": sha256_file(executor),
        "verifier_sha256": sha256_file(verifier),
        "source_subdir": archive["source_subdir"],
    }
    print(json.dumps(report, sort_keys=True, separators=(",", ":")))
    return 0


def check_members(args: argparse.Namespace) -> int:
    identity = load_identity(args.identity)
    expected_root = identity["archive"]["root"]
    seen: set[str] = set()
    for number, raw in enumerate(args.members.read_text(encoding="utf-8").splitlines(), 1):
        value = raw.removesuffix("/")
        if not safe_relative(value) or any(ord(character) < 32 or ord(character) == 127 for character in value):
            raise IdentityError(f"archive member {number} has an unsafe path")
        if pathlib.PurePosixPath(value).parts[0] != expected_root:
            raise IdentityError(f"archive member {number} is outside the frozen archive root")
        if value in seen:
            raise IdentityError(f"archive contains duplicate normalized member {value!r}")
        seen.add(value)
    if expected_root not in seen:
        raise IdentityError("archive lacks its declared top-level root")
    return 0


def export_github(args: argparse.Namespace) -> int:
    identity = load_identity(args.identity)
    fields = {
        "release_id": identity["release_id"],
        "identity_sha256": sha256_file(args.identity),
        "archive_url": identity["archive"]["url"],
        "archive_name": identity["archive"]["name"],
        "archive_root": identity["archive"]["root"],
        "archive_source_subdir": identity["archive"]["source_subdir"],
        "manifest_url": identity["manifest"]["url"],
        "manifest_name": identity["manifest"]["name"],
        "signature_url": identity["signature"]["url"],
        "signature_name": identity["signature"]["name"],
        "allowed_signers_url": identity["allowed_signers"]["url"],
        "allowed_signers_name": identity["allowed_signers"]["name"],
    }
    for key, value in fields.items():
        if not isinstance(value, str) or "\n" in value or "\r" in value:
            raise IdentityError(f"unsafe GitHub output value for {key}")
        print(f"{key}={value}")
    return 0


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    commands = result.add_subparsers(dest="command", required=True)
    check = commands.add_parser("check")
    check.add_argument("--identity", required=True, type=pathlib.Path)
    check.add_argument("--require-enabled", action="store_true")
    check.set_defaults(function=lambda args: (load_identity(args.identity, require_enabled=args.require_enabled), 0)[1])
    export = commands.add_parser("export-github")
    export.add_argument("--identity", required=True, type=pathlib.Path)
    export.set_defaults(function=export_github)
    verify = commands.add_parser("verify-files")
    verify.add_argument("--identity", required=True, type=pathlib.Path)
    verify.add_argument("--archive", required=True, type=pathlib.Path)
    verify.add_argument("--manifest", required=True, type=pathlib.Path)
    verify.add_argument("--signature", required=True, type=pathlib.Path)
    verify.add_argument("--allowed-signers", required=True, type=pathlib.Path)
    verify.set_defaults(function=verify_files)
    extracted = commands.add_parser("verify-extracted")
    extracted.add_argument("--identity", required=True, type=pathlib.Path)
    extracted.add_argument("--root", required=True, type=pathlib.Path)
    extracted.add_argument("--manifest", required=True, type=pathlib.Path)
    extracted.add_argument("--signature", required=True, type=pathlib.Path)
    extracted.add_argument("--allowed-signers", required=True, type=pathlib.Path)
    extracted.set_defaults(function=verify_extracted)
    members = commands.add_parser("check-members")
    members.add_argument("--identity", required=True, type=pathlib.Path)
    members.add_argument("--members", required=True, type=pathlib.Path)
    members.set_defaults(function=check_members)
    return result


def main() -> int:
    try:
        arguments = parser().parse_args()
        return int(arguments.function(arguments))
    except (IdentityError, OSError, UnicodeError, ValueError, json.JSONDecodeError) as error:
        print(f"Step 3 candidate identity error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())

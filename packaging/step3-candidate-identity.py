#!/usr/bin/env python3
"""Validate and consume the one frozen Step 3 release identity."""

from __future__ import annotations

import argparse
import hashlib
import json
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
    "archive": {"url", "name", "root", "sha256", "source_subdir", "manifest_path", "manifest_role"},
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
    "archive.manifest_path",
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
    if document["archive"]["manifest_role"] != "revisioned_distribution_archive":
        raise IdentityError("archive.manifest_role is not the frozen release role")
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
    if not safe_relative(archive["source_subdir"]):
        raise IdentityError("archive.source_subdir must be a safe relative path")
    if not safe_relative(archive["manifest_path"]):
        raise IdentityError("archive.manifest_path must be a safe relative path")

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
    artifacts = manifest.get("artifacts") if isinstance(manifest, dict) else None
    if not isinstance(artifacts, list):
        raise IdentityError("candidate manifest has no artifacts array")
    expected = {
        "path": identity["archive"]["manifest_path"],
        "role": identity["archive"]["manifest_role"],
        "sha256": identity["archive"]["sha256"],
    }
    matches = [
        item
        for item in artifacts
        if isinstance(item, dict)
        and all(item.get(field) == value for field, value in expected.items())
    ]
    if len(matches) != 1:
        raise IdentityError("candidate manifest does not bind exactly one frozen archive artifact")

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
        "archive_manifest_binding_verified": True,
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

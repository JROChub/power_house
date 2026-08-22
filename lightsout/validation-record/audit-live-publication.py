#!/usr/bin/env python3
"""Retrieve the staged Validation Record publication over HTTPS and seal the result.

Run only after the evidence inputs, signed record, and detached signature are
live.  The script refuses redirects, content changes, incomplete URL sets, and
existing output paths.  It makes the first live-HTTP claim in this publication
sequence; the prepublication record intentionally does not.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import ssl
import sys
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any


HERE = Path(__file__).resolve().parent
SITE_ROOT = HERE.parents[1]
PUBLIC_ROOT = "https://mfenx.com/lightsout/validation-record/"
DIGEST_RE = re.compile(r"^[0-9a-f]{64}$")


class AuditError(RuntimeError):
    pass


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):  # type: ignore[no-untyped-def]
        return None


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AuditError(message)


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def local_bytes(relative: str) -> bytes:
    require(relative and not relative.startswith("/") and "\\" not in relative, f"unsafe local path {relative!r}")
    target = (HERE / relative).resolve()
    require(target.is_relative_to(SITE_ROOT) and target.is_file() and not target.is_symlink(), f"missing or unsafe local file {relative}")
    return target.read_bytes()


def retrieve(opener: urllib.request.OpenerDirector, *, role: str, url: str, expected: bytes, media_type: str) -> dict[str, Any]:
    request = urllib.request.Request(
        url,
        headers={
            "Accept": "*/*",
            "Cache-Control": "no-cache",
            "Pragma": "no-cache",
            "User-Agent": "mfenx-post-publication-audit/1",
        },
        method="GET",
    )
    try:
        with opener.open(request, timeout=45) as response:
            observed = response.read(len(expected) + 1)
            status = response.status
            final_url = response.geturl()
            content_type = response.headers.get_content_type()
    except (OSError, urllib.error.URLError, urllib.error.HTTPError) as exc:
        raise AuditError(f"HTTPS retrieval failed for {role}: {exc}") from exc
    require(status == 200, f"unexpected HTTP status for {role}: {status}")
    require(final_url == url, f"redirect or URL rewrite for {role}: {final_url}")
    require(len(observed) == len(expected), f"retrieved size mismatch for {role}: expected {len(expected)}, got {len(observed)}")
    require(observed == expected, f"retrieved bytes differ for {role}")
    return {
        "role": role,
        "url": url,
        "status": "pass",
        "transport": "https",
        "http_status": status,
        "content_type_observed": content_type,
        "media_type_expected": media_type,
        "expected_sha256": sha256(expected),
        "observed_sha256": sha256(observed),
        "expected_size_bytes": len(expected),
        "observed_size_bytes": len(observed),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--audited-at", required=True, help="Exact RFC 3339 UTC timestamp")
    parser.add_argument("--record-sha256", required=True)
    parser.add_argument("--signature-sha256", required=True)
    parser.add_argument("--output", type=Path, default=HERE / "post-publication-retrieval-attestation.json")
    args = parser.parse_args()
    require(re.fullmatch(r"20[0-9]{2}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z", args.audited_at) is not None, "--audited-at is not canonical RFC 3339 UTC")
    require(DIGEST_RE.fullmatch(args.record_sha256) is not None, "invalid record SHA-256")
    require(DIGEST_RE.fullmatch(args.signature_sha256) is not None, "invalid signature SHA-256")
    require(not args.output.exists() and not args.output.is_symlink(), f"refusing to overwrite output {args.output}")

    index_bytes = local_bytes("release-index.json")
    index = json.loads(index_bytes.decode("utf-8"))
    require(index_bytes == canonical_bytes(index), "release index is not canonical JSON")
    require(index.get("schema") == "mfenx.public-release-index.v1" and index.get("status") == "publication_ready" and index.get("record_id") == "mfenx-local-v2-validation-record-20260822-a1", "release-index identity changed")
    require(index.get("all_input_evidence_staged_and_locally_verified") is True and index.get("all_input_evidence_live_http_retrieval_claimed") is False, "release-index prepublication boundary changed")
    require(index.get("validation_record_publication_state") == "not_yet_published" and index.get("validation_record_live_retrieval_claimed") is False and index.get("post_publication_retrieval_attestation_required") is True, "release-index publication lifecycle changed")
    artifacts = index.get("artifacts")
    require(isinstance(artifacts, list) and len(artifacts) >= 50 and index.get("artifact_count") == len(artifacts), "release-index artifact population changed")

    prepublication_bytes = local_bytes("prepublication-input-retrieval-verification.json")
    prepublication = json.loads(prepublication_bytes.decode("utf-8"))
    require(prepublication_bytes == canonical_bytes(prepublication), "prepublication verification is not canonical JSON")
    require(prepublication.get("schema") == "mfenx.prepublication-input-retrieval-verification.v1" and prepublication.get("status") == "publication_ready" and prepublication.get("lifecycle") == "prepublication_only", "prepublication verification identity changed")
    require(prepublication.get("validation_record_publication_state") == "not_yet_published" and prepublication.get("validation_record_live_retrieval_claimed") is False and prepublication.get("all_input_evidence_staged_and_locally_verified") is True and prepublication.get("all_input_evidence_live_http_retrieval_claimed") is False, "prepublication verification lifecycle changed")
    require(prepublication.get("release_index", {}).get("sha256") == sha256(index_bytes) and prepublication.get("release_index", {}).get("size_bytes") == len(index_bytes), "prepublication release-index binding changed")

    record_bytes = local_bytes("release/VALIDATION-RECORD.canonical.json")
    signature_bytes = local_bytes("release/VALIDATION-RECORD.canonical.json.sig")
    require(sha256(record_bytes) == args.record_sha256, "local record SHA-256 differs from argument")
    require(sha256(signature_bytes) == args.signature_sha256, "local signature SHA-256 differs from argument")
    record = json.loads(record_bytes.decode("utf-8"))
    require(record_bytes == canonical_bytes(record), "record is not canonical JSON")
    require(record.get("schema") == "mfenx.validation-record.v1" and record.get("record_id") == index.get("record_id"), "record and release-index identities differ")

    artifact_roles = {item["role"]: item for item in record["artifacts"]}
    require(len(artifact_roles) == record["artifact_count"], "record artifact roles are not unique")
    require(artifact_roles.get("public_release_index", {}).get("sha256") == sha256(index_bytes) and artifact_roles.get("prepublication_input_retrieval_verification", {}).get("sha256") == sha256(prepublication_bytes), "record publication-input bindings changed")
    for item in artifacts:
        signed = artifact_roles.get(item["role"])
        require(signed is not None and signed["sha256"] == item["sha256"] and signed["size_bytes"] == item["size_bytes"] and signed["media_type"] == item["media_type"], f"release-index role is not bound by record: {item['role']}")
    for role, relative in (
        ("claim_ledger", "release/CLAIM_LEDGER.md"),
        ("validation_record_allowed_signers", "release/allowed_signers"),
        ("release_public_key", "release/release-signing-key.pub"),
    ):
        expected = local_bytes(relative)
        signed = artifact_roles.get(role)
        require(signed is not None and signed["sha256"] == sha256(expected) and signed["size_bytes"] == len(expected), f"local policy artifact is not bound by record: {role}")

    ssl_context = ssl.create_default_context()
    opener = urllib.request.build_opener(NoRedirect(), urllib.request.HTTPSHandler(context=ssl_context))
    checks: list[dict[str, Any]] = []
    for item in artifacts:
        if item["public_path"].startswith("../"):
            relative = str((HERE / item["public_path"]).resolve().relative_to(SITE_ROOT))
            expected = (SITE_ROOT / relative).read_bytes()
        else:
            expected = local_bytes(item["public_path"])
        checks.append(retrieve(opener, role=item["role"], url=item["public_url"], expected=expected, media_type=item["media_type"]))

    web_summary = index["web_summary"]
    checks.append(retrieve(opener, role="public_validation_status", url=web_summary["public_url"], expected=local_bytes(web_summary["public_path"]), media_type=web_summary["media_type"]))

    specials = [
        ("public_release_index", PUBLIC_ROOT + "release-index.json", index_bytes, "application/json"),
        ("prepublication_input_retrieval_verification", PUBLIC_ROOT + "prepublication-input-retrieval-verification.json", prepublication_bytes, "application/json"),
        ("signed_validation_record", index["planned_validation_record_url"], record_bytes, "application/json"),
        ("signed_validation_record_signature", index["planned_signature_url"], signature_bytes, "application/octet-stream"),
        ("validation_record_allowed_signers", PUBLIC_ROOT + "release/allowed_signers", local_bytes("release/allowed_signers"), "text/plain"),
        ("validation_record_release_public_key", PUBLIC_ROOT + "release/release-signing-key.pub", local_bytes("release/release-signing-key.pub"), "text/plain"),
        ("validation_record_claim_ledger", PUBLIC_ROOT + "release/CLAIM_LEDGER.md", local_bytes("release/CLAIM_LEDGER.md"), "text/markdown"),
    ]
    for role, url, expected, media_type in specials:
        checks.append(retrieve(opener, role=role, url=url, expected=expected, media_type=media_type))

    urls = [row["url"] for row in checks]
    require(len(urls) == len(set(urls)), "live retrieval URL set contains duplicates")
    record = {
        "schema": "mfenx.post-publication-retrieval-attestation.v1",
        "status": "PASS",
        "audited_at_utc": args.audited_at,
        "release_id": index["release_id"],
        "record_id": index["record_id"],
        "site_origin": "https://mfenx.com",
        "live_http_retrieval_performed": True,
        "live_http_retrieval_claimed": True,
        "successes_and_failures_unfiltered": True,
        "release_index": {"url": PUBLIC_ROOT + "release-index.json", "sha256": sha256(index_bytes), "size_bytes": len(index_bytes)},
        "record": {"url": index["planned_validation_record_url"], "sha256": sha256(record_bytes), "size_bytes": len(record_bytes)},
        "signature": {"url": index["planned_signature_url"], "sha256": sha256(signature_bytes), "size_bytes": len(signature_bytes)},
        "checks": checks,
        "check_count": len(checks),
        "passed": len(checks),
        "failed": 0,
        "claim_boundary": "This record establishes exact HTTPS retrieval at the recorded time for the enumerated URLs. It does not authenticate publisher identity by URL alone, guarantee future availability, or alter any technical claim; verify pinned hashes and the detached signature independently.",
    }
    payload = canonical_bytes(record)
    args.output.write_bytes(payload)
    args.output.chmod(0o444)
    print(json.dumps({"path": str(args.output), "sha256": sha256(payload), "size_bytes": len(payload), "checks": len(checks)}, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (AuditError, OSError, ValueError, KeyError, json.JSONDecodeError) as error:
        print(f"audit-live-publication: ERROR: {error}", file=sys.stderr)
        raise SystemExit(1)

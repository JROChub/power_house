#!/usr/bin/env python3
"""Stage the bounded MFENX Validation Record public evidence package.

The output is deliberately a *prepublication* package.  It never claims that
the planned HTTPS routes were retrieved.  A separate, post-deploy audit must
make that claim after the signed Validation Record and site are live.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import mimetypes
import re
import shutil
import sys
from pathlib import Path
from typing import Any, Iterable


SITE_ROOT = Path(__file__).resolve().parents[2]
REPO_ROOT = Path(__file__).resolve().parents[3]
OUTPUT_ROOT = Path(__file__).resolve().parent
PUBLIC_ROOT = "https://mfenx.com/lightsout/validation-record/"
VALIDATION_RECORD_URL = PUBLIC_ROOT + "release/VALIDATION-RECORD.canonical.json"
SIGNATURE_URL = VALIDATION_RECORD_URL + ".sig"

ARTIFACT_KEYS = frozenset({"role", "path", "sha256", "size_bytes", "media_type"})
ROLE_RE = re.compile(r"^[a-z0-9][a-z0-9_]{1,95}$")
DIGEST_RE = re.compile(r"^[0-9a-f]{64}$")

# These five exact candidate objects are already published and browser-audited.
# Reusing their routes avoids a second copy of the signed archive.
EXISTING_SITE_ROUTES = {
    "validation_candidate_manifest": "candidate/release/VALIDATION-CANDIDATE-MANIFEST.canonical.json",
    "validation_candidate_signature": "candidate/release/VALIDATION-CANDIDATE-MANIFEST.canonical.json.sig",
    "validation_candidate_allowed_signers": "candidate/release/allowed_signers",
    "validation_candidate_signed_archive": "candidate/downloads/mfenx-local-v2-validation-candidate-20260822-a1-signed-x86_64-unknown-linux-gnu.tar.zst",
    "validation_candidate_archive_sidecar": "candidate/downloads/mfenx-local-v2-validation-candidate-20260822-a1-signed-x86_64-unknown-linux-gnu.tar.zst.sha256",
}

# Public routes use neutral release terminology even when a retained source
# artifact has a historical filename.
PUBLIC_BASENAMES = {
    "hosted_workflow_attestation": "three-host-aggregate.verification.json",
}


class PublicationError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise PublicationError(message)


def canonical_bytes(value: Any) -> bytes:
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")) + "\n").encode("utf-8")


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def safe_repo_file(raw_path: str) -> Path:
    require(isinstance(raw_path, str) and raw_path and not raw_path.startswith("/"), "artifact path is not relative")
    require("\\" not in raw_path, f"artifact path contains a backslash: {raw_path!r}")
    parts = Path(raw_path).parts
    require(all(part not in ("", ".", "..") for part in parts), f"artifact path is unsafe: {raw_path!r}")
    candidate = (REPO_ROOT / raw_path).resolve()
    require(candidate.is_relative_to(REPO_ROOT), f"artifact path left repository: {raw_path!r}")
    require(candidate.is_file() and not candidate.is_symlink(), f"artifact is absent or not a regular non-symlink file: {raw_path}")
    return candidate


def iter_artifacts(value: Any, *, route: tuple[str, ...] = ()) -> Iterable[dict[str, Any]]:
    # Publication artifacts are outputs of this script and must not be inputs to
    # their own release index.
    if route and route[0] == "publication":
        return
    if isinstance(value, dict):
        if ARTIFACT_KEYS.issubset(value):
            yield value
            return
        for key in sorted(value):
            yield from iter_artifacts(value[key], route=route + (key,))
    elif isinstance(value, list):
        for index, item in enumerate(value):
            yield from iter_artifacts(item, route=route + (str(index),))


def validate_artifact(record: dict[str, Any]) -> tuple[Path, dict[str, Any]]:
    role = record.get("role")
    digest = record.get("sha256")
    size = record.get("size_bytes")
    media_type = record.get("media_type")
    require(isinstance(role, str) and ROLE_RE.fullmatch(role) is not None, f"invalid artifact role: {role!r}")
    require(isinstance(digest, str) and DIGEST_RE.fullmatch(digest) is not None, f"invalid digest for {role}")
    require(isinstance(size, int) and not isinstance(size, bool) and size >= 0, f"invalid size for {role}")
    require(isinstance(media_type, str) and "/" in media_type, f"invalid media type for {role}")
    source = safe_repo_file(record["path"])
    actual_size = source.stat().st_size
    actual_digest = sha256_file(source)
    require(actual_size == size, f"size mismatch for {role}: expected {size}, got {actual_size}")
    require(actual_digest == digest, f"SHA-256 mismatch for {role}: expected {digest}, got {actual_digest}")
    return source, {
        "media_type": media_type,
        "role": role,
        "sha256": digest,
        "size_bytes": size,
        "source_path": record["path"],
    }


def stage_artifact(source: Path, record: dict[str, Any]) -> dict[str, Any]:
    role = record["role"]
    existing_route = EXISTING_SITE_ROUTES.get(role)
    if existing_route is not None:
        target = SITE_ROOT / "lightsout" / existing_route
        require(target.is_file() and not target.is_symlink(), f"existing public candidate route is absent: {existing_route}")
        public_path = "../" + existing_route
        staging_kind = "existing_site_asset"
    else:
        basename = PUBLIC_BASENAMES.get(role, source.name)
        require(basename not in ("", ".", ".."), f"unsafe basename for {role}")
        target = OUTPUT_ROOT / "inputs" / role / basename
        require(target.resolve().is_relative_to(OUTPUT_ROOT), f"staging target left output root for {role}")
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        public_path = "inputs/" + role + "/" + basename
        staging_kind = "copied_input_evidence"

    staged_size = target.stat().st_size
    staged_digest = sha256_file(target)
    require(staged_size == record["size_bytes"], f"staged size mismatch for {role}")
    require(staged_digest == record["sha256"], f"staged digest mismatch for {role}")
    return {
        "media_type": record["media_type"],
        "role": role,
        "sha256": record["sha256"],
        "size_bytes": record["size_bytes"],
        "public_path": public_path,
        "public_url": PUBLIC_ROOT + public_path if not public_path.startswith("../") else "https://mfenx.com/lightsout/" + existing_route,
        "staging_kind": staging_kind,
    }


def summary_from_input(data: dict[str, Any]) -> dict[str, Any]:
    scaling = data["scaling"]
    hosted = data["hosted_reproduction"]
    security_a2 = data["automated_quality"]["hosted_security_history"]["a2"]
    review = data["independent_code_security_review"]
    human = data["independent_human_review"]
    workload = data["external_workload"]
    commercial = data["commercial_evaluation"]
    paper = data["technical_paper"]
    adversarial = data["adversarial_evidence"]
    v2 = scaling["prior_failed_v2"]
    review_doc = json.loads(safe_repo_file(review["report"]["path"]).read_text(encoding="utf-8"))
    review_counts = review_doc.get("finding_counts", {})

    require(adversarial["a3"]["attempted_exactly_once"] == 214, "A3 attempt count changed")
    require(adversarial["a3"]["driver_passed"] == 214 and adversarial["a3"]["independent_passed"] == 214, "A3 verdict changed")
    require(v2["successful_outcomes"] == 52 and v2["failed_outcomes"] == 48, "scaling v2 verdict changed")
    require(scaling["successful_outcomes"] == 100 and scaling["failed_outcomes"] == 0, "scaling v3 verdict changed")
    require(hosted["planned_vm_jobs"] == 3 and hosted["attempted_vm_jobs"] == 3 and hosted["successful_jobs"] == 3 and hosted["claim_eligible"] is True, "hosted reproduction verdict changed")
    require(security_a2["required_automated_tool_outcomes"] == 17 and security_a2["successful_required_automated_tool_outcomes"] == 17, "security A2 outcome count changed")
    require(security_a2["unwaived_codeql_result_count"] == 1, "security A2 finding count changed")
    require(review["disposition"] == "pass_with_findings" and review_counts.get("critical") == 0 and review_counts.get("high") == 0 and review_counts.get("medium") == 0 and review_counts.get("low") == 1 and review_counts.get("informational") == 1, "bounded review disposition changed")
    require(human["status"] == "not_performed" and human["open"] is True and human["completion_claimed"] is False, "human review boundary changed")
    require(workload["executor_accepted"] is True and workload["standalone_verifier_accepted"] is True and workload["independent_exact_oracle_accepted"] is True, "external workload verdict changed")
    require(commercial["open_executor"] is True and commercial["open_verifier"] is True and commercial["software_license"] == "Apache-2.0", "commercial/open boundary changed")
    require(paper["peer_reviewed"] is False, "paper peer-review boundary changed")

    return {
        "schema": "mfenx.public-validation-status.v1",
        "status": "publication_ready",
        "release_id": data["release_identity"]["release_id"],
        "candidate": {
            "executor_sha256": data["release_identity"]["executor_sha256"],
            "manifest_sha256": data["release_identity"]["candidate_signing_record"]["manifest"]["sha256"],
            "signature_verified": data["release_identity"]["candidate_signing_record"]["status"] == "verified",
            "verifier_sha256": data["release_identity"]["verifier_sha256"],
        },
        "validation": {
            "execution_contract": "complete",
            "reference_verification": "complete",
            "measured_reproduction": "complete",
            "technical_evaluation": "release_ready",
        },
        "adversarial": {
            "a1": {"status": "failed_retained", "frozen_passed": adversarial["a1"]["frozen_validator_passed"], "frozen_failed": adversarial["a1"]["frozen_validator_failed"]},
            "a2": {"status": "failed_retained", "driver_passed": adversarial["a2"]["driver_passed"], "driver_failed": adversarial["a2"]["driver_failed"], "independent_passed": adversarial["a2"]["independent_passed"], "independent_failed": adversarial["a2"]["independent_failed"]},
            "a3": {"status": "complete", "attempted_exactly_once": 214, "driver_passed": 214, "driver_failed": 0, "independent_passed": 214, "independent_failed": 0, "mutation_cases": 192, "restart_cases": 22},
        },
        "scaling": {
            "v2": {"status": "failed_retained", "success": 52, "failure": 48},
            "v3": {"status": "complete", "success": 100, "failure": 0, "sample_filtering_applied": False},
            "claim_boundary": "The 1/2/4-lane rows are physical-core observations. The 8/16-lane rows oversubscribe four physical cores. This is one host and workload, not a universal or cross-machine result.",
        },
        "hosted_reproduction": {
            "status": "complete",
            "github_hosted_vm_jobs": 3,
            "output_manifest_root": hosted["output_manifest_root"],
            "physical_host_attestation": False,
            "administrative_domains": 1,
            "source_to_binary_reproducibility": False,
        },
        "security": {
            "hosted_a1": "failed_retained_unfiltered",
            "hosted_a2": {
                "status": "failed_retained_unfiltered",
                "required_tool_outcomes": 17,
                "successful_tool_outcomes": 17,
                "unwaived_codeql_results": 1,
            },
            "procedurally_separate_ai_review": {
                "disposition": "pass_with_findings",
                "critical": review_counts["critical"],
                "high": review_counts["high"],
                "medium": review_counts["medium"],
                "low": review_counts["low"],
                "informational": review_counts["informational"],
                "human": False,
                "organizationally_independent": False,
            },
            "independent_human_review": {
                "status": "not_performed",
                "open": True,
                "completion_claimed": False,
                "request_url": human["public_review_request_url"],
            },
        },
        "external_workload": {
            "name": workload["workload"],
            "status": "complete",
            "executor_accepted": True,
            "standalone_verifier_accepted": True,
            "independent_exact_oracle_accepted": True,
            "output_manifest_root": workload["output_manifest_root"],
        },
        "commercial_boundary": {
            "software_license": "Apache-2.0",
            "open_executor": True,
            "open_verifier": True,
            "proprietary_executor_claimed": False,
            "paid_design_partner_count": commercial["paid_design_partner_count"],
            "customer_claimed": False,
            "payment_claimed": False,
            "sla_claimed": False,
            "design_partner_intake_status": commercial["design_partner_intake_status"],
        },
        "paper": {"contribution": paper["contribution"], "peer_reviewed": False, "status": paper["status"]},
        "claim_boundary": {
            "software_not_hardware": True,
            "supercomputer_hardware_claimed": False,
            "top500_claimed": False,
            "universal_performance_claimed": False,
            "security_certification_claimed": False,
            "production_fitness_claimed": False,
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--finalization-input", required=True, type=Path)
    parser.add_argument("--prepared-at", required=True, help="Exact RFC 3339 UTC timestamp")
    args = parser.parse_args()
    require(re.fullmatch(r"20[0-9]{2}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z", args.prepared_at) is not None, "--prepared-at is not canonical RFC 3339 UTC")
    for protected in ("release-index.json", "prepublication-input-retrieval-verification.json", "validation-status.json"):
        require(not (OUTPUT_ROOT / protected).exists(), f"refusing to overwrite frozen prepublication output {protected}")

    data = json.loads(args.finalization_input.read_text(encoding="utf-8"))
    require(data.get("schema") == "mfenx.validation-record-finalization-input.v1", "unexpected finalization-input schema")
    require(data.get("draft") is False, "refusing to stage a draft finalization input")
    require(data.get("commercial_evaluation", {}).get("status") == "release_ready", "commercial evaluation is not release-ready")
    require(data.get("technical_paper", {}).get("status") == "final", "technical paper is not final")

    validated: list[tuple[Path, dict[str, Any]]] = []
    seen_roles: set[str] = set()
    seen_sources: set[str] = set()
    for artifact in iter_artifacts(data):
        source, record = validate_artifact(artifact)
        require(record["role"] not in seen_roles, f"duplicate artifact role: {record['role']}")
        require(record["source_path"] not in seen_sources, f"duplicate artifact source path: {record['source_path']}")
        seen_roles.add(record["role"])
        seen_sources.add(record["source_path"])
        validated.append((source, record))
    require(len(validated) >= 50, f"input-evidence inventory unexpectedly small: {len(validated)}")

    staged = [stage_artifact(source, record) for source, record in sorted(validated, key=lambda item: item[1]["role"])]
    urls = [item["public_url"] for item in staged]
    require(len(set(urls)) == len(urls), "public input-evidence URLs are not unique")
    require(all(url.startswith("https://") for url in urls), "non-HTTPS public URL generated")

    status = summary_from_input(data)
    status_bytes = canonical_bytes(status)
    status_path = OUTPUT_ROOT / "validation-status.json"
    status_path.write_bytes(status_bytes)
    status_record = {
        "role": "public_validation_status",
        "public_path": "validation-status.json",
        "public_url": PUBLIC_ROOT + "validation-status.json",
        "sha256": sha256_bytes(status_bytes),
        "size_bytes": len(status_bytes),
        "media_type": "application/json",
        "staging_kind": "derived_bounded_web_summary",
    }

    release_index = {
        "schema": "mfenx.public-release-index.v1",
        "status": "publication_ready",
        "prepared_at_utc": args.prepared_at,
        "release_id": data["release_identity"]["release_id"],
        "record_id": data["record_id"],
        "validation_record_publication_state": "not_yet_published",
        "validation_record_live_retrieval_claimed": False,
        "planned_validation_record_url": VALIDATION_RECORD_URL,
        "planned_signature_url": SIGNATURE_URL,
        "post_publication_retrieval_attestation_required": True,
        "all_input_evidence_staged_and_locally_verified": True,
        "all_input_evidence_live_http_retrieval_claimed": False,
        "input_evidence_live_http_retrieval_claimed": False,
        "input_evidence_verification_kind": "local_source_to_staged_publication_tree_byte_verification",
        "successes_and_failures_unfiltered": True,
        "artifacts": staged,
        "artifact_count": len(staged),
        "public_input_evidence_urls": urls,
        "public_input_evidence_url_count": len(urls),
        "web_summary": status_record,
        "claim_boundary": "This prepublication index binds local staged bytes to planned HTTPS routes. It does not claim those routes are live or that HTTP retrieval succeeded. The Validation Record and signature URLs are plans only until a separate post-deploy retrieval attestation exists.",
    }
    index_bytes = canonical_bytes(release_index)
    index_path = OUTPUT_ROOT / "release-index.json"
    index_path.write_bytes(index_bytes)

    checks: list[dict[str, Any]] = []
    for item in staged:
        if item["staging_kind"] == "existing_site_asset":
            local = (OUTPUT_ROOT / item["public_path"]).resolve()
        else:
            local = OUTPUT_ROOT / item["public_path"]
        require(local.is_file() and not local.is_symlink(), f"staged file disappeared: {item['role']}")
        checks.append({
            "role": item["role"],
            "public_url": item["public_url"],
            "expected_sha256": item["sha256"],
            "observed_sha256": sha256_file(local),
            "expected_size_bytes": item["size_bytes"],
            "observed_size_bytes": local.stat().st_size,
            "status": "pass",
            "verification_transport": "local_filesystem",
        })
    checks.append({
        "role": status_record["role"],
        "public_url": status_record["public_url"],
        "expected_sha256": status_record["sha256"],
        "observed_sha256": sha256_file(status_path),
        "expected_size_bytes": status_record["size_bytes"],
        "observed_size_bytes": status_path.stat().st_size,
        "status": "pass",
        "verification_transport": "local_filesystem",
    })

    verification = {
        "schema": "mfenx.prepublication-input-retrieval-verification.v1",
        "status": "publication_ready",
        "lifecycle": "prepublication_only",
        "prepared_at_utc": args.prepared_at,
        "release_id": data["release_identity"]["release_id"],
        "release_index": {
            "role": "public_release_index",
            "path": "site-console/lightsout/validation-record/release-index.json",
            "sha256": sha256_file(index_path),
            "size_bytes": index_path.stat().st_size,
            "media_type": "application/json",
            "planned_public_url": PUBLIC_ROOT + "release-index.json",
        },
        "validation_record_publication_state": "not_yet_published",
        "validation_record_live_retrieval_claimed": False,
        "planned_validation_record_url": VALIDATION_RECORD_URL,
        "planned_signature_url": SIGNATURE_URL,
        "post_publication_retrieval_attestation_required": True,
        "all_input_evidence_staged_and_locally_verified": True,
        "all_input_evidence_live_http_retrieval_claimed": False,
        "live_http_retrieval_performed": False,
        "live_http_retrieval_claimed": False,
        "verification_kind": "local_source_to_staged_publication_tree_byte_verification",
        "checks": checks,
        "check_count": len(checks),
        "passed": len(checks),
        "failed": 0,
        "public_input_evidence_urls": urls,
        "public_input_evidence_url_count": len(urls),
        "claim_boundary": "Every check compares local source bytes with the staged GitHub Pages publication tree. No check used HTTP. No input-evidence, Validation Record, signature, or site-live retrieval claim is made; those require a separate post-deploy record.",
    }
    verification_bytes = canonical_bytes(verification)
    verification_path = OUTPUT_ROOT / "prepublication-input-retrieval-verification.json"
    verification_path.write_bytes(verification_bytes)

    output = {
        "public_release_index": {
            "role": "public_release_index",
            "path": "site-console/lightsout/validation-record/release-index.json",
            "sha256": sha256_file(index_path),
            "size_bytes": index_path.stat().st_size,
            "media_type": "application/json",
        },
        "prepublication_input_retrieval_verification": {
            "role": "prepublication_input_retrieval_verification",
            "path": "site-console/lightsout/validation-record/prepublication-input-retrieval-verification.json",
            "sha256": sha256_file(verification_path),
            "size_bytes": verification_path.stat().st_size,
            "media_type": "application/json",
        },
        "planned_validation_record_url": VALIDATION_RECORD_URL,
        "planned_signature_url": SIGNATURE_URL,
        "public_input_evidence_urls": urls,
    }
    sys.stdout.buffer.write(canonical_bytes(output))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, KeyError, PublicationError) as error:
        print(f"prepare-publication: ERROR: {error}", file=sys.stderr)
        raise SystemExit(1)

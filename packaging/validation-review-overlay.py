#!/usr/bin/env python3
"""Build and verify the analysis-only workspace used by the validation review.

The validation candidate intentionally remains immutable.  Its local-source
workspace manifest names crates that are not included in the release archive,
while the standalone verifier source is shipped in a separate archive path.
This tool creates a new, explicitly recorded workspace containing only the
four signed crates that are actually present, followed by the commit-bound
review harness.  The resulting directory is an analysis adapter, not release
source.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import pathlib
import re
import shutil
import stat
import sys


SCHEMA = "mfenx.step3-review-overlay.v1"
IDENTITY_SCHEMA = "mfenx.step3-candidate-identity.v1"
LOCAL_CRATES = (
    "rarecomp-mfenx-ir",
    "rarecomp-mfenx-local",
    "rarecomp-mfenx-tensor-store",
)
VERIFIER_CRATE = "mfenx-contract-v1-verifier"
WORKSPACE_MEMBERS = tuple(f"crates/{name}" for name in (*LOCAL_CRATES, VERIFIER_CRATE))
LOCAL_SUPPORT_FILES = ("LICENSE", "rust-toolchain.toml")
SIGNED_LOCK_FILE = "Cargo.lock"
ADAPTER_LOCK_FILE = pathlib.PurePosixPath("workspace-adapter/Cargo.lock")


class OverlayError(RuntimeError):
    """The requested overlay was not complete or provenance-safe."""


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def canonical_sha256(value: object) -> str:
    encoded = json.dumps(value, sort_keys=True, separators=(",", ":")).encode("ascii")
    return hashlib.sha256(encoded).hexdigest()


def regular_inventory(root: pathlib.Path) -> list[dict[str, object]]:
    if not root.is_dir() or root.is_symlink():
        raise OverlayError(f"not a regular directory: {root}")
    entries: list[dict[str, object]] = []
    for path in sorted(root.rglob("*"), key=lambda item: item.as_posix()):
        relative = path.relative_to(root).as_posix()
        if path.is_symlink():
            raise OverlayError(f"symbolic link is forbidden: {root}/{relative}")
        if path.is_dir():
            continue
        if not path.is_file():
            raise OverlayError(f"special filesystem entry is forbidden: {root}/{relative}")
        entries.append(
            {
                "path": relative,
                "sha256": sha256_file(path),
                "size_bytes": path.stat().st_size,
            }
        )
    return entries


def inventory_report(root: pathlib.Path) -> dict[str, object]:
    files = regular_inventory(root)
    return {
        "file_count": len(files),
        "files": files,
        "inventory_sha256": canonical_sha256(files),
    }


def load_identity(path: pathlib.Path) -> dict[str, object]:
    try:
        raw = path.read_bytes()
        identity = json.loads(raw)
    except (OSError, json.JSONDecodeError) as error:
        raise OverlayError(f"cannot read identity {path}: {error}") from error
    if identity.get("schema") != IDENTITY_SCHEMA or identity.get("enabled") is not True:
        raise OverlayError("identity must be the enabled validation candidate identity")
    archive = identity.get("archive")
    if not isinstance(archive, dict) or not re.fullmatch(
        r"[0-9a-f]{64}", str(archive.get("sha256", ""))
    ):
        raise OverlayError("identity archive SHA-256 is missing or invalid")
    return {
        "identity": identity,
        "sha256": hashlib.sha256(raw).hexdigest(),
    }


def check_candidate_layout(candidate_source: pathlib.Path, verifier_source: pathlib.Path) -> None:
    for relative in ("Cargo.toml", SIGNED_LOCK_FILE, *LOCAL_SUPPORT_FILES):
        path = candidate_source / relative
        if not path.is_file() or path.is_symlink():
            raise OverlayError(f"signed local source is missing regular file {relative}")
    crates = candidate_source / "crates"
    actual = sorted(path.name for path in crates.iterdir() if path.is_dir())
    if actual != sorted(LOCAL_CRATES):
        raise OverlayError(
            "signed local source crate set differs from the frozen adapter contract: "
            f"expected {sorted(LOCAL_CRATES)}, found {actual}"
        )
    for crate in LOCAL_CRATES:
        if not (crates / crate / "Cargo.toml").is_file():
            raise OverlayError(f"signed local crate is incomplete: {crate}")
    verifier_manifest = verifier_source / "Cargo.toml"
    if not verifier_manifest.is_file() or verifier_manifest.is_symlink():
        raise OverlayError("signed verifier source is missing Cargo.toml")
    if 'name = "mfenx-contract-v1-verifier"' not in verifier_manifest.read_text(
        encoding="utf-8"
    ):
        raise OverlayError("signed verifier source has an unexpected package name")


def workspace_adapter(original: str) -> str:
    pattern = re.compile(r"(?ms)^(\[workspace\]\s*\nmembers\s*=\s*\[).*?^(\]\s*)$")
    matches = list(pattern.finditer(original))
    if len(matches) != 1:
        raise OverlayError("signed local Cargo.toml must contain one workspace member list")
    member_lines = "\n".join(f'    "{member}",' for member in WORKSPACE_MEMBERS)
    replacement = f"[workspace]\nmembers = [\n{member_lines}\n]"
    adapted = pattern.sub(replacement, original, count=1)
    return (
        "# Generated analysis-only workspace adapter; not validation-candidate source.\n"
        + adapted
    )


def copy_tree(source: pathlib.Path, destination: pathlib.Path) -> None:
    regular_inventory(source)
    shutil.copytree(source, destination, symlinks=False)


def make_owner_writable(root: pathlib.Path) -> None:
    for path in [root, *sorted(root.rglob("*"), key=lambda item: item.as_posix())]:
        mode = stat.S_IMODE(path.stat().st_mode)
        if path.is_dir():
            path.chmod(mode | stat.S_IRUSR | stat.S_IWUSR | stat.S_IXUSR)
        else:
            path.chmod(mode | stat.S_IRUSR | stat.S_IWUSR)


def classify_overlay_path(relative: str) -> str:
    if relative == "Cargo.toml":
        return "generated_workspace_adapter"
    if relative == SIGNED_LOCK_FILE:
        return "commit_bound_workspace_adapter"
    if relative.startswith(f"crates/{VERIFIER_CRATE}/"):
        return "signed_verifier_source"
    if relative.startswith("crates/") or relative in LOCAL_SUPPORT_FILES:
        return "signed_local_source"
    if relative.startswith("packaging/review/"):
        return "commit_bound_review_harness"
    raise OverlayError(f"overlay contains an unclassified path: {relative}")


def create(args: argparse.Namespace) -> int:
    candidate_source = args.candidate_source.resolve()
    verifier_source = args.verifier_source.resolve()
    review_harness = args.review_harness.resolve()
    output = args.output.resolve()
    record = args.record.resolve()
    if output.exists():
        raise OverlayError(f"refusing to merge with an existing overlay: {output}")
    if record.exists():
        raise OverlayError(f"refusing to overwrite an existing overlay record: {record}")

    identity_record = load_identity(args.identity.resolve())
    check_candidate_layout(candidate_source, verifier_source)
    adapter_lock = review_harness / ADAPTER_LOCK_FILE
    if not adapter_lock.is_file() or adapter_lock.is_symlink():
        raise OverlayError(f"commit-bound adapter lock is missing: {ADAPTER_LOCK_FILE}")
    signed_local = inventory_report(candidate_source)
    signed_verifier = inventory_report(verifier_source)
    committed_harness = inventory_report(review_harness)

    output.mkdir(parents=True)
    for relative in LOCAL_SUPPORT_FILES:
        shutil.copy2(candidate_source / relative, output / relative)
    shutil.copy2(adapter_lock, output / SIGNED_LOCK_FILE)
    (output / "crates").mkdir()
    for crate in LOCAL_CRATES:
        copy_tree(candidate_source / "crates" / crate, output / "crates" / crate)
    copy_tree(verifier_source, output / "crates" / VERIFIER_CRATE)
    (output / "packaging").mkdir()
    copy_tree(review_harness, output / "packaging" / "review")
    (output / "Cargo.toml").write_text(
        workspace_adapter((candidate_source / "Cargo.toml").read_text(encoding="utf-8")),
        encoding="utf-8",
    )
    make_owner_writable(output)

    overlay_files = regular_inventory(output)
    for item in overlay_files:
        item["provenance"] = classify_overlay_path(str(item["path"]))
    report = {
        "schema": SCHEMA,
        "kind": "analysis_only_workspace_overlay",
        "release_source_mutated": False,
        "may_be_represented_as_release_source": False,
        "identity": {
            "path": args.identity.resolve().as_posix(),
            "sha256": identity_record["sha256"],
            "release_id": identity_record["identity"]["release_id"],
            "archive_sha256": identity_record["identity"]["archive"]["sha256"],
        },
        "signed_inputs": {
            "local_source": signed_local,
            "standalone_verifier_source": signed_verifier,
        },
        "commit_bound_review_harness": committed_harness,
        "workspace_adapter": {
            "members": list(WORKSPACE_MEMBERS),
            "original_manifest_sha256": sha256_file(candidate_source / "Cargo.toml"),
            "generated_manifest_sha256": sha256_file(output / "Cargo.toml"),
            "original_lock_sha256": sha256_file(candidate_source / SIGNED_LOCK_FILE),
            "commit_bound_lock_sha256": sha256_file(output / SIGNED_LOCK_FILE),
        },
        "overlay": {
            "file_count": len(overlay_files),
            "files": overlay_files,
            "inventory_sha256": canonical_sha256(overlay_files),
        },
    }
    record.parent.mkdir(parents=True, exist_ok=True)
    record.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="ascii")
    return 0


def check(args: argparse.Namespace) -> int:
    record = json.loads(args.record.read_text(encoding="ascii"))
    if record.get("schema") != SCHEMA or record.get("release_source_mutated") is not False:
        raise OverlayError("overlay record schema or source-mutation boundary is invalid")
    actual = regular_inventory(args.output.resolve())
    expected = record.get("overlay", {}).get("files")
    if not isinstance(expected, list):
        raise OverlayError("overlay record has no file inventory")
    comparable = [
        {key: item[key] for key in ("path", "sha256", "size_bytes")} for item in expected
    ]
    if actual != comparable:
        raise OverlayError("analysis overlay bytes differ from the recorded inventory")
    if canonical_sha256(expected) != record["overlay"].get("inventory_sha256"):
        raise OverlayError("analysis overlay inventory digest is invalid")
    return 0


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    subparsers = result.add_subparsers(dest="command", required=True)

    create_parser = subparsers.add_parser("create")
    create_parser.add_argument("--identity", type=pathlib.Path, required=True)
    create_parser.add_argument("--candidate-source", type=pathlib.Path, required=True)
    create_parser.add_argument("--verifier-source", type=pathlib.Path, required=True)
    create_parser.add_argument("--review-harness", type=pathlib.Path, required=True)
    create_parser.add_argument("--output", type=pathlib.Path, required=True)
    create_parser.add_argument("--record", type=pathlib.Path, required=True)
    create_parser.set_defaults(function=create)

    check_parser = subparsers.add_parser("check")
    check_parser.add_argument("--output", type=pathlib.Path, required=True)
    check_parser.add_argument("--record", type=pathlib.Path, required=True)
    check_parser.set_defaults(function=check)
    return result


def main() -> int:
    args = parser().parse_args()
    try:
        return args.function(args)
    except (OverlayError, OSError, KeyError, json.JSONDecodeError) as error:
        print(f"validation review overlay: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())

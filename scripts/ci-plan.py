#!/usr/bin/env python3
"""Select expensive installer checks without skipping regular native validation."""
from __future__ import annotations

import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
PACKAGING_FILES = {
    "rust-toolchain.toml", "package.json", "pnpm-lock.yaml", "pnpm-workspace.yaml",
    "scripts/ci-plan.py", "scripts/core-release.json", "scripts/download-core.py",
    "scripts/check-csp-hashes.py", "scripts/native-resources.sh", "scripts/native-resources.ps1",
}
TAURI_CONFIG = "src-tauri/tauri.conf.json"


def policy_without_hashes(source: bytes) -> dict:
    """Compare every setting except generated script-src SHA-256 tokens."""
    config = json.loads(source)
    csp = config["app"]["security"]["csp"]
    directives = []
    for directive in csp.split(";"):
        tokens = directive.split()
        if tokens and tokens[0] == "script-src":
            tokens = [token for token in tokens if not re.fullmatch(r"'sha256-[^']+'", token)]
        directives.append(" ".join(tokens))
    config["app"]["security"]["csp"] = ";".join(directives)
    return config


def tauri_settings_changed(before: bytes | None, after: bytes | None) -> bool:
    if before is None or after is None:
        return True
    try:
        return policy_without_hashes(before) != policy_without_hashes(after)
    except (ValueError, KeyError, TypeError, AttributeError):
        # Unrecognized/deleted configuration must not silently avoid packaging.
        return True


def packaging_file(path: str) -> bool:
    if path in PACKAGING_FILES or path.startswith((".github/workflows/", ".github/actions/")):
        return True
    return path.startswith("src-tauri/") and not (
        path.startswith(("src-tauri/src/", "src-tauri/tests/"))
        or path.endswith((".md", ".gitignore", ".gitkeep"))
    )


def installers_required(event_name: str, ref: str, event: dict, changed: list[str], *, config_changed=True) -> bool:
    if event_name == "workflow_dispatch":
        value = event.get("inputs", {}).get("build_installers", True)
        if value in (True, "true"):
            return True
        if value in (False, "false"):
            return False
        raise ValueError("build_installers must be true or false")
    if event_name == "push":
        return ref == "refs/heads/main" or ref.startswith("refs/tags/v")
    if event_name == "pull_request":
        return any(packaging_file(path) and (path != TAURI_CONFIG or config_changed) for path in changed)
    raise ValueError(f"Unsupported CI event: {event_name}")


def git(*args: str) -> bytes:
    return subprocess.check_output(["git", *args], cwd=ROOT)


def config_at(ref: str) -> bytes | None:
    try:
        return subprocess.check_output(["git", "show", f"{ref}:{TAURI_CONFIG}"], cwd=ROOT, stderr=subprocess.DEVNULL)
    except subprocess.CalledProcessError:
        return None


def main() -> None:
    event_name = os.environ["GITHUB_EVENT_NAME"]
    event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text(encoding="utf-8"))
    changed = []
    config_changed = True
    if event_name == "pull_request":
        # Checkout the PR merge commit at depth 2: its first parent is the base.
        # Compare the actual tested merge to that parent, without an API file cap.
        changed = [os.fsdecode(path) for path in git("diff", "--no-renames", "--name-only", "-z", "HEAD^1", "HEAD").split(b"\0") if path]
        if TAURI_CONFIG in changed:
            config_changed = tauri_settings_changed(config_at("HEAD^1"), config_at("HEAD"))
    required = installers_required(event_name, os.environ["GITHUB_REF"], event, changed, config_changed=config_changed)
    with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
        output.write(f"installers={str(required).lower()}\n")
    print(f"Installers required: {required}; changed paths: {json.dumps(changed)}")


if __name__ == "__main__":
    main()

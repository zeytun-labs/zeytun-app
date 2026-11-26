#!/usr/bin/env python3
"""Plan checks from the entire tested change; unknown inputs fail toward coverage."""
from __future__ import annotations

import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
TAURI_CONFIG = "src-tauri/tauri.conf.json"
PLATFORMS = {
    "macos": dict(name="macOS", runner="macos-15", target="aarch64-apple-darwin",
                  core_target="macos-aarch64", bundles="app,dmg", installer="dmg/*.dmg"),
    "windows": dict(name="Windows", runner="windows-2025", target="x86_64-pc-windows-msvc",
                    core_target="windows-x86_64", bundles="nsis", installer="nsis/*-setup.exe"),
}
FRONTEND_CONFIG = {
    "package.json", "pnpm-lock.yaml", "pnpm-workspace.yaml", "tsconfig.json",
    "svelte.config.js", "vite.config.js", "vite.config.ts", "vitest.config.ts",
    ".nvmrc", ".node-version", ".npmrc",
}


def policy_without_hashes(source: bytes) -> dict:
    config = json.loads(source)
    directives = []
    for directive in config["app"]["security"]["csp"].split(";"):
        tokens = directive.split()
        if tokens and tokens[0] == "script-src":
            tokens = [t for t in tokens if not re.fullmatch(r"'sha256-[^']+'", t)]
        directives.append(" ".join(tokens))
    config["app"]["security"]["csp"] = ";".join(directives)
    return config


def tauri_settings_changed(before: bytes | None, after: bytes | None) -> bool:
    try:
        return policy_without_hashes(before) != policy_without_hashes(after)
    except (ValueError, KeyError, TypeError, AttributeError):
        return True


def boolean(value: object) -> bool:
    if value in (True, "true"):
        return True
    if value in (False, "false"):
        return False
    raise ValueError(f"Expected true or false, got {value!r}")


def select_plan(changed: list[str], *, full=False, installers=False,
                release_installers=False, config_changed=True) -> dict:
    full = full or release_installers
    native_checks: set[str] = set(PLATFORMS) if full else set()
    native: set[str] = set(native_checks)
    packages: set[str] = set(PLATFORMS) if installers or release_installers else set()
    frontend_tests = full
    rust = full
    for path in changed:
        if path == TAURI_CONFIG and not config_changed:
            continue
        # Platform-specific packaging inputs should not rebuild the other platform.
        if path.startswith("src-tauri/nsis/") or path in {"src-tauri/icons/icon.ico", "scripts/native-resources.ps1"}:
            packages.add("windows")
        elif path.startswith("src-tauri/icons/dock/") or path in {"src-tauri/icons/icon.icns", "src-tauri/Assets.car", "scripts/native-resources.sh"}:
            packages.add("macos")
        elif path.startswith(("src/", "static/")):
            frontend_tests = True
            native.update(PLATFORMS)  # Embed the changed UI and exercise each WebView.
        elif path.startswith(("src-tauri/src/", "src-tauri/tests/")):
            native_checks.update(PLATFORMS)
            rust = True
        elif path.startswith("proto/"):
            frontend_tests = rust = True
            native_checks.update(PLATFORMS)
        elif path in FRONTEND_CONFIG:
            frontend_tests = True
            native.update(PLATFORMS)
        elif path in {"rust-toolchain.toml", "src-tauri/Cargo.toml", "src-tauri/Cargo.lock"}:
            rust = True
            native_checks.update(PLATFORMS)
        elif path == ".github/workflows/resource-health.yml":
            pass  # This workflow only checks remote resource availability.
        elif path == "Makefile" or path.startswith((".github/actions/", ".github/workflows/", ".github/tauri-cli/")):
            frontend_tests = rust = True
            native_checks.update(PLATFORMS)
            packages.update(PLATFORMS)
        elif path.startswith("src-tauri/") or path in {
            "scripts/ci-plan.py", "scripts/ci-gate.py", "scripts/ci-resources.py",
            "scripts/ci-resources.json", "scripts/ci-smoke.py", "scripts/download-core.py",
            "scripts/core-release.json", "scripts/check-csp-hashes.py", "scripts/verify-release.py",
        }:
            native_checks.update(PLATFORMS)
            packages.update(PLATFORMS)
            rust = True
        elif path.startswith(("scripts/", ".githooks/")):
            pass  # Tooling tests always run in the inexpensive shared job.
        elif path.endswith(".md") or path.startswith(("docs/", ".github/ISSUE_TEMPLATE/")) or path in {".github/FUNDING.yml", ".gitignore", ".gitattributes", "LICENSE"}:
            pass
        else:
            # A new build input must not silently get classified as documentation.
            frontend_tests = rust = True
            native_checks.update(PLATFORMS)
            packages.update(PLATFORMS)
    native.update(native_checks | packages)
    profile = "release" if release_installers else "debug"
    entries = [dict(platform=p, **PLATFORMS[p], checks=p in native_checks,
                    package=p in packages, profile=profile)
               for p in PLATFORMS if p in native]
    return dict(frontend=frontend_tests or bool(native), frontend_tests=frontend_tests,
                native=bool(native), rust=rust,
                # Keep a valid matrix even when the job is intentionally skipped.
                matrix={"include": entries or [dict(platform="macos", **PLATFORMS["macos"],
                                                   checks=False, package=False, profile=profile)]})


def git(*args: str) -> bytes:
    return subprocess.check_output(["git", *args], cwd=ROOT)


def config_at(ref: str) -> bytes | None:
    try:
        return subprocess.check_output(["git", "show", f"{ref}:{TAURI_CONFIG}"], cwd=ROOT, stderr=subprocess.DEVNULL)
    except subprocess.CalledProcessError:
        return None


def changed_paths(event_name: str, event: dict) -> tuple[list[str], bool]:
    if event_name == "pull_request":
        base = "HEAD^1"  # First parent of the tested synthetic merge, not PR head.
    elif event_name == "push":
        base = event.get("before", "")
        if not re.fullmatch(r"[0-9a-f]{40}", base) or base == "0" * 40:
            return [], True
        if subprocess.run(["git", "cat-file", "-e", base], cwd=ROOT, capture_output=True).returncode:
            git("fetch", "--no-tags", "--depth=1", "origin", base)
    else:
        raise ValueError(f"Unsupported CI event: {event_name}")
    changed = [os.fsdecode(p) for p in git("diff", "--no-renames", "--name-only", "-z", base, "HEAD").split(b"\0") if p]
    config_changed = TAURI_CONFIG in changed and tauri_settings_changed(config_at(base), config_at("HEAD"))
    return changed, config_changed


def main() -> None:
    event_name = os.environ["GITHUB_EVENT_NAME"]
    event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text(encoding="utf-8"))
    full = boolean(os.environ.get("CI_FULL_VALIDATION", "false")) or event_name == "workflow_dispatch"
    installers = boolean(os.environ.get("CI_BUILD_INSTALLERS", "false"))
    release_installers = boolean(os.environ.get("CI_RELEASE_INSTALLERS", "false"))
    full = full or release_installers
    if full:
        changed, config_changed = [], True
    else:
        changed, config_changed = changed_paths(event_name, event)
        if event_name == "push" and not changed:
            # Empty or unavailable push history gets full validation, never a false green.
            full = True
    plan = select_plan(changed, full=full, installers=installers,
                       release_installers=release_installers, config_changed=config_changed)
    with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
        for key, value in plan.items():
            output.write(f"{key}={json.dumps(value, separators=(',', ':'))}\n")
    print(json.dumps(dict(changed=changed, **plan), indent=2))


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Validate staged inputs; stage only the generated CSP hash refresh."""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CONFIG = "src-tauri/tauri.conf.json"
FRONTEND_CONFIGS = {
    "package.json", "pnpm-lock.yaml", "pnpm-workspace.yaml", "tsconfig.json",
    "svelte.config.js", "vite.config.js", "vite.config.ts", "vitest.config.ts",
    "scripts/check-csp-hashes.py", "Makefile", ".nvmrc", ".node-version", ".npmrc", CONFIG,
}


def git(*args: str) -> bytes:
    return subprocess.check_output(["git", *args], cwd=ROOT)


def paths(*args: str) -> list[str]:
    return [os.fsdecode(name) for name in git(*args).split(b"\0") if name]


def frontend(name: str) -> bool:
    return name.startswith(("src/", "static/", "proto/")) or name in FRONTEND_CONFIGS


def native(name: str) -> bool:
    return name.startswith(("src-tauri/", "proto/"))


def tooling(name: str) -> bool:
    return name.startswith(("scripts/", ".githooks/", ".github/workflows/", ".github/actions/")) or name in {"Makefile", "rust-toolchain.toml"}


def unstaged_inputs(check_frontend: bool, check_native: bool) -> list[str]:
    changed = paths("diff", "--no-renames", "--name-only", "-z") + paths("ls-files", "--others", "--exclude-standard", "-z")
    return sorted({name for name in changed if (check_frontend and frontend(name)) or (check_native and native(name)) or tooling(name)})


def without_hashes(source: bytes) -> dict:
    config = json.loads(source)
    csp = config["app"]["security"]["csp"]
    # Whitespace between CSP source tokens is insignificant. Other directives,
    # sources, and every other JSON field still have to remain equal.
    directives = []
    for directive in csp.split(";"):
        tokens = directive.split()
        if tokens and tokens[0] == "script-src":
            tokens = [token for token in tokens if not re.fullmatch(r"'sha256-[^']+'", token)]
        directives.append(" ".join(tokens))
    config["app"]["security"]["csp"] = ";".join(directives)
    return config


def main() -> int:
    os.chdir(ROOT)
    try:
        staged = paths("diff", "--cached", "--no-renames", "--name-only", "-z")
        if not staged:
            return 0
        subprocess.run(["git", "diff", "--cached", "--check"], check=True)
        needs_frontend = any(frontend(name) for name in staged)
        needs_native = any(native(name) and name != CONFIG for name in staged)
        if CONFIG in staged:
            # A generated hash-only change does not require recompiling Rust.
            try:
                needs_native |= without_hashes(git("show", f"HEAD:{CONFIG}")) != without_hashes(git("show", f":{CONFIG}"))
            except subprocess.CalledProcessError:
                needs_native = True
        needs_tooling = any(tooling(name) for name in staged)
        if not (needs_frontend or needs_native or needs_tooling):
            print("Pre-commit: whitespace check passed (documentation/assets only).")
            return 0
        dirty = unstaged_inputs(needs_frontend, needs_native)
        if dirty:
            print("Pre-commit: build inputs differ from the staged commit:", file=sys.stderr)
            for name in dirty:
                print(f"  {name}", file=sys.stderr)
            print("Stage the intended complete input files, or commit from a clean worktree.\n"
                  "Unstaged edits are left intact; no stash or automatic source staging is performed.", file=sys.stderr)
            return 1
        original = (ROOT / CONFIG).read_bytes() if needs_frontend else None
        targets = []
        if needs_frontend:
            targets.append("prepare")
        elif needs_tooling:
            targets.append("check-tooling")
        if needs_native:
            targets += ["version", "check-rs"]
        subprocess.run([os.environ.get("MAKE", "make"), *targets], check=True)
        if needs_frontend:
            updated = (ROOT / CONFIG).read_bytes()
            if without_hashes(original) != without_hashes(updated):
                raise ValueError("Tauri configuration changed during checks. Review it and stage it explicitly.")
            dirty = [name for name in unstaged_inputs(needs_frontend, needs_native) if name != CONFIG]
            if dirty:
                raise ValueError("Build inputs changed during checks. Review and stage them before retrying: " + ", ".join(dirty))
            if original != updated:
                subprocess.run(["git", "add", "--", CONFIG], check=True)
                print("Pre-commit: staged the generated CSP hash refresh.")
        print("Pre-commit checks passed.")
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"Pre-commit failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

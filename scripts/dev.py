#!/usr/bin/env python3
"""Small, dependency-free helpers for the repository Makefile."""
from __future__ import annotations

import argparse
import json
import os
import platform
import re
import shlex
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def output(*args: str) -> str:
    return subprocess.check_output(args, cwd=ROOT, text=True, timeout=15).strip()


def doctor(native: bool) -> None:
    if sys.version_info < (3, 10):
        raise ValueError("Python 3.10 or newer is required.")
    pnpm_command = shlex.split(os.environ.get("PNPM", "pnpm"))
    tools = ["git", "make", "node", pnpm_command[0]]
    if native:
        if platform.system() != "Darwin":
            raise ValueError("Native builds require macOS. Use `make setup-web` for frontend work.")
        tools += ["cargo", "rustc", "go", "protoc", "curl", "gzip", "xcrun"]
    missing = [tool for tool in tools if shutil.which(tool) is None]
    if missing:
        raise ValueError("Missing tools: " + ", ".join(missing) + ". See CONTRIBUTING.md#prerequisites.")
    node = output("node", "--version")
    if int(node.lstrip("v").split(".")[0]) != 22:
        raise ValueError(f"Use Node.js 22 (CI uses 22); found {node}.")
    expected = json.loads((ROOT / "package.json").read_text())["packageManager"].split("@", 1)[1]
    pnpm = output(*pnpm_command, "--version")
    if pnpm != expected:
        raise ValueError(f"Use pnpm {expected} from package.json; found {pnpm}.")
    print(f"Frontend tools OK: Node {node}, pnpm {pnpm}, Python {platform.python_version()}")
    if native:
        go = re.search(r"go(\d+)\.(\d+)(?:\.(\d+))?", output("go", "version"))
        if not go or tuple(int(n or 0) for n in go.groups()) < (1, 25, 5):
            raise ValueError("Go 1.25.5 or newer is required for zeytun-core.")
        output("xcrun", "--find", "clang")
        output("cargo", "fmt", "--version")
        output("cargo", "clippy", "--version")
        print("Native tools OK (macOS SDK, Rust/Clippy/rustfmt, Go, protoc)")


def install_hooks() -> None:
    if Path(output("git", "rev-parse", "--show-toplevel")).resolve() != ROOT:
        raise ValueError("Run this command in the zeytun-app Git checkout.")
    result = subprocess.run(["git", "config", "--get", "core.hooksPath"], cwd=ROOT, capture_output=True, text=True)
    existing = result.stdout.strip()
    if existing and existing != ".githooks":
        raise ValueError(f"core.hooksPath is already {existing!r}. Integrate .githooks/pre-commit with your existing hooks.")
    legacy = Path(output("git", "rev-parse", "--git-path", "hooks/pre-commit"))
    if not legacy.is_absolute():
        legacy = ROOT / legacy
    if not existing and legacy.is_file() and os.access(legacy, os.X_OK):
        raise ValueError("An existing .git/hooks/pre-commit is installed. Integrate it before switching to .githooks.")
    hook = ROOT / ".githooks" / "pre-commit"
    hook.chmod(hook.stat().st_mode | 0o111)
    subprocess.run(["git", "config", "--local", "core.hooksPath", ".githooks"], cwd=ROOT, check=True)
    print("Installed .githooks/pre-commit for this clone.")


def version() -> None:
    package = json.loads((ROOT / "package.json").read_text())["version"]
    tauri = json.loads((ROOT / "src-tauri/tauri.conf.json").read_text())["version"]
    cargo = (ROOT / "src-tauri/Cargo.toml").read_text().split("[package]", 1)[1].split("\n[", 1)[0]
    match = re.search(r'^version\s*=\s*"([^"]+)"', cargo, re.M)
    if not match or package != tauri or package != match[1]:
        raise ValueError(f"Version drift: package={package}, tauri={tauri}, cargo={match[1] if match else 'missing'}")
    print(f"Versions OK: {package}")


def repair_dev() -> None:
    target = Path(os.environ.get("CARGO_TARGET_DIR", "target"))
    if not target.is_absolute():
        target = ROOT / "src-tauri" / target
    # Cargo's host and cross-compiled profile directories only; do not walk deps/.
    profiles = [target / name for name in ("debug", "release")]
    if target.is_dir():
        profiles += [child / name for child in target.iterdir() if child.is_dir() for name in ("debug", "release")]
    for profile in profiles:
        for binary in profile.glob("resources/bin/*/zeytun-core"):
            if not os.access(binary, os.W_OK):
                binary.unlink()
                print(f"Removed non-writable generated core copy: {binary}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["doctor", "hooks", "version", "disk", "repair-dev"])
    parser.add_argument("--native", action="store_true")
    args = parser.parse_args()
    os.chdir(ROOT)
    try:
        if args.command == "doctor":
            doctor(args.native)
        elif args.command == "hooks":
            install_hooks()
        elif args.command == "version":
            version()
        elif args.command == "repair-dev":
            repair_dev()
        else:
            free = shutil.disk_usage(ROOT).free / 1024**3
            print(f"{free:.1f} GiB free")
            if free < 6:
                raise ValueError("Native compilation needs at least 6 GiB free. Run `make clean` to remove build artifacts.")
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print(f"Development setup: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Run real native artifacts in their isolated CI identity and require a WebView/IPC handshake."""
from __future__ import annotations

import argparse
from contextlib import contextmanager
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import uuid

ROOT = Path(__file__).resolve().parents[1]


def one(paths) -> Path:
    matches = list(paths)
    if len(matches) != 1:
        raise ValueError(f"Expected one artifact, found: {matches}")
    return matches[0]


def handshake(executable: Path, output: Path) -> None:
    token = uuid.uuid4().hex
    env = dict(os.environ, ZEYTUN_CI_SMOKE_TOKEN=token, ZEYTUN_CI_SMOKE_OUTPUT=str(output))
    # A unique output prevents stale evidence from making a failed launch pass.
    output.unlink(missing_ok=True)
    with output.with_suffix(".log").open("w", encoding="utf-8") as log:
        try:
            subprocess.run([str(executable), "--ci-smoke-test"], env=env, stdout=log,
                           stderr=subprocess.STDOUT, check=True, timeout=60)
        except (subprocess.SubprocessError, OSError):
            print(output.with_suffix(".log").read_text(encoding="utf-8", errors="replace"))
            if output.exists():
                print(output.read_text(encoding="utf-8", errors="replace"))
            raise
    result = json.loads(output.read_text(encoding="utf-8"))
    if result != dict(token=token, webview=True, assets=True, ipc=True):
        raise ValueError(f"Invalid WebView/IPC handshake: {result}")
    print(f"WebView, embedded assets and IPC passed: {executable}")


def check_core(directory: Path, platform: str) -> None:
    executable = one(directory.rglob("zeytun-core.exe" if platform == "windows" else "zeytun-core"))
    result = subprocess.run([str(executable), "version"], capture_output=True, text=True,
                            check=True, timeout=20)
    if not result.stdout.strip():
        raise ValueError("Bundled core did not report its version")
    if platform == "windows" and not (executable.parent / "libcronet.dll").is_file():
        raise ValueError("Bundled core is missing libcronet.dll")
    one(directory.rglob("geoip.mmdb"))
    print(result.stdout.strip())


@contextmanager
def installed_bundle(bundle: Path, platform: str, temporary: Path):
    if platform == "macos":
        mount = temporary / "mounted"
        mount.mkdir()
        subprocess.run(["hdiutil", "attach", "-nobrowse", "-readonly", "-mountpoint", str(mount), str(bundle)],
                       check=True, timeout=60)
        try:
            app = one(mount.glob("*.app"))
            # Exercise the copied application, as a user would install it from the DMG.
            installed = temporary / app.name
            shutil.copytree(app, installed, symlinks=True)
            yield installed / "Contents/MacOS/Zeytun", installed / "Contents/Resources"
        finally:
            subprocess.run(["hdiutil", "detach", str(mount)], check=True, timeout=30)
    else:
        installed = temporary / "installed"
        subprocess.run([str(bundle), "/S", f"/D={installed}"], check=True, timeout=120)
        try:
            yield installed / "Zeytun.exe", installed
        finally:
            uninstaller = installed / "uninstall.exe"
            if uninstaller.exists():
                subprocess.run([str(uninstaller), "/S"], check=True, timeout=60)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target")
    parser.add_argument("--platform", choices=("macos", "windows"), required=True)
    parser.add_argument("--package", choices=("true", "false"))
    parser.add_argument("--profile", choices=("debug", "release"), default="debug")
    parser.add_argument("--verify-resources", type=Path)
    args = parser.parse_args()
    if args.verify_resources:
        check_core(args.verify_resources, args.platform)
        return
    if not args.target or args.package is None:
        parser.error("--target and --package are required for an application smoke test")
    artifacts = ROOT / "src-tauri/target" / args.target / args.profile
    results = ROOT / ".cache/smoke-results"
    results.mkdir(parents=True, exist_ok=True)
    if args.package == "true":
        pattern = "dmg/*.dmg" if args.platform == "macos" else "nsis/*-setup.exe"
        bundle = one((artifacts / "bundle").glob(pattern))
        with tempfile.TemporaryDirectory(prefix="zeytun-ci-") as directory:
            with installed_bundle(bundle, args.platform, Path(directory)) as (executable, resources):
                check_core(resources, args.platform)
                handshake(executable, results / "installed.json")
    else:
        executable = artifacts / ("Zeytun.exe" if args.platform == "windows" else "Zeytun")
        check_core(ROOT / "src-tauri/resources", args.platform)
        handshake(executable, results / "unbundled.json")


if __name__ == "__main__":
    main()

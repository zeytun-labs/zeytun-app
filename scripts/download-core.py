#!/usr/bin/env python3
"""Install only the core release pinned by the app; never compile a fallback."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import stat
import tempfile
import time
import urllib.error
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[1]
REPOSITORY = "zeytun-labs/zeytun-core"
TARGET_FILES = {
    "macos-aarch64": {"zeytun-core", "LICENSE", "NOTICE.md", "build-info.json"},
    "windows-x86_64": {"zeytun-core.exe", "libcronet.dll", "LICENSE", "NOTICE.md", "build-info.json"},
}
MAX_ARCHIVE = 256 * 1024 * 1024
MAX_UNPACKED = 512 * 1024 * 1024


def load_lock(path: Path) -> dict:
    lock = json.loads(path.read_text(encoding="utf-8"))
    if lock.get("schema") != 1 or lock.get("repository") != REPOSITORY:
        raise ValueError("Unsupported core release manifest")
    if not re.fullmatch(r"zeytun-core-v[0-9A-Za-z.+-]+", lock.get("tag", "")):
        raise ValueError("Expected a versioned Zeytun core release tag")
    if not re.fullmatch(r"[0-9a-f]{40}", lock.get("commit", "")):
        raise ValueError("Expected a pinned core source commit")
    for target in TARGET_FILES:
        validate_asset(lock["assets"][target], f"zeytun-core-{target}.zip")
    validate_asset(lock["source"], "zeytun-core-source.tar.gz")
    return lock


def validate_asset(asset: dict, name: str) -> None:
    if asset.get("name") != name or not re.fullmatch(r"[0-9a-f]{64}", asset.get("sha256", "")):
        raise ValueError(f"Invalid asset or SHA-256 pin: {name}")


def digest(path: Path) -> str:
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def fetch_asset(lock: dict, asset: dict, cache: Path) -> Path:
    cache.mkdir(parents=True, exist_ok=True)
    cached = cache / f'{asset["sha256"]}-{asset["name"]}'
    if cached.is_file() and digest(cached) == asset["sha256"]:
        return cached
    url = f'https://github.com/{REPOSITORY}/releases/download/{lock["tag"]}/{asset["name"]}'
    request = urllib.request.Request(url, headers={"User-Agent": "zeytun-core-downloader"})
    with tempfile.TemporaryDirectory(prefix="download-", dir=cache) as temporary:
        downloaded = Path(temporary) / asset["name"]
        for attempt in range(3):
            try:
                with urllib.request.urlopen(request, timeout=60) as response, downloaded.open("wb") as output:
                    size = 0
                    while chunk := response.read(1024 * 1024):
                        size += len(chunk)
                        if size > MAX_ARCHIVE:
                            raise ValueError("Core release asset exceeds size limit")
                        output.write(chunk)
                break
            except (urllib.error.URLError, TimeoutError, ConnectionError):
                if attempt == 2:
                    raise
                time.sleep(2 ** attempt)
        if digest(downloaded) != asset["sha256"]:
            raise ValueError(f'SHA-256 mismatch for {asset["name"]}')
        os.replace(downloaded, cached)
    return cached


def install_core(lock: dict, target: str, destination: Path, cache: Path) -> None:
    archive = fetch_asset(lock, lock["assets"][target], cache)
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".core-", dir=destination.parent) as temporary:
        stage = Path(temporary) / "staged"
        stage.mkdir()
        with zipfile.ZipFile(archive) as package:
            entries = package.infolist()
            names = [entry.filename for entry in entries]
            if len(names) != len(set(names)) or set(names) != TARGET_FILES[target]:
                raise ValueError("Unexpected, missing or duplicate files in core archive")
            if sum(entry.file_size for entry in entries) > MAX_UNPACKED:
                raise ValueError("Core archive exceeds unpacked size limit")
            for entry in entries:
                if entry.is_dir() or stat.S_ISLNK(entry.external_attr >> 16) or entry.file_size == 0:
                    raise ValueError("Core archive contains an invalid file")
                # Only exact, flat allowlisted filenames reach this write.
                with package.open(entry) as source, (stage / entry.filename).open("wb") as output:
                    shutil.copyfileobj(source, output)
        info = json.loads((stage / "build-info.json").read_text(encoding="utf-8"))
        if any(info.get(key) != value for key, value in {
            "repository": REPOSITORY, "commit": lock["commit"], "tag": lock["tag"], "target": target,
        }.items()):
            raise ValueError("Core binary metadata does not match the pinned release")
        executable = "zeytun-core.exe" if target == "windows-x86_64" else "zeytun-core"
        (stage / executable).chmod(0o755)
        # Resource folders contain a tracked empty placeholder in fresh clones.
        if (destination / ".gitkeep").is_file():
            (stage / ".gitkeep").touch()
        backup = Path(temporary) / "previous"
        if destination.exists():
            destination.rename(backup)
        try:
            stage.rename(destination)
        except OSError:
            if backup.exists():
                backup.rename(destination)
            raise
    print(f'Installed {lock["tag"]} ({lock["commit"][:12]}) for {target}')


def host_target() -> str:
    system, machine = platform.system(), platform.machine().lower()
    if system == "Darwin" and machine in ("arm64", "aarch64"):
        return "macos-aarch64"
    if system == "Windows" and machine in ("amd64", "x86_64"):
        return "windows-x86_64"
    raise ValueError("No published core for this host; specify a supported --target")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGET_FILES)
    parser.add_argument("--lock", type=Path, default=ROOT / "scripts/core-release.json")
    parser.add_argument("--destination", type=Path, help="Override the native resource directory")
    parser.add_argument("--source-output", type=Path, help="Also save verified corresponding source")
    args = parser.parse_args()
    lock = load_lock(args.lock)
    target = args.target or host_target()
    cache = ROOT / ".cache/core-downloads"
    install_core(lock, target, args.destination or ROOT / "src-tauri/resources/bin" / target, cache)
    if args.source_output:
        source = fetch_asset(lock, lock["source"], cache)
        args.source_output.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, args.source_output)


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, KeyError, zipfile.BadZipFile) as error:
        raise SystemExit(f"Core download failed: {error}. No source-build fallback is allowed.") from error

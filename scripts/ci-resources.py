#!/usr/bin/env python3
"""Install verified CI inputs without mutable package-manager feeds."""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import gzip
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import subprocess
import tempfile
import time
import urllib.error
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parents[1]
MAX_DOWNLOAD = 128 * 1024 * 1024
MAX_EXPANDED = 256 * 1024 * 1024
MMDB_MARKER = b"\xab\xcd\xefMaxMind.com"


def digest(path: Path) -> str:
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def fetch(asset: dict, cache: Path) -> Path:
    url, expected = asset["url"], asset["sha256"]
    if not url.startswith("https://") or not re.fullmatch(r"[0-9a-f]{64}", expected):
        raise ValueError("CI assets require HTTPS and a SHA-256 pin")
    cache.mkdir(parents=True, exist_ok=True)
    cached = cache / expected
    if cached.is_file() and digest(cached) == expected:
        return cached
    with tempfile.TemporaryDirectory(prefix="download-", dir=cache) as temporary:
        downloaded = Path(temporary) / "asset"
        for attempt in range(3):
            try:
                request = urllib.request.Request(url, headers={"User-Agent": "zeytun-ci"})
                with urllib.request.urlopen(request, timeout=60) as response, downloaded.open("wb") as output:
                    size = 0
                    while chunk := response.read(1024 * 1024):
                        size += len(chunk)
                        if size > MAX_DOWNLOAD:
                            raise ValueError("CI asset exceeds download size limit")
                        output.write(chunk)
                break
            except (urllib.error.URLError, TimeoutError, ConnectionError):
                if attempt == 2:
                    raise
                time.sleep(2 ** attempt)
        if digest(downloaded) != expected:
            raise ValueError(f"SHA-256 mismatch: {url}")
        os.replace(downloaded, cached)
    return cached


def validate_geoip(path: Path) -> None:
    with path.open("rb") as source:
        source.seek(max(0, path.stat().st_size - 131072))
        if MMDB_MARKER not in source.read():
            raise ValueError("Invalid or incomplete GeoIP database")


def install_geoip(asset: dict, cache: Path, destination: Path) -> None:
    archive = fetch(asset, cache)
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix=".geoip-", dir=destination.parent) as temporary:
        staged = Path(temporary) / "geoip.mmdb"
        with gzip.open(archive, "rb") as source, staged.open("wb") as output:
            size = 0
            while chunk := source.read(1024 * 1024):
                size += len(chunk)
                if size > MAX_EXPANDED:
                    raise ValueError("GeoIP database exceeds unpacked size limit")
                output.write(chunk)
        validate_geoip(staged)
        os.replace(staged, destination)
    print(f"Verified GeoIP installed: {destination}")


def install_protoc(lock: dict, target: str, cache: Path, destination: Path) -> Path:
    archive = fetch(lock["assets"][target], cache)
    executable = "bin/protoc.exe" if target.startswith("windows-") else "bin/protoc"
    destination.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(archive) as package:
        entries = package.infolist()
        if sum(e.file_size for e in entries) > MAX_EXPANDED:
            raise ValueError("protoc exceeds unpacked size limit")
        names = [e.filename for e in entries]
        if executable not in names or len(names) != len(set(names)):
            raise ValueError("Invalid protoc archive")
        for entry in entries:
            path = PurePosixPath(entry.filename)
            if path.is_absolute() or ".." in path.parts or "\\" in entry.filename or ":" in entry.filename or stat.S_ISLNK(entry.external_attr >> 16):
                raise ValueError("Unsafe path in protoc archive")
            if entry.is_dir() or not (entry.filename == executable or entry.filename.startswith("include/")):
                continue
            output = destination / entry.filename
            output.parent.mkdir(parents=True, exist_ok=True)
            with package.open(entry) as source, output.open("wb") as sink:
                shutil.copyfileobj(source, sink)
    binary = (destination / executable).resolve()
    binary.chmod(0o755)
    version = subprocess.check_output([str(binary), "--version"], text=True).strip()
    if version != f'libprotoc {lock["version"]}':
        raise ValueError(f"Unexpected protoc version: {version}")
    print(version)
    return binary


def check_live_geoip(cache: Path) -> None:
    """Network health belongs to an independent job, never the PR merge gate."""
    month = datetime.now(timezone.utc).strftime("%Y-%m")
    url = f"https://download.db-ip.com/free/dbip-country-lite-{month}.mmdb.gz"
    # Download without trusting it for production; print the candidate pin for review.
    request = urllib.request.Request(url, headers={"User-Agent": "zeytun-ci"})
    for attempt in range(3):
        try:
            with urllib.request.urlopen(request, timeout=60) as response:
                data = response.read(MAX_DOWNLOAD + 1)
            break
        except (urllib.error.URLError, TimeoutError, ConnectionError):
            if attempt == 2:
                raise
            time.sleep(2 ** attempt)
    if len(data) > MAX_DOWNLOAD:
        raise ValueError("Live GeoIP archive exceeds size limit")
    checksum = hashlib.sha256(data).hexdigest()
    cache.mkdir(parents=True, exist_ok=True)
    (cache / checksum).write_bytes(data)
    with tempfile.TemporaryDirectory() as temporary:
        install_geoip(dict(url=url, sha256=checksum), cache, Path(temporary) / "geoip.mmdb")
    print(json.dumps(dict(url=url, sha256=checksum), indent=2))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("keys", "protoc", "geoip", "health"))
    parser.add_argument("--target", choices=("macos-aarch64", "windows-x86_64"))
    args = parser.parse_args()
    lock = json.loads((ROOT / "scripts/ci-resources.json").read_text(encoding="utf-8"))
    if lock.get("schema") != 1:
        raise ValueError("Unsupported CI resource manifest")
    if args.action == "keys":
        if not args.target:
            parser.error("keys requires --target")
        with Path(os.environ["GITHUB_OUTPUT"]).open("a", encoding="utf-8") as output:
            output.write(f'protoc={lock["protoc"]["assets"][args.target]["sha256"]}\n')
            output.write(f'geoip={lock["geoip"]["sha256"]}\n')
    elif args.action == "protoc":
        if not args.target:
            parser.error("protoc requires --target")
        binary = install_protoc(lock["protoc"], args.target, ROOT / ".cache/protoc-downloads",
                                ROOT / ".cache/protoc" / lock["protoc"]["version"] / args.target)
        for variable, line in (("GITHUB_PATH", str(binary.parent)), ("GITHUB_ENV", f"PROTOC={binary}")):
            if variable in os.environ:
                with Path(os.environ[variable]).open("a", encoding="utf-8") as output:
                    output.write(line + "\n")
    elif args.action == "geoip":
        install_geoip(lock["geoip"], ROOT / ".cache/geoip-downloads", ROOT / "src-tauri/resources/geoip.mmdb")
    else:
        check_live_geoip(ROOT / ".cache/geoip-health")


if __name__ == "__main__":
    main()

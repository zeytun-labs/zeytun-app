"""Exercise release integrity, native runtime files and failed-install isolation."""
from __future__ import annotations

import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import stat
import tempfile
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("download_core", ROOT / "scripts/download-core.py")
core = importlib.util.module_from_spec(spec)
spec.loader.exec_module(core)


class CoreDownloadTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.cache = self.root / "cache"
        self.destination = self.root / "resources/core"
        self.destination.mkdir(parents=True)
        (self.destination / "existing").write_bytes(b"working core")
        self.lock = {
            "schema": 1, "repository": core.REPOSITORY,
            "tag": "zeytun-core-v0.1.0-alpha.1", "commit": "a" * 40,
            "assets": {}, "source": {"name": "zeytun-core-source.tar.gz", "sha256": "b" * 64},
        }
        for target in core.TARGET_FILES:
            self.lock["assets"][target] = {"name": f"zeytun-core-{target}.zip", "sha256": "c" * 64}

    def archive(self, target="windows-x86_64", mutate=None):
        info = {key: self.lock[key] for key in ("repository", "tag", "commit")}
        info["target"] = target
        files = {name: b"runtime file" for name in core.TARGET_FILES[target]}
        files["build-info.json"] = json.dumps(info).encode()
        if mutate:
            mutate(files)
        output = io.BytesIO()
        with zipfile.ZipFile(output, "w") as package:
            for name, data in files.items():
                package.writestr(name, data)
        data = output.getvalue()
        self.lock["assets"][target]["sha256"] = hashlib.sha256(data).hexdigest()
        return data

    def install(self, data, target="windows-x86_64"):
        with patch.object(core.urllib.request, "urlopen", return_value=io.BytesIO(data)):
            core.install_core(self.lock, target, self.destination, self.cache)

    def test_windows_installs_matching_dll_and_binary(self):
        self.install(self.archive())
        self.assertEqual({path.name for path in self.destination.iterdir()}, core.TARGET_FILES["windows-x86_64"])
        self.assertEqual((self.destination / "libcronet.dll").read_bytes(), b"runtime file")

    def test_install_preserves_the_tracked_resource_placeholder(self):
        (self.destination / ".gitkeep").touch()
        self.install(self.archive())
        self.assertEqual((self.destination / ".gitkeep").read_bytes(), b"")

    @unittest.skipIf(os.name == "nt", "Windows does not expose POSIX executable bits")
    def test_macos_binary_is_executable(self):
        self.install(self.archive("macos-aarch64"), "macos-aarch64")
        self.assertTrue((self.destination / "zeytun-core").stat().st_mode & stat.S_IXUSR)

    def test_checksum_failure_preserves_installed_core(self):
        data = self.archive()
        with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
            self.install(data + b"tampered")
        self.assertEqual((self.destination / "existing").read_bytes(), b"working core")

    def test_cached_archive_is_reverified_before_reuse(self):
        data = self.archive()
        self.install(data)
        cached = next(self.cache.iterdir())
        cached.write_bytes(b"damaged cache")
        self.install(data)
        self.assertEqual(cached.read_bytes(), data)
        with patch.object(core.urllib.request, "urlopen", side_effect=AssertionError("cache should be reused")):
            core.install_core(self.lock, "windows-x86_64", self.destination, self.cache)

    def test_unsafe_archive_paths_and_incomplete_runtime_are_rejected(self):
        for mutate in (
            lambda files: files.update({"../escape": b"unsafe"}),
            lambda files: files.pop("libcronet.dll"),
            lambda files: files.update({"build-info.json": b'{"commit":"wrong"}'}),
        ):
            with self.subTest(mutate=mutate), self.assertRaises(ValueError):
                self.install(self.archive(mutate=mutate))
            self.assertTrue((self.destination / "existing").is_file())
        self.assertFalse((self.destination.parent / "escape").exists())

    def test_duplicate_members_and_symlinks_are_rejected(self):
        for symlink in (False, True):
            output = io.BytesIO(self.archive())
            with zipfile.ZipFile(output, "a") as package:
                if symlink:
                    # Replace the archive to give one allowlisted name symlink attributes.
                    members = {name: package.read(name) for name in package.namelist()}
                else:
                    import warnings
                    with warnings.catch_warnings():
                        warnings.simplefilter("ignore", UserWarning)
                        package.writestr("zeytun-core.exe", b"duplicate")
            if symlink:
                output = io.BytesIO()
                with zipfile.ZipFile(output, "w") as package:
                    for name, data in members.items():
                        entry = zipfile.ZipInfo(name)
                        if name == "zeytun-core.exe":
                            entry.create_system = 3
                            entry.external_attr = (stat.S_IFLNK | 0o777) << 16
                        package.writestr(entry, data)
            data = output.getvalue()
            self.lock["assets"]["windows-x86_64"]["sha256"] = hashlib.sha256(data).hexdigest()
            with self.subTest(symlink=symlink), self.assertRaises(ValueError):
                self.install(data)
            self.assertTrue((self.destination / "existing").exists())

    def test_lock_rejects_moving_refs_or_unpinned_assets(self):
        path = self.root / "lock.json"
        path.write_text(json.dumps(self.lock))
        self.assertEqual(core.load_lock(path), self.lock)
        for field, value in (("tag", "latest"), ("commit", "zeytun-port"), ("repository", "other/core")):
            path.write_text(json.dumps(dict(self.lock, **{field: value})))
            with self.subTest(field=field), self.assertRaises(ValueError):
                core.load_lock(path)
        self.lock["source"]["sha256"] = ""
        path.write_text(json.dumps(self.lock))
        with self.assertRaises(ValueError):
            core.load_lock(path)


if __name__ == "__main__":
    unittest.main()

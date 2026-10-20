"""Verify cached input integrity, retry behavior and atomic GeoIP replacement offline."""
from __future__ import annotations

import gzip
import hashlib
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import urllib.error
import zipfile

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("ci_resources", ROOT / "scripts/ci-resources.py")
resources = importlib.util.module_from_spec(spec)
spec.loader.exec_module(resources)


class ResourceTests(unittest.TestCase):
    def test_retries_transient_downloads_and_reverifies_cached_bytes(self):
        data = b"verified archive"
        asset = dict(url="https://example.invalid/archive", sha256=hashlib.sha256(data).hexdigest())
        with tempfile.TemporaryDirectory() as directory:
            cache = Path(directory)
            with patch.object(resources.time, "sleep"), patch.object(resources.urllib.request, "urlopen",
                 side_effect=[urllib.error.URLError("503"), io.BytesIO(data)]) as request:
                downloaded = resources.fetch(asset, cache)
                self.assertEqual(request.call_count, 2)
            with patch.object(resources.urllib.request, "urlopen", side_effect=AssertionError("cache should be used")):
                self.assertEqual(resources.fetch(asset, cache), downloaded)
            downloaded.write_bytes(b"corrupt cache")
            with patch.object(resources.urllib.request, "urlopen", return_value=io.BytesIO(data)):
                self.assertEqual(resources.fetch(asset, cache).read_bytes(), data)

    def test_hash_failure_never_installs_or_retries_untrusted_content(self):
        with tempfile.TemporaryDirectory() as directory:
            cache = Path(directory)
            with patch.object(resources.urllib.request, "urlopen", return_value=io.BytesIO(b"wrong")) as request:
                with self.assertRaisesRegex(ValueError, "SHA-256"):
                    resources.fetch(dict(url="https://example.invalid/archive", sha256="a" * 64), cache)
                self.assertEqual(request.call_count, 1)
            self.assertEqual(list(cache.iterdir()), [])

    def test_invalid_geoip_preserves_existing_database(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            dest = root / "geoip.mmdb"
            dest.write_bytes(b"working database")
            for content, succeeds in ((b"invalid", False), (b"database" + resources.MMDB_MARKER, True)):
                archive = root / "archive"
                archive.write_bytes(gzip.compress(content))
                with patch.object(resources, "fetch", return_value=archive):
                    if succeeds:
                        resources.install_geoip({}, root, dest)
                        self.assertEqual(dest.read_bytes(), content)
                    else:
                        with self.assertRaises(ValueError):
                            resources.install_geoip({}, root, dest)
                        self.assertEqual(dest.read_bytes(), b"working database")

    def test_protoc_rejects_zip_traversal_before_executing_a_binary(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive = root / "archive.zip"
            with zipfile.ZipFile(archive, "w") as package:
                package.writestr("bin/protoc", b"fixture")
                package.writestr("../outside", b"bad")
            with patch.object(resources, "fetch", return_value=archive), patch.object(resources.subprocess, "check_output") as execute:
                with self.assertRaisesRegex(ValueError, "Unsafe"):
                    resources.install_protoc(dict(assets={"macos-aarch64": {}}, version="33.0"), "macos-aarch64", root, root / "install")
                execute.assert_not_called()
            self.assertFalse((root / "outside").exists())


if __name__ == "__main__":
    unittest.main()

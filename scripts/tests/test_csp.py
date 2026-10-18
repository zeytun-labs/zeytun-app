"""Portable CSP hash and configuration regressions, run on every frontend platform."""
from __future__ import annotations

import base64
import contextlib
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("check_csp_hashes", ROOT / "scripts/check-csp-hashes.py")
csp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(csp)


class CSPTests(unittest.TestCase):
    def test_nested_pages_and_exact_bytes_exclude_external_scripts(self):
        body = 'const value = "a&b";\r\n'
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "nested").mkdir()
            (root / "nested/index.html").write_bytes(
                f'<script src = "/external.js">ignored</script><script type="module">{body}</script>'.encode()
            )
            expected = base64.b64encode(hashlib.sha256(body.encode()).digest()).decode()
            self.assertEqual(csp.build_hashes(root), {expected})

    def test_write_preserves_unrelated_policy_and_is_idempotent(self):
        policy = "default-src 'self'; script-src 'self' 'sha256-old'; style-src 'self' 'sha256-style'; connect-src ipc:"
        with tempfile.TemporaryDirectory() as directory:
            conf = Path(directory) / "tauri.conf.json"
            config = {"app": {"security": {"csp": policy}}, "identifier": "com.example.dev"}
            conf.write_text(json.dumps(config, indent=4) + "\n")
            with contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(csp.sync_config(conf, {"new"}, True), 0)
                saved = conf.read_bytes()
                self.assertEqual(csp.sync_config(conf, {"new"}, True), 0)
            self.assertEqual(conf.read_bytes(), saved)
            after = json.loads(saved)
            self.assertEqual(after["identifier"], config["identifier"])
            self.assertIn("style-src 'self' 'sha256-style'; connect-src ipc:", after["app"]["security"]["csp"])
            self.assertEqual(csp.pinned_hashes(after["app"]["security"]["csp"]), {"new"})

    def test_write_preserves_utf8_and_crlf_on_every_platform(self):
        with tempfile.TemporaryDirectory() as directory:
            conf = Path(directory) / "tauri.conf.json"
            config = {
                "productName": "زیتون",
                "app": {"security": {"csp": "script-src 'self' 'sha256-old'; connect-src ipc:"}},
            }
            original = (json.dumps(config, ensure_ascii=False, indent=2) + "\n").replace("\n", "\r\n").encode("utf-8")
            conf.write_bytes(original)
            with contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(csp.sync_config(conf, {"new"}, True), 0)
            self.assertEqual(conf.read_bytes(), original.replace(b"'sha256-old'", b"'sha256-new'"))

    def test_restored_artifact_must_match_its_own_configuration(self):
        # Two valid platform builds can have different bootstrap import names.
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            first, second = root / "first", root / "second"
            first.mkdir()
            second.mkdir()
            (first / "index.html").write_bytes(b'<script>import("./app.macos.js")</script>')
            (second / "index.html").write_bytes(b'<script>import("./app.windows.js")</script>')
            conf = root / "tauri.conf.json"
            conf.write_text(json.dumps({"app": {"security": {"csp": "script-src 'self'"}}}), encoding="utf-8")
            first_hashes, second_hashes = csp.build_hashes(first), csp.build_hashes(second)
            self.assertNotEqual(first_hashes, second_hashes)
            with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(csp.sync_config(conf, first_hashes, True), 0)
                self.assertEqual(csp.sync_config(conf, first_hashes, False), 0)
                self.assertEqual(csp.sync_config(conf, second_hashes, False), 1)
                self.assertEqual(csp.sync_config(conf, second_hashes, True), 0)
                self.assertEqual(csp.sync_config(conf, second_hashes, False), 0)

    def test_check_failure_is_read_only_and_empty_build_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            conf = Path(directory) / "config.json"
            conf.write_text(json.dumps({"app": {"security": {"csp": "script-src 'self' 'sha256-old'"}}}))
            before = conf.read_bytes()
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(csp.sync_config(conf, {"new"}, False), 1)
            self.assertEqual(conf.read_bytes(), before)
            with self.assertRaises(ValueError):
                csp.build_hashes(Path(directory) / "missing")
            (Path(directory) / "empty.html").write_text("<main>No scripts</main>")
            with self.assertRaises(ValueError):
                csp.build_hashes(Path(directory))


if __name__ == "__main__":
    unittest.main()

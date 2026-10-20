"""A successful process exit or stale evidence must never substitute for a WebView handshake."""
from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("ci_smoke", ROOT / "scripts/ci-smoke.py")
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)


class ArtifactSelectionTests(unittest.TestCase):
    def test_installs_only_the_requested_profile_even_with_stale_other_artifacts(self):
        for platform, pattern in (("macos", "dmg/fixture.dmg"), ("windows", "nsis/fixture-setup.exe")):
            for profile in ("debug", "release"):
                with self.subTest(platform=platform, profile=profile), tempfile.TemporaryDirectory() as directory:
                    root = Path(directory)
                    for candidate in ("debug", "release"):
                        bundle = root / "src-tauri/target/test-target" / candidate / "bundle" / pattern
                        bundle.parent.mkdir(parents=True)
                        bundle.touch()
                    expected = root / "src-tauri/target/test-target" / profile / "bundle" / pattern
                    argv = ["ci-smoke.py", "--target", "test-target", "--platform", platform,
                            "--package", "true", "--profile", profile]
                    with patch.object(smoke, "ROOT", root), patch.object(sys, "argv", argv), \
                            patch.object(smoke, "installed_bundle") as install, \
                            patch.object(smoke, "check_core") as core, patch.object(smoke, "handshake") as handshake:
                        install.return_value.__enter__.return_value = (root / "executable", root / "resources")
                        smoke.main()
                        self.assertEqual(install.call_args.args[:2], (expected, platform))
                        core.assert_called_once_with(root / "resources", platform)
                        handshake.assert_called_once()
                        expected.unlink()
                        # Never fall back to a stale successful installer from the other profile.
                        with self.assertRaises(ValueError):
                            smoke.main()


@unittest.skipIf(os.name == "nt", "fixture uses a POSIX executable script")
class HandshakeTests(unittest.TestCase):
    def test_requires_fresh_matching_evidence_from_the_launched_process(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            executable = root / "fixture"
            output = root / "smoke.json"
            executable.write_text(f"#!{sys.executable}\nimport os, json\nfrom pathlib import Path\n"
                "Path(os.environ['ZEYTUN_CI_SMOKE_OUTPUT']).write_text(json.dumps(dict("
                "token=os.environ['ZEYTUN_CI_SMOKE_TOKEN'], webview=True, assets=True, ipc=True)))\n")
            executable.chmod(0o755)
            smoke.handshake(executable, output)
            self.assertTrue(json.loads(output.read_text())["ipc"])
            # An otherwise successful launch must not reuse the preceding result.
            executable.write_text(f"#!{sys.executable}\n")
            with self.assertRaises(FileNotFoundError):
                smoke.handshake(executable, output)
            self.assertFalse(output.exists())

    def test_malformed_handshake_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            executable = root / "fixture"
            executable.write_text(f"#!{sys.executable}\nimport os\nfrom pathlib import Path\n"
                "Path(os.environ['ZEYTUN_CI_SMOKE_OUTPUT']).write_text('{}')\n")
            executable.chmod(0o755)
            with self.assertRaises(ValueError):
                smoke.handshake(executable, root / "smoke.json")


if __name__ == "__main__":
    unittest.main()

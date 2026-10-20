"""A successful process exit or stale evidence must never substitute for a WebView handshake."""
from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("ci_smoke", ROOT / "scripts/ci-smoke.py")
smoke = importlib.util.module_from_spec(spec)
spec.loader.exec_module(smoke)


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

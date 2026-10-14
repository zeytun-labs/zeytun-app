"""Regression tests for CSP generation and commit/input isolation.

Git integration uses disposable standalone repositories, not the user's index.
No network, Node packages, native compilation, or release credentials are used.
"""
from __future__ import annotations

import base64
import contextlib
import hashlib
import importlib.util
import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]


def load_script(name):
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), ROOT / "scripts" / name)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


csp = load_script("check-csp-hashes.py")
dev = load_script("dev.py")


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


class HookTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.repo = Path(self.temporary.name)
        for directory in ("scripts", ".githooks", "src", "src-tauri/src", "build"):
            (self.repo / directory).mkdir(parents=True, exist_ok=True)
        for script in ("pre-commit.py", "check-csp-hashes.py"):
            shutil.copy2(ROOT / "scripts" / script, self.repo / "scripts" / script)
        shutil.copy2(ROOT / ".githooks/pre-commit", self.repo / ".githooks/pre-commit")
        (self.repo / "src/main.ts").write_text("export const value = 1;\n")
        (self.repo / "src-tauri/src/lib.rs").write_text("// native source\n")
        (self.repo / "README.md").write_text("Original documentation\n")
        (self.repo / ".gitignore").write_text("build/\n__pycache__/\nnative-check-ran\n")
        self.config = self.repo / "src-tauri/tauri.conf.json"
        self.config.write_text(json.dumps({"app": {"security": {"csp": "script-src 'self' 'sha256-old'"}}, "version": "1.0.0"}) + "\n")
        (self.repo / "build/index.html").write_text("<script>new bootstrap</script>")
        (self.repo / "Makefile").write_text(
            f"precommit:\n\t@{sys.executable} scripts/pre-commit.py\n"
            f"prepare:\n\t@test \"$(FAIL_PREPARE)\" != \"1\"\n\t@{sys.executable} scripts/check-csp-hashes.py --write\n"
            "version:\n\t@true\ncheck-rs:\n\t@touch native-check-ran\ncheck-tooling:\n\t@true\n"
        )
        self.git("init", "-q")
        self.git("config", "user.name", "Workflow Test")
        self.git("config", "user.email", "workflow@example.invalid")
        self.git("config", "commit.gpgsign", "false")
        self.git("add", ".")
        self.git("-c", "core.hooksPath=/dev/null", "commit", "-qm", "baseline")
        self.git("config", "core.hooksPath", ".githooks")

    def git(self, *args, check=True, env=None):
        result = subprocess.run(["git", *args], cwd=self.repo, capture_output=True, text=True, env=env)
        if check and result.returncode:
            self.fail(result.stdout + result.stderr)
        return result

    def stage_frontend(self):
        (self.repo / "src/main.ts").write_text("export const value = 2;\n")
        self.git("add", "src/main.ts")

    def test_real_commit_stages_generated_hash_only(self):
        self.stage_frontend()
        (self.repo / "README.md").write_text("Unstaged documentation\n")
        result = self.git("commit", "-qm", "frontend change")
        self.assertIn("staged the generated CSP hash refresh", result.stdout + result.stderr)
        committed = self.git("diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD").stdout.splitlines()
        self.assertEqual(set(committed), {"src/main.ts", "src-tauri/tauri.conf.json"})
        self.assertEqual(self.git("diff", "--name-only").stdout.strip(), "README.md")
        self.assertFalse((self.repo / "native-check-ran").exists())
        self.assertNotIn("sha256-old", self.git("show", "HEAD:src-tauri/tauri.conf.json").stdout)

    def test_partial_staging_is_rejected_without_losing_edits(self):
        self.stage_frontend()
        source = self.repo / "src/main.ts"
        source.write_text("export const value = 3;\n")
        before_index = self.git("diff", "--cached").stdout
        result = self.git("commit", "-qm", "partial frontend", check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("build inputs differ", result.stderr)
        self.assertEqual(source.read_text(), "export const value = 3;\n")
        self.assertEqual(self.git("diff", "--cached").stdout, before_index)

    def test_commit_all_uses_gits_temporary_commit_index(self):
        (self.repo / "src/main.ts").write_text("export const value = 4;\n")
        self.git("commit", "-am", "frontend via commit -a")
        changed = self.git("diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD").stdout.splitlines()
        self.assertEqual(set(changed), {"src/main.ts", "src-tauri/tauri.conf.json"})
        self.assertEqual(self.git("status", "--porcelain").stdout, "")

    def test_unstaged_config_is_not_accidentally_added(self):
        self.stage_frontend()
        self.config.write_text(self.config.read_text().replace("1.0.0", "2.0.0"))
        before = self.config.read_bytes()
        result = self.git("commit", "-qm", "frontend", check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.config.read_bytes(), before)
        self.assertEqual(self.git("diff", "--cached", "--name-only").stdout.strip(), "src/main.ts")

    def test_untracked_frontend_input_is_rejected(self):
        self.stage_frontend()
        (self.repo / "src/untracked.ts").write_text("export {};\n")
        result = self.git("commit", "-qm", "frontend", check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("src/untracked.ts", result.stderr)

    def test_failed_check_aborts_commit(self):
        self.stage_frontend()
        before = self.git("rev-parse", "HEAD").stdout
        result = self.git("commit", "-qm", "frontend", check=False, env={**os.environ, "FAIL_PREPARE": "1"})
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.git("rev-parse", "HEAD").stdout, before)

    def test_native_change_runs_native_checks(self):
        (self.repo / "src-tauri/src/lib.rs").write_text("// changed native source\n")
        self.git("add", "src-tauri/src/lib.rs")
        self.git("commit", "-qm", "native")
        self.assertTrue((self.repo / "native-check-ran").exists())

    def test_hash_only_commit_does_not_require_rust(self):
        self.config.write_text(self.config.read_text().replace("sha256-old", "sha256-staged"))
        self.git("add", "src-tauri/tauri.conf.json")
        self.git("commit", "-qm", "hash only")
        self.assertFalse((self.repo / "native-check-ran").exists())

    def test_hook_install_does_not_overwrite_custom_hooks(self):
        self.git("config", "core.hooksPath", "custom-hooks")
        with patch.object(dev, "ROOT", self.repo), self.assertRaises(ValueError):
            dev.install_hooks()
        self.assertEqual(self.git("config", "--get", "core.hooksPath").stdout.strip(), "custom-hooks")


class ResourceTests(unittest.TestCase):
    def test_production_build_passes_version_as_make_argument_and_installs_executable(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "scripts").mkdir()
            (root / "bin").mkdir()
            shutil.copy2(ROOT / "scripts/native-resources.sh", root / "scripts/native-resources.sh")
            source = root / "core checkout"
            source.mkdir()
            (source / "go.mod").write_text("module example.invalid/core\n")
            (source / "Makefile").write_text(
                "VERSION = upstream-default\nTAGS ?= upstream-tags\n"
                'build:\n\t@test "$(VERSION)" != "fail"\n'
                '\t@printf "%s\\n%s\\n" "$(VERSION)" "$(TAGS)" > build-args.txt\n'
                '\t@printf "#!/bin/sh\\nexit 0\\n" > sing-box\n'
            )
            subprocess.run(["git", "init", "-q"], cwd=source, check=True)
            subprocess.run(["git", "add", "."], cwd=source, check=True)
            subprocess.run(["git", "-c", "user.name=Workflow Test", "-c", "user.email=workflow@example.invalid",
                            "-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null", "commit", "-qm", "fixture"], cwd=source, check=True)
            uname = root / "bin/uname"
            uname.write_text("#!/bin/sh\necho Darwin\n")
            uname.chmod(0o755)
            env = {**os.environ, "PATH": str(root / "bin") + os.pathsep + os.environ["PATH"],
                   "CORE_SRC": str(source), "CORE_VERSION": "zeytun-test", "CORE_TAGS": "tag-one tag-two",
                   "TARGET": "macos-aarch64", "MAKE": shutil.which("make")}
            result = subprocess.run(["bash", "scripts/native-resources.sh", "build-core"], cwd=root, env=env, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual((source / "build-args.txt").read_text(), "zeytun-test\ntag-one tag-two\n")
            binary = root / "src-tauri/resources/bin/macos-aarch64/zeytun-core"
            self.assertTrue(os.access(binary, os.X_OK))
            before = binary.read_bytes()
            result = subprocess.run(["bash", "scripts/native-resources.sh", "build-core"], cwd=root,
                                    env={**env, "CORE_VERSION": "fail"}, capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(binary.read_bytes(), before)

    def test_failed_download_preserves_previous_database_and_success_replaces_it(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "scripts").mkdir()
            (root / "bin").mkdir()
            shutil.copy2(ROOT / "scripts/native-resources.sh", root / "scripts/native-resources.sh")
            database = root / "src-tauri/resources/geoip.mmdb"
            database.parent.mkdir(parents=True)
            database.write_bytes(b"existing database")
            curl = root / "bin/curl"
            curl.write_text(f"#!{sys.executable}\nimport gzip, os, sys\nfrom pathlib import Path\n"
                            "if os.environ.get('FAIL_DOWNLOAD'): sys.exit(22)\n"
                            "payload = b'new data\\xab\\xcd\\xefMaxMind.com'\n"
                            "Path(sys.argv[sys.argv.index('-o') + 1]).write_bytes(gzip.compress(payload))\n")
            curl.chmod(0o755)
            env = {**os.environ, "PATH": str(root / "bin") + os.pathsep + os.environ["PATH"],
                   "PYTHON": sys.executable, "CORE_TAGS": "test", "TARGET": "macos-aarch64"}
            result = subprocess.run(["bash", "scripts/native-resources.sh", "geoip"], cwd=root, env={**env, "FAIL_DOWNLOAD": "1"}, capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(database.read_bytes(), b"existing database")
            self.assertEqual(list(database.parent.glob("*.download.*")), [])
            result = subprocess.run(["bash", "scripts/native-resources.sh", "geoip"], cwd=root, env=env, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(database.read_bytes(), b"new data\xab\xcd\xefMaxMind.com")

    def test_repair_removes_only_nonwritable_generated_copy(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            binary = root / "src-tauri/target/debug/resources/bin/macos-aarch64/zeytun-core"
            binary.parent.mkdir(parents=True)
            binary.write_text("generated")
            binary.chmod(0o444)
            source = root / "src-tauri/resources/bin/macos-aarch64/zeytun-core"
            source.parent.mkdir(parents=True)
            source.write_text("source")
            with patch.object(dev, "ROOT", root), patch.dict(os.environ, {"CARGO_TARGET_DIR": "target"}), contextlib.redirect_stdout(io.StringIO()):
                dev.repair_dev()
            self.assertFalse(binary.exists())
            self.assertEqual(source.read_text(), "source")


if __name__ == "__main__":
    unittest.main()

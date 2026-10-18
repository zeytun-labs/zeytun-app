"""Regression coverage for installer selection and the required CI gate."""
from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("ci_plan", ROOT / "scripts/ci-plan.py")
plan = importlib.util.module_from_spec(spec)
spec.loader.exec_module(plan)


def config(script_hash="old", **overrides):
    value = {"version": "1.0.0", "app": {"security": {"csp": f"script-src 'self' 'sha256-{script_hash}'; connect-src ipc:"}}}
    value.update(overrides)
    return json.dumps(value).encode()


class InstallerPolicyTests(unittest.TestCase):
    def test_main_and_tags_require_installers_but_feature_pushes_do_not(self):
        for ref, expected in (("refs/heads/main", True), ("refs/tags/v1.0.0-alpha", True), ("refs/heads/windows", False)):
            with self.subTest(ref=ref):
                self.assertEqual(plan.installers_required("push", ref, {}, []), expected)

    def test_manual_defaults_to_full_and_can_select_fast_checks(self):
        self.assertTrue(plan.installers_required("workflow_dispatch", "", {}, []))
        for value in (True, "true", False, "false"):
            with self.subTest(value=value):
                self.assertEqual(plan.installers_required("workflow_dispatch", "", {"inputs": {"build_installers": value}}, []), value in (True, "true"))
        with self.assertRaises(ValueError):
            plan.installers_required("workflow_dispatch", "", {"inputs": {"build_installers": "unexpected"}}, [])

    def test_regular_frontend_and_rust_prs_use_fast_checks(self):
        paths = ["src/routes/+page.svelte", "src-tauri/src/main.rs", "src-tauri/tests/example.rs", "CONTRIBUTING.md"]
        self.assertFalse(plan.installers_required("pull_request", "", {}, paths))

    def test_packaging_dependency_core_and_ci_changes_require_installers(self):
        for path in (
            "src-tauri/Cargo.lock", "src-tauri/Cargo.toml", "src-tauri/build.rs",
            "src-tauri/tauri.ci.json", "src-tauri/icons/icon.ico", "src-tauri/capabilities/default.json",
            "src-tauri/nsis/custom.nsh", "src-tauri/new-bundler/config.json",
            "scripts/core-release.json", "scripts/download-core.py", "scripts/check-csp-hashes.py",
            "rust-toolchain.toml", "pnpm-lock.yaml", ".github/actions/prepare-desktop/action.yml",
            ".github/workflows/ci.yml", "scripts/ci-plan.py",
        ):
            with self.subTest(path=path):
                self.assertTrue(plan.installers_required("pull_request", "", {}, [path]))

    def test_generated_hash_only_updates_avoid_installers(self):
        self.assertFalse(plan.tauri_settings_changed(config("old"), config("new")))
        self.assertFalse(plan.installers_required("pull_request", "", {}, [plan.TAURI_CONFIG], config_changed=False))
        # A hash refresh must not hide another packaging change in the same PR.
        self.assertTrue(plan.installers_required("pull_request", "", {}, [plan.TAURI_CONFIG, "src-tauri/Cargo.lock"], config_changed=False))

    def test_security_and_other_configuration_changes_still_require_installers(self):
        original = config()
        for modified in (
            original.replace(b"'self'", b"'unsafe-inline'"),
            original.replace(b"connect-src ipc:", b"connect-src *"),
            original.replace(b"1.0.0", b"1.0.1"),
            config("new", bundle={"targets": ["nsis"]}),
            b"{}", b"invalid", None,
        ):
            with self.subTest(modified=modified):
                self.assertTrue(plan.tauri_settings_changed(original, modified))

    def test_unknown_events_fail_instead_of_silently_skipping_installers(self):
        with self.assertRaises(ValueError):
            plan.installers_required("unexpected", "", {}, [])

    def test_ci_gate_only_accepts_an_intentionally_skipped_installer(self):
        # Execute the actual gate shell, keeping this test stdlib-only in CI.
        workflow = (ROOT / ".github/workflows/ci.yml").read_text()
        gate = workflow.split("      - name: Require every check and matrix entry to succeed", 1)[1]
        shell = gate.split("        run: |\n", 1)[1]
        commands = []
        for line in shell.splitlines():
            if line.strip() and not line.startswith("          "):
                break
            commands.append(line.removeprefix("          "))
        shell = "\n".join(commands)
        baseline = {"CHECKS_RESULT": "success", "FRONTEND_RESULT": "success", "DESKTOP_RESULT": "success"}
        cases = [("false", "skipped", True), ("true", "success", True), ("true", "skipped", False), ("true", "failure", False), ("true", "cancelled", False), ("", "skipped", False)]
        for required, result, accepted in cases:
            with self.subTest(required=required, result=result):
                env = dict(os.environ, **baseline, INSTALLERS_REQUIRED=required, PACKAGE_RESULT=result)
                run = subprocess.run(["bash", "-e", "-c", shell], env=env, capture_output=True)
                self.assertEqual(run.returncode == 0, accepted)
        for name in baseline:
            for result in ("failure", "cancelled", "skipped"):
                with self.subTest(name=name, result=result):
                    env = dict(os.environ, **baseline, INSTALLERS_REQUIRED="false", PACKAGE_RESULT="skipped")
                    env[name] = result
                    self.assertNotEqual(subprocess.run(["bash", "-e", "-c", shell], env=env, capture_output=True).returncode, 0)


class MergeDiffTests(unittest.TestCase):
    def test_policy_reads_merge_parent_and_detects_removed_packaging_files(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)

            def git(*args):
                return subprocess.check_output(["git", *args], cwd=repo, stderr=subprocess.DEVNULL).strip()

            git("init", "-q")
            git("config", "user.name", "CI policy test")
            git("config", "user.email", "ci@example.invalid")
            config_path = repo / plan.TAURI_CONFIG
            config_path.parent.mkdir()
            config_path.write_bytes(config())
            git("add", ".")
            git("-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null", "commit", "-qm", "base")
            config_path.write_bytes(config("new"))
            git("add", ".")
            git("-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null", "commit", "-qm", "hash refresh")
            event = repo / "event.json"
            event.write_text("{}")
            output = repo / "output"
            env = {"GITHUB_EVENT_NAME": "pull_request", "GITHUB_REF": "refs/pull/1/merge", "GITHUB_EVENT_PATH": str(event), "GITHUB_OUTPUT": str(output)}
            with patch.object(plan, "ROOT", repo), patch.dict(os.environ, env):
                plan.main()
            self.assertEqual(output.read_text(), "installers=false\n")
            config_path.unlink()
            git("add", "-u")
            git("-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null", "commit", "-qm", "remove config")
            output.unlink()
            with patch.object(plan, "ROOT", repo), patch.dict(os.environ, env):
                plan.main()
            self.assertEqual(output.read_text(), "installers=true\n")


if __name__ == "__main__":
    unittest.main()

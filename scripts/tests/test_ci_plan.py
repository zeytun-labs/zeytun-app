"""Exercise change selection against real merge history and fail-closed job results."""
from __future__ import annotations

import copy
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]


def load(name):
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), ROOT / "scripts" / name)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


plan = load("ci-plan.py")
gate = load("ci-gate.py")


def config(value="old", **overrides):
    result = {"version": "1.0.0", "app": {"security": {"csp": f"script-src 'self' 'sha256-{value}'; connect-src ipc:"}}}
    result.update(overrides)
    return json.dumps(result).encode()


class SelectionTests(unittest.TestCase):
    def test_documentation_and_tooling_only_avoid_native_jobs(self):
        for paths in (["README.md"], ["docs/help.md", ".github/ISSUE_TEMPLATE/bug.yml"], ["scripts/tests/test_csp.py"], ["scripts/dev.py"]):
            with self.subTest(paths=paths):
                result = plan.select_plan(paths)
                self.assertFalse(result["frontend"])
                self.assertFalse(result["native"])
                self.assertFalse(result["rust"])

    def test_frontend_change_builds_both_webviews_without_retesting_rust(self):
        result = plan.select_plan(["src/routes/+page.svelte"])
        self.assertTrue(result["frontend_tests"])
        self.assertEqual({e["platform"] for e in result["matrix"]["include"]}, {"macos", "windows"})
        self.assertTrue(all(not e["checks"] and not e["package"] for e in result["matrix"]["include"]))

    def test_rust_change_builds_frontend_without_repeating_frontend_checks(self):
        result = plan.select_plan(["src-tauri/src/main.rs"])
        self.assertTrue(result["frontend"])
        self.assertFalse(result["frontend_tests"])
        self.assertTrue(result["rust"])
        self.assertTrue(all(e["checks"] and not e["package"] for e in result["matrix"]["include"]))

    def test_platform_packaging_inputs_select_only_the_affected_installer(self):
        for path, platform in (("src-tauri/nsis/custom.nsh", "windows"), ("src-tauri/icons/icon.ico", "windows"),
                               ("scripts/native-resources.ps1", "windows"), ("src-tauri/Assets.car", "macos")):
            with self.subTest(path=path):
                entries = plan.select_plan([path])["matrix"]["include"]
                self.assertEqual(len(entries), 1)
                self.assertEqual(entries[0]["platform"], platform)
                self.assertTrue(entries[0]["package"])

    def test_build_inputs_and_unknown_files_get_packaging_coverage(self):
        for path in ("Makefile", "proto/started_service.proto", "package.json", "src-tauri/Cargo.lock",
                     "src-tauri/tauri.release.json", "scripts/ci-resources.json", "scripts/ci-smoke.py",
                     ".github/tauri-cli/pnpm-lock.yaml", ".github/workflows/release.yml", "new-build.config"):
            with self.subTest(path=path):
                self.assertTrue(all(e["package"] for e in plan.select_plan([path])["matrix"]["include"]))

    def test_generated_hash_changes_do_not_hide_security_or_packaging_changes(self):
        self.assertFalse(plan.tauri_settings_changed(config("old"), config("new")))
        result = plan.select_plan([plan.TAURI_CONFIG], config_changed=False)
        self.assertFalse(result["native"])
        for after in (config().replace(b"'self'", b"'unsafe-inline'"), config(version="2.0.0"), b"{}", None):
            self.assertTrue(plan.tauri_settings_changed(config(), after))
        result = plan.select_plan([plan.TAURI_CONFIG, "src-tauri/Cargo.lock"], config_changed=False)
        self.assertTrue(all(e["package"] for e in result["matrix"]["include"]))

    def test_release_validation_does_not_build_an_unsigned_release_again(self):
        result = plan.select_plan([], full=True)
        self.assertTrue(result["frontend_tests"])
        self.assertTrue(all(e["checks"] and not e["package"] for e in result["matrix"]["include"]))
        result = plan.select_plan([], full=True, installers=True)
        self.assertTrue(all(e["package"] for e in result["matrix"]["include"]))

    def test_invalid_input_cannot_silently_disable_checks(self):
        for value in ("", "unexpected", None):
            with self.assertRaises(ValueError):
                plan.boolean(value)

    def test_minimal_cli_matches_the_root_lockfile(self):
        import re
        cli = json.loads((ROOT / ".github/tauri-cli/package.json").read_text())["dependencies"]["@tauri-apps/cli"]
        locked = re.search(r"      '@tauri-apps/cli':\n        specifier: [^\n]+\n        version: ([^\n]+)", (ROOT / "pnpm-lock.yaml").read_text())
        self.assertIsNotNone(locked)
        self.assertEqual(cli, locked[1])


class GateTests(unittest.TestCase):
    def state(self, selected):
        return {"checks": {"result": "success", "outputs": {"frontend": selected, "native": selected}},
                "frontend": {"result": "success" if selected == "true" else "skipped"},
                "native": {"result": "success" if selected == "true" else "skipped"}}

    def test_selected_jobs_must_succeed_and_unselected_jobs_must_be_skipped(self):
        for selected in ("true", "false"):
            baseline = self.state(selected)
            gate.validate(baseline)
            for job in baseline:
                for result in ("success", "skipped", "failure", "cancelled"):
                    if result == baseline[job]["result"]:
                        continue
                    state = copy.deepcopy(baseline)
                    state[job]["result"] = result
                    with self.subTest(job=job, selected=selected, result=result), self.assertRaises(ValueError):
                        gate.validate(state)

    def test_missing_or_invalid_plan_is_rejected(self):
        for selected in ("", "unexpected", None):
            state = self.state("false")
            state["checks"]["outputs"]["native"] = selected
            with self.assertRaises(ValueError):
                gate.validate(state)


class HistoryTests(unittest.TestCase):
    def test_depth_two_merge_diff_includes_the_entire_pr_and_deletions(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repo = root / "repo"
            repo.mkdir()

            def git(*args):
                return subprocess.check_output(["git", *args], cwd=repo, stderr=subprocess.DEVNULL).strip()

            def commit(message):
                git("add", ".")
                git("-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null", "commit", "-qm", message)

            git("init", "-q", "-b", "main")
            git("config", "user.name", "CI policy test")
            git("config", "user.email", "ci@example.invalid")
            (repo / "Makefile").write_text("old")
            (repo / "README.md").write_text("base")
            commit("base")
            before = git("rev-parse", "HEAD").decode()
            git("checkout", "-qb", "feature")
            (repo / "Makefile").unlink()
            commit("remove packaging input")
            (repo / "README.md").write_text("last commit only touches docs")
            commit("docs")
            git("checkout", "-q", "main")
            git("-c", "commit.gpgsign=false", "-c", "core.hooksPath=/dev/null", "merge", "--no-ff", "feature", "-m", "tested merge")
            clone = root / "shallow"
            subprocess.run(["git", "clone", "-q", "--depth=2", repo.as_uri(), str(clone)], check=True)
            with patch.object(plan, "ROOT", clone):
                changed, _ = plan.changed_paths("pull_request", {})
                self.assertEqual(set(changed), {"Makefile", "README.md"})
                self.assertTrue(all(e["package"] for e in plan.select_plan(changed)["matrix"]["include"]))
            with patch.object(plan, "ROOT", repo):
                changed, _ = plan.changed_paths("push", {"before": before})
                self.assertEqual(set(changed), {"Makefile", "README.md"})
                with self.assertRaises(ValueError):
                    plan.changed_paths("unexpected", {})

    def test_manual_and_release_runs_force_validation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            event = root / "event.json"
            event.write_text("{}")
            output = root / "output"
            env = {"GITHUB_EVENT_PATH": str(event), "GITHUB_OUTPUT": str(output), "CI_BUILD_INSTALLERS": "false"}
            for name, full in (("workflow_dispatch", "false"), ("push", "true")):
                output.write_text("")
                with patch.dict(os.environ, dict(env, GITHUB_EVENT_NAME=name, CI_FULL_VALIDATION=full)):
                    plan.main()
                self.assertIn("native=true\n", output.read_text())
                self.assertIn("frontend_tests=true\n", output.read_text())


if __name__ == "__main__":
    unittest.main()

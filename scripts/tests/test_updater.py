"""Release gate and isolated updater-lab regressions; no signing secrets or app launches."""
from __future__ import annotations
import importlib.util
import json
import os
from pathlib import Path
import plistlib
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]

def load(name):
    spec = importlib.util.spec_from_file_location(name.replace('-', '_'), ROOT / 'scripts' / name)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

lab = load('update_lab.py')
release = load('verify-release.py')

class LabTests(unittest.TestCase):
    def test_port_version_identity_and_key_are_compiled_together(self):
        config = lab.config(9876, '0.1.2-lab.1')
        self.assertEqual(config['identifier'], 'com.zeytun.app.lab')
        self.assertEqual(config['productName'], 'Zeytun Lab')
        self.assertEqual(config['version'], '0.1.2-lab.1')
        self.assertEqual(config['plugins']['updater']['endpoints'], ['http://127.0.0.1:9876/latest.json'])
        production = json.loads((ROOT / 'src-tauri/tauri.conf.json').read_text())
        self.assertNotEqual(config['plugins']['updater']['pubkey'], production['plugins']['updater']['pubkey'])

    def test_custom_public_key_is_isolated_and_production_key_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            public = Path(directory) / 'fixture.pub'
            public.write_text((ROOT / 'src-tauri/tests/fixtures/updater/fixture.pub').read_text())
            self.assertEqual(lab.config(8787, '3.0.0', public)['plugins']['updater']['pubkey'], public.read_text().strip())
            production = json.loads((ROOT / 'src-tauri/tauri.conf.json').read_text())
            public.write_text(production['plugins']['updater']['pubkey'])
            with self.assertRaises(ValueError): lab.config(8787, '3.0.0', public)

    def test_invalid_port_and_version_fail(self):
        for port in [0, 65536]:
            with self.assertRaises(ValueError): lab.config(port, '1.0.0')
        for version in ['1..2', '01.0.0', 'garbage']:
            with self.assertRaises(ValueError): lab.config(8787, version)

    def fixture(self, directory):
        config = lab.config(9876, '3.0.0')
        (directory / 'lab-config.json').write_text(json.dumps(config))
        info = directory / 'candidate/Zeytun Lab.app/Contents/Info.plist'
        info.parent.mkdir(parents=True)
        info.write_bytes(plistlib.dumps({'CFBundleIdentifier': 'com.zeytun.app.lab', 'CFBundleShortVersionString': '3.0.0'}))
        (directory / 'Zeytun Lab_3.0.0.app.tar.gz').write_bytes(b'fixture')
        (directory / 'Zeytun Lab_3.0.0.app.tar.gz.sig').write_text('signature verified by native gate before publication')
        return info

    def test_manifest_is_bound_to_candidate_version_port_and_platform(self):
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory); self.fixture(directory)
            value = lab.manifest(directory, '3.0.0', 9876, 'darwin-x86_64')
            self.assertIn('darwin-x86_64', value['platforms'])
            self.assertIn(':9876/Zeytun%20Lab_3.0.0.app.tar.gz', value['platforms']['darwin-x86_64']['url'])
            for version, port in [('4.0.0', 9876), ('3.0.0', 8787)]:
                with self.assertRaises(ValueError): lab.manifest(directory, version, port, 'darwin-aarch64')

    def test_production_identity_cannot_be_published_as_lab(self):
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory); info = self.fixture(directory)
            data = plistlib.loads(info.read_bytes()); data['CFBundleIdentifier'] = 'com.zeytun.app'
            info.write_bytes(plistlib.dumps(data))
            with self.assertRaises(ValueError): lab.manifest(directory, '3.0.0', 9876, 'darwin-aarch64')

    def test_dry_run_needs_no_private_key_and_uses_generated_config(self):
        with tempfile.TemporaryDirectory(prefix='updater-lab-', dir='/tmp') as directory:
            env = {**os.environ, 'ZEYTUN_UPDATE_LAB_DIR': directory, 'ZEYTUN_UPDATE_LAB_PORT': '9876',
                   'ZEYTUN_UPDATE_LAB_KEY_PATH': str(Path(directory) / 'deliberately-missing.key')}
            result = subprocess.run(['bash', str(ROOT / 'scripts/update-lab.sh'), 'build', '3.0.0', '--dry-run'],
                                    env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn('--features updater-lab', result.stdout)
            self.assertIn('9876/latest.json', result.stdout)
            self.assertFalse((Path(directory) / 'installed').exists())

class ReleaseTests(unittest.TestCase):
    def test_wrapper_propagates_cryptographic_verifier_failure(self):
        with patch.object(sys, 'argv', ['verify-release.py', '--manifest', '/tmp/manifest.json', '--artifacts', '/tmp', '--tag',
                                     'v' + json.loads((ROOT / 'src-tauri/tauri.conf.json').read_text())['version']]), \
             patch.object(release.subprocess, 'run', side_effect=subprocess.CalledProcessError(1, 'cargo')) as run:
            with self.assertRaises(subprocess.CalledProcessError): release.main()
            argv = run.call_args.args[0]
            self.assertIn('verify-update', argv)
            self.assertIn('--locked', argv)
            self.assertIn('--release', argv)
            self.assertEqual(argv[argv.index('--target') + 1], 'aarch64-apple-darwin')
            self.assertTrue(run.call_args.kwargs['check'])

    def test_tag_mismatch_fails_before_any_build_or_verification(self):
        with patch.object(sys, 'argv', ['verify-release.py', '--manifest', '/tmp/manifest.json', '--artifacts', '/tmp', '--tag', 'v999.0.0']), \
             patch.object(release.subprocess, 'run') as run:
            with self.assertRaises(ValueError): release.main()
            run.assert_not_called()

    def test_renderer_has_no_stock_updater_permission_or_insecure_production_config(self):
        for path in (ROOT / 'src-tauri/capabilities').glob('*.json'):
            for permission in json.loads(path.read_text())['permissions']:
                identifier = permission if isinstance(permission, str) else permission['identifier']
                self.assertFalse(identifier.startswith('updater:'), path)
        config = json.loads((ROOT / 'src-tauri/tauri.conf.json').read_text())
        for name in ['dangerousInsecureTransportProtocol', 'dangerousAcceptInvalidCerts', 'dangerousAcceptInvalidHostnames']:
            self.assertFalse(config['plugins']['updater'].get(name, False))

if __name__ == '__main__': unittest.main()

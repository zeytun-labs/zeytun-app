#!/usr/bin/env python3
"""Fail-closed release gate, using the native app's archive verifier."""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]

def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--manifest', type=Path, required=True)
    parser.add_argument('--artifacts', type=Path, required=True)
    parser.add_argument('--tag', required=True)
    args = parser.parse_args()
    config = ROOT / 'src-tauri/tauri.conf.json'
    version = json.loads(config.read_text())['version']
    if args.tag != f'v{version}':
        raise ValueError('Release tag does not match production config')
    subprocess.run(['cargo', 'run', '--release', '--target', 'aarch64-apple-darwin', '--locked',
                    '--manifest-path', str(ROOT / 'src-tauri/Cargo.toml'),
                    '--features', 'updater-tools', '--bin', 'verify-update', '--',
                    '--config', str(config), '--manifest', str(args.manifest.resolve()),
                    '--artifacts', str(args.artifacts.resolve()), '--target', 'darwin-aarch64'], check=True)

if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Generate the lab's compiled endpoint and version-bound manifest."""
from __future__ import annotations
import argparse
import base64
import copy
import json
from pathlib import Path
import plistlib
import re
from datetime import datetime, timezone
from urllib.parse import quote

ROOT = Path(__file__).resolve().parents[1]
SEMVER = re.compile(r'(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?')

def key_material(encoded: str) -> bytes:
    text = base64.b64decode(encoded.strip(), validate=True).decode('utf-8')
    raw = base64.b64decode(text.splitlines()[1], validate=True)
    if len(raw) != 42 or raw[:2] != b'Ed':
        raise ValueError('Invalid Minisign public key')
    return raw[10:]

def config(port: int, version: str | None, public_key: Path | None = None) -> dict:
    if not 1 <= port <= 65535:
        raise ValueError('Port must be between 1 and 65535')
    base = json.loads((ROOT / 'src-tauri/tauri.conf.json').read_text())
    result = copy.deepcopy(json.loads((ROOT / 'src-tauri/tauri.dev-updater.json').read_text()))
    result['version'] = version or base['version']
    if not SEMVER.fullmatch(result['version']):
        raise ValueError('Expected a semantic version such as 0.1.2-lab.1')
    if public_key:
        result['plugins']['updater']['pubkey'] = public_key.read_text().strip()
    if key_material(result['plugins']['updater']['pubkey']) == key_material(base['plugins']['updater']['pubkey']):
        raise ValueError('The lab must use a separate signing key')
    result['plugins']['updater']['endpoints'] = [f'http://127.0.0.1:{port}/latest.json']
    result['build'] = {'beforeBuildCommand': 'make frontend-release'}
    return result

def manifest(directory: Path, version: str, port: int, target: str) -> dict:
    compiled = json.loads((directory / 'lab-config.json').read_text())
    metadata = plistlib.loads((directory / 'candidate/Zeytun Lab.app/Contents/Info.plist').read_bytes())
    if version != metadata['CFBundleShortVersionString'] or version != compiled['version']:
        raise ValueError('Manifest version must equal the built bundle version; build that version first')
    if metadata['CFBundleIdentifier'] != 'com.zeytun.app.lab':
        raise ValueError('Candidate must have the isolated lab identity')
    if compiled['plugins']['updater']['endpoints'] != [f'http://127.0.0.1:{port}/latest.json']:
        raise ValueError('Port differs from the compiled endpoint; rebuild with the selected port')
    if target not in ['darwin-aarch64', 'darwin-x86_64']:
        raise ValueError('Unsupported lab target')
    archive = directory / f'Zeytun Lab_{version}.app.tar.gz'
    signature = archive.with_suffix(archive.suffix + '.sig').read_text().strip()
    if not archive.is_file() or not signature:
        raise ValueError('Missing signed lab archive')
    return {'version': version, 'notes': f'Isolated local lab build {version}',
            'pub_date': datetime.now(timezone.utc).isoformat().replace('+00:00', 'Z'),
            'platforms': {target: {'signature': signature,
                                  'url': f'http://127.0.0.1:{port}/{quote(archive.name)}'}}}

def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    build = sub.add_parser('config')
    build.add_argument('--port', type=int, required=True)
    build.add_argument('--version')
    build.add_argument('--public-key', type=Path)
    build.add_argument('--output', type=Path, required=True)
    emit = sub.add_parser('manifest')
    emit.add_argument('--directory', type=Path, required=True)
    emit.add_argument('--port', type=int, required=True)
    emit.add_argument('--version', required=True)
    emit.add_argument('--target', required=True)
    emit.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    value = config(args.port, args.version, args.public_key) if args.command == 'config' else manifest(args.directory, args.version, args.port, args.target)
    # The shell verifies candidate manifests before atomically publishing them.
    args.output.write_text(json.dumps(value, indent=2) + '\n')

if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Fail the build if the CSP script-src hashes no longer match the bundle.

The webview is SSG (adapter-static), so every page emits two inline scripts:
a theme no-flash guard and the SvelteKit hydration bootstrap. CSP with
script-src 'self' alone would silently break hydration — no console error,
the app just renders dead HTML. tauri.conf.json pins both hashes by hand.

If SvelteKit or the theme script changes, the hashes go stale and hydration
breaks silently. This script regenerates them so the fix is a copy-paste.

Usage: python3 scripts/check-csp-hashes.py
"""
from __future__ import annotations

import base64
import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BUILD = ROOT / "build"
CONF = ROOT / "src-tauri" / "tauri.conf.json"
INLINE = re.compile(r"<script(?![^>]*src=)[^>]*>(.*?)</script>", re.S)


def build_hashes() -> set[str]:
    hashes: set[str] = set()
    for page in sorted(BUILD.glob("*.html")):
        for body in INLINE.findall(page.read_text()):
            digest = hashlib.sha256(body.encode()).digest()
            hashes.add(base64.b64encode(digest).decode())
    return hashes


def pinned_hashes() -> set[str]:
    csp = json.loads(CONF.read_text())["app"]["security"]["csp"]
    return {
        tok[len("'sha256-") : -1]
        for tok in re.findall(r"'sha256-[^']+'", csp)
    }


def main() -> int:
    if not BUILD.exists():
        print("build/ missing — run `pnpm run build` first", file=sys.stderr)
        return 2

    built = build_hashes()
    pinned = pinned_hashes()

    if built == pinned:
        print(f"CSP hashes OK ({len(built)} inline script(s))")
        return 0

    missing = built - pinned
    stale = pinned - built
    print("CSP script-src hashes out of date", file=sys.stderr)
    for h in sorted(missing):
        print(f"  MISSING in tauri.conf.json: 'sha256-{h}'", file=sys.stderr)
    for h in sorted(stale):
        print(f"  STALE in tauri.conf.json:   'sha256-{h}'", file=sys.stderr)
    print(
        f"\nEdit {CONF} so script-src lists exactly these:\n"
        + "".join(f"  'sha256-{h}'" for h in sorted(built)),
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())

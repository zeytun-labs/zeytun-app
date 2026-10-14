#!/usr/bin/env python3
"""Verify CSP hashes in CI; --write refreshes only script-src SHA-256 tokens.

SvelteKit emits inline theme and hydration scripts. Their exact bytes must be
allowed by the desktop webview's CSP. Run `make prepare` after frontend edits.
"""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import re
import sys
from html.parser import HTMLParser
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BUILD = ROOT / "build"
CONF = ROOT / "src-tauri" / "tauri.conf.json"
HASH = re.compile(r"'sha256-[^']+'")
SCRIPT_SRC = re.compile(r"(^|;)(\s*script-src\s+)([^;]*)")


class InlineScripts(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=False)
        self.bodies: list[str] = []
        self._body: list[str] | None = None

    def handle_starttag(self, tag, attrs):
        if tag == "script":
            self._body = [] if all(key != "src" for key, _ in attrs) else None

    def handle_data(self, data):
        if self._body is not None:
            self._body.append(data)

    def handle_endtag(self, tag):
        if tag == "script" and self._body is not None:
            self.bodies.append("".join(self._body))
            self._body = None


def build_hashes(build: Path = BUILD) -> set[str]:
    pages = sorted(build.rglob("*.html"))
    if not pages:
        raise ValueError("No built HTML found. Run `make build-web` first.")
    hashes: set[str] = set()
    for page in pages:
        parser = InlineScripts()
        parser.feed(page.read_bytes().decode("utf-8"))
        for body in parser.bodies:
            digest = hashlib.sha256(body.encode()).digest()
            hashes.add(base64.b64encode(digest).decode())
    if not hashes:
        raise ValueError("No inline scripts found; refusing to clear the CSP allowlist.")
    return hashes


def pinned_hashes(csp: str) -> set[str]:
    directive = SCRIPT_SRC.search(csp)
    if directive is None:
        raise ValueError("CSP has no script-src directive; refusing to create one implicitly.")
    return {token[len("'sha256-") : -1] for token in HASH.findall(directive[3])}


def updated_csp(csp: str, hashes: set[str]) -> str:
    pinned_hashes(csp)

    def replace(match):
        sources = HASH.sub("", match[3]).split()
        sources.extend(f"'sha256-{h}'" for h in sorted(hashes))
        return match[1] + match[2] + " ".join(sources)

    return SCRIPT_SRC.sub(replace, csp, count=1)


def sync_config(conf: Path, built: set[str], write: bool) -> int:
    source = conf.read_text()
    config = json.loads(source)
    csp = config["app"]["security"]["csp"]
    pinned = pinned_hashes(csp)
    if built == pinned:
        print(f"CSP hashes OK ({len(built)} inline script(s))")
        return 0
    if write:
        # Retain formatting and every other setting by replacing only this JSON string.
        replacement = updated_csp(csp, built)
        old = json.dumps(csp, ensure_ascii=False)
        new = json.dumps(replacement, ensure_ascii=False)
        if source.count(old) != 1:
            raise ValueError("Cannot locate the CSP string unambiguously in tauri.conf.json.")
        conf.write_text(source.replace(old, new, 1))
        print(f"Updated CSP script-src hashes in {conf}")
        return 0
    print("CSP script-src hashes out of date", file=sys.stderr)
    for h in sorted(built - pinned):
        print(f"  MISSING: 'sha256-{h}'", file=sys.stderr)
    for h in sorted(pinned - built):
        print(f"  STALE:   'sha256-{h}'", file=sys.stderr)
    print("Run `make prepare`, then commit src-tauri/tauri.conf.json.", file=sys.stderr)
    return 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="refresh hashes in tauri.conf.json")
    args = parser.parse_args()
    try:
        return sync_config(CONF, build_hashes(), args.write)
    except (OSError, ValueError, KeyError) as error:
        print(f"CSP check failed: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())

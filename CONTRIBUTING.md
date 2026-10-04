# Contributing to Zeytun

Zeytun is split across three repositories:
- **zeytun-app** (this repo) — Tauri + Svelte desktop client
- [zeytun-core](https://github.com/zeytun-labs/zeytun-core) — Go network engine
- [zeytun-config](https://github.com/zeytun-labs/zeytun-config) — Rust link parser

## Get started

1. Clone: `git clone https://github.com/zeytun-labs/zeytun-app.git`
2. Follow the [README](README.md) to build the core and run the app.
3. Do not commit generated core binaries, GeoIP databases, or local credentials.

## Checks

| Component | Command |
|---|---|
| Frontend | `pnpm install --frozen-lockfile && pnpm run check && pnpm run build && pnpm run test` |
| Rust | `cd src-tauri && cargo fmt --all --check && cargo clippy --all-targets -- -D warnings -A clippy::result_large_err && cargo test --all-targets` |

Or from the root: `make check` (frontend) and `make check-rs` (Rust).

## Security

Do not post vulnerabilities, credentials, or exploit details in public
issues. Use GitHub's private vulnerability reporting; otherwise contact
a maintainer privately before disclosing details. Never include real
subscription URLs, proxy credentials, or signing keys in reports or tests.

Contributions are licensed under the repository's GPL-3.0 license.

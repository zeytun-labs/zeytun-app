# Contributing to Zeytun

Zeytun is split across three repositories:
- **zeytun-app** (this repo) — Tauri + Svelte desktop client
- [zeytun-core](https://github.com/zeytun-labs/zeytun-core) — Go network engine
- [zeytun-config](https://github.com/zeytun-labs/zeytun-config) — Rust link parser

## Development setup

The current release target is macOS on Apple Silicon. The instructions below
describe a fresh checkout, with the app and core repositories side by side.

### Prerequisites

- Xcode Command Line Tools, including the macOS SDK and C compiler.
- Node.js 22 and pnpm 10.27.0 (the version in `package.json`).
- Rust stable and Cargo.
- Go 1.25.5 or newer for the core build. Go is required for this setup, not
  an optional frontend dependency.
- Protocol Buffers compiler (`protoc`); on macOS, `brew install protobuf`.
- Git, Make, curl, and gzip.

### Clone and prepare resources

```bash
git clone https://github.com/zeytun-labs/zeytun-app.git
cd zeytun-app
pnpm install --frozen-lockfile

git clone --branch zeytun-port https://github.com/zeytun-labs/zeytun-core.git ../zeytun-core
CGO_ENABLED=1 make build-core
```

`make build-core` preserves the production feature tags, including
`with_naive_outbound`, and copies the binary to
`src-tauri/resources/bin/macos-aarch64/zeytun-core` on Apple Silicon.
Keep CGO enabled and the C compiler available for that build.

Download the country database required by the Tauri resource configuration:

```bash
set -o pipefail
curl -fsSL --retry 3 \
  "https://download.db-ip.com/free/dbip-country-lite-$(date -u +%Y-%m).mmdb.gz" \
  | gzip -dc > src-tauri/resources/geoip.mmdb
test -s src-tauri/resources/geoip.mmdb
```

If this download fails, do not continue with the empty or partial database;
retry the download before building the app.

### Run the app

```bash
pnpm run tauri dev
```

This starts Vite and the native Tauri app together. Do not start a second Vite
server if port 1420 is already occupied by a development session.

For a local release bundle without updater signing secrets:

```bash
pnpm exec tauri build --target aarch64-apple-darwin \
  --config '{"bundle":{"createUpdaterArtifacts":false}}'
```

This is a local build, not an Apple-notarized release or a signed updater test.

### Network permissions

The default local mixed proxy can run without elevating the app. TUN needs
administrator privileges; the app requests permission when macOS denies
creation of the tunnel interface. Do not run the entire desktop app as root.

Do not commit generated core binaries, GeoIP databases, local profiles,
subscription URLs, credentials, or signing keys.

## Checks

| Component | Command |
|---|---|
| Frontend | `pnpm install --frozen-lockfile && pnpm run check && pnpm run build && pnpm run test` |
| Rust | `cd src-tauri && cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test --all-targets` |

Or from the root: `make check` (frontend) and `make check-rs` (Rust).

`make check` only type-checks the frontend; run its build and tests separately.
Run checks relevant to your change and include their real results in your PR.

For documentation changes, check relative links, anchors, images, and rendered
Markdown, then run `git diff --check`. Preserve donation addresses exactly and
keep the README's `support` anchor aligned with `.github/FUNDING.yml`.

## Security

Do not post vulnerabilities, credentials, or exploit details in public
issues. Use GitHub's private vulnerability reporting; otherwise contact
a maintainer privately before disclosing details. Never include real
subscription URLs, proxy credentials, or signing keys in reports or tests.

Contributions are licensed under the repository's GPL-3.0 license.

# CI and releases

Pull requests run CI against GitHub's merge commit. Pushes to `main` validate the
merged change. Feature branches, including `windows`, do not also run push CI.
Version tags run Release, which calls the same CI workflow for that exact revision
before any signing credentials are made available. Manual CI runs validate all
platforms; installer builds are an explicit option.

## Jobs and change selection

| Change | Shared checks | Frontend | Native validation | Installer |
| --- | --- | --- | --- | --- |
| Documentation or tooling tests | Versions and Python tests | Skipped | Skipped | Skipped |
| UI or static assets | Same | Types, tests, build, CSP | Embed and launch on both platforms | Skipped |
| Rust sources/tests | Same, plus Rust formatting | Build and CSP | Clippy, Rust tests and launch on both platforms | Skipped |
| Platform-specific bundle inputs | Same | Build and CSP | Build and launch affected platform | Affected platform |
| Dependencies, build configuration or CI | Same, plus formatting when needed | As selected by the planner | Affected native checks | Affected platforms |
| Release revision | Full | Full, once | Full on both platforms | Production macOS bundle in Release |

`scripts/ci-plan.py` compares the entire tested PR to the merge commit's first
parent. It uses the push event's `before` revision for merged changes, including
multi-commit pushes. Deletions are included and renames are treated as a deletion
and addition. Unknown build inputs select full coverage. Generated CSP hash-only
changes do not force native work. `Makefile`, protobuf inputs, CLI locks and release
configuration are build inputs.

`CI passed` accepts a skipped job only when the planner explicitly deselected it.
Every selected matrix entry must succeed. Require this check in the `main` branch
protection settings, with the branch up to date before merge. No workflow-level
path filter can leave the required check indefinitely pending.

## One frontend and one native preparation

The frontend is built on Linux. Its `build/` directory and matching production
`tauri.conf.json` travel together as the `frontend` artifact. CSP script hashes are
generated from that build; the remaining security policy is preserved. Every
native consumer checks the pair again. CI and Release both consume this artifact
and disable Tauri's frontend rebuild hook. Local `make frontend-release` and
`make ci-frontend` use the same generation and verification sequence. Hash values
left by a contributor's local build are not treated as portable release inputs.

Native runners install only the separately locked Tauri CLI, rather than the UI's
full dependency tree. A regression check keeps that CLI version aligned with the
root lockfile. When an installer is selected, it is built on the runner that just
ran the native checks; a separate packaging job does not repeat preparation.

## Runtime smoke coverage

The `ci-smoke` Cargo feature enables an isolated entry point, invoked only by
`--ci-smoke-test`. It requires `com.zeytun.app.ci`, loads the embedded UI, imports
its modulepreload graph, and requires a fresh token-bearing WebView-to-Rust IPC
handshake before exiting. It bypasses production initialization, tray setup and
privileged core startup. Production Release builds never enable this feature.

CI also runs the bundled core's `version` command and checks its Windows DLL and
GeoIP resources. Installer checks silently install NSIS into a temporary directory
or mount a DMG and copy its application, then run the handshake from the installed
application. These checks cover installation, binary dependencies, embedded asset
loading, WebView startup and IPC. They do not claim to exercise administrator
prompts, a real VPN connection or every UI interaction.

Production-feature Rust tests and updater-lab tests run separately. The macOS
local DNS integration test uses localhost services; it does not rely on a public
resolver. Live DB-IP availability is checked by the independent weekly Resource
health workflow, outside the PR merge gate.

## Pinned resources and cache policy

`scripts/core-release.json` pins core binaries, corresponding source and their
SHA-256 checksums. `scripts/ci-resources.json` pins official protoc archives and a
DB-IP Country Lite snapshot. Downloads have bounded retries, size limits and
checksum verification. Cached archives are verified again before use. GeoIP
installation validates the database marker and atomically replaces an old file.
The same installer is used on Windows and macOS.

Review resource updates as normal pull requests. For a new protoc version, obtain
asset hashes from the official Protocol Buffers release and update both platform
pins. Resource health prints the current month's GeoIP URL and candidate hash;
review them and update the GeoIP pin monthly. DB-IP attribution and CC BY 4.0
remain part of the app and resource manifest. The health job never automatically
changes the production snapshot or commits files.

Protoc, core and GeoIP archives have independent content-based cache keys. The
GeoIP archive can be shared across operating systems. Rust dependency caches are
separated by target and by native-validation versus production-release usage;
they are saved only after successful jobs. They do not attempt to reuse a macOS
binary on Windows or a debug binary as an optimized release. Cache misses are
safe; they affect duration rather than correctness. Old cache generations may
be deleted after a replacement run succeeds.

CI omits debugger symbols from development/test profiles to reduce cache size and
linking time, while retaining debug assertions and overflow checks. Production
release optimization is unchanged. The updater verifier uses the same optimized
Apple Silicon target as the release bundle instead of rebuilding a second set of
host/debug dependencies after packaging.

## Release scope

Release produces an Apple Silicon macOS app/DMG with the existing signing policy
and updater key. It verifies the app signature, bundled resources, corresponding
core source and updater manifest, and creates a draft for human review. Dry runs
build and retain a DMG without accessing updater signing keys. Windows installer
validation is part of CI; Windows production publishing/updater distribution is
not yet enabled. Linux native desktop packaging is not a supported target.

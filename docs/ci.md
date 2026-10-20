# CI and releases

Pull requests run CI against GitHub's merge commit. Pushes to `main` validate the
merged change. Feature branches, including `windows`, do not also run push CI.
Version tags run Release, which calls the same CI workflow for that exact revision
before any signing credentials are made available. Manual CI runs validate all
platforms; debug installer builds and optimized installer validation are explicit
options. Routine PR and `main` checks never compile an optimized release.

## Jobs and change selection

| Change | Shared checks | Frontend | Native validation | Installer |
| --- | --- | --- | --- | --- |
| Documentation or tooling tests | Versions and Python tests | Skipped | Skipped | Skipped |
| UI or static assets | Same | Types, tests, build, CSP | Embed and launch on both platforms | Skipped |
| Rust sources/tests | Same, plus Rust formatting | Build and CSP | Clippy, Rust tests and launch on both platforms | Skipped |
| Frontend dependencies/configuration | Same | Types, tests, build, CSP | Embed and launch on both platforms | Skipped |
| Rust dependencies/toolchain or protobuf | Same, plus Rust formatting | Build and CSP; frontend checks for protobuf | Clippy, Rust tests and launch on both platforms | Skipped |
| Platform-specific bundle inputs | Same | Build and CSP | Build and launch affected platform | Debug, affected platform |
| Bundle resources, configuration or build workflows | Same, plus formatting when needed | As selected by the planner | Affected native checks | Debug, affected platforms |
| Explicit optimized installer validation | Full | Full, once | Full on both platforms | Optimized, both platforms |
| Release revision | Full | Full, once | Full on both platforms | Production macOS bundle in Release |

`scripts/ci-plan.py` compares the entire tested PR to the merge commit's first
parent. It uses the push event's `before` revision for merged changes, including
multi-commit pushes. Deletions are included and renames are treated as a deletion
and addition. Unknown build inputs select full coverage. Generated CSP hash-only
changes do not force native work. `Makefile`, protobuf inputs, CLI locks and release
configuration are build inputs.

All automatic native builds use the debug profile, including selected DMG/NSIS
installers. Tests, the unbundled app and debug installers use the same
`updater-tools,ci-smoke` feature set. Local DNS checks follow the production-path
tests; the updater-lab variant runs last to avoid switching back and recompiling
the test library. Debug assertions remain enabled.

Before merging changes whose behavior depends on release optimization or
`debug_assertions`, run optimized validation on the latest PR branch revision:

```sh
gh workflow run ci.yml --ref YOUR_BRANCH -f release_installers=true
```

This option forces full checks and optimized installer smoke tests on both
platforms, even when `build_installers` is false. Check that the successful run's
head SHA matches the PR before merging; the ordinary `CI passed` check does not
enforce this additional maintainer check. To test debug installers explicitly,
use `-f build_installers=true` instead. CI artifacts include their profile in the
name and use the isolated CI app identity; they are not distribution releases.

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
The debug installer tests installation and embedded assets without paying for
a second optimized compilation. Production Release still builds optimized
artifacts and performs its signing and bundle checks.

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
separated by target, debug versus optimized validation, and production release;
they are saved only after successful jobs. They do not attempt to reuse a macOS
binary on Windows or a debug binary as an optimized release. Cache misses are
safe; they affect duration rather than correctness. Old cache generations may
be deleted after a replacement run succeeds.

The pinned compiler is installed into a dedicated `RUSTUP_HOME` under the runner
tool cache before restoring Rust dependencies. The cache action considers every
installed toolchain, so isolating this directory prevents unused compiler
versions in different hosted runner images from changing the key. Only the
project's pinned toolchain is present there. This is runner-local setup; it does
not change contributors' Rust installations. Dependency caching still leaves the
application and test binaries to be compiled, and a cold run remains slower.

When comparing CI duration, record the Rust cache hit/miss, native build profile
and selected jobs. Compare the first run and a rerun of the same revision; include
cache restore/save time and report queue time separately. An installer-enabled
PR is a heavier workload than a source-only or documentation-only PR.

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

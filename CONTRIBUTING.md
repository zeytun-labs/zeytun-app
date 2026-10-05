# Contributing to Zeytun

Thanks for helping improve Zeytun. Bug reports, focused code changes,
documentation, and UI feedback are welcome.

## Before you start

Search existing issues and pull requests to avoid duplicating work.

- **Small bug fixes and documentation corrections:** you may open a PR directly.
  An issue is useful when the problem needs a separate explanation, but is not
  required for a typo or a straightforward fix.
- **New features and substantial UI changes:** open an issue describing the
  problem and proposed approach, and wait for maintainer agreement before
  implementing the change.
- **Architecture changes, new dependencies, database migrations, or changes to
  networking behavior:** agree on the scope and risks in an issue first.
- **Security vulnerabilities:** follow [Security](#security), not the public
  bug-report or PR process.

Agreement on an approach is not a promise to merge its implementation. Changes
must fit the product's direction and remain practical to maintain.

### Choose the right repository

- **zeytun-app** (this repo) — desktop UI, Tauri integration, and app-side behavior
- [zeytun-core](https://github.com/zeytun-labs/zeytun-core) — Go network engine
- [zeytun-config](https://github.com/zeytun-labs/zeytun-config) — Rust link parser

Submit changes to the repository that owns the code. For cross-repository work,
link the related issues and PRs, explain their dependencies, and state the
required merge order.

## Reporting bugs and proposing features

Use the [issue forms](https://github.com/zeytun-labs/zeytun-app/issues/new/choose).
Report one problem per issue.

For bugs, include the Zeytun version or source commit, macOS version, steps to
reproduce, and expected versus actual behavior. For networking problems, include
the relevant capture and routing modes. Attach sanitized logs or screenshots
only when they help explain the problem.

For features, explain the user problem, the proposed behavior, and alternatives
you have considered. Discuss significant changes before writing the code.

## Opening a pull request

1. Fork this repository on GitHub. Follow [Development setup](#development-setup),
   replacing the app clone URL with your fork's URL. If you already have a
   checkout, keep it and configure a remote for your fork instead of cloning
   again. The commands below assume `origin` points to your fork and `upstream`
   points to `zeytun-labs/zeytun-app`; check `git remote -v` before pushing.
2. Create a topic branch from the current upstream `main`. In a fresh fork
   checkout, `origin` is your fork; add `upstream` once:

   ```bash
   git remote add upstream https://github.com/zeytun-labs/zeytun-app.git
   git fetch upstream
   git switch -c fix/describe-the-change upstream/main
   ```

   If `upstream` already exists, skip adding it. Use a descriptive branch name;
   `fix/`, `feat/`, and `docs/` are useful prefixes, not enforced requirements.
3. Make a focused change, add or update tests and documentation where needed,
   and run the relevant [checks](#checks).
4. Commit the intended files and push your branch to your fork:

   ```bash
   git push -u origin HEAD
   ```

5. Open a PR on `zeytun-labs/zeytun-app` with **base branch `main`** and your
   fork's topic branch as the head. Fill in the PR template. Use a **Draft PR**
   for unfinished work or early feedback, and mark it ready when it is reviewable.

### Scope and style

- Keep each PR focused on one independently reviewable change. Separate unrelated
  fixes, refactors, and formatting changes.
- Follow the surrounding code and existing tools. Explain why a new dependency
  is necessary; prefer existing dependencies when they meet the need.
- Add a regression test for a bug fix when practical. If automated coverage is
  not practical, explain why and provide reproducible manual verification steps.
- Document changes to user-visible behavior. For breaking changes, identify who
  is affected and any migration or recovery steps.
- Do not change release versions, signing configuration, or generated release
  artifacts without prior coordination. Do not include unrelated lockfile churn.
- Never commit credentials, signing keys, subscription URLs, local profiles,
  personal databases, core binaries, or GeoIP downloads.

### PR title and description

Use an English title with a short type prefix, for example
`fix: preserve a temporary rule's expiry`, `feat: add an inspector filter`, or
`docs: clarify development setup`. Individual work-in-progress commits do not
need this format, and signed commits are not required by this contribution policy.

Explain the problem, the solution and its rationale, and link a related issue
when one exists. Use `Fixes #123` only when merging the PR should close that issue.
Include the exact check commands you ran and their results. List checks you did
not run and explain why; never report an unexecuted check as passed.

For UI changes, include before/after screenshots or a short recording from the
actual app. Remove identifying network details. Describe any limitations of the
evidence, including unavailable native-app or network-mode testing.

### Review and merge

Keep review discussion respectful and technical. Respond to feedback and push
follow-up commits to the same PR branch. Keep the PR description and verification
results current as the implementation changes.

Before merge, relevant CI checks must pass, review concerns must be resolved,
and a maintainer must approve the change. Passing CI alone does not establish
that the app behaves correctly or that a change fits the project. Fork workflow
runs may need maintainer approval before they start; contributors do not need
release credentials or signing keys.

Maintainers normally use **squash merge** so a focused PR becomes one clear commit.
You do not need to squash your intermediate commits yourself. Maintainers may
request changes or decline a PR because of scope, maintenance cost, or product
direction, and will explain the reason. There is no guaranteed review timeline.

### AI-assisted contributions

AI tools are allowed, but you are responsible for everything you submit. Read,
understand, review, and test generated code and documentation before opening a
PR. Be able to explain the implementation and address review feedback. Plausible
generated output is not evidence that a check was run or a behavior was verified.

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
| Rust | `cd src-tauri && cargo fmt --all --check && cargo clippy --all-targets --all-features -- -D warnings -A clippy::result_large_err && cargo test --all-targets` |

For frontend changes, also run `python3 scripts/check-csp-hashes.py` after the
build, as CI does. Rust checks require the core and GeoIP resources prepared
above. These checks do not require updater signing secrets.

`make check` only type-checks the frontend; run its build and tests separately.
`make check-rs` runs formatting, Clippy, and tests, but its Clippy flags differ
from CI. Use the Rust commands in the table for CI parity.

Run checks relevant to your change and include their real results in your PR.
For changes to routing, permissions, or app lifecycle, also describe the manual
native-app scenarios you exercised and their outcomes. CI's core build omits
`with_naive_outbound`; it is not a substitute for verifying a production build
when a change affects that integration.

For documentation changes, check relative links, anchors, images, and rendered
Markdown, then run `git diff --check`. Preserve donation addresses exactly and
keep the README's `support` anchor aligned with `.github/FUNDING.yml`.

## Security

Do not post vulnerabilities, credentials, or exploit details in public
issues or PRs. Use **Security → Report a vulnerability** if private reporting
is available; otherwise contact a maintainer privately before disclosing
details. If you do not have a private contact route, open an issue asking how
to contact a maintainer without describing the vulnerability.

Never include real subscription URLs, proxy credentials, signing keys, or
identifying traffic details in reports, screenshots, logs, or test fixtures.

## License

By submitting a contribution, you agree that it is licensed under the
repository's [GPL-3.0 license](LICENSE). You must have the right to submit the
code, documentation, and assets you include. Preserve applicable copyright and
license notices, and identify the source and license of third-party material.

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

Clone **this public repository** directly. All commands below run from its root;
no private parent repository or sibling checkout is required. The native target
is macOS (Apple Silicon releases). Frontend checks also run on Linux.

### Prerequisites

For all contributors:

- Node.js 22 (`.nvmrc`) and pnpm 10.27.0 (`package.json`).
- Python 3.10 or newer, Git, and Make. No Python packages are required.

For native development and app bundles:

- macOS with Xcode Command Line Tools (`xcode-select --install`).
- Rust stable with rustfmt and Clippy (`rustup component add rustfmt clippy`).
- Go 1.25.5 or newer, with CGO and the C compiler available for the production
  core, including Naive.
- Protocol Buffers compiler (`brew install protobuf`), curl, and gzip.

`make doctor-web` checks frontend tools; `make doctor` also checks native tools.
The tools must already be installed; setup does not change system toolchains.

### One-time setup

```bash
git clone https://github.com/zeytun-labs/zeytun-app.git
cd zeytun-app
make setup
make dev
```

Use your fork's URL if contributing. `make setup` installs locked frontend
packages, installs the pre-commit hook for this clone, clones the public
[zeytun-core](https://github.com/zeytun-labs/zeytun-core) `zeytun-port` branch
into ignored `.cache/zeytun-core`, builds the production core, and downloads the
DB-IP country database. Existing core/data files are validated and reused.
Failed downloads leave an existing database intact.

For frontend work on macOS or Linux without building the native core:

```bash
make setup-web
make dev-web
```

Vite alone does not provide Tauri IPC; native API calls need `make dev`.

### Daily commands

Run `make` or `make help` to list commands.

| Command | Purpose |
|---|---|
| `make dev` | Start Vite and **Zeytun Dev** together, using the separate development app identifier |
| `make dev-web` | Start Vite only |
| `make build` | Build a local app/DMG with fresh frontend/CSP, without updater signing secrets |
| `make prepare` | Type-check, test, build the frontend, and refresh generated CSP hashes |
| `make ci-frontend` | Run frontend and tooling checks exactly as CI does, including CSP verification |
| `make check-rs` | Run Rust format check, Clippy, and tests with CI flags |
| `make ci` | Run both frontend and Rust checks |
| `make fmt` | Format Rust before staging it |
| `make build-core` | Rebuild the full production core from the cached source checkout |
| `make geoip` | Refresh the country database with a validated download |
| `make hooks` | Install the hook in an existing clone |
| `make clean` | Remove frontend and Cargo build outputs, keeping cached source and bundled resources |

`make build-ui` remains an alias for `make build`; `make check` only type-checks
the frontend. Local native bundles are not Apple-notarized releases or signed
updater tests. Maintainer releases continue through the Release workflow.

Do not run a second Vite server on port 1420. Run development/build commands as
your normal user. Core startup currently requests administrator permission to
set up the core executable. `make dev`, `make build`, and `make check-rs` remove
non-writable **generated** core copies before Cargo needs to replace them; source
files and the cached core checkout are retained.

To develop the core itself, edit `.cache/zeytun-core`, then run `make build-core`.
Existing core checkouts are never automatically pulled or reset. You can also
use your own public core checkout:

```bash
make build-core CORE_SRC=/absolute/path/to/zeytun-core
```

`CORE_REPO`, `CORE_REF`, `CORE_TAGS`, `CORE_VERSION`, and `TARGET` are overridable
Make variables. `CORE_REF` selects the branch/tag for a new clone; it does not
switch an existing checkout. The bundled binary path is selected from the host
architecture. Apple Silicon is the release target; Intel builds are not released.
CI's `make build-core-ci` omits Naive/CGO and is not a substitute for checking a
production build when a change affects that integration.

### Before committing: the pre-commit hook

`make setup` and `make setup-web` configure this clone's `core.hooksPath` to
`.githooks`. The hooks are versioned here, require no extra package, and are not
installed automatically by cloning. Setup preserves an existing custom hook
configuration and reports how to integrate it instead of silently replacing it.

After editing, stage the intended files and commit normally:

```bash
git add src/path/to/changed-file.svelte
git commit -m "fix: describe the change"
```

The hook checks whitespace for every commit. For frontend/build-input changes
it runs `make prepare`: version consistency, tooling tests, Svelte/TypeScript
checks, frontend tests, a fresh production build, and CSP synchronization. It
adds **only the generated CSP hash refresh** to the commit. For Rust/native
changes it runs `make check-rs` too. A CSP hash-only edit does not trigger Rust
compilation. Documentation-only commits need only the whitespace check.

A build must describe the files in the commit. If relevant input files have
unstaged edits (including partially staged files or untracked frontend source),
the hook stops with their names. Stage the intended complete files or use a
clean worktree for that commit. Unrelated unstaged documentation can stay.
The hook never stashes edits or stages source code for you. Failed checks abort
the commit and print the failed command.

You may also run `make prepare` before staging; then explicitly stage the
updated `src-tauri/tauri.conf.json` with your source changes. Use `make fmt`
before staging Rust if its formatting check fails. Each clone needs setup once;
CI verifies the same checks even when local hooks have not been installed.

### Why CSP hashes change

The desktop webview's Content Security Policy (CSP) allows scripts from the app
and specifically approved inline scripts. The static frontend contains a theme
initialization script and SvelteKit's hydration bootstrap. The bootstrap turns
rendered HTML into the interactive app. Its SHA-256 hash can change when the
frontend output changes, even if you did not edit a script tag directly.

The generated hashes in `src-tauri/tauri.conf.json` must match the exact built
script bytes. Otherwise the webview can block hydration. `make prepare` and the
hook refresh the hashes; CI's `make ci-frontend` verifies them without modifying
the configuration. This keeps the policy restrictive without manual hash
copying or enabling `unsafe-inline` for scripts.

## Checks

CI runs on pull requests and pushes to `main` and `windows`. The Windows job
checks frontend types/tests and generated CSP hashes, runs Rust formatting,
Clippy and tests on `windows-latest`, and builds an unsigned NSIS installer.
It uses `scripts/native-resources.ps1` to build the Windows core and download
GeoIP. No release or signing secrets are needed. The CI-only
`src-tauri/tauri.windows-ci.json` overlay skips rebuilding the already-verified
frontend and disables updater signing artifacts; it does not change the shared
security policy. Generated CSP hashes must be committed before CI runs.

Use `make ci-frontend` for frontend/tooling changes, `make check-rs` for Rust,
and `make ci` before a PR that affects both. Rust checks require the core and
GeoIP resources from `make setup`. No updater signing secrets are needed.

Report the commands actually run and their results in your PR. For routing,
permissions, or lifecycle changes, also describe the native-app scenarios you
exercised. Passing automated checks does not establish runtime behavior.

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

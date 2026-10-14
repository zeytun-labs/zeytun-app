# Isolated updater lab

The lab exercises the real About → Check → Download → Restart flow without GitHub releases. Every candidate must be a newly built, signed bundle with its actual version. Changing only `latest.json` cannot advance or downgrade the installed application.

The lab uses `com.zeytun.app.lab`, product name `Zeytun Lab`, separate application data, and native ports 29090/29999/26060–26062. It requires both the `updater-lab` Cargo feature and the lab identifier before the native checker will use its loopback HTTP endpoint. Production uses GitHub discovery, HTTPS, the production public key, and strict version checks. Renderer access to stock updater IPC is disabled in every build.

## Signing keys

The checked-in lab public key is **74EAAC25262944EC**. The production public key is **93C4275E873370FC**. They are separate trust domains. Do not use the production private key in the lab or change the production public key to make a lab update pass.

By default, build reads the lab private key from `~/.tauri/zeytun-lab.key` and its password from `~/.tauri/zeytun-lab.key.password`. To use another existing lab key that matches the checked-in public key, set `ZEYTUN_UPDATE_LAB_KEY_PATH` and `ZEYTUN_UPDATE_LAB_PASSWORD_FILE`.

Alternatively, create your own disposable lab key and point the generated build configuration at its public half:

```bash
pnpm exec tauri signer generate -w ~/.tauri/zeytun-lab.key
# Store its password in ~/.tauri/zeytun-lab.key.password (empty for no password).
export ZEYTUN_UPDATE_LAB_PUBLIC_KEY_PATH="$HOME/.tauri/zeytun-lab.key.pub"
```

The generated lab config includes this public key without editing production configuration. Native checks reject reusing production key material, including an identical key with a changed untrusted comment. Use a fresh lab directory when changing lab keys: an existing installed baseline retains its embedded trust key. Keep private keys/passwords outside the serve directory.

## Baseline and a real upgrade

```bash
scripts/update-lab.sh build 0.1.1-lab.1
scripts/update-lab.sh run
scripts/update-lab.sh serve
```

In another terminal, with the same environment variables:

```bash
scripts/update-lab.sh build 0.1.1-lab.2
scripts/update-lab.sh manifest 0.1.1-lab.2
# In the running baseline: About → Check for updates → Download → Restart.
```

`build` initializes `installed/Zeytun Lab.app` only once. Subsequent builds replace `candidate/Zeytun Lab.app` and stage a version-specific signed archive, leaving the running installation and earlier archive URLs intact. The update installs into `installed`, then relaunches it. Repeat with a higher embedded version for each actual upgrade.

`manifest VERSION` requires that VERSION equal the candidate's embedded version and the generated build config. It reuses the detached signature from the build, runs the same cryptographic/archive verifier as CI and the application, and publishes the manifest atomically only on success. It never re-signs an old bundle under a new label. Normal, lab, and direct native calls all reject equal versions and downgrades; the old `allowDowngrades` devtools shortcut is unavailable.

## Port, directory, architecture and diagnostics

- `ZEYTUN_UPDATE_LAB_PORT` defaults to `8787`. Set it before **building**, serving and creating the manifest. `build` generates `lab-config.json` with that exact port and passes it to Tauri; a later mismatch is rejected.
- `ZEYTUN_UPDATE_LAB_DIR` defaults to `/tmp/zeytun-update-lab`. Its canonical path must be a child of `/tmp` or this checkout's `.lab-update`. The directory contains public artifacts and generated public configuration, never private signing keys.
- The manifest target follows the native build architecture (`darwin-aarch64` or `darwin-x86_64`). The verifier checks the signed executable's Mach-O architecture.
- `build [VERSION] --dry-run` shows the generated isolated configuration without requiring a private key or launching/building an app. Omitting VERSION uses the checkout version.
- `status` shows the installed/candidate versions, current manifest and server status. `serve` binds only to `127.0.0.1` and shuts down its own server on exit. `LAB_CHECK_DELAY` and `LAB_RATE_KB_S` retain the latency/bandwidth controls.

The lab bundles use ad-hoc macOS code signing. Minisign verification remains mandatory; HTTP is allowed only for the explicit lab build and loopback origin. An unbundled `tauri dev` process can check metadata but cannot install, and an isolated development identity cannot accept a production bundle. Use the lab bundle for verified download/install testing. Use permanent frontend tests for fast UI-state iteration, and `cargo test --all-targets --all-features` for signature, replay, archive metadata, endpoint and CLI verification regressions. No test needs a production signing secret.

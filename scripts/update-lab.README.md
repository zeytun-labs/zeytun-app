# Update lab — test the self-update flow forever, locally

Zero GitHub Releases, zero rebuilds per cycle. The plugin only compares
`version` and verifies the minisign signature, so re-signing the *same*
`.app.tar.gz` while bumping `latest.json` yields unlimited update cycles.

Lab dir: `/tmp/zeytun-update-lab` (env `ZEYTUN_UPDATE_LAB_DIR`), port `8787`
(env `ZEYTUN_UPDATE_LAB_PORT`). All commands: `scripts/update-lab.sh`
(build [--dry-run] | manifest VERSION | serve | run | status).

## One-time setup

```bash
scripts/update-lab.sh build      # ~minutes: Rust release build, stages .app + .app.tar.gz + .sig
scripts/update-lab.sh manifest 0.1.2
```

`build` merges `src-tauri/tauri.dev-updater.json` over `tauri.conf.json`, which
points the endpoint at `http://127.0.0.1:8787/latest.json` and sets
`dangerousInsecureTransportProtocol: true` — required in a *release* bundle,
because `validate_endpoints()` only warns under `debug_assertions` and hard-fails
(`Error::InsecureTransportProtocol`) in non-debug builds (updater 2.11.0
`src/config.rs:160-179`). The pubkey is the real one, so real signatures verify.

## One update cycle (repeat forever)

Terminal A — keep it running:

```bash
scripts/update-lab.sh serve
```

Terminal B — each cycle:

```bash
scripts/update-lab.sh manifest 0.1.2   # re-sign + rewrite latest.json
scripts/update-lab.sh run              # launch the staged .app (never /Applications)
# in the app: Settings → About → Check for updates → Download → Restart
```

After the relaunch the app *is* the staged `.app`, so it still points at
127.0.0.1:8787. Next cycle is just `manifest 0.1.3` + the in-app click.
`status` shows staged version, manifest version, server state, tarball sha256.

## 100 consecutive updates

```bash
scripts/update-lab.sh serve &                     # terminal A, leave running
scripts/update-lab.sh run
for v in $(seq 2 101); do
  scripts/update-lab.sh manifest "0.1.$v"
  scripts/update-lab.sh run                       # only if you quit it yourself
  # click Check → Download → Restart in the app; wait for relaunch
done
```

The clicks can't be scripted here (real window), so the loop is: manifest →
in-app Download → Restart. Each `manifest` call takes ~1s.

Downgrades also work: `check` accepts `allowDowngrades`, which swaps the
comparator from "greater than" to "not equal" (updater 2.11.0
`src/commands.rs:67-69`, JS binding exposes `allowDowngrades?: boolean`). The
app's `updaterStore.check()` does **not** pass it today — so to walk versions
*down* you must either keep bumping up, or invoke it directly from the devtools
console of a `pnpm tauri dev` window:

```js
await window.__TAURI__.core.invoke("plugin:updater|check",
  { allowDowngrades: true })
```

## Tier 2 — frontend-only loop (no bundle, fast UI iteration)

Drives the real `updaterStore` state machine against a mocked plugin, via CDP on
the vite dev server (port 1420 — already running; never start or kill it). Use it
for phase transitions, progress bar, toasts, sidebar badge, error paths:

```js
// in a CDP Runtime.evaluate on http://localhost:1420
// mock check/download/install, then walk the store through every phase
```

This exercises `idle → checking → available → downloading → ready` and the
`available` getter, `#notifyAvailable` localStorage dedupe, proxy fallback, and
failure branches — all the frontend logic — in milliseconds.

## Caveats

- `pnpm tauri dev` **cannot** test `install()`: there is no `.app` bundle on disk
  to replace, so `update.install()` has nothing to swap and the relaunch is
  meaningless. Only `check()`/`download()` are exercisable in dev (and `check()`
  needs the insecure-transport flag or an https endpoint, since dev builds only
  warn). For the real install path use Tier 1 — the staged `.app`.
- The staged `.app` is unsigned/ad-hoc for local use; macOS Gatekeeper may prompt.
  Run it from the lab dir, never copy it to `/Applications`.
- The lab serves plain HTTP on loopback. It is a test harness: the
  `dangerousInsecureTransportProtocol` fragment must never be merged into a
  shipped build.
- `serve` traps INT/TERM/EXIT and kills its own `http.server` only. Nothing here
  deletes outside the lab dir, and the script refuses to run if the resolved lab
  path is not under `/tmp`.

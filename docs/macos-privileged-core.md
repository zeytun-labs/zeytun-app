# Running zeytun-core with root privileges on macOS

TUN mode needs root: creating a `utun` device and editing the routing table
are privileged on macOS. This document covers how the app gives zeytun-core
that privilege, what went wrong with the first approach, the stopgap that
ships now, and the design we should move to.

## The problem with the original design

Up to v0.1.0-beta, `grant_macos_core_privileges` ran

```
chown root <app-data>/core/bin/<platform>/zeytun-core-<ver> && chmod 4755 ...
```

once, behind an admin prompt. After that:

1. **Any process running as the user got root without a prompt.** The setuid
   binary sat in a user-owned directory and ran `zeytun-core run -c <any
   path>`. The config decides which files the core writes (`log.output`,
   `experimental.cache_file.path`) and reads (`rule_set[].path`), so a
   crafted config is a root file-write primitive. Other subcommands
   (`format -w`, `merge`, `rule-set compile -o`, `tools fetch`) write or
   fetch on the caller's behalf too, and `-D` makes root `mkdir` + `chdir`.
2. **No integrity check.** Whatever file sat at the install path when the
   user typed the password became setuid root. A process running as the user
   could plant its own binary there first.
3. **The env was trusted.** `SUDO_UID`/`SUDO_GID` decided who owned files
   the root core created.

## Option A: launchd privileged helper (recommended long-term)

A small root daemon registered with `SMAppService.daemon(plistName:)`
(macOS 13+), shipped inside the signed app bundle at
`Contents/Library/LaunchDaemons/`. The user approves it once in System
Settings > Login Items. The app talks to it over XPC (or a unix socket in a
root-owned directory).

How it works:

- The helper owns the core process. The app sends **intent**, not a config
  file path: "start with this compiled config", "reload", "stop".
- The helper checks who is calling. With XPC:
  `NSXPCConnection.setCodeSigningRequirement` (macOS 13+) pinned to our
  Team ID and bundle ID, so only our signed app can connect. With a unix
  socket: `getpeereid` + `LOCAL_PEERPID`, then `SecCodeCreateWithPID` and
  `SecCodeCheckValidity` against the same requirement.
- The helper writes the config into its own root-owned directory, after
  validating it: no `log.output` outside that directory, cache path forced,
  rule-set paths limited to the app's rule-set directory.
- The core binary runs from the signed bundle (or a root-owned copy the
  helper verifies with `SecStaticCodeCheckValidity`), so no setuid bit
  exists anywhere.

Pros:

- No setuid binary on disk at all.
- One approval, then no password prompts for start, stop or reload.
- The helper can enforce config policy, so any field of the core config can
  be audited in one place.
- Apple's supported mechanism (`SMJobBless` is deprecated).

Cons:

- New component: a Swift (or Rust + `objc2`) daemon, its own plist, signing
  and notarization of both targets, and an XPC protocol to version.
- macOS 13+ only for `SMAppService` (older needs `SMJobBless`).
- Upgrade and uninstall need care: `SMAppService.unregister()`, and a
  version handshake so an old helper never runs a new core or the reverse.
- Development builds need signing to test the code-signing requirement.

Rough cost: 1–2 weeks including CI signing work.

## Option B: hardened setuid core (shipping now)

Keep the setuid model, but make sure nothing the user can write reaches the
root process.

| Area | Change | Where |
|---|---|---|
| Install location | The setuid copy lives in `/Library/Application Support/Zeytun/bin/zeytun-core-<ver>`, `root:wheel` 4755, in `root:wheel` 755 directories. Older versions there are deleted on install. | `privileged_core.rs` |
| Integrity | `build.rs` bakes the bundled core's SHA-256 into the signed app binary (`ZEYTUN_CORE_SHA256`); release builds without it refuse TUN. The root script copies the bundle into a root-owned temp file (reading the source with `sudo -u <caller>`), hashes it, and only then runs `chown`/`chmod 4755`/`mv`. A mismatch deletes the temp file. The app re-checks owner, mode and hash before every privileged spawn. | `privileged_core.rs` |
| Old setuid copies | Any setuid `zeytun-core*` in the app-data bin dir is deleted at startup. The directory is the user's, so no privilege is needed. The unprivileged app-data copy is reinstalled whenever its hash differs from the bundle. | `manager.rs` |
| Config | TUN configs are staged to `/Library/Application Support/Zeytun/run/config.json`, `root:wheel` 0600, in the same admin prompt, plus a root-owned, world-readable `config.sha256`. When the core is trusted and that marker matches the config to run, no prompt is shown. After staging, the app re-checks the marker, so a config swapped mid-copy is rejected. | `privileged_core.rs`, `zeytun_core_process.rs` |
| Non-TUN | Runs the app-data copy as the user. `check` always runs unprivileged on the user's file. | `zeytun_core_process.rs` |
| Core: config trust | With euid 0, zeytun-core opens each config, `fstat`s that same descriptor and refuses unless the file is a regular file owned by root and not group- or world-writable. The directory that holds it must pass the same rule. Config from `stdin` is refused. | `cmd/sing-box/privilege*.go` |
| Core: setuid mode | When real uid ≠ effective uid, only `run`, `check`, `version` and `help` are allowed. This is checked in `main()` before cobra runs, because `geoip`/`geosite` override `PersistentPreRun`. `-D` is refused and `SUDO_*` is ignored. `run`/`check` need exactly one `-c` file; the core `chdir`s into its directory and validates it by inode, so relative paths (`cache.db`) can't follow a symlink planted in the caller's cwd. Created files stay root-owned (no chown to the caller) and the caller's environment is dropped. | `cmd/sing-box/privilege*.go`, `cmd.go` |

What this does not fix, and the UX cost:

- **One admin prompt whenever the TUN config changes.** Starting TUN again
  on an unchanged config (restart, login autostart) skips the prompt via the
  root-written `config.sha256`. Rules and overlays pushed through the Clash
  API don't reload, so they don't prompt. Changes that rebuild the config
  (proxy edits, mode switches) do.
- `core_start` stages before taking the core lock, so its prompt doesn't
  block other commands. Profile switches, inbound-mode changes and rule-set
  retries that restart into TUN still prompt under the lock.
- The root core still **reads** user-owned rule-set files referenced by
  absolute path. That is parser attack surface, but not a write primitive.
- Anyone can still run the setuid core with a root-owned config they cannot
  edit, such as `/etc/hosts`. The core rejects it as invalid JSON. A valid
  root-owned config would have to come from an admin.
- The last staged config stays in `run/` and anyone can start the setuid
  core on it. It is a config the admin approved, but a process running as
  the user can swap the app-data config just before staging, and the user
  approves whatever the prompt copies. The prompt cannot show what it copies.
- The expected core hash is pinned at build time, so replacing the core
  inside the app bundle is caught. Replacing the whole signed app binary is
  out of scope (code signing / Gatekeeper).

## Recommendation

Ship **Option B now**. It closes the passwordless root escalation and the
planted-binary path with small changes we can test. Then build **Option A**
before leaving beta: the per-start password prompt is a real UX regression
for TUN users, and a helper that receives intents instead of files is the
only design where the root side never parses a user-written file.

## Follow-ups

1. Option A: SMAppService daemon + XPC with a code-signing requirement.
2. In the helper design, write the config from validated intent instead of
   copying a file. Until then, consider rejecting `log.output` and absolute
   `cache_file.path` in the core whenever euid is 0.
3. Uninstall: remove `/Library/Application Support/Zeytun` (admin prompt or
   helper).

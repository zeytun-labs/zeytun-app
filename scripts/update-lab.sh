#!/usr/bin/env bash
# Isolated self-update lab: real versioned bundles, no GitHub Releases.
# See update-lab.README.md
set -euo pipefail

SERVE_DIR="${ZEYTUN_UPDATE_LAB_DIR:-/tmp/zeytun-update-lab}"
PORT="${ZEYTUN_UPDATE_LAB_PORT:-8787}"
BASE_URL="http://127.0.0.1:$PORT"
KEY_PATH="${ZEYTUN_UPDATE_LAB_KEY_PATH:-$HOME/.tauri/zeytun-lab.key}"
KEY_PW_FILE="${ZEYTUN_UPDATE_LAB_PASSWORD_FILE:-$HOME/.tauri/zeytun-lab.key.password}"
APP_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DRY_RUN=0

say() { printf '%s\n' "$*"; }
die() { printf 'lab: error: %s\n' "$*" >&2; exit 1; }

# Resolve symlinks before fencing any writes/removals to the lab workspace.
resolve_dir() {
  SERVE_DIR="$(python3 - "$SERVE_DIR" "$APP_ROOT" <<'PYCODE'
from pathlib import Path
import sys
path=Path(sys.argv[1]).resolve()
roots=[Path('/tmp').resolve(), Path(sys.argv[2]).resolve()/'.lab-update']
if not any(path != root and path.is_relative_to(root) for root in roots):
    raise SystemExit('Lab directory must resolve under /tmp or this checkout/.lab-update')
path.mkdir(parents=True, exist_ok=True)
print(path)
PYCODE
)" || die "unsafe lab directory"
}

app_version() {
  python3 - "$1/Contents/Info.plist" <<'PYCODE'
import plistlib,sys
print(plistlib.load(open(sys.argv[1], 'rb'))['CFBundleShortVersionString'])
PYCODE
}

lab_target() {
  case "$(uname -m)" in
    arm64|aarch64) echo darwin-aarch64 ;;
    x86_64) echo darwin-x86_64 ;;
    *) die "unsupported lab architecture" ;;
  esac
}

run_verifier() {
  (cd "$APP_ROOT" && cargo run --locked --manifest-path src-tauri/Cargo.toml \
    --features updater-tools --bin verify-update -- --mode lab \
    --config "$SERVE_DIR/lab-config.json" --manifest "$1" \
    --artifacts "$SERVE_DIR" --target "$(lab_target)")
}

do_build() {
  local version="${1:-}"
  local config_args=(--port "$PORT" --output "$SERVE_DIR/lab-config.json")
  [ -z "$version" ] || config_args+=(--version "$version")
  [ -z "${ZEYTUN_UPDATE_LAB_PUBLIC_KEY_PATH:-}" ] || config_args+=(--public-key "$ZEYTUN_UPDATE_LAB_PUBLIC_KEY_PATH")
  python3 "$APP_ROOT/scripts/update_lab.py" config "${config_args[@]}"
  local pnpm_cmd; read -r -a pnpm_cmd <<< "${PNPM:-pnpm}"
  local cmd=("${pnpm_cmd[@]}" exec tauri build --bundles app --features updater-lab -c "$SERVE_DIR/lab-config.json")
  say "==> isolated lab build (endpoint $BASE_URL/latest.json)"
  if [ "$DRY_RUN" = 1 ]; then
    printf '[dry-run] %s\n' "${cmd[*]}"
    cat "$SERVE_DIR/lab-config.json"
    return 0
  fi
  [ -f "$KEY_PATH" ] || die "missing lab signing key: $KEY_PATH"
  [ -f "$KEY_PW_FILE" ] || die "missing lab key password file: $KEY_PW_FILE"
  (cd "$APP_ROOT" && \
    TAURI_SIGNING_PRIVATE_KEY="$KEY_PATH" \
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat "$KEY_PW_FILE")" "${cmd[@]}")
  local bundle="$APP_ROOT/src-tauri/target/release/bundle/macos"
  local name="Zeytun Lab.app"
  [ -f "$bundle/$name.tar.gz.sig" ] || die "missing signed lab archive"
  mkdir -p "$SERVE_DIR/candidate"
  rm -rf "$SERVE_DIR/candidate/$name"
  cp -R "$bundle/$name" "$SERVE_DIR/candidate/"
  local version_built; version_built="$(app_version "$SERVE_DIR/candidate/$name")"
  local asset="Zeytun Lab_${version_built}.app.tar.gz"
  cp "$bundle/$name.tar.gz" "$SERVE_DIR/$asset"
  cp "$bundle/$name.tar.gz.sig" "$SERVE_DIR/$asset.sig"
  # Never overwrite the running bundle when building the next candidate.
  if [ ! -d "$SERVE_DIR/installed/$name" ]; then
    mkdir -p "$SERVE_DIR/installed"
    cp -R "$bundle/$name" "$SERVE_DIR/installed/"
  fi
  say "==> staged candidate $(app_version "$SERVE_DIR/candidate/$name")"
}

do_manifest() {
  local version="${1:-}"
  [ -n "$version" ] || die "usage: $0 manifest VERSION (must match the candidate bundle)"
  local pending="$SERVE_DIR/latest.pending.json"
  python3 "$APP_ROOT/scripts/update_lab.py" manifest --directory "$SERVE_DIR" \
    --version "$version" --port "$PORT" --target "$(lab_target)" --output "$pending"
  run_verifier "$pending"
  mv "$pending" "$SERVE_DIR/latest.json"
  say "==> published verified manifest $version"
}

do_serve() {
  resolve_dir
  say "==> serving $SERVE_DIR on $BASE_URL (ctrl-c to stop)"
  say "    (throttled: 2s check delay, ~4 MB/s download speed)"
  python3 - "$SERVE_DIR" "$PORT" <<'PY' &
import functools, http.server, os, sys, time

serve_dir = sys.argv[1]
port = int(sys.argv[2])
check_delay = float(os.environ.get("LAB_CHECK_DELAY", "2.0"))
rate_kb = float(os.environ.get("LAB_RATE_KB_S", "4000"))

class SlowHandler(http.server.SimpleHTTPRequestHandler):
    def do_GET(self):
        if self.path.endswith("latest.json") and check_delay > 0:
            time.sleep(check_delay)
        return super().do_GET()

    def copyfile(self, source, outputfile):
        chunk_size = 64 * 1024
        sleep_per_chunk = (chunk_size / 1024) / rate_kb if rate_kb > 0 else 0
        try:
            while True:
                buf = source.read(chunk_size)
                if not buf:
                    break
                outputfile.write(buf)
                if sleep_per_chunk > 0:
                    time.sleep(sleep_per_chunk)
        except (BrokenPipeError, ConnectionResetError):
            pass

server = http.server.ThreadingHTTPServer(("127.0.0.1", port), functools.partial(SlowHandler, directory=serve_dir))
server.serve_forever()
PY
  local pid=$!
  trap 'say "==> stopping server"; kill "$pid" 2>/dev/null || true; wait "$pid" 2>/dev/null || true' INT TERM EXIT
  wait "$pid"
}

do_run() {
  resolve_dir
  local app="$SERVE_DIR/installed/Zeytun Lab.app"
  [ -d "$app" ] || die "no installed lab bundle; run build first"
  say "==> launching isolated lab $(app_version "$app") ($BASE_URL/latest.json)"
  open "$app"
}

do_status() {
  resolve_dir
  say "lab dir: $SERVE_DIR"
  for kind in installed candidate; do
    local app="$SERVE_DIR/$kind/Zeytun Lab.app"
    [ ! -d "$app" ] || say "$kind: $(app_version "$app")"
  done
  [ ! -f "$SERVE_DIR/latest.json" ] || cat "$SERVE_DIR/latest.json"
  if curl -fsS -m 2 "$BASE_URL/latest.json" >/dev/null 2>&1; then
    say "server: UP on $PORT"
  else
    say "server: DOWN (run: $0 serve)"
  fi
}

usage() {
  cat <<EOF
zeytun update lab

  $0 build [VERSION] [--dry-run]  build a versioned isolated lab bundle
  $0 manifest VERSION      verify and publish the matching candidate
  $0 serve                 serve lab dir on $BASE_URL (foreground)
  $0 run                   launch the staged .app
  $0 status                show lab state

env: ZEYTUN_UPDATE_LAB_DIR (default /tmp/zeytun-update-lab), ZEYTUN_UPDATE_LAB_PORT (default 8787)
EOF
}

main() {
  local cmd="${1:-}"
  [ $# -gt 0 ] && shift || true
  case "$cmd" in
    build)
      local version=""
      for a in "$@"; do
        if [ "$a" = "--dry-run" ]; then DRY_RUN=1
        elif [ -z "$version" ]; then version="$a"
        else die "unexpected build argument: $a"; fi
      done
      resolve_dir; do_build "$version" ;;
    manifest) resolve_dir; do_manifest "${1:-}" ;;
    serve) do_serve ;;
    run) do_run ;;
    status) do_status ;;
    ""|-h|--help|help) usage ;;
    *) usage >&2; die "unknown subcommand '$cmd'" ;;
  esac
}

main "$@"

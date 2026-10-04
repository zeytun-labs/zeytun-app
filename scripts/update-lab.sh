#!/usr/bin/env bash
# Local self-update lab: unlimited update cycles, no GitHub Releases.
# See update-lab.README.md
set -euo pipefail

SERVE_DIR="${ZEYTUN_UPDATE_LAB_DIR:-/tmp/zeytun-update-lab}"
PORT="${ZEYTUN_UPDATE_LAB_PORT:-8787}"
BASE_URL="http://127.0.0.1:$PORT"
KEY_PATH="$HOME/.tauri/zeytun.key"
KEY_PW_FILE="$HOME/.tauri/zeytun.key.password"
APP_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DRY_RUN=0

say() { printf '%s\n' "$*"; }
die() { printf 'lab: error: %s\n' "$*" >&2; exit 1; }

# Resolve + fence the lab dir. Refuse anything not under /tmp or the repo's expected lab root.
resolve_dir() {
  case "$SERVE_DIR" in
    /tmp/*|*/.lab-update/*) ;;
    *) die "serve dir '$SERVE_DIR' not under /tmp — refusing (override with ZEYTUN_UPDATE_LAB_DIR=/tmp/...)" ;;
  esac
  mkdir -p "$SERVE_DIR"
  SERVE_DIR="$(cd "$SERVE_DIR" && pwd)"
}

tarball() {
  local t
  t="$(ls -1 "$SERVE_DIR"/*.app.tar.gz 2>/dev/null | head -1 || true)"
  [ -n "$t" ] || die "no staged .app.tar.gz in $SERVE_DIR — run: $0 build"
  printf '%s' "$t"
}

staged_app() {
  local a
  a="$(ls -1d "$SERVE_DIR"/*.app 2>/dev/null | head -1 || true)"
  [ -n "$a" ] || die "no staged .app in $SERVE_DIR — run: $0 build"
  printf '%s' "$a"
}

app_version() {
  /usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' \
    "$(staged_app)/Contents/Info.plist" 2>/dev/null || echo "?"
}

do_build() {
  say "==> building (dev-updater config, endpoint $BASE_URL/latest.json)"
  [ -f "$KEY_PATH" ] || die "missing $KEY_PATH"
  [ -f "$KEY_PW_FILE" ] || die "missing $KEY_PW_FILE"
  local cmd=(pnpm tauri build --bundles app -c src-tauri/tauri.dev-updater.json)
  if [ "$DRY_RUN" = 1 ]; then
    say "[dry-run] would run (in $APP_ROOT):"
    printf '  %s\n' "${cmd[*]}"
    say "[dry-run] would then stage into $SERVE_DIR:"
    printf '  src-tauri/target/release/bundle/macos/zeytun.app\n'
    printf '  src-tauri/target/release/bundle/macos/zeytun.app.tar.gz\n'
    printf '  src-tauri/target/release/bundle/macos/zeytun.app.tar.gz.sig\n'
    return 0
  fi
  ( cd "$APP_ROOT" && \
    TAURI_SIGNING_PRIVATE_KEY="$(cat "$KEY_PATH")" \
    TAURI_SIGNING_PRIVATE_KEY_PATH="$KEY_PATH" \
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat "$KEY_PW_FILE")" \
    "${cmd[@]}" )

  local bundle="$APP_ROOT/src-tauri/target/release/bundle/macos"
  [ -d "$bundle" ] || die "no bundle dir: $bundle"
  local app src
  app="$(ls -1d "$bundle"/*.app | head -1)"
  src="$(ls -1 "$bundle"/*.app.tar.gz | head -1)"
  [ -f "$src.sig" ] || die "missing $src.sig (createUpdaterArtifacts off?)"

  say "==> staging into $SERVE_DIR"
  rm -rf "$SERVE_DIR"/*.app "$SERVE_DIR"/*.app.tar.gz "$SERVE_DIR"/*.app.tar.gz.sig
  cp -R "$app" "$SERVE_DIR/"
  cp "$src" "$src.sig" "$SERVE_DIR/"
  say "produced:"
  printf '  %s\n  %s\n  %s\n' "$SERVE_DIR/$(basename "$app")" "$SERVE_DIR/$(basename "$src")" "$SERVE_DIR/$(basename "$src").sig"
}

do_manifest() {
  local version="${1:-}"
  [ -n "$version" ] || die "usage: $0 manifest VERSION"
  case "$version" in
    *[!0-9.]*) die "version must be dotted numeric, got '$version'" ;;
  esac
  [ -f "$KEY_PATH" ] || die "missing $KEY_PATH"
  [ -f "$KEY_PW_FILE" ] || die "missing $KEY_PW_FILE"

  local tb sig name pubdate
  tb="$(tarball)"
  sig="$tb.sig"
  name="$(basename "$tb")"
  say "==> re-signing $name for version $version"
  ( cd "$APP_ROOT" && \
    TAURI_SIGNING_PRIVATE_KEY_PATH="$KEY_PATH" \
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat "$KEY_PW_FILE")" \
    npx tauri signer sign -f "$KEY_PATH" "$tb" ) >"$sig.new" 2>"$sig.err" \
    || { cat "$sig.err" >&2; rm -f "$sig.new" "$sig.err"; die "signing failed"; }
  rm -f "$sig.err"
  # signer prints the base64 signature on stdout; minisign sigs are base64 of
  # "untrusted comment:..." i.e. start with dW50cnVzdGVk. Grab that line.
  local signature
  signature="$(grep -E '^dW50cnVzdGVk[A-Za-z0-9+/=]+$' "$sig.new" | tail -1 || true)"
  rm -f "$sig.new"
  [ -n "$signature" ] || die "empty signature output"
  [ -f "$sig" ] || die "signer did not write $sig"
  say "    sig: ${signature:0:32}...  ($(wc -c <"$sig" | tr -d ' ') bytes at $(basename "$sig"))"

  pubdate="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  python3 - "$SERVE_DIR/latest.json" "$version" "$pubdate" "$signature" "$BASE_URL/$name" <<'PY'
import json, sys
path, version, pub_date, signature, url = sys.argv[1:6]
json.dump({
    "version": version,
    "notes": "Local update-lab build %s. Not stable." % version,
    "pub_date": pub_date,
    "platforms": {"darwin-aarch64": {"signature": signature, "url": url}},
}, open(path, "w"), indent=2)
PY
  say "==> wrote $SERVE_DIR/latest.json"
  cat "$SERVE_DIR/latest.json"
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
  local app; app="$(staged_app)"
  say "==> launching $app"
  say "    (endpoint $BASE_URL/latest.json; keep '$0 serve' running)"
  open "$app"
}

do_status() {
  resolve_dir
  say "lab dir     : $SERVE_DIR"
  if [ -d "$SERVE_DIR" ]; then
    local app tb
    app="$(ls -1d "$SERVE_DIR"/*.app 2>/dev/null | head -1 || true)"
    if [ -n "$app" ]; then
      say "staged app  : $(basename "$app")  version $(app_version)"
    else
      say "staged app  : none (run: $0 build)"
    fi
    tb="$(ls -1 "$SERVE_DIR"/*.app.tar.gz 2>/dev/null | head -1 || true)"
    if [ -n "$tb" ]; then
      say "tarball     : $(basename "$tb")"
      say "  sha256    : $(shasum -a 256 "$tb" | cut -d' ' -f1)"
      say "  sig file  : $([ -f "$tb.sig" ] && echo present || echo MISSING)"
    else
      say "tarball     : none"
    fi
  fi
  if [ -f "$SERVE_DIR/latest.json" ]; then
    say "manifest    : version $(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["version"])' "$SERVE_DIR/latest.json")"
    say "  url       : $(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["platforms"]["darwin-aarch64"]["url"])' "$SERVE_DIR/latest.json")"
  else
    say "manifest    : none (run: $0 manifest VERSION)"
  fi
  if curl -fsS -m 2 "$BASE_URL/latest.json" >/dev/null 2>&1; then
    say "server      : UP on $PORT"
  else
    say "server      : DOWN (run: $0 serve)"
  fi
}

usage() {
  cat <<EOF
zeytun update lab

  $0 build [--dry-run]     build .app with local endpoint + stage artifacts
  $0 manifest VERSION      re-sign tarball + rewrite latest.json (no rebuild)
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
      for a in "$@"; do [ "$a" = "--dry-run" ] && DRY_RUN=1; done
      resolve_dir; do_build ;;
    manifest) resolve_dir; do_manifest "${1:-}" ;;
    serve) do_serve ;;
    run) do_run ;;
    status) do_status ;;
    ""|-h|--help|help) usage ;;
    *) usage >&2; die "unknown subcommand '$cmd'" ;;
  esac
}

main "$@"

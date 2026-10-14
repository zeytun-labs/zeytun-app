#!/bin/bash
# Public native resources live inside this clone, never in a parent repository.
set -euo pipefail
cd "$(dirname "$0")/.."
core_src="${CORE_SRC:-.cache/zeytun-core}"
core_repo="${CORE_REPO:-https://github.com/zeytun-labs/zeytun-core.git}"
core_ref="${CORE_REF:-zeytun-port}"
core_version="${CORE_VERSION:-zeytun-custom}"
core_tags="${CORE_TAGS:?Run this script through the repository Makefile.}"
target="${TARGET:?Run this script through the repository Makefile.}"
core_dest="src-tauri/resources/bin/$target/zeytun-core"
geoip_dest="src-tauri/resources/geoip.mmdb"
python="${PYTHON:-python3}"
temporary=""
trap 'if [ -n "$temporary" ]; then rm -rf -- "$temporary"; fi' EXIT

core_source() {
  if [ ! -d "$core_src" ]; then
    mkdir -p "$(dirname "$core_src")"
    temporary="$(mktemp -d "${core_src}.clone.XXXXXX")"
    GIT_TERMINAL_PROMPT=0 git clone --depth 1 --branch "$core_ref" "$core_repo" "$temporary"
    mv "$temporary" "$core_src"
    temporary=""
  fi
  if [ ! -f "$core_src/go.mod" ]; then
    echo "Not a zeytun-core checkout: $core_src (set CORE_SRC to a public core checkout)." >&2
    exit 1
  fi
  echo "Core source: $core_src ($(git -C "$core_src" rev-parse --short HEAD))"
}

build_core() {
  core_source
  if [ "$(uname -s)" != Darwin ]; then
    echo "The desktop core bundle currently targets macOS. Use make setup-web on Linux." >&2
    exit 1
  fi
  if [ "$1" = ci ]; then
    (
      cd "$core_src"
      CGO_ENABLED=0 go build -trimpath -tags "$core_tags" \
        -ldflags "-X github.com/sagernet/sing-box/constant.Version=$core_version -s -w -buildid=" \
        -o sing-box ./cmd/sing-box
    )
  else
    # The upstream Makefile handles the production build, including Naive/CGO.
    (cd "$core_src" && CGO_ENABLED="${CGO_ENABLED:-1}" "${MAKE:-make}" build "VERSION=$core_version" "TAGS=$core_tags")
  fi
  mkdir -p "$(dirname "$core_dest")"
  temporary="$(mktemp "${core_dest}.XXXXXX")"
  cp "$core_src/sing-box" "$temporary"
  chmod 755 "$temporary"
  mv -f "$temporary" "$core_dest"
  temporary=""
  echo "Core installed: $core_dest"
}

validate_geoip() {
  "$python" - "$1" <<'PY'
import sys
from pathlib import Path
path = Path(sys.argv[1])
with path.open('rb') as file:
    file.seek(max(0, path.stat().st_size - 131072))
    if b'\xab\xcd\xefMaxMind.com' not in file.read():
        raise SystemExit(f'Invalid or incomplete GeoIP database: {path}; run make geoip.')
PY
}

download_geoip() {
  mkdir -p "$(dirname "$geoip_dest")"
  temporary="$(mktemp -d "${geoip_dest}.download.XXXXXX")"
  url="https://download.db-ip.com/free/dbip-country-lite-$(date -u +%Y-%m).mmdb.gz"
  curl --fail --location --silent --show-error --retry 3 "$url" -o "$temporary/country.mmdb.gz"
  gzip -dc "$temporary/country.mmdb.gz" > "$temporary/country.mmdb"
  validate_geoip "$temporary/country.mmdb"
  mv -f "$temporary/country.mmdb" "$geoip_dest"
  rm -rf "$temporary"
  temporary=""
  echo "GeoIP installed: $geoip_dest"
}

check_resources() {
  if [ ! -x "$core_dest" ]; then
    echo "Missing executable: $core_dest. Run make setup or make build-core." >&2
    exit 1
  fi
  if [ ! -s "$geoip_dest" ]; then
    echo "Missing GeoIP database. Run make geoip." >&2
    exit 1
  fi
  validate_geoip "$geoip_dest"
  echo "Native resources OK ($target)"
}

case "${1:-}" in
  core-source) core_source ;;
  build-core) build_core production ;;
  build-core-ci) build_core ci ;;
  geoip) download_geoip ;;
  resources)
    if [ ! -x "$core_dest" ]; then build_core production; fi
    if [ ! -s "$geoip_dest" ]; then download_geoip; fi
    check_resources
    ;;
  check) check_resources ;;
  *) echo "Usage: make setup | build-core | geoip | resources-check" >&2; exit 2 ;;
esac

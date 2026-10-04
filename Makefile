.PHONY: dev build-core build-ui check check-rs version disk install-deps clean

# Architecture settings
UNAME_M := $(shell uname -m)
ifeq ($(UNAME_M),arm64)
	TARGET := macos-aarch64
else
	TARGET := macos-x86_64
endif

CORE_TAGS := with_quic with_grpc with_dhcp with_wireguard with_utls with_acme with_clash_api with_v2ray_api with_gvisor with_embedded_tor with_tailscale with_ccm with_ocm with_naive_outbound with_cloudflared badlinkname
CORE_DEST := src-tauri/resources/bin/$(TARGET)/zeytun-core

# Build the required network daemon (requires zeytun-core cloned alongside this repo)
build-core:
	@echo "=> Building zeytun-core daemon for $(TARGET)..."
	@if [ ! -d "../zeytun-core" ]; then \
	  echo "Error: ../zeytun-core directory missing"; \
	  echo "Clone it: git clone https://github.com/zeytun-labs/zeytun-core.git ../zeytun-core"; \
	  exit 1; \
	fi
	@cd ../zeytun-core && VERSION="zeytun-custom" TAGS="$(CORE_TAGS)" $(MAKE)
	@mkdir -p $(dir $(CORE_DEST))
	@cp ../zeytun-core/sing-box $(CORE_DEST)
	@chmod +x $(CORE_DEST)
	@echo "=> Build complete. Binary placed in $(CORE_DEST)"
	@echo "=> NOTE: For local proxy connections, apply setuid root:"
	@echo "   sudo chown root $(CORE_DEST) && sudo chmod 4755 $(CORE_DEST)"

# Run the UI locally
dev:
	@echo "=> Starting Tauri + Svelte dev server..."
	@pnpm run tauri dev

# Run Tauri release build
build-ui:
	@echo "=> Building Tauri App Bundle..."
	@pnpm run tauri build

# Type check frontend
check:
	@echo "=> Checking Svelte typings..."
	@pnpm run check

# Fail loudly before cargo dies with ENOSPC (os error 28).
disk:
	@avail=$$(df -g . | awk 'NR==2 {print $$4}'); \
	echo "=> $$avail GB free"; \
	if [ "$$avail" -lt 6 ]; then \
	  echo "ERROR: <6GB free. Run: cd src-tauri && cargo clean --release"; exit 1; \
	fi

# Rust parity with CI: fmt + clippy -D warnings + hermetic tests.
check-rs: disk
	@echo "=> Rust fmt + clippy + tests..."
	@cd src-tauri && cargo fmt --all --check
	@cd src-tauri && cargo clippy --all-targets -- -D warnings
	@cd src-tauri && cargo test --all-targets

# Fail when the three manifests drift apart.
version:
	@t=$$(node -p "require('./src-tauri/tauri.conf.json').version"); \
	p=$$(node -p "require('./package.json').version"); \
	c=$$(sed -n 's/^version = "\(.*\)"/\1/p' src-tauri/Cargo.toml | head -1); \
	echo "=> tauri=$$t package=$$p cargo=$$c"; \
	[ "$$t" = "$$p" ] && [ "$$t" = "$$c" ] || { echo "ERROR: version drift"; exit 1; }

# Install Node dependencies
install-deps:
	@pnpm install

# Clean up binaries
clean:
	@echo "=> Cleaning up core binary..."
	@rm -f $(CORE_DEST)

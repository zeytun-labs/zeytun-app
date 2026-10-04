# Zeytun

[![Download Latest Release](https://img.shields.io/badge/Download-Latest%20Release-blue?style=for-the-badge&logo=apple)](https://github.com/zeytun-labs/zeytun-release/releases)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-green.svg?style=for-the-badge)](LICENSE)

Zeytun is a free and open-source macOS desktop network manager. It shows
every connection your machine makes, attributes it to the app that opened
it, and lets you route, hold, or refuse it — with live rules, policies,
DNS control, and a checkpoint that asks before anything new connects.

## Architecture

Zeytun is composed of three independent repositories:

| Repository | Language | Description |
|---|---|---|
| **zeytun-app** (this repo) | TypeScript + Rust | Tauri v2 + Svelte 5 desktop client |
| [zeytun-core](https://github.com/zeytun-labs/zeytun-core) | Go | Network engine — a sing-box derivative (GPLv3) |
| [zeytun-config](https://github.com/zeytun-labs/zeytun-config) | Rust | Proxy link parser & config generator |

The app fetches a pre-built `zeytun-core` binary from
[zeytun-core releases](https://github.com/zeytun-labs/zeytun-core/releases)
in CI. For local development, build it from source (below).

## Prerequisites

- **macOS** (Apple Silicon)
- **Node.js 22** + pnpm 10
- **Rust** + Cargo
- **Go** 1.25+ (only for building zeytun-core locally)

## Quick start

```bash
git clone https://github.com/zeytun-labs/zeytun-app.git
cd zeytun-app
pnpm install

# Build the core daemon (optional — skip if you only need the UI):
git clone https://github.com/zeytun-labs/zeytun-core.git ../zeytun-core
make build-core
sudo chown root src-tauri/resources/bin/macos-aarch64/zeytun-core
sudo chmod 4755 src-tauri/resources/bin/macos-aarch64/zeytun-core

# Start dev:
pnpm run tauri dev
```

Without the setuid step, the app starts but every proxy fails to establish.

## Makefile targets

| Target | Description |
|---|---|
| `make build-core` | Compile zeytun-core (from `../zeytun-core`) and stage the binary |
| `make dev` | Start the Tauri + Svelte dev server |
| `make build-ui` | Build the Tauri release bundle |
| `make check` | Run svelte-check (frontend type checking) |
| `make check-rs` | Run Rust fmt + clippy + tests |
| `make version` | Verify version parity across manifests |
| `make install-deps` | Install Node dependencies |
| `make clean` | Remove the staged core binary |

## Downloads

Pre-built binaries are available on the [releases page](https://github.com/zeytun-labs/zeytun-release/releases).

## Donations & Support

If you find Zeytun useful, donations are welcome:

| Network / Asset | Address |
|---|---|
| **Bitcoin (BTC)** | `bc1q2a7aywmekl3kmqzw9t5rg5ds643avetnymg5n6` |
| **TRON (TRX / USDT-TRC20)** | `TPEtVbd7rrguo9KjfDvBfPZnqsiURY1fp7` |
| **TON / Gram** | `UQADcpf4-JT_4vrR17DntsHH0Ryj0o2LBycr3iEyGu7L8WVK` |
| **Solana (SOL / USDT-SPL)** | `5wHu7Zra6SbeEmbJbx79ixdnhjoWhfqwQCFjeBcSKZcz` |
| **EVM (ETH, BSC, Arbitrum, Base, Polygon)** | `0x1308E77d3C332F862A3cE87eeA8CfAFF019e90A3` |

## License

Zeytun is free and open-source software licensed under the GNU General
Public License v3.0 (GPL-3.0). See [LICENSE](LICENSE) for details.

`zeytun-core` is a derivative work of [sing-box](https://github.com/SagerNet/sing-box),
also distributed under GPLv3.

<div align="center">

# Zeytun

**A macOS app that shows every connection your Mac makes, names the app that opened it, and lets you allow, route, or refuse it.**

Free and open source. No account. No telemetry.

[![Latest release](https://img.shields.io/github/v/release/zeytun-labs/zeytun-release?label=release&color=6B8F71)](https://github.com/zeytun-labs/zeytun-release/releases)
![macOS](https://img.shields.io/badge/macOS-Apple%20Silicon-black?logo=apple)
[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-green)](LICENSE)

[Download](https://github.com/zeytun-labs/zeytun-release/releases) · [Features](#features) · [Support](#support)

</div>

---

## What it does

Most proxy apps hide the network behind a single on/off switch. Zeytun puts the connections on screen.

You see which app is talking, where it is going, and what happened to it. A new connection can wait for you before it proceeds. Rules and policies decide the rest, and you can change them without restarting anything.

## Features

- **See every connection** — live traffic, attributed to the app that opened it
- **Ask before connecting** — hold a new connection until you allow or refuse it
- **Route per app, domain, or process** — rules you can edit while the tunnel is up
- **Policies** — group proxies and pick how traffic is chosen between them
- **DNS** — your own servers, rules, and local hosts
- **Subscriptions** — import a link, update it automatically, keep several profiles
- **Dark and light** — follows the system, or stays where you set it

## Download

macOS, Apple Silicon.

Grab the latest build from [Releases](https://github.com/zeytun-labs/zeytun-release/releases).

## Support

Zeytun is free. If it is useful to you, a donation helps keep it that way.

| Network | Address |
|---|---|
| Bitcoin | `bc1q2a7aywmekl3kmqzw9t5rg5ds643avetnymg5n6` |
| TRON (TRX, USDT) | `TPEtVbd7rrguo9KjfDvBfPZnqsiURY1fp7` |
| TON | `UQADcpf4-JT_4vrR17DntsHH0Ryj0o2LBycr3iEyGu7L8WVK` |
| Solana (SOL, USDT) | `5wHu7Zra6SbeEmbJbx79ixdnhjoWhfqwQCFjeBcSKZcz` |
| EVM (ETH, BSC, Arbitrum, Base, Polygon) | `0x1308E77d3C332F862A3cE87eeA8CfAFF019e90A3` |

A star on this repo helps too.

## Build from source

macOS, Apple Silicon. Node.js 22, pnpm 10, Rust. Go 1.25+ only if you also build the network engine.

```bash
git clone https://github.com/zeytun-labs/zeytun-app.git
cd zeytun-app
pnpm install

git clone --branch zeytun-port https://github.com/zeytun-labs/zeytun-core.git ../zeytun-core
make build-core
sudo chown root src-tauri/resources/bin/macos-aarch64/zeytun-core
sudo chmod 4755 src-tauri/resources/bin/macos-aarch64/zeytun-core

pnpm run tauri dev
```

The setuid step is required. Without it the app opens, but no proxy connects.

`make help` lists the other targets. The engine is [zeytun-core](https://github.com/zeytun-labs/zeytun-core), a sing-box derivative. Proxy link parsing lives in [zeytun-config](https://github.com/zeytun-labs/zeytun-config).

## Contributing

Issues and pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

[GPL-3.0](LICENSE).

The network engine is a derivative of [sing-box](https://github.com/SagerNet/sing-box), also under GPLv3. It does not use the sing-box name and is not affiliated with it.

<div align="center">

Built by [AmirHossein Sadeghi](https://github.com/sadeqi-ah).

</div>

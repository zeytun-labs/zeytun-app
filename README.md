<div align="center">

<img src="src-tauri/icons/128x128@2x.png" alt="Zeytun app icon" width="96" height="96">

# Zeytun

**App-aware network control for macOS.**

Understand your connections. Decide how they're routed.

Free and open source.

[![Status: Alpha](https://img.shields.io/badge/status-alpha-6B8F71)](https://github.com/zeytun-labs/zeytun-app/releases) ![macOS](https://img.shields.io/badge/macOS-Apple%20Silicon-black?logo=apple) [![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-green)](LICENSE)

[Features](#features) · [Get started](#get-started) · [Support development](#support)

</div>

---

## Your traffic, your rules

Zeytun brings connection visibility and routing decisions into one place. Inspect the traffic it handles, review the available app and destination details, and choose how connections are routed.

Build rules around the apps and destinations that matter to you, use temporary rules for short-lived changes, and decide what happens when a connection matches no rule.

## Features

### 🔎 Understand what is connecting

Inspect active and recent connections, filter by app or destination, and review the available process, route, and traffic details. Turn a connection into a destination or process rule without starting from an empty form.

The inspector shows traffic handled by Zeytun. Capture depends on your network mode, and app/process information is not available for every connection.

### 🎯 Give each app the right route

Create rules for application bundles, executable paths, domains, and IP ranges. Send matching traffic directly, through a proxy route, or to rejection. Use temporary rules when a change should expire instead of becoming permanent.

Routing-rule edits can be applied while the core is running. Existing connections are closed so subsequent connections use the new rules; this is not an interruption-free switch.

### ✋ Decide when no rule matches

Enable **Connection Ask** in Rule-based Proxy mode to pause unmatched TCP connections while you choose direct access, a proxy route, or rejection. You can remember a routing choice as a process rule.

Prompts have a timeout. If you do not respond, the configured final route applies; Connection Ask is not a default-deny firewall.

### Keep the rest of your setup together

- **Proxy selection and groups** — choose a route manually, select by latency, or distribute traffic with balancing strategies.
- **DNS controls** — configure DNS servers, destination rules, and local host mappings alongside your routing rules.
- **Network modes** — use the local proxy, set it as the system proxy, or enable TUN. What Zeytun sees depends on the mode and on how applications send traffic.

## Get started

**Release target:** macOS on Apple Silicon (`arm64`). Intel, Windows, and Linux builds are not currently offered.

> [!NOTE]
> Zeytun is in alpha. The first macOS release is being prepared, and there is no public download yet. Follow [Releases](https://github.com/zeytun-labs/zeytun-app/releases) for published builds, or [build from source](CONTRIBUTING.md#development-setup).

### From installation to your first route

Once a public build is available:

1. Download the Apple Silicon `.dmg` from Releases and move **Zeytun.app** to **Applications**.
2. Open Zeytun. These alpha builds are not Apple-notarized, so macOS may block the first launch. If you trust the source, follow [Apple's instructions](https://support.apple.com/en-us/102445#openanyway) to approve this app in **System Settings → Privacy & Security → Open Anyway**. Do not disable Gatekeeper globally.
3. Add a subscription from the profile menu, or import a proxy link in **Policy**. Select a proxy in the group you want to use.
4. Choose **Global Proxy** to use your selected proxy, or **Rule-based Proxy** to follow routing rules. Zeytun starts in **Direct Outbound** mode on each launch.
5. Enable **System Proxy** for applications that respect macOS proxy settings, or **TUN** for tunnel-based capture. TUN requires administrator permission.
6. Open **Traffic Monitor** to inspect the connections handled by Zeytun and create rules from them.

Some settings need **Restart Core** before they take effect. To try Connection Ask, enable it under **Settings → Network**, restart the core, and use Rule-based Proxy mode.

### Core and country data

Release builds include the network core and a DB-IP Country Lite database; no separate core download is required. The core is updated with the app.

Country lookups for IP details and flags use the bundled database until a downloaded update is available. **Settings → Network** offers a manual update and an optional startup check; a current bundled database is not downloaded again. Updates are validated and stored in app data, leaving the app bundle unchanged. Country-based routing uses separate rule sets, not this display database.

## Help and feedback

Found a bug or have a use case the app does not cover? [Open an issue](https://github.com/zeytun-labs/zeytun-app/issues) with your macOS version, Zeytun version, and steps to reproduce it.

Remove subscription URLs, proxy credentials, and identifying network details from screenshots and logs. For security concerns, follow the [private reporting guidance](CONTRIBUTING.md#security) instead of posting exploit details publicly.

## Support

☕ Zeytun is free to use. If it makes your daily setup easier, an optional donation helps support development. Donations are not required to use the app.

Use the repository's **Sponsor** button or one of the addresses below. Make sure the asset and network match before sending.

| Network | Address |
|---|---|
| Bitcoin | `bc1q2a7aywmekl3kmqzw9t5rg5ds643avetnymg5n6` |
| TRON (TRX, USDT) | `TPEtVbd7rrguo9KjfDvBfPZnqsiURY1fp7` |
| TON | `UQADcpf4-JT_4vrR17DntsHH0Ryj0o2LBycr3iEyGu7L8WVK` |
| Solana (SOL, USDT) | `5wHu7Zra6SbeEmbJbx79ixdnhjoWhfqwQCFjeBcSKZcz` |
| EVM (ETH, BSC, Arbitrum, Base, Polygon) | `0x1308E77d3C332F862A3cE87eeA8CfAFF019e90A3` |

A star, a useful bug report, or a contribution helps too.

## For developers

Want to build Zeytun or contribute a change? See [CONTRIBUTING.md](CONTRIBUTING.md#development-setup) for prerequisites, core and GeoIP setup, development commands, and checks.

The network engine is [zeytun-core](https://github.com/zeytun-labs/zeytun-core); proxy-link conversion is provided by [zeytun-config](https://github.com/zeytun-labs/zeytun-config).

## License

[GPL-3.0](LICENSE).

The network engine is a derivative of [sing-box](https://github.com/SagerNet/sing-box), also under GPLv3.

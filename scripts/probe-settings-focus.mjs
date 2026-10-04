#!/usr/bin/env node
// Runtime check for the Settings dialog focus behaviour (plan task 1).
//
// Asserts, in headless Chrome against the vite dev server:
//   1. opening Settings does NOT move focus onto a category button
//      (bits-ui auto-focuses the first tabbable element otherwise — that is the
//      stray blue square the user reported),
//   2. Tab still reaches the category list, and the focused control renders the
//      design-system ring (olive `--ring`) rather than the browser default.
//
// Usage: pnpm dev  (in another shell)  then  node scripts/probe-settings-focus.mjs
// Optional env: APP_URL (default http://localhost:1420), CHROME.

import { spawn } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const APP_URL = process.env.APP_URL ?? "http://localhost:1420";
const CHROME =
  process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const PORT = 9333;

// Minimal Tauri shim. Without metadata.currentWindow the layout's
// getCurrentWindow().show() throws and boot hangs at "Starting Zeytun Core…".
const MOCK = `
window.__TAURI_INTERNALS__ = {
  metadata: {
    currentWindow: { label: "main" },
    currentWebview: { label: "main", windowLabel: "main" },
    currentWebviewWindow: { label: "main" },
  },
  invoke: (cmd) => {
    // Hangs forever so the probe can assert the spinner survives a tab switch.
    if (cmd === "core_update_geoip_db") return new Promise(() => {});
    return Promise.resolve({
      core_profile_get: {
        id: "mock", version: 1, outbound_mode: "rule", final_policy: null,
        local_proxy: { port: 7890, socks_port: 7891, mixed_port: 6060, allow_lan: false, mode: "rule", bind_address: "127.0.0.1" },
        dns: { enable: false, listen: "127.0.0.1:0", fake_ip: false },
        policies: [], proxies: [], rules: [], temp_rules: [], rule_sets: [],
      },
      core_profile_list: [],
      core_status: { phase: "running", message: null },
      core_network_policy_get: {},
    }[cmd] ?? {});
  },
  transformCallback: () => 0,
  convertFileSrc: (p) => p,
};
`;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function cdpTargets() {
  const res = await fetch(`http://localhost:${PORT}/json/list`);
  return res.json();
}

function connect(wsUrl) {
  const ws = new WebSocket(wsUrl);
  let id = 0;
  const pending = new Map();
  ws.addEventListener("message", (ev) => {
    const msg = JSON.parse(ev.data);
    if (msg.id && pending.has(msg.id)) {
      const { resolve, reject } = pending.get(msg.id);
      pending.delete(msg.id);
      msg.error ? reject(new Error(JSON.stringify(msg.error))) : resolve(msg.result);
    }
  });
  const ready = new Promise((resolve, reject) => {
    ws.addEventListener("open", resolve);
    ws.addEventListener("error", reject);
  });
  const send = (method, params = {}) =>
    new Promise((resolve, reject) => {
      const msgId = ++id;
      pending.set(msgId, { resolve, reject });
      ws.send(JSON.stringify({ id: msgId, method, params }));
    });
  return { ready, send, close: () => ws.close() };
}

const failures = [];
const check = (ok, label, detail = "") => {
  console.log(`${ok ? "PASS" : "FAIL"}  ${label}${detail ? ` — ${detail}` : ""}`);
  if (!ok) failures.push(label);
};

const chrome = spawn(CHROME, [
  "--headless=new",
  `--remote-debugging-port=${PORT}`,
  `--user-data-dir=${mkdtempSync(join(tmpdir(), "zeytun-focus-"))}`,
  "--no-first-run",
  "--disable-gpu",
  "--disable-background-networking",
  "--window-size=1280,900",
  "about:blank",
]);
chrome.on("error", (e) => {
  console.error("failed to launch Chrome:", e.message);
  process.exit(1);
});

try {
  // Wait for the debugger socket.
  let targets;
  for (let i = 0; i < 40; i++) {
    try {
      targets = await cdpTargets();
      break;
    } catch {
      await sleep(250);
    }
  }
  if (!targets) throw new Error(`no CDP endpoint on :${PORT}`);

  // Blank tab first, inject the mock, THEN navigate — a mock added after a live
  // navigation never applies to the already-loading document.
  const created = await fetch(
    `http://localhost:${PORT}/json/new?about:blank`,
    { method: "PUT" },
  ).then((r) => r.json());

  const { ready, send, close } = connect(created.webSocketDebuggerUrl);
  await ready;
  await send("Page.enable");
  await send("Runtime.enable");
  await send("Page.addScriptToEvaluateOnNewDocument", { source: MOCK });
  await send("Page.navigate", { url: APP_URL });

  const evaluate = async (expression) => {
    const { result, exceptionDetails } = await send("Runtime.evaluate", {
      expression,
      awaitPromise: true,
      returnByValue: true,
    });
    if (exceptionDetails) throw new Error(exceptionDetails.text ?? "eval failed");
    return result.value;
  };

  // Boot: wait for the rail's Settings trigger to actually exist — `main` shows
  // up during the loading gate too, so keying off it races the mount.
  const findTrigger = `[...document.querySelectorAll('a,button')]
      .find((n) => (n.getAttribute('aria-label') || n.textContent || '').trim().includes('Settings'))`;
  let booted = false;
  for (let i = 0; i < 80; i++) {
    booted = await evaluate(`!!(${findTrigger})`).catch(() => false);
    if (booted) break;
    await sleep(250);
  }
  if (!booted) {
    const dump = await evaluate(
      `[...document.querySelectorAll('a,button')].map(n => (n.getAttribute('aria-label')||n.textContent||'').trim().slice(0,20)).join(' | ')`,
    ).catch(() => "<eval failed>");
    throw new Error(
      `app never rendered the Settings trigger — is \`pnpm dev\` running? buttons: ${dump}`,
    );
  }

  // Open Settings the way the user does: click the rail item.
  const opened = await evaluate(`(() => {
    const el = ${findTrigger};
    if (!el) return 'no settings trigger';
    el.click();
    return 'clicked';
  })()`);
  if (opened !== "clicked") throw new Error(opened);

  await sleep(600); // dialog mount + the focus-scope rAF

  const dialogOpen = await evaluate(
    `!!document.querySelector('[data-slot="dialog-content"]')`,
  );
  check(dialogOpen, "settings dialog opened");

  // 1. Focus must NOT have landed on a category button.
  const focused = await evaluate(`(() => {
    const a = document.activeElement;
    if (!a) return "none";
    return (a.textContent || "").trim().slice(0, 24) || a.tagName;
  })()`);
  check(
    !["General", "Network", "DNS", "Diagnostics", "About"].includes(focused),
    "dialog does not auto-focus a category",
    `activeElement: ${focused}`,
  );

  // 2. Keyboard focus reaches the category list and paints the design ring.
  await send("Input.dispatchKeyEvent", {
    type: "rawKeyDown",
    windowsVirtualKeyCode: 9,
    key: "Tab",
    code: "Tab",
  });
  await send("Input.dispatchKeyEvent", {
    type: "keyUp",
    windowsVirtualKeyCode: 9,
    key: "Tab",
    code: "Tab",
  });
  await sleep(150);

  const ring = await evaluate(`(() => {
    const a = document.activeElement;
    if (!a) return null;
    const cs = getComputedStyle(a);
    return {
      label: (a.textContent || "").trim().slice(0, 24),
      inDialog: !!a.closest('[data-slot="dialog-content"]'),
      // The design system draws focus with a box-shadow ring (Tailwind ring-3),
      // not the UA outline.
      shadow: cs.boxShadow,
      outline: cs.outlineStyle,
    };
  })()`);
  check(ring?.inDialog === true, "Tab keeps focus inside the dialog", ring?.label);
  check(
    !!ring && ring.shadow !== "none",
    "focused control paints the design-system ring",
    `box-shadow: ${ring?.shadow}`,
  );

  // 3. Height chain: the scroll area viewport must be the scrolling element, not
  //    the dialog. If the chain is unbounded the dialog itself overflows and the
  //    footer gets pushed off-screen (documented castle-svelte-ui pitfall).
  await evaluate(`(() => {
    const cats = [...document.querySelectorAll('[data-slot="dialog-content"] button')];
    cats.find((b) => b.textContent.trim() === 'Network')?.click();
  })()`);
  await sleep(300);

  const chain = await evaluate(`(() => {
    const dialog = document.querySelector('[data-slot="dialog-content"]');
    const vp = [...dialog.querySelectorAll('[data-slot="scroll-area-viewport"]')].at(-1);
    return {
      dialogOverflows: dialog.scrollHeight > dialog.clientHeight + 1,
      viewportScrolls: vp.scrollHeight > vp.clientHeight,
      viewportBottom: Math.round(vp.getBoundingClientRect().bottom),
      dialogBottom: Math.round(dialog.getBoundingClientRect().bottom),
    };
  })()`);
  check(!chain.dialogOverflows, "dialog itself does not scroll (height chain bounded)");
  // Guards the overlap check below from passing trivially on non-scrolling content.
  check(chain.viewportScrolls, "Network section content actually overflows");

  // 4. The footer must be a flex sibling BELOW the scroll area, not floating on
  //    top of it — the reported bug. Dirty a field to reveal it.
  const revealed = await evaluate(`(() => {
    const input = document.querySelector('#listenPort');
    if (!input) return 'no listen port input';
    input.focus();
    input.value = String(Number(input.value || 6060) + 1);
    input.dispatchEvent(new Event('input', { bubbles: true }));
    return 'dirtied';
  })()`);
  check(revealed === "dirtied", "listen port field is reachable", revealed);
  await sleep(500); // slide transition

  const footer = await evaluate(`(() => {
    const dialog = document.querySelector('[data-slot="dialog-content"]');
    const save = [...dialog.querySelectorAll('button')]
      .find((b) => b.textContent.trim() === 'Save');
    if (!save) return null;
    const bar = save.parentElement;
    const vp = [...dialog.querySelectorAll('[data-slot="scroll-area-viewport"]')].at(-1);
    const b = bar.getBoundingClientRect();
    const v = vp.getBoundingClientRect();
    return {
      position: getComputedStyle(bar).position,
      // The whole point: no vertical overlap with the scrolling content.
      overlapPx: Math.round(Math.max(0, v.bottom - b.top)),
      applyStillThere: [...dialog.querySelectorAll('button')]
        .some((x) => x.textContent.trim() === 'Apply'),
    };
  })()`);
  check(!!footer, "footer appears once a field is dirty");
  check(footer?.position === "static", "footer is in normal flow, not absolute", footer?.position);
  check(footer?.overlapPx === 0, "footer does not overlap scrolling content", `${footer?.overlapPx}px overlap`);
  check(footer?.applyStillThere === false, "Apply button is gone");

  // 5. The sidebar's Restart Core rule and the Save/Cancel rule read as one line
  //    across the column divider, so their top borders must share a baseline.
  const rules = await evaluate(`(() => {
    const dialog = document.querySelector('[data-slot="dialog-content"]');
    const byText = (t) => [...dialog.querySelectorAll('button')]
      .find((b) => b.textContent.trim() === t);
    const left = byText('Restart Core')?.closest('div[class*="border-t"]');
    const right = byText('Save')?.parentElement;
    if (!left || !right) return null;
    const l = getComputedStyle(left);
    const r = getComputedStyle(right);
    return {
      deltaPx: Math.abs(
        Math.round(left.getBoundingClientRect().top - right.getBoundingClientRect().top),
      ),
      sameColor: l.borderTopColor === r.borderTopColor,
      leftColor: l.borderTopColor,
      rightColor: r.borderTopColor,
    };
  })()`);
  check(!!rules, "both footer rules found");
  check(rules?.deltaPx === 0, "footer rules share a baseline", `${rules?.deltaPx}px off`);
  check(
    rules?.sameColor === true,
    "footer rules use the same border colour",
    `${rules?.leftColor} vs ${rules?.rightColor}`,
  );

  // 6. GeoIP progress is global: it must survive a tab switch mid-download.
  const geoip = await evaluate(`(() => {
    const dialog = document.querySelector('[data-slot="dialog-content"]');
    const btn = [...dialog.querySelectorAll('button')]
      .find((b) => b.textContent.trim().startsWith('Update Now'));
    if (!btn) return 'no update button';
    btn.click();
    return 'clicked';
  })()`);
  check(geoip === "clicked", "GeoIP Update Now is reachable", geoip);
  await sleep(150);

  const survived = await evaluate(`(() => {
    const dialog = document.querySelector('[data-slot="dialog-content"]');
    const byText = (t) => [...dialog.querySelectorAll('button')]
      .find((b) => b.textContent.trim() === t);
    // Leave to DNS and come back — the section unmounts, so component-local
    // state would be lost here.
    byText('DNS')?.click();
    return new Promise((resolve) => setTimeout(() => {
      byText('Network')?.click();
      setTimeout(() => {
        const btn = [...dialog.querySelectorAll('button')]
          .find((b) => /Updating|Update Now/.test(b.textContent));
        resolve({
          label: btn?.textContent.trim(),
          disabled: btn?.disabled ?? null,
          spinner: !!btn?.querySelector('[role="status"]'),
        });
      }, 350);
    }, 350));
  })()`);
  check(
    survived.label?.startsWith("Updating"),
    "GeoIP spinner survives a tab switch",
    `label: ${survived.label}, disabled: ${survived.disabled}, spinner: ${survived.spinner}`,
  );
  check(survived.disabled === true, "GeoIP button stays disabled mid-download");

  close();
} catch (err) {
  console.error(`ERROR  ${err.message}`);
  failures.push(err.message);
} finally {
  chrome.kill();
}

if (failures.length) {
  console.error(`\n${failures.length} check(s) failed`);
  process.exit(1);
}
console.log("\nall checks passed");

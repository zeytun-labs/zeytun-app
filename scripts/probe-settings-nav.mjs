#!/usr/bin/env node
// Runtime check for the Settings dialog sidebar's sliding pill + the trimmed
// "Zeytun's Own Traffic" route options.
//
// Asserts, in headless Chrome against the vite dev server:
//   1. the nav list renders exactly TWO blobs (hover + active) before its first
//      item, with the active one resting on the open category (General);
//   2. clicking another category SLIDES the active blob onto it (rect matches the
//      item) and the active item's text colour flips to primary-foreground via
//      data-blob-on, while its own background stays transparent;
//   3. a pointermove over a non-active category moves the HOVER blob, not the
//      active one; pointerleave slides the hover blob away and the active one
//      back to the selected category;
//   4. every route Select in the Network section offers exactly two items —
//      Direct and Through Zeytun — even when the profile HAS named policies.
//
// Usage: pnpm dev (another shell), then node scripts/probe-settings-nav.mjs
// Optional env: APP_URL (default http://localhost:1420), CHROME.

import { spawn } from "node:child_process";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const APP_URL = process.env.APP_URL ?? "http://localhost:1420";
const CHROME =
  process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const PORT = Number(process.env.PORT ?? 9339);

// Tauri shim. The profile deliberately carries a named policy: if the Network
// section ever re-adds policy options, assertion 4 fails.
const MOCK = `
window.__TAURI_INTERNALS__ = {
  metadata: {
    currentWindow: { label: "main" },
    currentWebview: { label: "main", windowLabel: "main" },
    currentWebviewWindow: { label: "main" },
  },
  invoke: (cmd) => Promise.resolve({
    core_profile_get: {
      id: "mock", version: 1, outbound_mode: "rule", final_policy: null,
      local_proxy: { port: 7890, socks_port: 7891, mixed_port: 6060, allow_lan: false, mode: "rule", bind_address: "127.0.0.1" },
      dns: { enable: false, listen: "127.0.0.1:0", fake_ip: false },
      policies: [
        { tag: "root-policy", name: "Global Proxy", type: "manual", outbounds: ["p1"], default: "p1" },
        { tag: "p1", name: "US Auto", type: "url-test", outbounds: ["x1"], tolerance: 50 },
      ],
      proxies: [], rules: [], temp_rules: [], rule_sets: [],
    },
    core_profile_list: [],
    core_status: { phase: "running", message: null },
    core_network_policy_get: { geoip: { kind: "policy", tag: "p1" } },
  }[cmd] ?? {}),
  transformCallback: () => 0,
  convertFileSrc: (p) => p,
};
`;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

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

// Blob geometry is read with offset*/getBoundingClientRect on the blob itself and
// on the item, so a mid-slide sample would be caught as a mismatch rather than
// passing by accident. Two rAFs let the transition settle.
const SETTLE = `await new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r)))`;

const chrome = spawn(CHROME, [
  "--headless=new",
  `--remote-debugging-port=${PORT}`,
  `--user-data-dir=${mkdtempSync(join(tmpdir(), "zeytun-nav-"))}`,
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
  let targets;
  for (let i = 0; i < 40; i++) {
    try {
      targets = await fetch(`http://localhost:${PORT}/json/list`).then((r) => r.json());
      break;
    } catch {
      await sleep(250);
    }
  }
  if (!targets) throw new Error(`no CDP endpoint on :${PORT}`);

  const created = await fetch(`http://localhost:${PORT}/json/new?about:blank`, {
    method: "PUT",
  }).then((r) => r.json());

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

  // --- boot, then open Settings the way the user does ---
  const findTrigger = `[...document.querySelectorAll('a,button')]
      .find((n) => (n.getAttribute('aria-label') || n.textContent || '').trim().includes('Settings'))`;
  let booted = false;
  for (let i = 0; i < 80; i++) {
    booted = await evaluate(`!!(${findTrigger})`).catch(() => false);
    if (booted) break;
    await sleep(250);
  }
  if (!booted) throw new Error("app never rendered the Settings trigger — is `pnpm dev` running?");

  const opened = await evaluate(`(() => { const el = ${findTrigger}; if (!el) return 'none'; el.click(); return 'clicked'; })()`);
  if (opened !== "clicked") throw new Error(opened);
  await sleep(700);

  const NAV = `document.querySelector('[data-slot="dialog-content"] [data-scroll-area-content] > div')`;
  const ITEMS = `[...${NAV}.querySelectorAll('[data-slot="settings-item"]')]`;

  // 1. two blobs, prepended before the first item
  const blobs = await evaluate(`(() => {
    const nav = ${NAV};
    if (!nav) return null;
    const bs = [...nav.querySelectorAll(':scope > div[aria-hidden="true"]')];
    return { count: bs.length, firstIsBlob: bs.length && nav.firstElementChild === bs[0],
             info: bs.map(b => ({ cls: b.className, opacity: b.style.opacity, w: b.offsetWidth, h: b.offsetHeight })) };
  })()`);
  if (!blobs) throw new Error("settings nav container not found");
  check(blobs.count === 2, "nav renders exactly two blobs", JSON.stringify(blobs.count));
  check(blobs.firstIsBlob, "blobs are prepended before the first item");
  const activeBlob = await evaluate(`(() => {
    const bs = [...${NAV}.querySelectorAll(':scope > div[aria-hidden="true"]')];
    return bs.findIndex(b => b.className.includes('bg-primary'));
  })()`);
  check(activeBlob === 1, "active blob paints above the hover blob (index 1)", `idx=${activeBlob}`);

  // 2. active blob rests on General
  const itemRect = (label) =>
    evaluate(`(() => {
      const it = ${ITEMS}.find(n => n.textContent.includes(${JSON.stringify(label)}));
      const r = it.getBoundingClientRect();
      return { x: r.x, y: r.y, w: r.width, h: r.height };
    })()`);

  const blobRect = (idx) =>
    evaluate(`(() => {
      const b = ${NAV}.querySelectorAll(':scope > div[aria-hidden="true"]')[${idx}];
      const r = b.getBoundingClientRect();
      return { x: r.x, y: r.y, w: r.width, h: r.height, opacity: b.style.opacity,
               radius: getComputedStyle(b).borderRadius };
    })()`);

  await evaluate(`(async () => { ${SETTLE}; })()`);
  await sleep(300);
  const gen = await itemRect("General");
  const rest = await blobRect(activeBlob);
  check(rest.opacity === "1", "active blob visible on open");
  check(
    Math.abs(rest.y - gen.y) < 3 && Math.abs(rest.w - gen.w) < 3 && Math.abs(rest.h - gen.h) < 3,
    "active blob rests on the open category (General)",
    `blob=${JSON.stringify({ y: rest.y, w: rest.w, h: rest.h })} item=${JSON.stringify({ y: gen.y, w: gen.w, h: gen.h })}`,
  );
  check(rest.radius !== "0px" && Number.parseFloat(rest.radius) > 10, "blob is pill-shaped", rest.radius);

  // 2b. blob IS the indicator: active item has no background of its own
  const activeStyle = await evaluate(`(() => {
    const it = ${ITEMS}.find(n => n.dataset.active !== undefined);
    const cs = getComputedStyle(it);
    return { bg: cs.backgroundColor, color: cs.color, blobOn: it.hasAttribute('data-blob-on') };
  })()`);
  check(
    activeStyle.bg === "rgba(0, 0, 0, 0)" || activeStyle.bg === "transparent",
    "selected item has no background of its own (blob carries it)",
    activeStyle.bg,
  );
  check(activeStyle.blobOn === true, "selected item carries data-blob-on");
  const dimmed = await evaluate(
    `getComputedStyle(${ITEMS}.find(n => n.dataset.active === undefined)).color`,
  );
  check(
    activeStyle.color !== dimmed,
    "selected item text is not the dimmed inactive colour",
    `active=${activeStyle.color} inactive=${dimmed}`,
  );

  // 2c. clicking another category slides the active blob onto it
  await evaluate(`(() => { ${ITEMS}.find(n => n.textContent.includes('Network')).click(); })()`);
  await sleep(450);
  const net = await itemRect("Network");
  const after = await blobRect(activeBlob);
  check(
    Math.abs(after.y - net.y) < 3 && Math.abs(after.w - net.w) < 3,
    "active blob slides onto the clicked category",
    `blob.y=${after.y} item.y=${net.y}`,
  );
  const movedOn = await evaluate(`(() => {
    const it = ${ITEMS}.find(n => n.textContent.includes('Network'));
    return { blobOn: it.hasAttribute('data-blob-on'), color: getComputedStyle(it).color,
             generalBlobOn: ${ITEMS}.find(n => n.textContent.includes('General')).hasAttribute('data-blob-on') };
  })()`);
  check(movedOn.blobOn && !movedOn.generalBlobOn, "data-blob-on moved to the new item");
  check(
    movedOn.color === activeStyle.color,
    "newly selected item uses the same on-blob text colour",
    `${movedOn.color} vs ${activeStyle.color}`,
  );

  // 3. pointer drives the hover blob only
  const hoverBefore = await blobRect(0);
  await evaluate(`(() => {
    const it = ${ITEMS}.find(n => n.textContent.includes('DNS'));
    const r = it.getBoundingClientRect();
    it.dispatchEvent(new PointerEvent('pointermove', { bubbles: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));
  })()`);
  await sleep(350);
  const dns = await itemRect("DNS");
  const hoverAfter = await blobRect(0);
  check(hoverAfter.opacity === "1", "hover blob appears on pointermove");
  check(
    Math.abs(hoverAfter.y - dns.y) < 3 && Math.abs(hoverAfter.w - dns.w) < 3,
    "hover blob follows the pointer onto DNS",
    `blob.y=${hoverAfter.y} item.y=${dns.y}`,
  );
  const activeUnmoved = await blobRect(activeBlob);
  check(
    Math.abs(activeUnmoved.y - net.y) < 3,
    "active blob stays on Network while hovering DNS",
    `active.y=${activeUnmoved.y} network.y=${net.y}`,
  );

  // 3b. pointerleave slides the hover blob away
  await evaluate(`(() => { ${NAV}.dispatchEvent(new PointerEvent('pointerleave', { bubbles: true })); })()`);
  await sleep(350);
  const hoverGone = await blobRect(0);
  check(hoverGone.opacity === "0", "hover blob hides on pointerleave", hoverGone.opacity);
  const activeBack = await blobRect(activeBlob);
  check(Math.abs(activeBack.y - net.y) < 3, "active blob still on the selected category after leave");

  // 4. route Selects offer exactly Direct + Through Zeytun
  const triggerCount = await evaluate(
    `document.querySelectorAll('[data-slot="dialog-content"] [data-slot="select-trigger"]').length`,
  );
  check(triggerCount === 3, "three traffic-route selects rendered", String(triggerCount));

  // bits-ui opens on pointerdown, not click — a bare .click() leaves the menu shut.
  await evaluate(`(() => {
    const t = document.querySelectorAll('[data-slot="dialog-content"] [data-slot="select-trigger"]')[0];
    const r = t.getBoundingClientRect();
    for (const type of ['pointerdown', 'pointerup', 'click']) {
      t.dispatchEvent(new PointerEvent(type, { bubbles: true, clientX: r.x + r.width / 2, clientY: r.y + r.height / 2 }));
    }
  })()`);
  await sleep(400);
  const menuOpen = await evaluate(
    `!!document.querySelector('[data-slot="select-content"]')`,
  );
  check(menuOpen, "route select menu opens on pointerdown");
  const options = await evaluate(
    `[...document.querySelectorAll('[data-slot="select-item"]')].map(n => n.textContent.trim())`,
  );
  check(
    options.length === 2,
    "route select lists exactly two options",
    JSON.stringify(options),
  );
  check(
    options[0] === "Direct" && options[1] === "Through Zeytun",
    "options are Direct and Through Zeytun",
    JSON.stringify(options),
  );
  check(
    !options.some((o) => o.includes("US Auto") || o.includes("Global Proxy")),
    "no named policies leak into the route select",
    JSON.stringify(options),
  );

  // A stored named policy must read back as Through Zeytun, not as a stray value.
  const labels = await evaluate(
    `[...document.querySelectorAll('[data-slot="dialog-content"] [data-slot="select-trigger"]')].map(n => n.textContent.trim())`,
  );
  check(
    labels.every((l) => l === "Direct" || l === "Through Zeytun"),
    "stored legacy policy row renders as a known label",
    JSON.stringify(labels),
  );
  check(
    labels.filter((l) => l === "Through Zeytun").length === 1,
    "the geoip row (stored as policy:p1) degrades to Through Zeytun",
    JSON.stringify(labels),
  );
} catch (e) {
  console.error("PROBE ERROR:", e.message);
  failures.push(`probe error: ${e.message}`);
} finally {
  chrome.kill();
}

console.log(failures.length ? `\n${failures.length} FAILED` : "\nall assertions passed");
process.exit(failures.length ? 1 : 0);

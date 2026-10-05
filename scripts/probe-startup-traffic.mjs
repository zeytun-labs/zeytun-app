#!/usr/bin/env node
// Build first, serve build/ with a static server, then run this probe.
// APP_URL defaults to http://127.0.0.1:1431; SCREENSHOT_DIR is optional.
// Tauri's packaged startup URL is /index.html, unlike Vite's /.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const url = process.env.APP_URL ?? "http://127.0.0.1:1431";
const port = Number(process.env.PORT ?? 9341);
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const chrome = spawn(process.env.CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome", [
  "--headless=new", `--remote-debugging-port=${port}`,
  `--user-data-dir=${mkdtempSync(join(tmpdir(), "zeytun-startup-"))}`,
  "--no-first-run", "--disable-background-networking", "--window-size=1280,1000", "about:blank",
]);
const mock = `window.__TAURI_INTERNALS__ = {
  metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main", windowLabel: "main" }, currentWebviewWindow: { label: "main" } },
  invoke: (cmd) => Promise.resolve({
    core_profile_get: { id: "mock", version: 1, outbound_mode: "direct", final_policy: null,
      local_proxy: { port: 7890, socks_port: 7891, mixed_port: 6060, allow_lan: false, mode: "mixed", bind_address: "127.0.0.1" },
      dns: { enable: false, fake_ip: false, servers: [], rules: [] },
      policies: [], proxies: [], rules: [], temp_rules: [], rule_sets: [] },
    core_profile_list: [], core_status: { phase: "running", message: null },
    core_get_traffic_analytics: { series: Array.from({ length: 24 }, (_, i) => ({ ts: Math.floor(Date.now()/3600000)*3600-i*3600, up: 1048576, down: 4194304 })), top_domains: [], top_policies: [], top_processes: [] },
    core_get_traffic_summary: { direct_up: 0, direct_down: 0, proxy_up: 0, proxy_down: 0 },
  }[cmd] ?? {}), transformCallback: () => 0, convertFileSrc: (p) => p
};`;
let ws;
const failures = [];
const check = (ok, label, detail) => {
  console.log(`${ok ? "PASS" : "FAIL"} ${label}: ${JSON.stringify(detail)}`);
  if (!ok) failures.push(label);
};
try {
  let targets;
  for (let i = 0; i < 80 && !targets; i++) {
    try { targets = await fetch(`http://localhost:${port}/json/list`).then((r) => r.json()); }
    catch { await sleep(100); }
  }
  assert(targets, "Chrome CDP did not start");
  ws = new WebSocket(targets.find((t) => t.type === "page").webSocketDebuggerUrl);
  await new Promise((resolve, reject) => { ws.addEventListener("open", resolve); ws.addEventListener("error", reject); });
  let id = 0;
  const pending = new Map();
  ws.addEventListener("message", ({ data }) => {
    const message = JSON.parse(data);
    if (pending.has(message.id)) {
      const { resolve, reject } = pending.get(message.id);
      pending.delete(message.id);
      message.error ? reject(new Error(JSON.stringify(message.error))) : resolve(message.result);
    }
  });
  const send = (method, params = {}) => new Promise((resolve, reject) => {
    pending.set(++id, { resolve, reject }); ws.send(JSON.stringify({ id, method, params }));
  });
  const evaluate = async (expression) => {
    const result = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
    assert(!result.exceptionDetails, JSON.stringify(result.exceptionDetails));
    return result.result.value;
  };
  await send("Page.enable");
  await send("Page.addScriptToEvaluateOnNewDocument", { source: mock });
  const boot = async (path) => {
    await send("Page.navigate", { url: url + path });
    for (let i = 0; i < 100; i++) {
      if (await evaluate(`!!document.querySelector('nav[aria-label="Primary"]')`).catch(() => false)) return;
      await sleep(100);
    }
    throw new Error(`Startup never completed on ${path}`);
  };
  const navState = () => evaluate(`(() => ({ path: location.pathname,
    active: [...document.querySelectorAll('aside a[data-active]')].map(a => a.textContent.trim()),
    title: document.querySelector('header .sr-only')?.textContent,
    background: getComputedStyle(document.querySelector('nav[aria-label="Primary"] a')).backgroundColor
  }))()`);
  for (const path of ["/", "/index.html"]) {
    await boot(path);
    await sleep(350);
    const state = await navState();
    check(state.path === "/" && state.active.length === 1 && state.active[0] === "Home" && state.title === "Home" && state.background !== "rgba(0, 0, 0, 0)", `cold boot ${path} selects and titles Home`, state);
  }
  // Hover the actual traffic SVG. Data is deterministic, but rendering is real.
  for (let i = 0; i < 100; i++) {
    if (await evaluate(`!!document.querySelector('#stripe-up')`)) break;
    await sleep(100);
  }
  const point = await evaluate(`(() => {
    const svg = document.querySelector('#stripe-up').closest('svg');
    svg.scrollIntoView({block:'center'});
    const r = svg.getBoundingClientRect(); return {x:r.x+r.width/2, y:r.y+r.height/2};
  })()`);
  await sleep(700);
  await send("Input.dispatchMouseEvent", { type: "mouseMoved", ...point });
  await sleep(400);
  const colors = await evaluate(`(() => {
    const rows = [...document.querySelectorAll('div.flex.w-full.items-center.gap-2')].filter(n => n.firstElementChild?.classList.contains('shrink-0') && ['Upload','Download'].includes(n.querySelector('span')?.textContent));
    return rows.map(row => ({name:row.querySelector('span').textContent, color:getComputedStyle(row.firstElementChild).backgroundColor,
      expected: (() => { const n=document.createElement('div'); n.style.backgroundColor=row.querySelector('span').textContent==='Upload'?'var(--chart-2)':'var(--chart-1)'; row.append(n); const c=getComputedStyle(n).backgroundColor; n.remove(); return c; })()}));
  })()`);
  check(new Set(colors.map(c => c.name)).size === 2 && colors.every(c => c.color === c.expected && c.color !== "rgba(0, 0, 0, 0)") && new Set(colors.map(c => c.color)).size === 2, "traffic tooltip has distinct visible chart colors", colors);
  if (process.env.SCREENSHOT_DIR) {
    mkdirSync(process.env.SCREENSHOT_DIR, { recursive: true });
    const shot = await send("Page.captureScreenshot", { format: "png" });
    writeFileSync(join(process.env.SCREENSHOT_DIR, "startup-traffic.png"), Buffer.from(shot.data, "base64"));
  }
  // Client navigation must remain reactive after the packaged initial URL.
  await evaluate(`document.querySelector('aside a[href="/logs"]').click()`);
  await sleep(500);
  let state = await navState();
  check(state.active.length === 1 && state.active[0] === "Logs" && state.title === "Logs", "navigate to Logs", state);
  await evaluate(`document.querySelector('aside a[href="/"]').click()`);
  await sleep(500);
  state = await navState();
  check(state.active.length === 1 && state.active[0] === "Home" && state.title === "Home", "navigate back to Home", state);
  assert.equal(failures.length, 0, failures.join("; "));
} finally {
  ws?.close(); chrome.kill();
}

// Runnable check for rulesetLive map (mirrors notifications.svelte.ts).

function apply(live, p) {
  if (p.scope !== 3) return live;
  const tag = p.attrs?.tag;
  if (!tag) return live;
  const ts = p.ts_ms || 0;
  const prev = live[tag];
  if (prev && ts < prev.ts_ms) return live;

  const next = { ...live };
  if (p.code === "RULESET_READY" || p.code === "RULESET_UPDATED") {
    next[tag] = { status: "ready", ts_ms: ts, error: null };
    return next;
  }
  if (
    p.code === "RULESET_INITIAL_FETCH_FAILED" ||
    p.code === "RULESET_UPDATE_FAILED"
  ) {
    if (p.code === "RULESET_UPDATE_FAILED" && prev?.status === "ready") {
      next[tag] = { ...prev, ts_ms: ts };
      return next;
    }
    next[tag] = {
      status: "failed",
      ts_ms: ts,
      error: p.message || p.title || null,
    };
  }
  return next;
}

function markDownloading(live, tags) {
  const ts = Date.now();
  const next = { ...live };
  for (const tag of tags) next[tag] = { status: "downloading", ts_ms: ts, error: null };
  return next;
}

function status(live, tag) {
  return live[tag]?.status ?? "unknown";
}

const tag = "ruleset-abc";
const cases = [
  {
    name: "retry downloading then ready",
    run: () => {
      let live = markDownloading({}, [tag]);
      live = apply(live, {
        scope: 3,
        code: "RULESET_READY",
        ts_ms: Date.now() + 1,
        attrs: { tag },
      });
      return status(live, tag);
    },
    want: "ready",
  },
  {
    name: "retry downloading then fail → not downloaded",
    run: () => {
      let live = markDownloading({}, [tag]);
      live = apply(live, {
        scope: 3,
        code: "RULESET_UPDATE_FAILED",
        ts_ms: Date.now() + 1,
        attrs: { tag },
        message: "proxy down",
      });
      return status(live, tag);
    },
    want: "failed",
  },
  {
    name: "quiet update fail after ready stays ready",
    run: () => {
      let live = apply(
        {},
        { scope: 3, code: "RULESET_READY", ts_ms: 100, attrs: { tag } },
      );
      live = apply(live, {
        scope: 3,
        code: "RULESET_UPDATE_FAILED",
        ts_ms: 200,
        attrs: { tag },
      });
      return status(live, tag);
    },
    want: "ready",
  },
  {
    name: "only initial fail",
    run: () =>
      status(
        apply(
          {},
          {
            scope: 3,
            code: "RULESET_INITIAL_FETCH_FAILED",
            ts_ms: 50,
            attrs: { tag },
          },
        ),
        tag,
      ),
    want: "failed",
  },
  {
    name: "clear history keeps live",
    run: () => {
      let live = apply(
        {},
        { scope: 3, code: "RULESET_READY", ts_ms: 1, attrs: { tag } },
      );
      // clear items only
      return status(live, tag);
    },
    want: "ready",
  },
];

let failed = 0;
for (const c of cases) {
  const got = c.run();
  if (got !== c.want) {
    console.error(`FAIL ${c.name}: got ${got} want ${c.want}`);
    failed++;
  } else console.log(`ok ${c.name}`);
}
if (failed) process.exit(1);
console.log("all ok");

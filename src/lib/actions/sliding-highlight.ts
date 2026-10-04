/** Floating pill behind menu/select items. Pointer owns while inside; keyboard when not. */
const ITEM_SEL =
  '[data-slot$="-item"], [data-slot$="-trigger"], [role="menuitem"], [role="menuitemcheckbox"], [role="menuitemradio"], [role="option"]';

// Apply a state suffix to EVERY selector in a list (appending to the joined
// string would only filter the last one).
const withSuffix = (suffix: string, items: string) =>
  items
    .split(",")
    .map((s) => `${s}${suffix}`)
    .join(",");

const BLOB_CLS =
  "pointer-events-none absolute z-0 transition-[left,width,height,transform,opacity] duration-200 ease-out";

export function slidingHighlight(
  node: HTMLElement,
  opts: {
    blobClass?: string;
    hoverBlobClass?: string;
    radius?: string;
    /** Caller-owned item selector for hand-rolled navs that carry no data-slot. */
    itemSelector?: string;
  } = {},
) {
  // Callers own the corner radius: menus/tabs are rounded-2xl, the sidebars are
  // rounded-full pills. Two classes of the same specificity can't be resolved by
  // attribute order, so the default lives here and is overridden, not appended to.
  const radius = opts.radius ?? "rounded-2xl";
  const ITEM = opts.itemSelector ?? ITEM_SEL;
  // Active marker = the design system's own `data-active` variant definition:
  // `[data-state="active"]` (bits-ui) OR `[data-active]:not([data-active="false"])`
  // (hand-rolled navs). Harmless for menus/tabs — bits-ui never sets data-active.
  const ACTIVE_ITEM_SEL = [
    withSuffix('[data-state="active"]', ITEM),
    withSuffix('[data-active]:not([data-active="false"])', ITEM),
  ].join(",");
  // Two-blob mode (opts.blobClass set): a resting/active blob (e.g. primary
  // green) that follows the active item + a hover blob (faint) that follows
  // the pointer. Single-blob mode: one blob does both (menus/select/default).
  const twoBlobs = !!opts.blobClass;

  if (getComputedStyle(node).position === "static") {
    node.style.position = "relative";
  }

  const makeBlob = (cls: string) => {
    const b = document.createElement("div");
    b.setAttribute("aria-hidden", "true");
    b.className = `${BLOB_CLS} ${radius} ${cls}`;
    b.style.opacity = "0";
    b.style.top = "0";
    b.style.willChange = "transform, opacity";
    return b;
  };

  // Layout coordinates of el relative to root (padding-box space). Uses
  // offset* values — NOT getBoundingClientRect() — so ancestor transforms
  // (content zoom-in animations) can't corrupt the geometry.
  function offsetWithin(el: HTMLElement, root: HTMLElement) {
    let left = 0;
    let top = 0;
    let cur: HTMLElement | null = el;
    while (cur && cur !== root) {
      left += cur.offsetLeft;
      top += cur.offsetTop;
      const p = cur.offsetParent as HTMLElement | null;
      if (!p || p === cur || p === root) break;
      cur = p;
    }
    return { left, top };
  }

  // mark: this blob is the "selected" indicator — the item under it carries
  // data-blob-on (callers use it for text contrast). Set inside the rAF so the
  // text flips as the blob arrives, not as the slide starts.
  const makePlacer = (blob: HTMLElement, mark = false) => {
    let raf = 0;
    let target: HTMLElement | null = null;
    function place(el: HTMLElement | null) {
      target = el;
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(() => {
        if (!el || !node.isConnected || !node.contains(el)) {
          blob.style.opacity = "0";
          if (mark) clearBlobMark();
          return;
        }
        // offset* is transform-proof: the content opens with a zoom
        // (data-open:zoom-in-95) and getBoundingClientRect returns the SCALED
        // box while animating — a blob placed then stays undersized (gap on
        // the right) until hover re-measures at scale 1. The blob shares the
        // transform, so layout values align it exactly at every frame.
        const { left, top } = offsetWithin(el, node);
        blob.style.left = `${left}px`;
        blob.style.width = `${el.offsetWidth}px`;
        blob.style.height = `${el.offsetHeight}px`;
        blob.style.transform = `translateY(${top}px)`;
        blob.style.opacity = "1";
        if (mark) {
          clearBlobMark();
          el.setAttribute("data-blob-on", "");
        }
      });
    }
    return {
      place,
      blob,
      get target() {
        return target;
      },
    };
  };

  const clearBlobMark = () =>
    node
      .querySelectorAll<HTMLElement>("[data-blob-on]")
      .forEach((t) => t.removeAttribute("data-blob-on"));

  const blobs: HTMLElement[] = [];
  const hover = makePlacer(makeBlob(opts.hoverBlobClass ?? "bg-foreground/10"));
  blobs.push(hover.blob);
  node.prepend(hover.blob);

  let active: ReturnType<typeof makePlacer> | null = null;
  if (twoBlobs) {
    const b = makeBlob(opts.blobClass!);
    blobs.push(b);
    // Both blobs precede the triggers in tree order (paint under trigger
    // text); active is inserted after hover so it paints above it.
    node.insertBefore(b, hover.blob.nextSibling);
    active = makePlacer(b, true);
  }

  let pointerInside = false;

  function keyboardItem(): HTMLElement | null {
    return (
      node.querySelector<HTMLElement>(withSuffix("[data-highlighted]", ITEM)) ??
      node.querySelector<HTMLElement>(withSuffix(":focus", ITEM)) ??
      node.querySelector<HTMLElement>(ACTIVE_ITEM_SEL)
    );
  }

  function itemFromEvent(e: Event): HTMLElement | null {
    const t = (e.target as Element | null)?.closest?.(ITEM);
    return t instanceof HTMLElement && node.contains(t) ? t : null;
  }

  function activeItem(): HTMLElement | null {
    return node.querySelector<HTMLElement>(ACTIVE_ITEM_SEL);
  }

  function onPointerMove(e: PointerEvent) {
    pointerInside = true;
    const item = itemFromEvent(e);
    if (!item) return;
    if (active) {
      // Hover blob follows only NON-active items — hovering the active tab
      // keeps the resting (primary) blob clean.
      if (item.matches(ACTIVE_ITEM_SEL)) {
        hover.place(null);
      } else if (item !== hover.target) {
        hover.place(item);
      }
    } else if (item !== hover.target) {
      hover.place(item);
    }
  }

  function onPointerLeave() {
    pointerInside = false;
    const rest = activeItem();
    if (active) {
      hover.place(null);
      if (rest) active.place(rest);
    } else if (rest) {
      hover.place(rest);
    }
  }

  function onFocusIn(e: FocusEvent) {
    if (pointerInside) return;
    const item = itemFromEvent(e);
    if (!item) return;
    // Keyboard focus reads as hover; the active blob is state-driven only.
    if (active) {
      if (item.matches(ACTIVE_ITEM_SEL)) hover.place(null);
      else hover.place(item);
    } else {
      hover.place(item);
    }
  }

  function onScroll() {
    if (hover.target) hover.place(hover.target);
    if (active?.target) active.place(active.target);
  }

  const mo = new MutationObserver(() => {
    if (active) {
      // The active blob is state-driven — it must follow data-state changes
      // even while the pointer is inside (a real click activates a tab).
      // The hover blob is pointer-owned; the MO never snaps it.
      const k = keyboardItem();
      if (k) active.place(k);
      return;
    }
    if (pointerInside) return; // avoid snap-back while hovering
    const k = keyboardItem();
    if (k) hover.place(k);
  });
  mo.observe(node, {
    attributes: true,
    attributeFilter: ["data-highlighted", "data-state", "data-active"],
    subtree: true,
  });

  node.addEventListener("pointermove", onPointerMove);
  node.addEventListener("pointerleave", onPointerLeave);
  node.addEventListener("focusin", onFocusIn);
  node.addEventListener("scroll", onScroll, { passive: true });

  // Rest on an already-active item (tabs) instead of waiting for hover/focus.
  const initial = activeItem();
  if (initial) (active ?? hover).place(initial);

  return {
    destroy() {
      mo.disconnect();
      node.removeEventListener("pointermove", onPointerMove);
      node.removeEventListener("pointerleave", onPointerLeave);
      node.removeEventListener("focusin", onFocusIn);
      node.removeEventListener("scroll", onScroll);
      clearBlobMark();
      blobs.forEach((b) => b.remove());
    },
  };
}

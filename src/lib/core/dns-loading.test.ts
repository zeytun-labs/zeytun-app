import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { createDnsLoadingIndicator, shouldCompareSystem } from "./dns-loading";

beforeEach(() => { vi.useFakeTimers(); vi.setSystemTime(0); });
afterEach(() => vi.useRealTimers());

test("fast cache hits complete without ever displaying a loading state", () => {
  const change = vi.fn();
  const loading = createDnsLoadingIndicator(change);
  loading.start();
  vi.advanceTimersByTime(100);
  loading.finish();
  vi.runAllTimers();
  expect(change).not.toHaveBeenCalled();
});

test("a displayed indicator survives briefly without delaying request results", () => {
  const change = vi.fn();
  const loading = createDnsLoadingIndicator(change);
  loading.start();
  vi.advanceTimersByTime(140);
  expect(change).toHaveBeenLastCalledWith(true);
  vi.advanceTimersByTime(40);
  loading.finish();
  // finish returns synchronously; only the visual indicator has a remaining timer.
  vi.advanceTimersByTime(159);
  expect(change).toHaveBeenCalledTimes(1);
  vi.advanceTimersByTime(1);
  expect(change).toHaveBeenLastCalledWith(false);
});

test("a new request interrupts a pending hide without flashing off", () => {
  const change = vi.fn();
  const loading = createDnsLoadingIndicator(change);
  loading.start();
  vi.advanceTimersByTime(160);
  loading.finish();
  vi.advanceTimersByTime(40);
  loading.start();
  vi.advanceTimersByTime(300);
  expect(change.mock.calls).toEqual([[true]]);
  loading.finish();
  vi.runAllTimers();
  expect(change.mock.calls).toEqual([[true], [false]]);
});

test("disposing before or after reveal prevents future callbacks", () => {
  const change = vi.fn();
  const hidden = createDnsLoadingIndicator(change);
  hidden.start();
  hidden.dispose();
  vi.runAllTimers();
  expect(change).not.toHaveBeenCalled();
  const visible = createDnsLoadingIndicator(change);
  visible.start();
  vi.advanceTimersByTime(140);
  visible.finish();
  visible.dispose();
  vi.runAllTimers();
  expect(change.mock.calls).toEqual([[true]]);
});

test("System comparison is limited to Automatic A/AAAA queries even if its hidden state was selected", () => {
  expect(shouldCompareSystem(null, "A", true)).toBe(true);
  expect(shouldCompareSystem(null, "AAAA", true)).toBe(true);
  expect(shouldCompareSystem("local-dns", "A", true)).toBe(false);
  expect(shouldCompareSystem("custom-server", "AAAA", true)).toBe(false);
  expect(shouldCompareSystem(null, "CNAME", true)).toBe(false);
  expect(shouldCompareSystem(null, "A", false)).toBe(false);
});

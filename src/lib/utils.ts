import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type WithoutChild<T> = T extends { child?: any } ? Omit<T, "child"> : T;
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type WithoutChildren<T> = T extends { children?: any }
  ? Omit<T, "children">
  : T;
export type WithoutChildrenOrChild<T> = WithoutChildren<WithoutChild<T>>;
export type WithElementRef<T, U extends HTMLElement = HTMLElement> = T & {
  ref?: U | null;
};

export function isActive(href: string, currentPath: string): boolean {
  if (href === "/") return currentPath === "/";
  if (href === "#") return false;
  return currentPath.startsWith(href);
}

export function formatLatency(ms: number | null | undefined): string {
  if (ms === undefined) return "—";
  if (ms === null || ms === 0) return "timeout";
  return `${ms} ms`;
}

export function getLatencyColor(ms: number | null | undefined): string {
  if (ms === undefined) return "text-muted-foreground";
  if (ms === null || ms === 0) return "text-red-400";
  if (ms < 200) return "text-lime-500";
  if (ms < 350) return "text-yellow-500";
  return "text-orange-500";
}

export function formatBytes(bytes: number, decimals = 1) {
  if (bytes === 0) return { value: 0, unit: "B", toString: () => `0 B` };
  const k = 1024;
  const dm = decimals < 0 ? 0 : decimals;
  const sizes = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  const value = parseFloat((bytes / Math.pow(k, i)).toFixed(dm));

  return { value, unit: sizes[i], toString: () => `${value} ${sizes[i]}` };
}

export function formatSpeed(bytesPerSec: number) {
  return formatBytes(bytesPerSec).toString() + "/s";
}

// Pure validators & parsers for settings fields — no Svelte/state here, unit-testable.

export type ConnectivityField =
  | "internetTestUrl"
  | "proxyTestUrl"
  | "networkConfigUrl"
  | "stunServer";

export type ConnectivityValues = Record<ConnectivityField, string>;

/** Every field that can carry an inline error in the settings dialog. */
export type SettingsField = ConnectivityField | "listenPort";

export function isValidUrl(value: string): boolean {
  if (!value.trim()) return false;
  // Allow bare hostnames (e.g. "google.com") and full URLs
  try {
    new URL(value.startsWith("http") ? value : `https://${value}`);
    return true;
  } catch {
    return false;
  }
}

export function isValidStunServer(value: string): boolean {
  if (!value.trim()) return false;
  // host:port format
  const match = value.match(/^([a-zA-Z0-9._-]+):(\d+)$/);
  if (!match) return false;
  const port = parseInt(match[2], 10);
  return port > 0 && port <= 65535;
}

export function isValidListenPort(value: string): boolean {
  const port = parseInt(value, 10);
  return !isNaN(port) && port > 0 && port <= 65535;
}

export function validateConnectivity(
  values: ConnectivityValues,
): Partial<Record<ConnectivityField, string>> {
  const errors: Partial<Record<ConnectivityField, string>> = {};

  if (!isValidUrl(values.internetTestUrl)) {
    errors.internetTestUrl = "Enter a valid URL";
  }
  if (!isValidUrl(values.proxyTestUrl)) {
    errors.proxyTestUrl = "Enter a valid URL";
  }
  if (!isValidUrl(values.networkConfigUrl)) {
    errors.networkConfigUrl = "Enter a valid URL";
  }
  if (!isValidStunServer(values.stunServer)) {
    errors.stunServer = "Enter host:port (e.g. stun.example.com:3478)";
  }

  return errors;
}

/** One non-empty address per line. */

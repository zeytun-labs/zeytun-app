import { describe, it, expect } from "vitest";
import { proxyFormSchema, applyProtocolDefaults, type ProxyFormSchema } from "./proxy-schema";

// settings is non-optional in the schema; tests only exercise protocolFields,
// so a single helper builds complete inputs.
function node(
  type: ProxyFormSchema["type"],
  protocolFields: ProxyFormSchema["protocolFields"],
  overrides: Partial<ProxyFormSchema> = {},
): ProxyFormSchema {
  return {
    name: "n",
    address: "1.2.3.4",
    port: 443,
    type,
    protocolFields,
    settings: {} as ProxyFormSchema["settings"],
    ...overrides,
  };
}

const VALID_UUID = "11111111-1111-1111-1111-111111111111";

describe("proxyFormSchema", () => {
  it("accepts a complete vless node", () => {
    const r = proxyFormSchema.safeParse(node("vless", { uuid: VALID_UUID }));
    expect(r.success).toBe(true);
  });

  it("rejects vless without a uuid", () => {
    const r = proxyFormSchema.safeParse(node("vless", {}));
    expect(r.success).toBe(false);
    expect(JSON.stringify(r.error)).toContain("uuid");
  });

  it("rejects a malformed uuid", () => {
    const r = proxyFormSchema.safeParse(node("vless", { uuid: "not-a-uuid" }));
    expect(r.success).toBe(false);
  });

  it("requires both encryption and password for shadowsocks", () => {
    const r = proxyFormSchema.safeParse(node("shadowsocks", { password: "p" }));
    expect(r.success).toBe(false);
    expect(JSON.stringify(r.error)).toContain("encryption");
  });

  it("accepts socks with no protocol fields", () => {
    const r = proxyFormSchema.safeParse(node("socks", {}));
    expect(r.success).toBe(true);
  });

  it("accepts http with no protocol fields", () => {
    const r = proxyFormSchema.safeParse(node("http", {}));
    expect(r.success).toBe(true);
  });

  it("rejects an out-of-range port", () => {
    const r = proxyFormSchema.safeParse(node("http", {}, { port: 70000 }));
    expect(r.success).toBe(false);
  });

  it("rejects an unknown protocol", () => {
    // @ts-expect-error — wireguard is not a registered protocol
    const r = proxyFormSchema.safeParse(node("wireguard", {}));
    expect(r.success).toBe(false);
  });

  it("requires password for trojan", () => {
    const r = proxyFormSchema.safeParse(node("trojan", {}));
    expect(r.success).toBe(false);
    expect(JSON.stringify(r.error)).toContain("password");
  });

  it("requires uuid and password for tuic", () => {
    const onlyUuid = proxyFormSchema.safeParse(node("tuic", { uuid: VALID_UUID }));
    expect(onlyUuid.success).toBe(false);
    expect(JSON.stringify(onlyUuid.error)).toContain("password");

    const both = proxyFormSchema.safeParse(node("tuic", { uuid: VALID_UUID, password: "p" }));
    expect(both.success).toBe(true);
  });

  it("reports the failing protocol in the message", () => {
    const r = proxyFormSchema.safeParse(node("trojan", {}));
    expect(r.success).toBe(false);
    if (!r.success) {
      expect(r.error.issues[0].message).toContain("trojan");
    }
  });
});

describe("applyProtocolDefaults", () => {
  it("fills vmess defaults without clobbering set values", () => {
    const out = applyProtocolDefaults(node("vmess", { uuid: VALID_UUID, security: "aes-128-gcm" }));
    expect(out.protocolFields.alter_id).toBe(0);
    expect(out.protocolFields.security).toBe("aes-128-gcm");
  });

  it("fills socks version", () => {
    const out = applyProtocolDefaults(node("socks", {}));
    expect(out.protocolFields.version).toBe(5);
  });

  it("fills hysteria2 speeds", () => {
    const out = applyProtocolDefaults(node("hysteria2", { password: "p" }));
    expect(out.protocolFields.up_mbps).toBe(50);
    expect(out.protocolFields.down_mbps).toBe(100);
  });

  it("fills tuic transport defaults", () => {
    const out = applyProtocolDefaults(node("tuic", { uuid: VALID_UUID, password: "p" }));
    expect(out.protocolFields.congestion_control).toBe("cubic");
    expect(out.protocolFields.udp_relay_mode).toBe("native");
  });

  it("leaves vless alone", () => {
    const out = applyProtocolDefaults(node("vless", { uuid: VALID_UUID }));
    expect(out.protocolFields.uuid).toBe(VALID_UUID);
    expect(out.protocolFields.flow).toBeUndefined();
  });
});

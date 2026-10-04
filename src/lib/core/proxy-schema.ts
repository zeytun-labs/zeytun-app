import { z } from "zod";

export const tlsSchema = z.object({
  enabled: z.boolean().default(false),
  allow_insecure: z.boolean().default(false),
  sni: z.string().optional(),
  alpn: z.string().optional(),
  fingerprint: z.string().optional(),
  fragment: z.boolean().default(false),
  record_fragment: z.boolean().default(false),
  fallback_delay: z.string().optional(),
  reality_pbk: z.string().optional(),
  reality_sid: z.string().optional(),
});

export const multiplexSchema = z.object({
  enabled: z.boolean().default(false),
  tcp_brutal: z.boolean().default(false),
  brutal_download_speed: z.number().optional(),
  brutal_upload_speed: z.number().optional(),
});

export const advancedSchema = z.object({
  enabled: z.boolean().default(false),
  reuse_address: z.boolean().default(false),
  tcp_fast_open: z.boolean().default(false),
  udp_fragment: z.boolean().default(false),
  tcp_multi_path: z.boolean().default(false),
  connect_timeout: z.number().optional(),
  disable_sni: z.boolean().default(false),
  tls_min_version: z.string().optional(),
  tls_max_version: z.string().optional(),
  enable_ech: z.boolean().default(false),
  ech_config: z.string().optional(),
  certificate_sha256: z.string().optional(),
  client_cert: z.string().optional(),
  client_key: z.string().optional(),
});

export const settingsSchema = z.object({
  network: z
    .enum(["tcp", "ws", "grpc", "http", "httpupgrade", "quic", "xhttp"])
    .default("tcp")
    .optional(),
  path: z.string().optional(),
  host: z.string().optional(),
  mode: z.string().optional(),
  service_name: z.string().optional(),
  download: z
    .object({
      enabled: z.boolean().default(false),
      address: z.string().optional(),
      port: z.number().min(1).max(65535).optional(),
      path: z.string().optional(),
      host: z.string().optional(),
      detour: z.string().optional(),
      security: z.enum(["none", "tls", "reality"]).default("none").optional(),
      sni: z.string().optional(),
      allow_insecure: z.boolean().default(false).optional(),
      alpn: z.string().optional(),
      fingerprint: z.string().optional(),
      reality_pbk: z.string().optional(),
      reality_sid: z.string().optional(),
    })
    .optional(),
  tls: tlsSchema.default({ enabled: false, allow_insecure: false, fragment: false, record_fragment: false }),
  multiplex: multiplexSchema.default({ enabled: false, tcp_brutal: false }),
  advanced: advancedSchema.default({
    enabled: false,
    reuse_address: false,
    tcp_fast_open: false,
    udp_fragment: false,
    tcp_multi_path: false,
    disable_sni: false,
    enable_ech: false,
  }),
});

export const PROXY_PROTOCOLS = [
  "vless",
  "vmess",
  "trojan",
  "shadowsocks",
  "hysteria2",
  "tuic",
  "socks",
  "http",
] as const;

export type ProxyProtocolId = (typeof PROXY_PROTOCOLS)[number];

/**
 * One flat, all-optional shape instead of a discriminated union.
 *
 * The union made `protocolFields` a union of 8 object types, so every field
 * access in the 11 proxy-form components needed `as any` — ~85 casts that
 * also silenced validation. Flat + optional means `$formData.protocolFields.uuid`
 * type-checks everywhere; per-protocol requirements move to `superRefine`.
 */
export const protocolFieldsSchema = z.object({
  // vless / vmess / tuic
  uuid: z.string().optional(),
  flow: z.string().optional(),
  packet_encoding: z.string().optional(),
  // vmess
  alter_id: z.number().optional(),
  security: z.string().optional(),
  // trojan / shadowsocks / hysteria2 / tuic / socks / http
  password: z.string().optional(),
  username: z.string().optional(),
  // shadowsocks
  encryption: z.string().optional(),
  plugin: z.string().optional(),
  plugin_args: z.string().optional(),
  udp_over_tcp: z.boolean().optional(),
  // hysteria2
  up_mbps: z.number().optional(),
  down_mbps: z.number().optional(),
  obf_password: z.string().optional(),
  server_ports: z.string().optional(),
  hop_interval: z.string().optional(),
  // tuic
  congestion_control: z.string().optional(),
  udp_relay_mode: z.string().optional(),
  udp_over_stream: z.boolean().optional(),
  zero_rtt_handshake: z.boolean().optional(),
  heartbeat: z.string().optional(),
  // socks
  version: z.number().optional(),
});

/** Per-protocol requirements that the flat schema cannot express statically. */
const REQUIRED: Record<ProxyProtocolId, (keyof z.infer<typeof protocolFieldsSchema>)[]> = {
  vless: ["uuid"],
  vmess: ["uuid"],
  trojan: ["password"],
  shadowsocks: ["encryption", "password"],
  hysteria2: ["password"],
  tuic: ["uuid", "password"],
  socks: [],
  http: [],
};

/** Defaults the union applied per protocol; kept out of the schema itself so
 *  forms still validate `undefined` fields as "not filled in". */
const PROTOCOL_DEFAULTS: Record<ProxyProtocolId, Record<string, unknown>> = {
  vless: {},
  vmess: { alter_id: 0, security: "auto" },
  trojan: {},
  shadowsocks: { udp_over_tcp: false },
  hysteria2: { up_mbps: 50, down_mbps: 100 },
  tuic: {
    congestion_control: "cubic",
    udp_relay_mode: "native",
    udp_over_stream: false,
    zero_rtt_handshake: false,
  },
  socks: { version: 5 },
  http: {},
};

const UUID_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export const proxyFormSchema = z
  .object({
    type: z.enum(PROXY_PROTOCOLS),
    name: z.string().min(1, "Name is required"),
    address: z.string().min(1, "Address is required"),
    port: z.number().min(1).max(65535),
    protocolFields: protocolFieldsSchema,
    settings: settingsSchema,
  })
  .superRefine((data, ctx) => {
    for (const key of REQUIRED[data.type]) {
      const value = data.protocolFields[key];
      if (value === undefined || value === null || value === "") {
        ctx.addIssue({
          code: "custom",
          path: ["protocolFields", key],
          message: `${String(key)} is required for ${data.type}`,
        });
      }
    }
    if (
      (data.type === "vless" || data.type === "vmess" || data.type === "tuic") &&
      data.protocolFields.uuid &&
      !UUID_RE.test(data.protocolFields.uuid)
    ) {
      ctx.addIssue({
        code: "custom",
        path: ["protocolFields", "uuid"],
        message: "Invalid UUID",
      });
    }
  });

/** Apply the per-protocol defaults the discriminated union used to carry. */
export function applyProtocolDefaults<T extends z.infer<typeof proxyFormSchema>>(
  data: T,
): T {
  const defaults = PROTOCOL_DEFAULTS[data.type];
  if (!defaults) return data;
  return {
    ...data,
    protocolFields: { ...defaults, ...data.protocolFields },
  };
}

export type ProxyFormSchema = z.infer<typeof proxyFormSchema>;

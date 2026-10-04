<script lang="ts">
  import { setContext, untrack } from "svelte";
  import * as Sheet from "$lib/components/ui/sheet/index.js";
  import { Button } from "$lib/components/ui/button/index.js";
  import { Input } from "$lib/components/ui/input/index.js";
  import { ScrollArea } from "$lib/components/ui/scroll-area/index.js";
  import * as Select from "$lib/components/ui/select/index.js";
  import type {
    Proxy,
    ProxyProtocol,
    ProxyServerConfig,
    TlsConfig,
  } from "$lib/core/types";

  import VlessForm from "./proxy-forms/vless-form.svelte";
  import VmessForm from "./proxy-forms/vmess-form.svelte";
  import TrojanForm from "./proxy-forms/trojan-form.svelte";
  import ShadowsocksForm from "./proxy-forms/shadowsocks-form.svelte";
  import Hysteria2Form from "./proxy-forms/hysteria2-form.svelte";
  import TuicForm from "./proxy-forms/tuic-form.svelte";
  import SocksForm from "./proxy-forms/socks-form.svelte";
  import HttpForm from "./proxy-forms/http-form.svelte";
  import TlsFields from "./proxy-forms/tls-fields.svelte";
  import MultiplexFields from "./proxy-forms/multiplex-fields.svelte";
  import AdvancedFields from "./proxy-forms/advanced-fields.svelte";
  import { Switch } from "../ui/switch";
  import { Separator } from "../ui/separator";
  import * as Form from "$lib/components/ui/form/index.js";
  import { superForm, defaults } from "sveltekit-superforms";
  import { zod4 } from "sveltekit-superforms/adapters";
  import {
    proxyFormSchema,
    applyProtocolDefaults,
    type ProxyFormSchema,
  } from "$lib/core/proxy-schema.js";

  interface Props {
    open: boolean;
    mode: "create" | "edit";
    proxy?: Proxy | null;
    saving: boolean;
    onSave: (config: ProxyServerConfig, title: string) => void;
  }

  let {
    open = $bindable(false),
    mode,
    proxy = null,
    saving,
    onSave,
  }: Props = $props();

  const protocols: { value: ProxyProtocol; label: string }[] = [
    { value: "vless", label: "VLESS" },
    { value: "vmess", label: "VMess" },
    { value: "trojan", label: "Trojan" },
    { value: "shadowsocks", label: "Shadowsocks" },
    { value: "hysteria2", label: "Hysteria2" },
    { value: "tuic", label: "TUIC" },
    { value: "socks", label: "SOCKS" },
    { value: "http", label: "HTTP" },
  ];

  function getDefaultFormData(type: ProxyProtocol = "vless") {
    const base = {
      name: "",
      address: "",
      port: 443,
      settings: {
        network: "tcp",
        tls: { enabled: false, allow_insecure: false, fragment: false, record_fragment: false },
        multiplex: { enabled: false, tcp_brutal: false },
        advanced: {
          enabled: false,
          reuse_address: false,
          tcp_fast_open: false,
          udp_fragment: false,
          tcp_multi_path: false,
          disable_sni: false,
          enable_ech: false,
        },
        download: {
          enabled: false,
          security: "none",
          allow_insecure: false,
        },
      },
    };

    const protocolFieldsMap: Record<ProxyProtocol, any> = {
      vless: {
        uuid: "",
        flow: "",
        packet_encoding: "",
      },
      vmess: {
        uuid: "",
        alter_id: 0,
        security: "auto",
        packet_encoding: "",
      },
      trojan: {
        password: "",
      },
      shadowsocks: {
        encryption: "aes-256-gcm",
        password: "",
        plugin: "",
        plugin_args: "",
        udp_over_tcp: false,
      },
      hysteria2: {
        password: "",
        up_mbps: 50,
        down_mbps: 100,
        obf_password: "",
        server_ports: "",
        hop_interval: "",
      },
      tuic: {
        uuid: "",
        password: "",
        congestion_control: "cubic",
        udp_relay_mode: "native",
        udp_over_stream: false,
        zero_rtt_handshake: false,
        heartbeat: "",
      },
      socks: {
        version: 5,
        username: "",
        password: "",
      },
      http: {
        username: "",
        password: "",
      },
      chain: {
        proxies: [],
      },
    };

    return {
      type,
      ...base,
      protocolFields: protocolFieldsMap[type] || {},
    };
  }

  const rawForm = superForm(
    {
      type: "http",
      name: "",
      address: "",
      port: 443,
      protocolFields: {},
      settings: {},
    },
    {
      SPA: true,
      validators: zod4(proxyFormSchema as any),
      dataType: "json",
      onUpdate({ form: f }) {
        if (f.valid) {
          handleSave(f.data as ProxyFormSchema);
        }
      },
    },
  ) as unknown as ReturnType<typeof superForm<ProxyFormSchema>>;

  const { form: formData, enhance, reset } = rawForm;

  setContext("proxyForm", rawForm);

  let selectedProtocol = $derived($formData.type as ProxyProtocol);

  const protocolSupportsTls = (p: ProxyProtocol | null): boolean => {
    return (
      p === "vless" ||
      p === "vmess" ||
      p === "trojan" ||
      p === "hysteria2" ||
      p === "tuic" ||
      p === "http"
    );
  };

  const protocolSupportsNetwork = (p: ProxyProtocol | null): boolean => {
    return p === "vless" || p === "vmess" || p === "trojan";
  };

  const protocolSupportsMultiplex = (p: ProxyProtocol | null): boolean => {
    return (
      p === "vless" || p === "vmess" || p === "trojan" || p === "shadowsocks"
    );
  };

  const protocolSupportsAdvanced = (p: ProxyProtocol | null): boolean => {
    return p !== "http";
  };

  const protocolSupportsSettings = (p: ProxyProtocol | null): boolean => {
    return (
      protocolSupportsNetwork(p) ||
      protocolSupportsTls(p) ||
      protocolSupportsMultiplex(p) ||
      protocolSupportsAdvanced(p)
    );
  };

  const protocolRequiresTls = (p: ProxyProtocol | null): boolean => {
    return p === "hysteria2" || p === "tuic";
  };

  function selectProtocol(proto: ProxyProtocol) {
    if (mode === "edit") return;

    const defaults = getDefaultFormData(proto);
    let newSettings: any = {};

    if (protocolRequiresTls(proto)) {
      newSettings = {
        tls: {
          enabled: true,
          allow_insecure: false,
          fragment: false,
          record_fragment: false,
        },
      };
    }

    if (protocolSupportsNetwork(proto)) {
      newSettings.network = "tcp";
    }

    if (protocolSupportsMultiplex(proto)) {
      newSettings.multiplex = {
        enabled: false,
        tcp_brutal: false,
      };
    }

    if (protocolSupportsAdvanced(proto)) {
      newSettings.advanced = {
        enabled: false,
        reuse_address: false,
        tcp_fast_open: false,
        udp_fragment: false,
        tcp_multi_path: false,
        disable_sni: false,
        enable_ech: false,
      };
    }

    reset({
      data: {
        ...$formData,
        type: proto,
        protocolFields: defaults.protocolFields,
        settings: newSettings,
      } as any,
    });
  }

  function handleSave(data: ProxyFormSchema) {
    const fields = applyProtocolDefaults(data).protocolFields;
    const config: ProxyServerConfig = {
      tag: proxy?.tag || "",
      name: data.name,
      address: data.address,
      port: data.port,
      type: data.type as ProxyProtocol,
      ...fields,
    };

    if (
      data.settings?.network &&
      protocolSupportsNetwork(data.type as ProxyProtocol)
    ) {
      config.network = data.settings.network;
      const path = data.settings.path?.trim();
      const host = data.settings.host?.trim();
      const mode = data.settings.mode?.trim();
      const service_name = data.settings.service_name?.trim();
      const dl = data.settings.download;
      const download =
        data.settings.network === "xhttp" && dl?.enabled
          ? {
              address: dl.address?.trim() || null,
              port: dl.port || null,
              path: dl.path?.trim() || null,
              host: dl.host?.trim() || null,
              detour: dl.detour?.trim() || null,
              security:
                dl.security && dl.security !== "none" ? dl.security : null,
              sni: dl.sni?.trim() || null,
              allow_insecure: dl.allow_insecure || false,
              alpn: dl.alpn
                ? dl.alpn
                    .split(",")
                    .map((s) => s.trim())
                    .filter(Boolean)
                : null,
              fingerprint: dl.fingerprint?.trim() || null,
              reality_pbk: dl.reality_pbk?.trim() || null,
              reality_sid: dl.reality_sid?.trim() || null,
            }
          : null;
      if (path || host || mode || service_name || download) {
        config.transport = {
          path: path || null,
          host: host || null,
          mode: mode || null,
          service_name: service_name || null,
          download,
        };
      }
    }

    if (
      protocolSupportsTls(data.type as ProxyProtocol) &&
      (data.settings?.tls?.enabled ||
        protocolRequiresTls(data.type as ProxyProtocol))
    ) {
      const t = data.settings?.tls;
      config.tls = {
        allow_insecure: t?.allow_insecure || false,
        sni: t?.sni || null,
        alpn: t?.alpn
          ? t.alpn
              .split(",")
              .map((s) => s.trim())
              .filter(Boolean)
          : null,
        fragment: t?.fragment || false,
        record_fragment: t?.record_fragment || false,
        fallback_delay: t?.fallback_delay || null,
        fingerprint: t?.fingerprint || null,
        reality_pbk: t?.reality_pbk || null,
        reality_sid: t?.reality_sid || null,
      };
    }

    if (
      data.settings?.multiplex?.enabled &&
      protocolSupportsMultiplex(data.type as ProxyProtocol)
    ) {
      config.mux = true;
      config.tcp_brutal = data.settings.multiplex.tcp_brutal || false;
      config.brutal_dl_speed =
        data.settings.multiplex.brutal_download_speed || null;
      config.brutal_up_speed =
        data.settings.multiplex.brutal_upload_speed || null;
    } else if (protocolSupportsMultiplex(data.type as ProxyProtocol)) {
      config.mux = false;
      config.tcp_brutal = null;
      config.brutal_dl_speed = null;
      config.brutal_up_speed = null;
    }

    if (
      data.settings?.advanced?.enabled &&
      protocolSupportsAdvanced(data.type as ProxyProtocol)
    ) {
      config.advanced = true;
      config.reuse_address = data.settings.advanced.reuse_address || false;
      config.tcp_fast_open = data.settings.advanced.tcp_fast_open || false;
      config.udp_fragment = data.settings.advanced.udp_fragment || false;
      config.tcp_multi_path = data.settings.advanced.tcp_multi_path || false;
      config.connect_timeout = data.settings.advanced.connect_timeout || null;

      config.tls_disable_sni = data.settings.advanced.disable_sni || false;
      config.tls_min_version = data.settings.advanced.tls_min_version || null;
      config.tls_max_version = data.settings.advanced.tls_max_version || null;
      config.tls_enable_ech = data.settings.advanced.enable_ech || false;
      config.tls_ech_config = data.settings.advanced.ech_config || null;
      config.tls_certificate_sha256 =
        data.settings.advanced.certificate_sha256 || null;
      config.tls_client_cert = data.settings.advanced.client_cert || null;
      config.tls_client_key = data.settings.advanced.client_key || null;
    } else if (protocolSupportsAdvanced(data.type as ProxyProtocol)) {
      config.advanced = false;
      config.reuse_address = null;
      config.tcp_fast_open = null;
      config.udp_fragment = null;
      config.tcp_multi_path = null;
      config.connect_timeout = null;
      config.tls_disable_sni = null;
      config.tls_min_version = null;
      config.tls_max_version = null;
      config.tls_enable_ech = null;
      config.tls_ech_config = null;
      config.tls_certificate_sha256 = null;
      config.tls_client_cert = null;
      config.tls_client_key = null;
    }

    onSave(config, data.name);
  }

  $effect(() => {
    if (open && mode && (mode === "create" || proxy)) {
      untrack(() => {
        if (mode === "create") {
          reset({ data: getDefaultFormData("vless") as ProxyFormSchema });
        } else if (mode === "edit" && proxy && proxy.config) {
          const c = proxy.config;
          let tlsData = undefined;
          if (c.tls) {
            tlsData = {
              enabled: true,
              allow_insecure: c.tls.allow_insecure || false,
              sni: c.tls.sni || "",
              alpn: c.tls.alpn?.join(",") || "",
              fragment: c.tls.fragment || false,
              record_fragment: c.tls.record_fragment || false,
              fallback_delay: c.tls.fallback_delay || "",
              fingerprint: c.tls.fingerprint || "",
              reality_pbk: c.tls.reality_pbk || "",
              reality_sid: c.tls.reality_sid || "",
            };
          } else if (protocolRequiresTls(c.type)) {
            tlsData = {
              enabled: true,
              allow_insecure: false,
              sni: "",
              alpn: "",
              fragment: false,
              record_fragment: false,
              fallback_delay: "",
              fingerprint: "",
              reality_pbk: "",
              reality_sid: "",
            };
          }

          const dl = c.transport?.download;
          const settingsData = {
            network: c.network || "tcp",
            path: c.transport?.path || "",
            host: c.transport?.host || "",
            mode: c.transport?.mode || "",
            service_name: c.transport?.service_name || "",
            download: {
              enabled: !!dl,
              address: dl?.address || "",
              port: dl?.port || undefined,
              path: dl?.path || "",
              host: dl?.host || "",
              detour: dl?.detour || "",
              security: (dl?.security as any) || "none",
              sni: dl?.sni || "",
              allow_insecure: !!dl?.allow_insecure,
              alpn: dl?.alpn?.join(",") || "",
              fingerprint: dl?.fingerprint || "",
              reality_pbk: dl?.reality_pbk || "",
              reality_sid: dl?.reality_sid || "",
            },
            tls: tlsData,
            multiplex: {
              enabled: !!c.mux,
              tcp_brutal: c.tcp_brutal || false,
              brutal_download_speed: c.brutal_dl_speed || undefined,
              brutal_upload_speed: c.brutal_up_speed || undefined,
            },
            advanced: {
              enabled: !!c.advanced,
              reuse_address: c.reuse_address || false,
              tcp_fast_open: c.tcp_fast_open || false,
              udp_fragment: c.udp_fragment || false,
              tcp_multi_path: c.tcp_multi_path || false,
              connect_timeout: c.connect_timeout || undefined,
              disable_sni: c.tls_disable_sni || false,
              tls_min_version: c.tls_min_version || "",
              tls_max_version: c.tls_max_version || "",
              enable_ech: c.tls_enable_ech || false,
              ech_config: c.tls_ech_config || "",
              certificate_sha256: c.tls_certificate_sha256 || "",
              client_cert: c.tls_client_cert || "",
              client_key: c.tls_client_key || "",
            },
          };

          let protoFields: any = {};
          switch (c.type) {
            case "vless":
              protoFields = {
                uuid: c.uuid || "",
                flow: c.flow || "",
                packet_encoding: c.packet_encoding || "",
              };
              break;
            case "vmess":
              protoFields = {
                uuid: c.uuid || "",
                alter_id: c.alter_id || 0,
                security: c.security || "auto",
                packet_encoding: c.packet_encoding || "",
              };
              break;
            case "trojan":
              protoFields = { password: c.password || "" };
              break;
            case "shadowsocks":
              protoFields = {
                encryption: c.encryption || "aes-256-gcm",
                password: c.password || "",
                plugin: c.plugin || "",
                plugin_args: c.plugin_args || "",
                udp_over_tcp: c.udp_over_tcp || false,
              };
              break;
            case "hysteria2":
              protoFields = {
                password: c.password || "",
                up_mbps: c.up_mbps || 50,
                down_mbps: c.down_mbps || 100,
                obf_password: c.obf_password || "",
                server_ports: c.server_ports || "",
                hop_interval: c.hop_interval || "",
              };
              break;
            case "tuic":
              protoFields = {
                uuid: c.uuid || "",
                password: c.password || "",
                congestion_control: c.congestion_control || "cubic",
                udp_relay_mode: c.udp_relay_mode || "native",
                udp_over_stream: c.udp_over_stream || false,
                zero_rtt_handshake: c.zero_rtt_handshake || false,
                heartbeat: c.heartbeat || "",
              };
              break;
            case "socks":
              protoFields = {
                version: c.version || 5,
                username: c.username || "",
                password: c.password || "",
              };
              break;
            case "http":
              protoFields = {
                username: c.username || "",
                password: c.password || "",
              };
              break;
          }

          reset({
            data: {
              type: c.type,
              name: c.name || proxy.title || "",
              address: c.address || "",
              port: c.port || 443,
              settings: settingsData,
              protocolFields: protoFields,
            },
          } as any);
        }
      });
    }
  });
</script>

<form method="POST" use:enhance id="proxy-form">
  <Sheet.Root bind:open>
    <Sheet.Content
      side="right"
      class="flex flex-col gap-0 px-1 transition-all duration-300 {($formData
        .settings?.tls?.enabled ||
        protocolRequiresTls(selectedProtocol)) &&
      protocolSupportsTls(selectedProtocol)
        ? 'w-3xl! max-w-3xl!'
        : ''}"
    >
      <ScrollArea class="h-0 flex-1 min-h-0">
        <Sheet.Header class="px-3">
          <Sheet.Title
            >{mode === "create" ? "Add new Proxy" : "Edit Proxy"}</Sheet.Title
          >
          <Sheet.Description>
            Configure the proxy settings and protocol below.
          </Sheet.Description>
        </Sheet.Header>
        <div
          class="grid gap-3 px-3 {($formData.settings?.tls?.enabled ||
            protocolRequiresTls(selectedProtocol)) &&
          protocolSupportsTls(selectedProtocol)
            ? 'md:grid-cols-2'
            : 'grid-cols-1'}"
        >
          <fieldset class="flex flex-col gap-3">
            <Form.Field form={rawForm} name="type">
              <Form.Control>
                {#snippet children({ props })}
                  <Form.Label>Type</Form.Label>
                  <Select.Root
                    type="single"
                    name={props.name}
                    value={selectedProtocol}
                    onValueChange={(value) =>
                      selectProtocol(value as ProxyProtocol)}
                  >
                    <Select.Trigger
                      {...props}
                      disabled={mode === "edit"}
                      class="w-full"
                    >
                      {protocols.find((p) => p.value === selectedProtocol)
                        ?.label || "Select a protocol..."}
                    </Select.Trigger>
                    <Select.Content>
                      <Select.Group>
                        {#each protocols as proto}
                          <Select.Item value={proto.value} label={proto.label}
                            >{proto.label}</Select.Item
                          >
                        {/each}
                      </Select.Group>
                    </Select.Content>
                  </Select.Root>
                {/snippet}
              </Form.Control>
            </Form.Field>
            <Form.Field form={rawForm} name="name">
              <Form.Control>
                {#snippet children({ props })}
                  <Form.Label>Name</Form.Label>
                  <Input
                    {...props}
                    bind:value={$formData.name}
                    placeholder="My Fast Server"
                  />
                {/snippet}
              </Form.Control>
              <Form.FieldErrors />
            </Form.Field>

            <div class="grid grid-cols-[1fr_100px] gap-3">
              <Form.Field form={rawForm as any} name="address">
                <Form.Control>
                  {#snippet children({ props })}
                    <Form.Label>Address</Form.Label>
                    <Input
                      {...props}
                      bind:value={$formData.address}
                      placeholder="server.example.com or IP"
                    />
                  {/snippet}
                </Form.Control>
                <Form.FieldErrors />
              </Form.Field>
              <Form.Field form={rawForm as any} name="port">
                <Form.Control>
                  {#snippet children({ props })}
                    <Form.Label>Port</Form.Label>
                    <Input
                      {...props}
                      type="number"
                      bind:value={$formData.port}
                      min="1"
                      max="65535"
                    />
                  {/snippet}
                </Form.Control>
                <Form.FieldErrors />
              </Form.Field>
            </div>

            <div
              class="grid grid-cols-[min-content_1fr] items-center gap-4 py-1"
            >
              <p class="text-xs uppercase text-muted-foreground">
                {selectedProtocol}
              </p>
              <Separator></Separator>
            </div>

            <!-- Protocol-Specific Fields -->
            {#if selectedProtocol === "vless"}
              <VlessForm />
            {:else if selectedProtocol === "vmess"}
              <VmessForm />
            {:else if selectedProtocol === "trojan"}
              <TrojanForm />
            {:else if selectedProtocol === "shadowsocks"}
              <ShadowsocksForm />
            {:else if selectedProtocol === "hysteria2"}
              <Hysteria2Form />
            {:else if selectedProtocol === "tuic"}
              <TuicForm />
            {:else if selectedProtocol === "socks"}
              <SocksForm />
            {:else if selectedProtocol === "http"}
              <HttpForm />
            {/if}

            {#if protocolSupportsSettings(selectedProtocol)}
              <div
                class="grid grid-cols-[min-content_1fr] items-center gap-4 py-1"
              >
                <p class="text-xs uppercase text-muted-foreground">settings</p>
                <Separator class="w-auto!"></Separator>
              </div>
            {/if}

            {#if protocolSupportsNetwork(selectedProtocol)}
              <Form.Field form={rawForm} name="settings.network">
                <Form.Control>
                  {#snippet children({ props })}
                    <Form.Label>Network</Form.Label>
                    <Select.Root
                      type="single"
                      bind:value={$formData.settings.network}
                      name={props.name}
                    >
                      <Select.Trigger class="w-full" {...props}>
                        {$formData.settings?.network || "tcp"}
                      </Select.Trigger>
                      <Select.Content>
                        <Select.Item value="tcp" label="TCP">TCP</Select.Item>
                        <Select.Item value="ws" label="WebSocket (WS)"
                          >WebSocket (WS)</Select.Item
                        >
                        <Select.Item value="grpc" label="gRPC">gRPC</Select.Item
                        >
                        <Select.Item value="http" label="HTTP">HTTP</Select.Item
                        >
                        <Select.Item value="httpupgrade" label="HTTPUpgrade"
                          >HTTPUpgrade</Select.Item
                        >
                        <Select.Item value="xhttp" label="XHTTP"
                          >XHTTP</Select.Item
                        >
                        <Select.Item value="quic" label="QUIC">QUIC</Select.Item
                        >
                      </Select.Content>
                    </Select.Root>
                  {/snippet}
                </Form.Control>
                <Form.FieldErrors />
              </Form.Field>
            {/if}

            {#if protocolSupportsNetwork(selectedProtocol) && $formData.settings?.network && $formData.settings.network !== "tcp" && $formData.settings.network !== "quic"}
              <Form.Field form={rawForm} name="settings.path">
                <Form.Control>
                  {#snippet children({ props })}
                    <Form.Label>Path / Service</Form.Label>
                    <Input
                      {...props}
                      bind:value={$formData.settings.path}
                      placeholder={$formData.settings.network === "grpc"
                        ? "service name"
                        : "/path"}
                    />
                  {/snippet}
                </Form.Control>
                <Form.FieldErrors />
              </Form.Field>
              {#if $formData.settings.network === "ws" || $formData.settings.network === "http" || $formData.settings.network === "httpupgrade" || $formData.settings.network === "xhttp"}
                <Form.Field form={rawForm} name="settings.host">
                  <Form.Control>
                    {#snippet children({ props })}
                      <Form.Label>Host</Form.Label>
                      <Input
                        {...props}
                        bind:value={$formData.settings.host}
                        placeholder="example.com"
                      />
                    {/snippet}
                  </Form.Control>
                  <Form.FieldErrors />
                </Form.Field>
              {/if}
              {#if $formData.settings.network === "xhttp"}
                <Form.Field form={rawForm} name="settings.mode">
                  <Form.Control>
                    {#snippet children({ props })}
                      <Form.Label>XHTTP Mode</Form.Label>
                      <Select.Root
                        type="single"
                        bind:value={$formData.settings.mode}
                        name={props.name}
                      >
                        <Select.Trigger class="w-full" {...props}>
                          {$formData.settings?.mode || "auto"}
                        </Select.Trigger>
                        <Select.Content>
                          <Select.Item value="auto" label="auto">auto</Select.Item>
                          <Select.Item value="packet-up" label="packet-up"
                            >packet-up</Select.Item
                          >
                          <Select.Item value="stream-up" label="stream-up"
                            >stream-up</Select.Item
                          >
                          <Select.Item value="stream-one" label="stream-one"
                            >stream-one</Select.Item
                          >
                          <Select.Item value="stream-down" label="stream-down"
                            >stream-down</Select.Item
                          >
                        </Select.Content>
                      </Select.Root>
                    {/snippet}
                  </Form.Control>
                  <Form.FieldErrors />
                </Form.Field>

                <Form.Field
                  form={rawForm}
                  name="settings.download.enabled"
                  class="flex items-center"
                >
                  <Form.Control>
                    {#snippet children({ props })}
                      <Form.Label class="w-full">Download Settings</Form.Label>
                      <Switch
                        name={props.name}
                        checked={!!$formData.settings?.download?.enabled}
                        onCheckedChange={(checked) => {
                          if (checked) {
                            $formData.settings.download = {
                              enabled: true,
                              address: "",
                              path: "",
                              host: "",
                              detour: "",
                              security: "none",
                              sni: "",
                              allow_insecure: false,
                              alpn: "",
                              fingerprint: "",
                              reality_pbk: "",
                              reality_sid: "",
                            };
                          } else if ($formData.settings.download) {
                            $formData.settings.download.enabled = false;
                          }
                        }}
                      />
                    {/snippet}
                  </Form.Control>
                  <Form.FieldErrors />
                </Form.Field>

                {#if $formData.settings?.download?.enabled}
                  {@const dlForm = $formData.settings.download}
                  <Form.Field form={rawForm} name="settings.download.address">
                    <Form.Control>
                      {#snippet children({ props })}
                        <Form.Label>DL Address</Form.Label>
                        <Input
                          {...props}
                          value={dlForm?.address ?? ""}
                          oninput={(e) => {
                            if ($formData.settings?.download)
                              $formData.settings.download.address =
                                e.currentTarget.value;
                          }}
                          placeholder="optional host"
                        />
                      {/snippet}
                    </Form.Control>
                    <Form.FieldErrors />
                  </Form.Field>
                  <Form.Field form={rawForm} name="settings.download.port">
                    <Form.Control>
                      {#snippet children({ props })}
                        <Form.Label>DL Port</Form.Label>
                        <Input
                          {...props}
                          type="number"
                          value={dlForm?.port ?? ""}
                          oninput={(e) => {
                            if ($formData.settings?.download) {
                              const n = Number(e.currentTarget.value);
                              $formData.settings.download.port = Number.isFinite(n)
                                ? n
                                : undefined;
                            }
                          }}
                          placeholder="443"
                        />
                      {/snippet}
                    </Form.Control>
                    <Form.FieldErrors />
                  </Form.Field>
                  <Form.Field form={rawForm} name="settings.download.path">
                    <Form.Control>
                      {#snippet children({ props })}
                        <Form.Label>DL Path</Form.Label>
                        <Input
                          {...props}
                          value={dlForm?.path ?? ""}
                          oninput={(e) => {
                            if ($formData.settings?.download)
                              $formData.settings.download.path =
                                e.currentTarget.value;
                          }}
                          placeholder="/path"
                        />
                      {/snippet}
                    </Form.Control>
                    <Form.FieldErrors />
                  </Form.Field>
                  <Form.Field form={rawForm} name="settings.download.host">
                    <Form.Control>
                      {#snippet children({ props })}
                        <Form.Label>DL Host</Form.Label>
                        <Input
                          {...props}
                          value={dlForm?.host ?? ""}
                          oninput={(e) => {
                            if ($formData.settings?.download)
                              $formData.settings.download.host =
                                e.currentTarget.value;
                          }}
                          placeholder="example.com"
                        />
                      {/snippet}
                    </Form.Control>
                    <Form.FieldErrors />
                  </Form.Field>
                  <Form.Field form={rawForm} name="settings.download.detour">
                    <Form.Control>
                      {#snippet children({ props })}
                        <Form.Label>DL Detour</Form.Label>
                        <Input
                          {...props}
                          value={dlForm?.detour ?? ""}
                          oninput={(e) => {
                            if ($formData.settings?.download)
                              $formData.settings.download.detour =
                                e.currentTarget.value;
                          }}
                          placeholder="direct"
                        />
                      {/snippet}
                    </Form.Control>
                    <Form.FieldErrors />
                  </Form.Field>
                  <Form.Field form={rawForm} name="settings.download.security">
                    <Form.Control>
                      {#snippet children({ props })}
                        <Form.Label>DL Security</Form.Label>
                        <Select.Root
                          type="single"
                          value={dlForm?.security ?? "none"}
                          onValueChange={(v) => {
                            if ($formData.settings?.download)
                              $formData.settings.download.security = v as any;
                          }}
                          name={props.name}
                        >
                          <Select.Trigger class="w-full" {...props}>
                            {dlForm?.security || "none"}
                          </Select.Trigger>
                          <Select.Content>
                            <Select.Item value="none" label="none">none</Select.Item>
                            <Select.Item value="tls" label="tls">tls</Select.Item>
                            <Select.Item value="reality" label="reality"
                              >reality</Select.Item
                            >
                          </Select.Content>
                        </Select.Root>
                      {/snippet}
                    </Form.Control>
                    <Form.FieldErrors />
                  </Form.Field>
                  {#if dlForm?.security === "tls" || dlForm?.security === "reality"}
                    <Form.Field form={rawForm} name="settings.download.sni">
                      <Form.Control>
                        {#snippet children({ props })}
                          <Form.Label>DL SNI</Form.Label>
                          <Input
                            {...props}
                            value={dlForm?.sni ?? ""}
                            oninput={(e) => {
                              if ($formData.settings?.download)
                                $formData.settings.download.sni =
                                  e.currentTarget.value;
                            }}
                            placeholder="sni.example.com"
                          />
                        {/snippet}
                      </Form.Control>
                      <Form.FieldErrors />
                    </Form.Field>
                    <Form.Field form={rawForm} name="settings.download.fingerprint">
                      <Form.Control>
                        {#snippet children({ props })}
                          <Form.Label>DL Fingerprint</Form.Label>
                          <Input
                            {...props}
                            value={dlForm?.fingerprint ?? ""}
                            oninput={(e) => {
                              if ($formData.settings?.download)
                                $formData.settings.download.fingerprint =
                                  e.currentTarget.value;
                            }}
                            placeholder="chrome"
                          />
                        {/snippet}
                      </Form.Control>
                      <Form.FieldErrors />
                    </Form.Field>
                    <Form.Field form={rawForm} name="settings.download.alpn">
                      <Form.Control>
                        {#snippet children({ props })}
                          <Form.Label>DL ALPN</Form.Label>
                          <Input
                            {...props}
                            value={dlForm?.alpn ?? ""}
                            oninput={(e) => {
                              if ($formData.settings?.download)
                                $formData.settings.download.alpn =
                                  e.currentTarget.value;
                            }}
                            placeholder="h2,http/1.1"
                          />
                        {/snippet}
                      </Form.Control>
                      <Form.FieldErrors />
                    </Form.Field>
                    <Form.Field
                      form={rawForm}
                      name="settings.download.allow_insecure"
                      class="flex items-center"
                    >
                      <Form.Control>
                        {#snippet children({ props })}
                          <Form.Label class="w-full">DL Allow Insecure</Form.Label>
                          <Switch
                            name={props.name}
                            checked={!!dlForm?.allow_insecure}
                            onCheckedChange={(checked) => {
                              if ($formData.settings?.download) {
                                $formData.settings.download.allow_insecure =
                                  checked;
                              }
                            }}
                          />
                        {/snippet}
                      </Form.Control>
                      <Form.FieldErrors />
                    </Form.Field>
                  {/if}
                  {#if dlForm?.security === "reality"}
                    <Form.Field form={rawForm} name="settings.download.reality_pbk">
                      <Form.Control>
                        {#snippet children({ props })}
                          <Form.Label>DL Reality PBK</Form.Label>
                          <Input
                            {...props}
                            value={dlForm?.reality_pbk ?? ""}
                            oninput={(e) => {
                              if ($formData.settings?.download)
                                $formData.settings.download.reality_pbk =
                                  e.currentTarget.value;
                            }}
                          />
                        {/snippet}
                      </Form.Control>
                      <Form.FieldErrors />
                    </Form.Field>
                    <Form.Field form={rawForm} name="settings.download.reality_sid">
                      <Form.Control>
                        {#snippet children({ props })}
                          <Form.Label>DL Reality SID</Form.Label>
                          <Input
                            {...props}
                            value={dlForm?.reality_sid ?? ""}
                            oninput={(e) => {
                              if ($formData.settings?.download)
                                $formData.settings.download.reality_sid =
                                  e.currentTarget.value;
                            }}
                          />
                        {/snippet}
                      </Form.Control>
                      <Form.FieldErrors />
                    </Form.Field>
                  {/if}
                {/if}
              {/if}
            {/if}

            {#if protocolSupportsTls(selectedProtocol)}
              <Form.Field
                form={rawForm}
                name="settings.tls.enabled"
                class="flex items-center"
              >
                <Form.Control>
                  {#snippet children({ props })}
                    <Form.Label class="w-full">TLS</Form.Label>
                    <Switch
                      name={props.name}
                      checked={!!$formData.settings?.tls?.enabled ||
                        protocolRequiresTls(selectedProtocol)}
                      disabled={protocolRequiresTls(selectedProtocol)}
                      onCheckedChange={(checked) => {
                        $formData.settings.tls = {
                          enabled: checked,
                          allow_insecure: $formData.settings.tls.allow_insecure,
                          fragment: $formData.settings.tls.fragment,
                          record_fragment: $formData.settings.tls.record_fragment,
                        };
                      }}
                    />
                  {/snippet}
                </Form.Control>
                <Form.FieldErrors />
              </Form.Field>
            {/if}

            {#if protocolSupportsMultiplex(selectedProtocol)}
              <Form.Field
                form={rawForm}
                name="settings.multiplex.enabled"
                class="flex items-center"
              >
                <Form.Control>
                  {#snippet children({ props })}
                    <Form.Label class="w-full">Multiplex</Form.Label>
                    <Switch
                      name={props.name}
                      checked={!!$formData.settings?.multiplex?.enabled}
                      onCheckedChange={(checked) => {
                        if (!$formData.settings.multiplex) {
                          $formData.settings.multiplex = {
                            enabled: false,
                            tcp_brutal: false,
                          };
                        }

                        if (checked) {
                          $formData.settings.multiplex = {
                            enabled: true,
                            tcp_brutal: false,
                          };
                        } else if ($formData.settings.multiplex) {
                          $formData.settings.multiplex.enabled = false;
                        }
                      }}
                    />
                  {/snippet}
                </Form.Control>
                <Form.FieldErrors />
              </Form.Field>

              <!-- Multiplex Settings -->
              {#if !!$formData.settings?.multiplex?.enabled}
                <MultiplexFields />
              {/if}
            {/if}

            {#if protocolSupportsAdvanced(selectedProtocol)}
              <Form.Field
                form={rawForm}
                name="settings.advanced.enabled"
                class="flex items-center"
              >
                <Form.Control>
                  {#snippet children({ props })}
                    <Form.Label class="w-full">Advanced Settings</Form.Label>
                    <Switch
                      name={props.name}
                      checked={!!$formData.settings?.advanced?.enabled}
                      onCheckedChange={(checked) => {
                        if (!$formData.settings.advanced) {
                          $formData.settings.advanced = {
                            enabled: false,
                            reuse_address: false,
                            tcp_fast_open: false,
                            udp_fragment: false,
                            tcp_multi_path: false,
                            disable_sni: false,
                            enable_ech: false,
                          };
                        }

                        if (checked) {
                          $formData.settings.advanced.enabled = true;
                        } else if ($formData.settings.advanced) {
                          $formData.settings.advanced.enabled = false;
                        }
                      }}
                    />
                  {/snippet}
                </Form.Control>
                <Form.FieldErrors />
              </Form.Field>
              <!-- Advanced Settings -->
              {#if !!$formData.settings?.advanced?.enabled}
                <AdvancedFields />
              {/if}
            {/if}
          </fieldset>

          <!-- TLS Settings -->
          {#if protocolSupportsTls(selectedProtocol) && !!$formData.settings?.tls?.enabled}
            <TlsFields />
          {/if}

          <div class="h-4"></div>
        </div></ScrollArea
      >

      <Sheet.Footer
        class="-mx-1 bg-background border-t border-border flex-row justify-end"
      >
        <Button variant="outline" onclick={() => (open = false)}>Cancel</Button>
        <Button
          class="flex-1"
          type="submit"
          form="proxy-form"
          disabled={saving}
        >
          {saving ? "Saving..." : "Save"}
        </Button>
      </Sheet.Footer>
    </Sheet.Content>
  </Sheet.Root>
</form>

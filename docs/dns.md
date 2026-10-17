# DNS behavior and native TUN integration

## Final resolver and server routes

There is one DNS configuration. **Final** is always editable in DNS Rules and
selects a configured DNS server or Local (System) for unmatched real queries.
There is no Automatic/Manual policy switch. Google DoH is seeded as an ordinary,
editable server (`https://8.8.8.8/dns-query`), never injected over a chosen Final.
The IP endpoint avoids a bootstrap DNS query; normal HTTPS certificate verification
remains enabled. Users can edit it or replace Final with any other server.

Each DNS server has its own connection route:

- **Follow connection mode** (default for newly added servers): Global and Rule
  send DNS through the same live root proxy policy as Global traffic. Direct
  connects to that DNS server directly. Changing selected proxies or traffic mode
  takes effect immediately without recompiling DNS. Rule mode does not attempt
  to infer DNS routing from IP-based traffic rules before resolving the name.
- **Direct**, a specific policy, or a specific proxy: use that explicit route,
  regardless of the traffic mode. Legacy omitted routes retain Direct.
- Follow-mode proxy failures do not retry through Direct/Local. When no usable
  proxy exists, its proxy transport is blocked. A policy explicitly selecting
  Direct remains an explicit user choice.

Resolution order is Hosts, enabled user DNS rules in order, local names, and
then Final, with FakeIP for unmatched A/AAAA before Final in Global/Rule. Direct
mode bypasses FakeIP for unmatched requests. User rules remain explicit overrides.
`.local`, `.home.arpa`, localhost, and single-label names stay on Local (System)
unless an earlier explicit Hosts entry or DNS rule overrides them.

Follow-mode servers compile into a proxy transport with the configured server's
tag and a Direct transport named `__dns-direct-{tag}`. Mode-matched Direct variants
are emitted before each user rule targeting such a server, preserving its
precedence. A Direct catch-all for Final precedes FakeIP. The transports share the
same endpoint, port, protocol, and DoH path. Lookup maps an explicitly selected
follow-mode server to its Direct transport when the connection mode is Direct.
Generated IDs appear only in diagnostic Technical details.

Mode/member changes flush real DNS response caches while preserving FakeIP
mappings held by applications. Changing Final, server routes/endpoints, rules, or
FakeIP requires the normal Core restart. The page labels saved settings while a
restart is pending; Lookup uses the running configuration.

Proxy endpoint hostnames bootstrap through Local (System), independently of Final,
so a proxied DNS server cannot depend on itself to connect the first proxy hop.
Explicit endpoint resolver settings are retained. Chained endpoint hostnames keep
remote resolution through their detour. DNS-provider hostnames sent over a proxy
are also passed to that proxy rather than resolved locally. Direct provider
hostnames use an available Direct IP DNS resolver for bootstrap, or Local (System).
An IP-based DNS endpoint avoids this bootstrap lookup.

The unreleased database migrations are consolidated into `0001_init.sql`. The
initial schema seeds Google DoH with Follow connection mode and an editable Final;
there is no retired DNS mode column or intermediate data conversion. New profiles
receive the same defaults. Native exports use schema 2; imports accept schema 1 and materialize its
old defaults, while schema 2 preserves an explicit Local Final without reinserting
Google. The retired DNS mode is never emitted in new exports.

## Configuration

The DNS page supports adding, editing and deleting UDP, TCP, DNS over TLS,
DNS over HTTPS and DNS over QUIC servers. Edits preserve the server tag and its
position: existing rules and the final resolver continue to reference the same
server. Display-only renames do not require a core restart. Changes to an
endpoint or outbound apply after restarting the core.

Addresses are validated before persistence and again during compilation. The
parser preserves explicit ports, IPv6 addresses, and HTTPS paths and queries.
Credentials, fragments, unsupported schemes and invalid ports are rejected.
Importing native profiles uses the same validation. A referenced server cannot
be deleted until its rules and final-resolver selection are changed.

A server's outbound can be Direct, a proxy policy or an individual proxy. For a
hostname endpoint sent through a proxy, the core passes the hostname to that
outbound; the app does not first resolve it using the local network. Direct
hostname endpoints bootstrap through a configured IP-address DNS server, or
Local (System) when none is available. The compiler does not inject an unrelated
public DNS service over Final.

Local (System) is always available as a rule target, even with a remote final
resolver. It uses the underlying network's DNS. It therefore inherits any
filtering or incorrect answers from that network. Select a remote resolver in
the Final DNS rule to use it for otherwise unmatched real queries.

Enabled Local Hosts entries run first, followed by enabled user rules in order.
When FakeIP is enabled, unmatched A/AAAA queries use FakeIP, except that
Direct mode resolves real addresses through Final. Local queries stay on Local (System).
Other query types use the effective real resolver. User
rules can override this fallback. Reverse mapping is enabled for real answers
so routed connections can retain their domain identity.

The Lookup Tool queries the active core configuration. A pending-restart notice
means newly saved changes are not being tested yet. The optional System
comparison makes a separate macOS resolver lookup; the core result itself does
not directly test the operating system's DNS path.

## Which traffic uses these settings?

| Runtime mode | DNS coverage |
| --- | --- |
| TUN enabled | Native macOS system lookups use the temporary TUN resolver. Ordinary port-53 DNS entering the TUN is also handled by the core. |
| System Proxy enabled, TUN disabled | Applications that honor the HTTP/HTTPS/SOCKS proxy send connections to the core, but the operating system's DNS configuration is unchanged. An application can still resolve locally before connecting. A proxy connection carrying a hostname may instead resolve at the remote proxy. |
| Both enabled | TUN covers system DNS; proxy-aware applications can also use the local proxy listener. Enabling System Proxy does not add a second set of DNS rules. |
| Both disabled, core running | The DNS rules remain available to the core and its Lookup Tool. Applications explicitly configured for the local proxy can still use it; other system traffic is unaffected. |
| Core stopped | None of the profile's DNS rules, Local Hosts entries or FakeIP mappings are serving requests. The temporary TUN resolver is removed. |

DNS Rules choose **how to resolve a name**. Traffic rules and Direct/Global/Rule
mode choose **where a connection goes**. DNS servers using Follow connection mode
coordinate their path with the live traffic mode. Explicit Direct/policy/proxy
routes retain their own outbound selection.

An application's own DoH/DoT connection is encrypted traffic. TUN can route that
connection, but the app's encrypted DNS questions do not automatically become
Zeytun DNS Rules queries. FakeIP likewise does not, by itself, capture DNS from
applications outside the core's DNS path.

## Diagnostics and shared controls

**Settings** is the first DNS view and contains FakeIP, the Core cache action,
and the effective saved resolution order. These controls do not repeat above the
other tabs. Final is always selectable in DNS Rules; there is no policy switch.
Server creation/editing exposes Follow connection mode alongside explicit routes.
Each DNS server and authored DNS rule has a trailing vertical-ellipsis menu and
a right-click context menu containing Edit and Delete; header actions retain only
Add (plus rule search). Both menus operate on their own row, regardless of the
previous selection. Referenced-server deletion guards remain in effect, and the
always-enabled Final row retains its resolver selector without a Delete action.

Resolution details is an always-visible rounded panel with wrapping first-match
steps, configuration status, and resolver/route columns. It derives the order,
Final name/address, and that server's route
from the current profile and connection mode. Follow connection mode reflects the
current Global/Rule/Direct path; fixed Direct/policy/proxy routes remain fixed.
FakeIP appears in the order only when effective. Pending/stopped configurations
are labeled Saved and are not presented as already applied to the Core.

Select lists are capped at 16rem (256px), reduced further to the floating layer's
available height. Their internal viewport scrolls; keyboard navigation can reach
the last item without stretching the page. DNS row dropdown/context menus also
use a 16rem/available-height cap and contain overscroll.

Lookup keeps domain, record type, resolver, and Lookup action directly visible,
with response-cache bypass and System comparison below. System comparison is
shown only for automatic DNS routing (the Lookup selector that executes DNS
rules, Hosts and Final) and supports A/AAAA. This Lookup choice is unrelated to
the retired Automatic/Manual policy switch. Result cards show readable resolver
names, while raw runtime IDs and trace fields remain under **Technical details**.

The DNS page now includes the profile's FakeIP switch, response/FakeIP cache
flush action and the effective saved resolution order. These use the same
profile state as Settings; opening Settings must not restore a stale local
FakeIP preference. Changes to FakeIP require a core restart, while flushing acts
on the currently running core and does not flush macOS's cache.

Lookup supports A, AAAA and CNAME, automatic DNS routing or an explicit active
server, and bypassing the response cache for an individual query. An explicit
server skips DNS rules, Hosts and implicit FakeIP routing; response-cache bypass
does not reset FakeIP mappings. Every returned answer, authority and additional
record is preserved with its own name, data, type and TTL.

The native core supplies request-scoped runtime attribution: selected server and
transport, matching compiled rule and its position, configured outbound/policy,
response source and cache-hit state. This metadata belongs to the winning DNS
response, including speculative rule races. Nested hostname-bootstrap queries
are excluded from the parent trace. The configured outbound may itself be a
policy selecting a child proxy; it is not a claim about that child's identity.

The new core exposes `GET /dns/capabilities` and accepts `server` and
`bypass_cache=true` on `GET /dns/query`. The app checks capabilities before using
advanced options so older cores cannot silently ignore an explicit server
selection. Older core responses without trace data remain usable for ordinary
lookups and are shown with attribution unavailable.

System comparison uses macOS's resolver API for A/AAAA without changing DNS
settings. It cannot report wire RCODE, CNAME chains, TTL, cache state or upstream
server; those fields remain unknown rather than being inferred. Blocking OS
lookups are bounded to two simultaneous calls and the UI receives a timeout
after 12 seconds. A timed-out OS call retains its permit until it really exits.
Core diagnostic HTTP requests also have a bounded timeout.

A Core/System mismatch is descriptive, not an automatic diagnosis of poisoning
or a leak: FakeIP, different CDN answers and cache lifetimes can all produce
legitimate differences. Each result/error is independent, so System can succeed
even if Core is stopped or a selected upstream fails.

## macOS system DNS while TUN runs

The native core installs a supplemental catch-all resolver pointing at the TUN
DNS addresses after the TUN and DNS transports have started. This makes native
system lookups, including the Discord updater's, follow the configured DNS
rules. It does not rewrite persistent Wi-Fi or Ethernet DNS settings.

The resolver is owned by a SystemConfiguration dynamic-store session using
`kSCDynamicStoreUseSessionKeys`. Normal shutdown removes it before tearing down
TUN; macOS removes it automatically if the core crashes or receives SIGKILL.
There is no persistent resolver file or periodic cleanup process.

Local (System) selects the scoped DNS configuration for the physical default
interface and excludes the core's registered TUN DNS addresses to prevent a
resolver loop. Existing DNS-generation and interface-change notifications cause
this selection to refresh as the network changes.

This integration requires a macOS CGO production build and administrator
privileges, which native TUN already requires. A macOS core built without CGO
reports an explicit startup error instead of silently leaving system DNS outside
TUN. The integration is skipped for platform-managed TUNs, manual routing, and
explicitly disabled DNS handling. Other platforms retain their existing behavior.

The app and core changes live in separate repositories. Build this core source
before packaging the app; an older bundled binary does not contain the native
system DNS integration:

```sh
make build-core CORE_SRC=/path/to/zeytun-core
```

## Verification

App tests cover endpoint parsing, edit identity, final/rule references, atomic
rejection of invalid edits, Local (System), FakeIP ordering, proxied hostname
bootstrap, and native core config acceptance for all five transports:

```sh
node node_modules/vitest/vitest.mjs run
cd src-tauri
cargo test --all-targets --features updater-tools
# Isolated real-core TLS DNS + two SOCKS exits; requires python3, openssl, localhost sockets:
cargo test --test dns_policy --features updater-tools dns_runtime -- --ignored --nocapture
```

Core checks:

```sh
go test ./adapter ./dns ./experimental/clashapi ./common/systemdns ./dns/transport/local/systemconfig ./protocol/tun
go test -race ./adapter ./dns ./experimental/clashapi ./common/systemdns ./dns/transport/local/systemconfig
```

The opt-in macOS integration test requires root and uses a unique `.invalid`
scope, leaving the system's default DNS route unchanged. It verifies normal and
SIGKILL session-key cleanup:

```sh
go test -c -o /tmp/zeytun-systemdns.test ./common/systemdns
sudo env ZEYTUN_SYSTEMDNS_TEST=1 /tmp/zeytun-systemdns.test -test.run=TestSessionResolverCleanup -test.v
```

A controlled native TUN run also verified system lookup of `updates.discord.com`
and `stable.dl2.discordapp.net`, a Local (System) rule, HTTP 200 from the Discord
app-manifest endpoint without an explicit HTTP proxy, and removal of the native
resolver after both SIGTERM and SIGKILL. Starting TUN replaced a cached poisoned
answer without manually flushing the system cache; HTTPS through a FakeIP
answer also reached the upstream server. Physical Wi-Fi/Ethernet handoff and
sleep/wake still require testing on those actual network transitions.

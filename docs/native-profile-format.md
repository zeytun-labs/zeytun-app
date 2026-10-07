# Native Zeytun Profile Format (.zeytun.json)

## 1. Overview & Format Envelope

Zeytun Native Profiles are self-contained JSON documents with the extension `.zeytun.json`. A profile defines its own routing intent, DNS configuration, rule sets, policy groups, and either independent manual proxies or upstream subscription sources.

```json
{
  "format": "zeytun-profile",
  "schema_version": 1,
  "profile": {
    "name": "Everyday",
    "icon": null,
    "dns": null,
    "sources": [],
    "proxies": [],
    "policies": [],
    "rules": [],
    "rule_sets": []
  },
  "assets": []
}
```

- `format`: Must be exactly `"zeytun-profile"`.
- `schema_version`: `1` (integer). Future versions are rejected with a clear error.
- `assets`: List of managed embedded assets (such as local rule-set files) with base64 data.

## 2. Export Modes

1. **Full Profile (`full`)**:
   - Preserves all subscription sources (including private URLs, intervals, and metadata).
   - Preserves all proxies with their original provenance (`origin: "subscription"`, `source_id`).
   - Warning: Unencrypted file containing private subscription credentials and URLs. Intended for personal backup and cross-device migration.

2. **Offline Snapshot (`snapshot`)**:
   - Strips all subscription definitions (`sources: []`).
   - Converts all subscription-backed proxies to `origin: "manual"` and clears `source_id`.
   - Preserves all proxy configs, tags, policy memberships, weights, selections, and rules identically.
   - Allows sharing a working routing graph without leaking upstream subscription links.

## 3. Privacy & Portability Boundaries

### Included in the Document:
- Profile name and icon
- DNS configuration (servers, fake IP, hosts mapping)
- Routing rules (domain, ip_cidr, process, final)
- Policy groups (manual selector, urltest, balancer with weights and tolerance)
- Manual and subscription-backed proxies (outbound protocol configurations)
- Subscription source definitions (Full mode only)
- Embedded managed assets (local rule sets)

### Excluded from the Document (Device / Runtime State):
- Local proxy listen ports (`socks_port`, `http_port`, `mixed_port`) and `allow_lan`
- System proxy and TUN device settings
- Core logs, traffic analytics, and latency measurements
- Active runtime status and applied revisions
- Volatile session rules and Connection Ask cache
- App theme, autostart preferences, and system tray state

## 4. Policy Domain Independence

Policies in Zeytun only consume proxy and policy **tags**. They have no knowledge of subscription URLs, update intervals, or source IDs.
- Upstream sync adds new proxies to the registry, but never mutates user-authored policy memberships automatically.
- Deleting a subscription source checks for dependencies and offers transactional conversion to Manual nodes (`Keep as manual`), preventing rule or policy breakage.
- Unavailable or failed nodes compile fail-closed (`OutboundType::Block`) rather than leaking traffic directly.

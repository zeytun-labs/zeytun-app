-- Route for the app's own outbound HTTP, per traffic class.
-- JSON-encoded NetworkPolicy. An absent row means Direct — no seeding.
CREATE TABLE IF NOT EXISTS network_policy (
  traffic TEXT PRIMARY KEY,   -- app_update | geoip | subscription
  policy  TEXT NOT NULL       -- {"kind":"direct"} | {"kind":"local_proxy"} | {"kind":"policy","tag":"…"}
);

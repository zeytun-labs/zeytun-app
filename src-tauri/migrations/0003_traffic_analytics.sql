-- Historical traffic, bucketed per minute and categorized by domain / policy
-- (outbound tag) / process, split by proxied vs direct. Rows are upserted in
-- batches from the daemon buffer, so the composite key is the dedup dimension.
CREATE TABLE IF NOT EXISTS traffic_analytics (
  ts_minute  INTEGER NOT NULL, -- epoch seconds truncated to the minute
  domain     TEXT    NOT NULL,
  policy     TEXT    NOT NULL, -- outbound tag the connection was routed through
  process    TEXT    NOT NULL,
  is_proxy   INTEGER NOT NULL, -- 1 = routed through a proxy, 0 = direct
  up_bytes   INTEGER NOT NULL DEFAULT 0,
  down_bytes INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (ts_minute, domain, policy, process, is_proxy)
) WITHOUT ROWID;

CREATE INDEX IF NOT EXISTS idx_traffic_ts ON traffic_analytics (ts_minute);

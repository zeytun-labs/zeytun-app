-- Strategy + weights were written into ProxyPolicy in memory by the UI
-- but never persisted: read_policies hardcoded strategy=None / weights=None
-- and this table had no columns for them, so a restart silently dropped any
-- balancer strategy ("ping") and all member weights.
ALTER TABLE policy ADD COLUMN strategy TEXT;
ALTER TABLE policy ADD COLUMN weights_json TEXT;

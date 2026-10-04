ALTER TABLE local_proxy_config ADD COLUMN connection_ask_enabled INTEGER NOT NULL DEFAULT 0;
ALTER TABLE local_proxy_config ADD COLUMN connection_ask_timeout_ms INTEGER NOT NULL DEFAULT 60000;
ALTER TABLE local_proxy_config ADD COLUMN connection_ask_group_by TEXT NOT NULL DEFAULT 'process';
ALTER TABLE local_proxy_config ADD COLUMN connection_ask_remember_default INTEGER NOT NULL DEFAULT 1;

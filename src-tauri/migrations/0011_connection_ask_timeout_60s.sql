-- Bump connection-ask default hold from 20s → 60s (process group needs more decide time).
UPDATE local_proxy_config
SET connection_ask_timeout_ms = 60000
WHERE connection_ask_timeout_ms = 20000;

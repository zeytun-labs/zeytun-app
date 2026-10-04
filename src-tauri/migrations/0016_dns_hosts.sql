CREATE TABLE IF NOT EXISTS dns_host (
    profile_id TEXT NOT NULL REFERENCES profile(id) ON DELETE CASCADE,
    id TEXT NOT NULL,
    domain TEXT NOT NULL,
    address TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    position INTEGER NOT NULL,
    PRIMARY KEY (profile_id, id)
);

CREATE INDEX IF NOT EXISTS idx_dns_host_profile
    ON dns_host(profile_id, position);

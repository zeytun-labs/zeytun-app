CREATE TABLE IF NOT EXISTS dns_rule (
    profile_id TEXT NOT NULL REFERENCES profile(id) ON DELETE CASCADE,
    id TEXT NOT NULL,
    kind TEXT NOT NULL,
    value TEXT NOT NULL,
    target TEXT NOT NULL,
    comment TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    position INTEGER NOT NULL,
    PRIMARY KEY (profile_id, id)
);

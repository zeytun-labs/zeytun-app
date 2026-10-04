CREATE TABLE temp_route_rule (
    profile_id TEXT NOT NULL REFERENCES profile(id) ON DELETE CASCADE,
    id INTEGER NOT NULL,
    kind TEXT NOT NULL,
    value TEXT NOT NULL,
    outbound TEXT NOT NULL,
    comment TEXT NOT NULL DEFAULT '',
    rule_set TEXT,
    orphaned INTEGER NOT NULL DEFAULT 0,
    expires_at INTEGER NOT NULL,
    position INTEGER NOT NULL,
    PRIMARY KEY (profile_id, id)
);

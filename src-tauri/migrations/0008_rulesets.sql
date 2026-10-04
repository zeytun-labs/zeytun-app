DROP TABLE IF EXISTS rule_set;
CREATE TABLE rule_set (
    profile_id TEXT NOT NULL REFERENCES profile(id) ON DELETE CASCADE,
    id TEXT NOT NULL,
    tag TEXT NOT NULL,
    type TEXT NOT NULL,
    source TEXT NOT NULL,
    action TEXT NOT NULL,
    comment TEXT,
    position INTEGER NOT NULL,
    PRIMARY KEY (profile_id, id)
);

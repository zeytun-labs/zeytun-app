-- Reject inconsistent authored weights before changing any ownership. The
-- migration runner commits this validation, all DDL/backfill and marker together.
CREATE TEMP TABLE subscription_weights_validation (
    valid INTEGER CONSTRAINT policy_weights_must_match_member_count CHECK(valid = 1)
);
INSERT INTO subscription_weights_validation
SELECT CASE WHEN json_valid(weights_json) THEN
    CASE WHEN json_type(weights_json) = 'array' AND json_array_length(weights_json) =
        (SELECT count(*) FROM policy_member m WHERE m.profile_id = p.profile_id AND m.policy_tag = p.tag)
    THEN 1 ELSE 0 END ELSE 0 END
FROM policy p WHERE weights_json IS NOT NULL;
DROP TABLE subscription_weights_validation;

ALTER TABLE profile ADD COLUMN config_revision INTEGER NOT NULL DEFAULT 0;
CREATE TABLE subscription_source (
    profile_id TEXT NOT NULL,
    id TEXT NOT NULL,
    name TEXT NOT NULL,
    url TEXT NOT NULL,
    enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),
    auto_update INTEGER NOT NULL CHECK(auto_update IN (0,1)),
    update_interval_hours INTEGER NOT NULL CHECK(update_interval_hours > 0),
    generation INTEGER NOT NULL CHECK(generation >= 0),
    position INTEGER NOT NULL,
    provider_title TEXT,
    suggested_interval_hours INTEGER,
    userinfo_json TEXT,
    last_attempt_at INTEGER,
    last_success_at INTEGER,
    last_error TEXT,
    last_sync_summary_json TEXT,
    unread_summary INTEGER NOT NULL DEFAULT 0 CHECK(unread_summary IN (0,1)),
    PRIMARY KEY(profile_id,id),
    FOREIGN KEY(profile_id) REFERENCES profile(id) ON DELETE CASCADE
);
INSERT INTO subscription_source (
    profile_id,id,name,url,enabled,auto_update,update_interval_hours,generation,position,
    userinfo_json,last_attempt_at,last_success_at,last_sync_summary_json,unread_summary
)
SELECT id,'legacy',name,subscription_url,1,NOT skip_auto_update,update_interval_hours,0,0,
    CASE WHEN sub_upload IS NOT NULL OR sub_download IS NOT NULL OR sub_total IS NOT NULL OR sub_expire IS NOT NULL
    THEN json_object('upload',coalesce(sub_upload,0),'download',coalesce(sub_download,0),
                     'total',coalesce(sub_total,0),'expire',coalesce(sub_expire,0)) END,
    last_updated_unix_ms,
    CASE WHEN last_sync_summary IS NULL OR
        (json_valid(last_sync_summary) AND coalesce(json_array_length(last_sync_summary,'$.errors'),0)=0)
    THEN last_updated_unix_ms END,
    last_sync_summary,unread_summary
FROM profile WHERE subscription_url IS NOT NULL AND trim(subscription_url) <> '';

-- No table references proxy as an FK parent; rebuilding it cannot cascade the
-- authored members/rules. Sources reference profile, never individual proxies.
ALTER TABLE proxy RENAME TO proxy_legacy;
CREATE TABLE proxy (
    profile_id TEXT NOT NULL,
    tag TEXT NOT NULL,
    origin TEXT NOT NULL,
    source_id TEXT,
    available INTEGER NOT NULL DEFAULT 1 CHECK(available IN (0,1)),
    title TEXT NOT NULL,
    protocol TEXT NOT NULL,
    link TEXT NOT NULL,
    enabled INTEGER NOT NULL,
    transport TEXT,
    config_json TEXT,
    position INTEGER NOT NULL,
    PRIMARY KEY(profile_id,tag),
    CHECK((origin = 'manual' AND source_id IS NULL) OR (origin = 'subscription' AND source_id IS NOT NULL)),
    FOREIGN KEY(profile_id) REFERENCES profile(id) ON DELETE CASCADE,
    FOREIGN KEY(profile_id,source_id) REFERENCES subscription_source(profile_id,id)
);
INSERT INTO proxy (profile_id,tag,origin,source_id,title,protocol,link,enabled,transport,config_json,position)
SELECT p.profile_id,p.tag,
    CASE WHEN p.origin='subscription' AND s.id IS NOT NULL THEN 'subscription' ELSE 'manual' END,
    CASE WHEN p.origin='subscription' THEN s.id END,
    p.title,p.protocol,p.link,p.enabled,p.transport,p.config_json,p.position
FROM proxy_legacy p LEFT JOIN subscription_source s ON p.profile_id=s.profile_id AND s.id='legacy';
DROP TABLE proxy_legacy;
CREATE INDEX idx_proxy_profile ON proxy(profile_id,position);
CREATE INDEX idx_proxy_source ON proxy(profile_id,source_id);

-- Legacy columns remain structurally but are no longer read/written. Adapters
-- read source-owned state; there is no second persisted subscription model.
UPDATE profile SET subscription_url=NULL,last_updated_unix_ms=NULL,last_sync_summary=NULL,
    unread_summary=0,skip_auto_update=0,update_interval_hours=12,
    sub_upload=NULL,sub_download=NULL,sub_total=NULL,sub_expire=NULL;

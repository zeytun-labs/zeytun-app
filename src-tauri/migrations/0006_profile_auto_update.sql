-- Background auto-update settings + subscription usage tracking on the profile
-- registry. All ADD COLUMN (safe on existing rows). Usage columns are nullable
-- and stay NULL until the first sync parses a `subscription-userinfo` header.

ALTER TABLE profile ADD COLUMN skip_auto_update     INTEGER NOT NULL DEFAULT 0;
ALTER TABLE profile ADD COLUMN update_interval_hours INTEGER NOT NULL DEFAULT 12;
ALTER TABLE profile ADD COLUMN sub_upload   INTEGER;
ALTER TABLE profile ADD COLUMN sub_download INTEGER;
ALTER TABLE profile ADD COLUMN sub_total    INTEGER;
ALTER TABLE profile ADD COLUMN sub_expire   INTEGER;

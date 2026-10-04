-- Custom icon for a profile, chosen from a fixed set of hugeicons names
-- defined in the frontend. Nullable — the UI falls back to an
-- initial-letter avatar when unset.

ALTER TABLE profile ADD COLUMN icon TEXT;

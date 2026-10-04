-- route_rule already has enabled (0005). Temp + rulesets get the same.
ALTER TABLE temp_route_rule ADD COLUMN enabled INTEGER NOT NULL DEFAULT 1;
ALTER TABLE rule_set ADD COLUMN enabled INTEGER NOT NULL DEFAULT 1;

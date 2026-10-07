-- Authored graph revisions protect destructive previews independently of
-- runtime config_revision. Telemetry and source status writes do not bump it.
ALTER TABLE profile ADD COLUMN graph_revision INTEGER NOT NULL DEFAULT 0;
CREATE TRIGGER graph_revision_proxy_insert AFTER INSERT ON proxy
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (NEW.profile_id); END;
CREATE TRIGGER graph_revision_proxy_update AFTER UPDATE ON proxy
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (OLD.profile_id,NEW.profile_id); END;
CREATE TRIGGER graph_revision_proxy_delete AFTER DELETE ON proxy
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (OLD.profile_id); END;
CREATE TRIGGER graph_revision_policy_insert AFTER INSERT ON policy
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (NEW.profile_id); END;
CREATE TRIGGER graph_revision_policy_update AFTER UPDATE ON policy
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (OLD.profile_id,NEW.profile_id); END;
CREATE TRIGGER graph_revision_policy_delete AFTER DELETE ON policy
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (OLD.profile_id); END;
CREATE TRIGGER graph_revision_policy_member_insert AFTER INSERT ON policy_member
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (NEW.profile_id); END;
CREATE TRIGGER graph_revision_policy_member_update AFTER UPDATE ON policy_member
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (OLD.profile_id,NEW.profile_id); END;
CREATE TRIGGER graph_revision_policy_member_delete AFTER DELETE ON policy_member
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (OLD.profile_id); END;
CREATE TRIGGER graph_revision_route_rule_insert AFTER INSERT ON route_rule
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (NEW.profile_id); END;
CREATE TRIGGER graph_revision_route_rule_update AFTER UPDATE ON route_rule
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (OLD.profile_id,NEW.profile_id); END;
CREATE TRIGGER graph_revision_route_rule_delete AFTER DELETE ON route_rule
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (OLD.profile_id); END;
CREATE TRIGGER graph_revision_temp_route_rule_insert AFTER INSERT ON temp_route_rule
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (NEW.profile_id); END;
CREATE TRIGGER graph_revision_temp_route_rule_update AFTER UPDATE ON temp_route_rule
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (OLD.profile_id,NEW.profile_id); END;
CREATE TRIGGER graph_revision_temp_route_rule_delete AFTER DELETE ON temp_route_rule
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (OLD.profile_id); END;
CREATE TRIGGER graph_revision_rule_set_insert AFTER INSERT ON rule_set
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (NEW.profile_id); END;
CREATE TRIGGER graph_revision_rule_set_update AFTER UPDATE ON rule_set
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (OLD.profile_id,NEW.profile_id); END;
CREATE TRIGGER graph_revision_rule_set_delete AFTER DELETE ON rule_set
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (OLD.profile_id); END;
CREATE TRIGGER graph_revision_dns_config_insert AFTER INSERT ON dns_config
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (NEW.profile_id); END;
CREATE TRIGGER graph_revision_dns_config_update AFTER UPDATE ON dns_config
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (OLD.profile_id,NEW.profile_id); END;
CREATE TRIGGER graph_revision_dns_config_delete AFTER DELETE ON dns_config
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (OLD.profile_id); END;
CREATE TRIGGER graph_revision_dns_server_insert AFTER INSERT ON dns_server
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (NEW.profile_id); END;
CREATE TRIGGER graph_revision_dns_server_update AFTER UPDATE ON dns_server
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (OLD.profile_id,NEW.profile_id); END;
CREATE TRIGGER graph_revision_dns_server_delete AFTER DELETE ON dns_server
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (OLD.profile_id); END;
CREATE TRIGGER graph_revision_dns_rule_insert AFTER INSERT ON dns_rule
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (NEW.profile_id); END;
CREATE TRIGGER graph_revision_dns_rule_update AFTER UPDATE ON dns_rule
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (OLD.profile_id,NEW.profile_id); END;
CREATE TRIGGER graph_revision_dns_rule_delete AFTER DELETE ON dns_rule
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (OLD.profile_id); END;
CREATE TRIGGER graph_revision_dns_host_insert AFTER INSERT ON dns_host
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (NEW.profile_id); END;
CREATE TRIGGER graph_revision_dns_host_update AFTER UPDATE ON dns_host
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (OLD.profile_id,NEW.profile_id); END;
CREATE TRIGGER graph_revision_dns_host_delete AFTER DELETE ON dns_host
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id IN (OLD.profile_id); END;
CREATE TRIGGER graph_revision_profile AFTER UPDATE OF version,outbound_mode ON profile
WHEN NEW.version IS NOT OLD.version OR NEW.outbound_mode IS NOT OLD.outbound_mode
BEGIN UPDATE profile SET graph_revision=graph_revision+1 WHERE id=NEW.id; END;

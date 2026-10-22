use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::adapters::clash_api::ClashApiClient;
use crate::core::{
    compiler::tag_allocator::allocate_named_tag,
    dto::{
        CorePathsInfo, CorePreflightReport, CoreRuntimeStatus, CreatePolicyInput,
        CreateProfileInput, CreateProxyInput, DnsHostEntry, InboundMode, OutboundMode, ProfileMeta,
        ProxyOrigin, SelectPolicyMemberInput, SyncSummary, UpdateProfileInput, UpdateProxyInput,
        ZeytunCore, DEFAULT_FINAL_OUTBOUND, DEFAULT_PROFILE_ID, DEFAULT_SELECTOR_POLICY_TAG,
    },
    events::EventBus,
    lifecycle::CoreLifecycle,
    link_parser,
    models::proxy::ProxyServer,
    storage::{Db, Storage},
    subscription::SubscriptionManager,
};

const DNS_HOSTS_SERVER_TAG: &str = "local-hosts";

pub struct CoreManager {
    app_data_dir: PathBuf,
    resource_dir: Option<PathBuf>,
    core_dir: PathBuf,
    bin_dir: PathBuf,
    platform_bin_dir: PathBuf,
    runtime_dir: PathBuf,
    config_dir: PathBuf,
    db: Db,
    events: Arc<EventBus>,

    /// The currently-active profile's routing bundle, held in memory. Swapped
    /// wholesale by `profile_switch`. `profile.id` is the active id.
    profile: ZeytunCore,
    lifecycle: CoreLifecycle,
}

impl CoreManager {
    pub fn new(
        app_data_dir: PathBuf,
        resource_dir: Option<PathBuf>,
        events: Arc<EventBus>,
    ) -> Result<Self, crate::error::CommandError> {
        let core_dir = app_data_dir.join("core");
        let bin_dir = core_dir.join("bin");
        let platform_bin_dir = bin_dir.join(platform_key());
        let runtime_dir = core_dir.join("runtime");
        let config_dir = runtime_dir.join("config");
        let db_path = core_dir.join("zeytun.db");
        let db = Db::new(db_path);

        fs::create_dir_all(&platform_bin_dir)
            .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?;
        fs::create_dir_all(&config_dir)
            .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?;

        // The migration guarantees at least the Default profile exists and is
        // active. Load it and persist once so a fresh install's normalized
        // defaults (root policy + final rule) land in the DB.
        let active_id = db.active_profile_id()?;
        let mut profile = db.load_profile(&active_id)?;
        profile.normalize_policy_graph();
        // Drop expired temp rules so compile/start never injects stale matches.
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        profile.temp_rules.retain(|r| !r.is_expired(now_ms));
        db.save_profile(&profile)?;

        let binary_path = if events.is_headless() {
            // Isolated backend fixtures must never find/start the bundled core,
            // whose normal startup performs process cleanup and system routing.
            platform_bin_dir.join("headless-core-not-installed")
        } else {
            // Clear any dangling system proxy settings left by an earlier killed/crashed session
            let _ = crate::adapters::system_proxy::SystemProxyManager.disable();
            resolve_zeytun_core_binary_path(&bin_dir, &platform_bin_dir, resource_dir.as_deref())
        };

        let lifecycle = CoreLifecycle::new(binary_path, config_dir.clone(), events.clone());

        let manager = Self {
            app_data_dir,
            resource_dir,
            core_dir,
            bin_dir,
            platform_bin_dir,
            runtime_dir,
            config_dir,
            db,
            events,
            profile,
            lifecycle,
        };

        Ok(manager)
    }

    pub fn preflight(&self) -> CorePreflightReport {
        let paths = self.paths_info();
        let binary_path = self.lifecycle.process_manager().binary_path();
        let mut report = self.lifecycle.preflight(binary_path);

        report.paths = paths;
        report
    }

    pub fn profile_get(&self) -> &ZeytunCore {
        &self.profile
    }

    pub fn subscription_delete_preview(
        &self,
        profile_id: &str,
        source_id: &str,
    ) -> Result<crate::core::dto::SubscriptionDeletePreview, crate::error::CommandError> {
        let session_rules = if self.profile.id == profile_id {
            self.profile.session_rules.as_slice()
        } else {
            &[]
        };
        Ok(self
            .db
            .subscription_delete_preview(profile_id, source_id, session_rules)?)
    }

    pub fn subscription_delete(
        &mut self,
        input: crate::core::dto::DeleteSubscriptionInput,
    ) -> Result<(), crate::error::CommandError> {
        let session_rules = if self.profile.id == input.profile_id {
            self.profile.session_rules.as_slice()
        } else {
            &[]
        };
        let profile = self.db.delete_subscription(&input, session_rules)?;
        if self.profile.id == input.profile_id {
            self.profile = profile;
        }
        Ok(())
    }

    /// Whether the zeytun-core process is currently running.
    pub fn is_running(&self) -> bool {
        self.lifecycle.is_running()
    }

    pub fn profile_set(&mut self, profile: ZeytunCore) -> Result<(), crate::error::CommandError> {
        let mut profile = profile;
        profile.local_proxy = self.profile.local_proxy.clone();
        profile.normalize_policy_graph();
        crate::core::dns_config::normalize_profile_dns(&mut profile)
            .map_err(crate::error::CommandError::InvalidInput)?;
        validate_profile_dns_rules(&profile)?;
        self.profile = profile;
        self.db.save_profile(&self.profile)?;
        Ok(())
    }

    pub fn reset(&mut self) -> Result<ZeytunCore, crate::error::CommandError> {
        let _ = self.lifecycle.stop();

        if self.runtime_dir.exists() {
            fs::remove_dir_all(&self.runtime_dir)
                .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?;
        }

        fs::create_dir_all(&self.platform_bin_dir)
            .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?;
        fs::create_dir_all(&self.config_dir)
            .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?;

        self.profile = ZeytunCore::default();
        self.profile.normalize_policy_graph();
        self.db.save_local_proxy(&self.profile.local_proxy)?;
        self.db.save_profile(&self.profile)?;
        self.db.set_active(&self.profile.id)?;

        let binary_path = resolve_zeytun_core_binary_path(
            &self.bin_dir,
            &self.platform_bin_dir,
            self.resource_dir.as_deref(),
        );
        self.lifecycle =
            CoreLifecycle::new(binary_path, self.config_dir.clone(), self.events.clone());

        Ok(self.profile.clone())
    }

    pub fn proxy_import_link(
        &mut self,
        link: &str,
    ) -> Result<crate::core::dto::Proxy, crate::error::CommandError> {
        // Manually imported proxies belong to the active profile and survive
        // subscription syncs.
        let preview =
            link_parser::proxy_from_link(link, ProxyOrigin::Manual, "proxy-preview".to_string())?;
        let tag = self.allocate_proxy_tag(&preview.title);
        let proxy = link_parser::proxy_from_link(link, ProxyOrigin::Manual, tag)?;

        self.profile.proxies.insert(0, proxy.clone());

        self.profile.normalize_policy_graph();
        self.db.save_profile(&self.profile)?;
        // Don't auto-reload; let user click Restart Core in topbar
        Ok(proxy)
    }

    pub fn proxy_create(
        &mut self,
        input: CreateProxyInput,
    ) -> Result<crate::core::dto::Proxy, crate::error::CommandError> {
        let server: ProxyServer = serde_json::from_value(input.config).map_err(|e| {
            crate::error::CommandError::InvalidInput(format!("invalid proxy config: {e}"))
        })?;

        let tag = self.allocate_proxy_tag(&input.title);
        let protocol_name = server.protocol.name().to_string();
        let transport = server.protocol.transport();

        let proxy = crate::core::dto::Proxy {
            tag: tag.clone(),
            origin: ProxyOrigin::Manual,
            source_id: None,
            available: true,
            title: input.title,
            protocol: protocol_name,
            link: String::new(),
            enabled: true,
            transport,
            config: serde_json::to_value(&server).ok(),
        };

        self.profile.proxies.insert(0, proxy.clone());

        self.profile.normalize_policy_graph();
        self.db.save_profile(&self.profile)?;
        Ok(proxy)
    }

    pub fn start(&mut self) -> Result<CoreRuntimeStatus, crate::error::CommandError> {
        let preflight = self.preflight();
        if !preflight.ok {
            let message = format!("preflight failed: {}", preflight.errors.join(", "));
            self.events.error("Core failed to start", &message);
            return Err(crate::error::CommandError::Core(message));
        }

        // Session rules die with the core process — never survive a fresh start.
        self.profile.session_rules.clear();

        match self.start_snapshot() {
            Ok(()) => Ok(self.status()),
            Err(e) => {
                self.events.error("Core failed to start", &e);
                Err(crate::error::CommandError::Core(e))
            }
        }
    }

    pub fn stop(&mut self) -> CoreRuntimeStatus {
        self.lifecycle.stop()
    }

    pub fn restart(&mut self) -> Result<CoreRuntimeStatus, crate::error::CommandError> {
        let _ = self.lifecycle.stop();
        self.start()
    }

    pub fn status(&mut self) -> CoreRuntimeStatus {
        let mut status = self.lifecycle.status();
        status.saved_profile_id = self.profile.id.clone();
        status.saved_revision = self.saved_revision(&self.profile.id).unwrap_or(u64::MAX);
        status.pending_apply = status.applied_profile_id.as_deref()
            != Some(self.profile.id.as_str())
            || status.applied_revision != Some(status.saved_revision);
        status
    }

    fn start_snapshot(&mut self) -> Result<(), String> {
        let (mut snapshot, revision) = self
            .db
            .profile_snapshot(&self.profile.id)
            .map_err(|e| e.to_string())?;
        snapshot.session_rules.clear();
        self.lifecycle.start(&snapshot, revision)?;
        self.profile = self
            .db
            .load_profile(&snapshot.id)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    // ----- Profile registry -------------------------------------------------

    /// All profiles, with `is_active` reflecting the current selection.
    pub fn profile_list(&self) -> Result<Vec<ProfileMeta>, crate::error::CommandError> {
        self.db.list_profiles().map_err(Into::into)
    }

    pub fn active_profile_id(&self) -> &str {
        &self.profile.id
    }

    /// Create a new profile. If a subscription URL is given, immediately does a
    /// foreground sync into it. The name is optional: a user-supplied name wins;
    /// otherwise the subscription's `profile-title` header is used; otherwise a
    /// generated fallback. Returns the created meta.
    pub fn profile_create(
        &mut self,
        input: CreateProfileInput,
    ) -> Result<ProfileMeta, crate::error::CommandError> {
        let user_name = input
            .name
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty());
        let url = input.subscription_url.filter(|u| !u.trim().is_empty());

        // Provisional name; may be replaced by the profile-title header below.
        let provisional = user_name
            .clone()
            .unwrap_or_else(|| "New Profile".to_string());
        let id = self.allocate_profile_id(&provisional);
        let meta = ProfileMeta {
            id: id.clone(),
            name: provisional.clone(),
            subscription_url: url.clone(),
            last_updated_unix_ms: None,
            is_active: false,
            last_sync_summary: None,
            unread_summary: false,
            skip_auto_update: input.skip_auto_update.unwrap_or(false),
            update_interval_hours: input.update_interval_hours.unwrap_or(12),
            sub_upload: None,
            sub_download: None,
            sub_total: None,
            sub_expire: None,
            icon: input.icon,
        };
        self.db.create_profile(&meta)?;
        // Seed the routing bundle (default policy + final rule) for the new id.
        let mut bundle = ZeytunCore {
            id: id.clone(),
            ..ZeytunCore::default()
        };
        bundle.normalize_policy_graph();

        self.db.save_profile(&bundle)?;
        // The IPC adapter fetches after releasing the manager lock.

        self.events.info("Profile updated", &meta.name);
        Ok(self
            .db
            .list_profiles()?
            .into_iter()
            .find(|p| p.id == id)
            .unwrap_or(meta))
    }

    pub fn profile_update(
        &mut self,
        input: UpdateProfileInput,
    ) -> Result<ProfileMeta, crate::error::CommandError> {
        let mut meta = self
            .db
            .list_profiles()?
            .into_iter()
            .find(|p| p.id == input.id)
            .ok_or_else(|| {
                crate::error::CommandError::NotFound(format!("profile `{}` not found", input.id))
            })?;

        if let Some(name) = input.name {
            let name = name.trim().to_string();
            if !name.is_empty() {
                meta.name = name;
            }
        }
        if let Some(url) = input.subscription_url {
            meta.subscription_url = url.filter(|u| !u.trim().is_empty());
        }
        if let Some(skip) = input.skip_auto_update {
            meta.skip_auto_update = skip;
        }
        if let Some(hours) = input.update_interval_hours.filter(|h| *h > 0) {
            meta.update_interval_hours = hours;
        }
        if let Some(icon) = input.icon {
            meta.icon = icon;
        }

        let dns_profile = if let Some(dns) = input.dns {
            let mut profile = self.db.load_profile(&input.id)?;
            let mut dns = merge_dns_config(dns, profile.dns.take());
            if let Some(hosts) = dns.hosts.take() {
                let hosts = normalize_dns_hosts(hosts)?;
                dns.hosts = (!hosts.is_empty()).then_some(hosts);
            }
            profile.dns = Some(dns);
            crate::core::dns_config::normalize_profile_dns(&mut profile)
                .map_err(crate::error::CommandError::InvalidInput)?;
            validate_profile_dns_rules(&profile)?;
            Some(profile)
        } else {
            None
        };

        self.db.update_profile_meta(&meta)?;

        if let Some(profile) = dns_profile {
            self.db.save_profile(&profile)?;
            if meta.is_active {
                // Keep in-memory profile in sync so the new DNS config is ready
                // for the next core start/reload triggered by the user via the UI.
                self.profile.dns = profile.dns;
            }
        }

        Ok(meta)
    }

    /// Delete a profile. Blocked when it is the last one (the app must always
    /// have at least one profile). If the active profile is deleted, another is
    /// activated.
    pub fn profile_delete(&mut self, id: &str) -> Result<(), crate::error::CommandError> {
        let profiles = self.db.list_profiles()?;
        if profiles.len() <= 1 {
            return Err(crate::error::CommandError::InvalidInput(
                "cannot delete the last profile".to_string(),
            ));
        }
        if !profiles.iter().any(|p| p.id == id) {
            return Err(crate::error::CommandError::NotFound(format!(
                "profile `{id}` not found"
            )));
        }

        let deleting_active = self.profile.id == id;
        self.db.delete_profile(id)?;

        if deleting_active {
            // Activate the first remaining profile.
            let next = self.db.list_profiles()?.into_iter().next().ok_or_else(|| {
                crate::error::CommandError::State("no profiles remain".to_string())
            })?;
            self.activate(&next.id)?;
        }
        Ok(())
    }

    /// Switch the active profile: regenerate the config from that profile only,
    /// reset to Direct mode (persisted), and restart the core.
    pub fn profile_switch(&mut self, id: &str) -> Result<(), crate::error::CommandError> {
        if !self.db.list_profiles()?.iter().any(|p| p.id == id) {
            return Err(crate::error::CommandError::NotFound(format!(
                "profile `{id}` not found"
            )));
        }
        self.activate(id)
    }

    /// Central activation path used by switch / delete-active. Loads the target
    /// profile, forces Direct mode (persisted), marks it active, and restarts
    /// the core if it was running.
    fn activate(&mut self, id: &str) -> Result<(), crate::error::CommandError> {
        let was_running = self.lifecycle.is_running();
        let _ = self.lifecycle.stop();

        let mut bundle = self.db.load_profile(id)?;
        bundle.outbound_mode = OutboundMode::Direct; // default state on activation
        bundle.normalize_policy_graph();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        bundle.temp_rules.retain(|r| !r.is_expired(now_ms));
        self.db.save_profile(&bundle)?;
        self.db.set_active(id)?;
        self.profile = bundle;

        self.events.info("Profile switched", id);

        if was_running {
            self.start_snapshot()?;
        }
        Ok(())
    }

    pub fn profile_export(
        &self,
        profile_id: &str,
        mode: crate::core::profile_document::ExportMode,
    ) -> Result<String, crate::error::CommandError> {
        let meta = self
            .db
            .list_profiles()?
            .into_iter()
            .find(|p| p.id == profile_id)
            .ok_or_else(|| crate::error::CommandError::NotFound("profile not found".into()))?;
        let bundle = self.db.load_profile(profile_id)?;
        let sources = self.db.list_subscriptions(profile_id)?;
        crate::core::profile_document::ProfileDocument::encode(
            &meta.name,
            meta.icon,
            &bundle,
            &sources,
            mode,
            vec![],
        )
        .map_err(crate::error::CommandError::InvalidInput)
    }

    pub fn profile_import_preview(
        &self,
        raw: &str,
    ) -> Result<crate::core::profile_document::ImportPreview, crate::error::CommandError> {
        let doc = crate::core::profile_document::ProfileDocument::decode(raw)
            .map_err(crate::error::CommandError::InvalidInput)?;
        Ok(doc.preview())
    }

    pub fn profile_import_confirm(
        &mut self,
        raw: &str,
        _preview_token: &str,
        custom_name: Option<String>,
    ) -> Result<ProfileMeta, crate::error::CommandError> {
        let doc = crate::core::profile_document::ProfileDocument::decode(raw)
            .map_err(crate::error::CommandError::InvalidInput)?;
        let chosen_name = custom_name
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| doc.profile.name.clone());

        let id = self.allocate_profile_id(&chosen_name);
        let meta = ProfileMeta {
            id: id.clone(),
            name: chosen_name,
            subscription_url: None,
            last_updated_unix_ms: None,
            is_active: false,
            last_sync_summary: None,
            unread_summary: false,
            skip_auto_update: false,
            update_interval_hours: 12,
            sub_upload: None,
            sub_download: None,
            sub_total: None,
            sub_expire: None,
            icon: doc.profile.icon.clone(),
        };

        self.db.create_profile(&meta)?;
        let (mut bundle, sources) = doc.to_zeytun_core(&id);
        bundle.id = id.clone();
        self.db.save_profile(&bundle)?;
        for source in &sources {
            self.db.create_subscription(&id, source)?;
        }

        self.events.info("Profile imported", &meta.name);
        Ok(self
            .db
            .list_profiles()?
            .into_iter()
            .find(|p| p.id == id)
            .unwrap_or(meta))
    }

    /// Re-fetch the active (or given) profile's subscription and reconcile.
    /// `background = true` persists the summary with `unread_summary` set;
    /// `background = false` (foreground/manual) returns it and leaves it read.
    pub fn legacy_subscription_id(&self, id: &str) -> Result<String, crate::error::CommandError> {
        Ok(self.db.legacy_subscription(id)?.id)
    }

    pub fn profile_apply_refresh(
        &mut self,
        id: &str,
        raw: &str,
        headers: crate::core::subscription::SubHeaders,
        background: bool,
    ) -> Result<SyncSummary, crate::error::CommandError> {
        let source = self.db.legacy_subscription(id)?;
        let staged = SubscriptionManager::stage(raw, headers).map_err(|e| e.to_string());
        self.apply_sync_result(id, &source, staged, background)
    }

    pub(crate) fn apply_sync_result(
        &mut self,
        id: &str,
        source: &crate::core::dto::SubscriptionSource,
        staged: Result<crate::core::subscription::StagedSubscription, String>,
        background: bool,
    ) -> Result<SyncSummary, crate::error::CommandError> {
        let staged = match staged {
            Ok(staged) => staged,
            Err(error) => {
                let result = SubscriptionManager::failed_result(error);
                self.db
                    .record_subscription_failure(id, source, &result.summary)?;
                return Ok(result.summary);
            }
        };
        let refreshing_active = self.profile.id == id;
        let session_rules = if refreshing_active {
            self.profile.session_rules.as_slice()
        } else {
            &[]
        };
        let (bundle, result) =
            match self
                .db
                .apply_subscription(id, source, staged, background, session_rules)
            {
                Ok(accepted) => accepted,
                Err(crate::core::storage::StorageError::StaleSubscription) => {
                    return Err(crate::core::storage::StorageError::StaleSubscription.into())
                }
                Err(_) => {
                    // The transaction has rolled back. Never expose DB/parser payloads,
                    // and recheck generation in the failure writer before recording.
                    let result = SubscriptionManager::failed_result(
                        "subscription candidate could not be applied".into(),
                    );
                    self.db
                        .record_subscription_failure(id, source, &result.summary)?;
                    return Ok(result.summary);
                }
            };

        self.events.info(
            "Profile updated",
            format!(
                "+{} -{} kept={} updated={}",
                result.summary.proxies_added,
                result.summary.proxies_removed,
                result.summary.proxies_kept,
                result.summary.proxies_updated,
            ),
        );

        if refreshing_active {
            self.profile = bundle;
        }
        Ok(result.summary)
    }

    pub fn profile_mark_summary_read(
        &mut self,
        id: &str,
    ) -> Result<(), crate::error::CommandError> {
        self.db.mark_summary_read(id).map_err(Into::into)
    }

    pub fn subscription_list(
        &self,
        profile_id: &str,
    ) -> Result<Vec<crate::core::dto::SubscriptionSource>, crate::error::CommandError> {
        if !self.db.list_profiles()?.iter().any(|p| p.id == profile_id) {
            return Err(crate::error::CommandError::NotFound(
                "profile not found".into(),
            ));
        }
        Ok(self.db.list_subscriptions(profile_id)?)
    }

    pub fn subscription_create(
        &mut self,
        profile_id: &str,
        settings: crate::core::dto::SubscriptionSettings,
    ) -> Result<crate::core::dto::SubscriptionSource, crate::error::CommandError> {
        if !self.db.list_profiles()?.iter().any(|p| p.id == profile_id) {
            return Err(crate::error::CommandError::NotFound(
                "profile not found".into(),
            ));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let existing = self.db.list_subscriptions(profile_id)?;
        let position = existing.len() as i64;
        let source = crate::core::dto::SubscriptionSource {
            id,
            settings,
            generation: 0,
            position,
            provider_title: None,
            suggested_interval_hours: None,
            userinfo: None,
            last_attempt_at: None,
            last_success_at: None,
            last_error: None,
            last_sync_summary: None,
            unread_summary: false,
        };
        self.db.create_subscription(profile_id, &source)?;
        Ok(source)
    }

    pub fn subscription_update(
        &mut self,
        profile_id: &str,
        source_id: &str,
        settings: crate::core::dto::SubscriptionSettings,
    ) -> Result<crate::core::dto::SubscriptionSource, crate::error::CommandError> {
        if !self.db.list_profiles()?.iter().any(|p| p.id == profile_id) {
            return Err(crate::error::CommandError::NotFound(
                "profile not found".into(),
            ));
        }
        self.db
            .update_subscription(profile_id, source_id, &settings)?;
        self.db
            .list_subscriptions(profile_id)?
            .into_iter()
            .find(|s| s.id == source_id)
            .ok_or_else(|| {
                crate::error::CommandError::NotFound("subscription source not found".into())
            })
    }

    pub fn subscription_mark_summary_read(
        &mut self,
        profile_id: &str,
        source_id: &str,
    ) -> Result<(), crate::error::CommandError> {
        self.db
            .mark_subscription_summary_read(profile_id, source_id)?;
        Ok(())
    }

    pub fn saved_revision(&self, profile_id: &str) -> Result<u64, crate::error::CommandError> {
        Ok(self.db.config_revision(profile_id)?)
    }

    pub fn reload_if_running(&mut self) -> Result<(), crate::error::CommandError> {
        if self.lifecycle.is_running() {
            // Session rules die on reload: they're never baked into the config,
            // so the post-reload live-rules seed/push would drop them anyway —
            // clear the app-side mirror to match.
            self.profile.session_rules.clear();
            self.lifecycle.reload(&self.profile)?;
        }
        Ok(())
    }

    fn restart_if_running(&mut self) -> Result<(), crate::error::CommandError> {
        if self.lifecycle.is_running() {
            let _ = self.lifecycle.stop();
            self.start_snapshot()?;
        }
        Ok(())
    }

    pub fn set_inbound_mode(&mut self, mode: &str) -> Result<(), crate::error::CommandError> {
        let prev_mode = self.profile.local_proxy.mode.clone();
        let inbound_mode = match mode {
            "mixed" => InboundMode::Mixed,
            "tun" => InboundMode::Tun,
            _ => {
                return Err(crate::error::CommandError::InvalidInput(format!(
                    "invalid inbound mode: {mode}"
                )))
            }
        };
        self.profile.local_proxy.mode = Some(inbound_mode);
        self.profile.normalize_policy_graph();
        self.db.save_local_proxy(&self.profile.local_proxy)?;
        // TUN mode changes require a full restart to add/remove network routes and privileges
        if let Err(e) = self.restart_if_running() {
            // Revert mode if restart failed (e.g. UAC cancelled)
            self.profile.local_proxy.mode = prev_mode;
            self.profile.normalize_policy_graph();
            let _ = self.db.save_local_proxy(&self.profile.local_proxy);
            return Err(e);
        }
        Ok(())
    }

    pub fn set_system_proxy(&mut self, enabled: bool) -> Result<(), crate::error::CommandError> {
        // Only macOS network settings change; restarting the core can collide on its listen port.
        self.lifecycle.set_system_proxy(enabled, &self.profile)?;
        self.profile.local_proxy.system_proxy = Some(enabled);
        self.db.save_local_proxy(&self.profile.local_proxy)?;
        Ok(())
    }

    /// Re-evaluates and re-applies system proxy settings to all currently active network
    /// services without restarting the core or rewriting the database.
    /// Safe to call on system wake or network interface changes.
    pub fn reapply_system_proxy(&mut self) -> Result<(), crate::error::CommandError> {
        if self.profile.local_proxy.system_proxy.unwrap_or(false) && self.is_running() {
            self.lifecycle.set_system_proxy(true, &self.profile)?;
        }
        Ok(())
    }

    pub fn set_allow_lan(&mut self, enabled: bool) -> Result<(), crate::error::CommandError> {
        self.profile.local_proxy.listen = if enabled {
            "0.0.0.0".to_string()
        } else {
            "127.0.0.1".to_string()
        };
        self.profile.normalize_policy_graph();
        self.db.save_local_proxy(&self.profile.local_proxy)?;
        // No auto-restart: persisted only. The listen-address change takes effect
        // when the user hits "Restart Core" (frontend flags pendingRestart).
        Ok(())
    }

    pub fn set_listen_port(&mut self, port: u16) -> Result<(), crate::error::CommandError> {
        self.profile.local_proxy.mixed_port = Some(port);
        self.profile.local_proxy.http_port = port;
        self.profile.local_proxy.socks_port = port;
        self.profile.normalize_policy_graph();
        self.db.save_local_proxy(&self.profile.local_proxy)?;
        // No auto-restart: persisted only. The new port takes effect when the
        // user hits "Restart Core" (frontend flags pendingRestart).
        Ok(())
    }

    pub fn set_log_level(&mut self, level: String) -> Result<(), crate::error::CommandError> {
        self.profile.local_proxy.log_level = Some(level);
        self.profile.normalize_policy_graph();
        self.db.save_local_proxy(&self.profile.local_proxy)?;
        // No auto-reload: the change is persisted but only takes effect when the
        // user hits "Restart Core" (frontend flags pendingRestart). Reloading here
        // would drop the gRPC link and flash the status light for nothing.
        Ok(())
    }

    /// Stored route for every app-originated traffic class. Absent keys mean
    /// [`NetworkPolicy::Direct`].
    pub fn network_policies(
        &self,
    ) -> Result<
        std::collections::HashMap<String, crate::core::network_policy::NetworkPolicy>,
        crate::error::CommandError,
    > {
        Ok(self.db.load_network_policies()?)
    }

    pub fn set_network_policy(
        &mut self,
        traffic: &str,
        policy: &crate::core::network_policy::NetworkPolicy,
    ) -> Result<(), crate::error::CommandError> {
        if !crate::core::network_policy::TRAFFIC_KINDS.contains(&traffic) {
            return Err(crate::error::CommandError::InvalidInput(format!(
                "unknown traffic class `{traffic}`"
            )));
        }
        self.db.save_network_policy(traffic, policy)?;
        Ok(())
    }

    /// Use only the ready, applied inbound; never silently fall back to Direct.
    pub fn proxy_for_traffic(
        &self,
        traffic: &str,
    ) -> Result<Option<String>, crate::error::CommandError> {
        let policy = self
            .db
            .load_network_policies()?
            .remove(traffic)
            .unwrap_or_default();

        if matches!(policy, crate::core::network_policy::NetworkPolicy::Direct) {
            return Ok(None);
        }
        if !self.is_running() {
            return Err(crate::error::CommandError::State(
                "local proxy download requires a ready core".into(),
            ));
        }

        let port = self
            .lifecycle
            .applied_profile()
            .ok_or_else(|| crate::error::CommandError::State("no applied inbound endpoint".into()))?
            .local_proxy
            .mixed_port
            .unwrap_or(crate::core::constants::DEFAULT_MIXED_PORT);
        let (url, degraded) = policy.resolve(port);
        if degraded {
            println!(
                "[NetworkPolicy] `{traffic}` asked for a named policy; app traffic can only \
                 reach the local proxy, so routing depends on the profile's rules"
            );
        }
        Ok(url)
    }

    pub fn set_mode(&mut self, mode: OutboundMode) -> Result<(), crate::error::CommandError> {
        self.profile.outbound_mode = mode;
        self.profile.normalize_policy_graph();
        self.db.save_profile(&self.profile)?;

        // If core is running, update the clash API mode without restart
        if self.lifecycle.is_running() {
            let mode_str = serde_json::to_value(self.profile.outbound_mode)
                .ok()
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_else(|| "rule".to_string());
            let clash_client = self.lifecycle.clash_api().cloned().ok_or_else(|| {
                crate::error::CommandError::State("clash api is not available".to_string())
            })?;
            clash_client
                .patch_config(&mode_str)
                .map_err(|e| crate::error::CommandError::Api(format!("clash api error: {e:?}")))?;

            if matches!(self.profile.outbound_mode, OutboundMode::Global) {
                let _ = clash_client.select_proxy("GLOBAL", DEFAULT_SELECTOR_POLICY_TAG);
            }

            self.lifecycle.update_applied_mode(mode);

            if let Err(error) = clash_client.flush_dns_response_cache() {
                crate::clog!("Could not refresh DNS answers after mode change: {error:?}");
            }

            // Temp overlay follows the mode: switching to Rule re-pushes the
            // full temp+session list; switching to Direct/Global clears it
            // (empty push) so nothing leaks past the clash-mode catch-all.
            // The in-memory lists stay intact — returning to Rule restores.
            self.push_temp_rules_overlay()?;

            // Flush all connections on mode switch to prevent browser keep-alive issues
            let api_clone = clash_client.clone();
            std::thread::spawn(move || {
                if let Err(e) = api_clone.close_all_connections() {
                    crate::clog!(
                        "Failed to close all connections during mode switch: {:?}",
                        e
                    );
                }
            });
        }
        Ok(())
    }

    pub fn update_proxy(
        &mut self,
        input: UpdateProxyInput,
    ) -> Result<crate::core::dto::Proxy, crate::error::CommandError> {
        let proxy = self
            .profile
            .proxies
            .iter_mut()
            .find(|p| p.tag == input.tag)
            .ok_or_else(|| {
                crate::error::CommandError::NotFound(format!("proxy `{}` not found", input.tag))
            })?;

        if let Some(title) = input.title {
            proxy.title = title;
        }
        if let Some(config_value) = input.config {
            let server: ProxyServer = serde_json::from_value(config_value).map_err(|e| {
                crate::error::CommandError::InvalidInput(format!("invalid proxy config: {e}"))
            })?;
            proxy.protocol = server.protocol.name().to_string();
            proxy.transport = server.protocol.transport();
            proxy.config = serde_json::to_value(&server).ok();
            proxy.link = String::new();
        }
        if let Some(enabled) = input.enabled {
            proxy.enabled = enabled;
        }
        let updated_proxy = proxy.clone();

        self.profile.normalize_policy_graph();
        self.db.save_profile(&self.profile)?;
        Ok(updated_proxy)
    }

    pub fn proxy_export_link(&self, proxy_tag: &str) -> Result<String, crate::error::CommandError> {
        let proxy = self
            .profile
            .proxies
            .iter()
            .find(|p| p.tag == proxy_tag)
            .ok_or_else(|| {
                crate::error::CommandError::NotFound(format!("proxy `{proxy_tag}` not found"))
            })?;
        link_parser::proxy_to_link(proxy).map_err(crate::error::CommandError::InvalidInput)
    }

    pub fn delete_proxy(&mut self, proxy_tag: &str) -> Result<(), crate::error::CommandError> {
        if !self.profile.proxies.iter().any(|p| p.tag == proxy_tag) {
            return Err(crate::error::CommandError::NotFound(format!(
                "proxy `{}` not found",
                proxy_tag
            )));
        }
        self.profile.proxies.retain(|p| p.tag != proxy_tag);
        // Clean up policy member references
        for policy in &mut self.profile.policies {
            if let Some(members) = policy.members.as_mut() {
                members.retain(|m| m != proxy_tag);
            }
        }
        // Option A: repoint any rule targeting the deleted proxy to the default
        // final outbound and flag it orphaned, so the config stays valid.
        for rule in &mut self.profile.rules {
            if rule.outbound == proxy_tag {
                rule.outbound = DEFAULT_FINAL_OUTBOUND.to_string();
                rule.orphaned = true;
            }
        }
        self.profile.normalize_policy_graph();
        self.db.save_profile(&self.profile)?;
        Ok(())
    }

    pub fn create_policy(
        &mut self,
        input: CreatePolicyInput,
    ) -> Result<crate::core::dto::ProxyPolicy, crate::error::CommandError> {
        let tag = self.allocate_policy_tag(&input.name);
        let policy = crate::core::dto::ProxyPolicy {
            tag: tag.clone(),
            name: input.name,
            kind: Some(input.kind),
            members: if input.members.is_empty() {
                None
            } else {
                Some(input.members)
            },
            selected_member_tag: input.selected_member_tag,
            test_url: input.test_url,
            interval_seconds: input.interval_seconds,
            tolerance_ms: input.tolerance_ms,
            strategy: input.strategy,
            weights: input.weights,
        };
        self.profile.policies.push(policy.clone());
        self.profile.normalize_policy_graph();
        self.db.save_profile(&self.profile)?;
        Ok(self
            .profile
            .policies
            .iter()
            .find(|p| p.tag == tag)
            .cloned()
            .unwrap_or(policy))
    }

    pub fn update_policy(
        &mut self,
        input: crate::core::dto::UpdatePolicyInput,
    ) -> Result<crate::core::dto::ProxyPolicy, crate::error::CommandError> {
        if input.tag == crate::core::dto::DEFAULT_SELECTOR_POLICY_TAG {
            return Err(crate::error::CommandError::InvalidInput(
                "cannot edit the default policy".to_string(),
            ));
        }

        let policy = self
            .profile
            .policies
            .iter_mut()
            .find(|p| p.tag == input.tag)
            .ok_or_else(|| {
                crate::error::CommandError::NotFound(format!("policy `{}` not found", input.tag))
            })?;

        if let Some(name) = input.name {
            policy.name = name;
        }
        if let Some(kind) = input.kind {
            policy.kind = Some(kind);
        }
        if let Some(members) = input.members {
            policy.members = if members.is_empty() {
                None
            } else {
                Some(members)
            };
        }
        if let Some(selected_member_tag) = input.selected_member_tag {
            policy.selected_member_tag = Some(selected_member_tag);
        }
        if let Some(strategy) = input.strategy {
            policy.strategy = Some(strategy);
        }
        if let Some(weights) = input.weights {
            policy.weights = Some(weights);
        }
        if let Some(test_url) = input.test_url {
            policy.test_url = Some(test_url);
        }
        if let Some(interval_seconds) = input.interval_seconds {
            policy.interval_seconds = Some(interval_seconds);
        }
        if let Some(tolerance_ms) = input.tolerance_ms {
            policy.tolerance_ms = Some(tolerance_ms);
        }

        let updated_policy = policy.clone();

        self.profile.normalize_policy_graph();
        self.db.save_profile(&self.profile)?;
        Ok(updated_policy)
    }

    pub fn delete_policy(&mut self, policy_tag: &str) -> Result<(), crate::error::CommandError> {
        if policy_tag == DEFAULT_SELECTOR_POLICY_TAG {
            return Err(crate::error::CommandError::InvalidInput(
                "cannot delete the default policy".to_string(),
            ));
        }
        if !self.profile.policies.iter().any(|p| p.tag == policy_tag) {
            return Err(crate::error::CommandError::NotFound(format!(
                "policy `{}` not found",
                policy_tag
            )));
        }
        self.profile.policies.retain(|p| p.tag != policy_tag);

        self.profile.normalize_policy_graph();
        self.db.save_profile(&self.profile)?;
        Ok(())
    }

    pub fn validate_applied_selection(
        &self,
        policy_tag: &str,
        member_tag: &str,
    ) -> Result<(), crate::error::CommandError> {
        let applied = self
            .lifecycle
            .applied_profile()
            .ok_or_else(|| crate::error::CommandError::State("no applied graph".into()))?;
        let resolved = crate::core::compiler::profile_converter::ProfileConverter::convert(applied);
        if resolved
            .proxy_policies
            .iter()
            .any(|p| p.tag == policy_tag && p.outbounds.iter().any(|t| t == member_tag))
        {
            Ok(())
        } else {
            Err(crate::error::CommandError::InvalidInput(
                "selection is not a member of the applied graph".into(),
            ))
        }
    }

    pub fn select_policy_member(
        &mut self,
        input: SelectPolicyMemberInput,
    ) -> Result<(), crate::error::CommandError> {
        if self.lifecycle.is_running() {
            self.validate_applied_selection(&input.policy_tag, &input.member_tag)?;
        }
        let policy = self
            .profile
            .policies
            .iter_mut()
            .find(|p| p.tag == input.policy_tag)
            .ok_or_else(|| {
                crate::error::CommandError::NotFound(format!(
                    "policy `{}` not found",
                    input.policy_tag
                ))
            })?;
        policy.selected_member_tag = Some(input.member_tag.clone());

        self.profile.normalize_policy_graph();
        self.db.save_profile(&self.profile)?;

        // If core is running, use clash API to hot-swap the proxy selection
        if self.lifecycle.is_running() {
            if let Some(clash_api) = self.lifecycle.clash_api().cloned() {
                clash_api
                    .select_proxy(&input.policy_tag, &input.member_tag)
                    .map_err(|e| {
                        crate::error::CommandError::Api(format!("clash api error: {e:?}"))
                    })?;

                self.lifecycle
                    .update_applied_selection(&input.policy_tag, &input.member_tag);

                if let Err(error) = clash_api.flush_dns_response_cache() {
                    crate::clog!("Could not refresh DNS answers after proxy change: {error:?}");
                }

                // Flush all connections on proxy switch to prevent browser keep-alive issues
                let api_clone = clash_api.clone();
                std::thread::spawn(move || {
                    if let Err(e) = api_clone.close_all_connections() {
                        crate::clog!(
                            "Failed to close all connections during proxy switch: {:?}",
                            e
                        );
                    }
                });
            }
        }
        Ok(())
    }

    pub fn create_ruleset(
        &mut self,
        app_data_dir: std::path::PathBuf,
        kind: String,
        source: String,
        action: String,
        comment: Option<String>,
        file_bytes: Option<Vec<u8>>,
    ) -> Result<crate::core::dto::RuleSet, crate::error::CommandError> {
        let id = uuid::Uuid::new_v4().to_string();
        let tag = format!("ruleset-{}", id.chars().take(8).collect::<String>());

        let final_source = if kind == "local" {
            Self::write_ruleset_file(&app_data_dir, &id, file_bytes)?
        } else {
            source
        };

        let ruleset = crate::core::dto::RuleSet {
            id,
            tag,
            kind,
            source: final_source,
            action,
            comment,
            download_policy: None, // default root-policy at compile time
            enabled: true,
            name: None,
        };

        let mut rule_sets = self.profile.rule_sets.clone().unwrap_or_default();
        rule_sets.push(ruleset.clone());
        self.profile.rule_sets = Some(rule_sets);
        self.profile.normalize_policy_graph();
        self.db.save_profile(&self.profile)?;
        Ok(ruleset)
    }

    /// Write .srs bytes under app data; returns absolute path. Used for draft local create/edit.
    pub fn stage_ruleset_file(
        app_data_dir: std::path::PathBuf,
        file_bytes: Vec<u8>,
    ) -> Result<String, crate::error::CommandError> {
        let id = uuid::Uuid::new_v4().to_string();
        Self::write_ruleset_file(&app_data_dir, &id, Some(file_bytes))
    }

    fn write_ruleset_file(
        app_data_dir: &std::path::Path,
        id: &str,
        file_bytes: Option<Vec<u8>>,
    ) -> Result<String, crate::error::CommandError> {
        let bytes = file_bytes.ok_or_else(|| {
            crate::error::CommandError::Internal("No file content provided".to_string())
        })?;
        let ruleset_dir = app_data_dir.join("rulesets");
        std::fs::create_dir_all(&ruleset_dir)
            .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?;
        let file_path = ruleset_dir.join(format!("{id}.srs"));
        std::fs::write(&file_path, bytes)
            .map_err(|e| crate::error::CommandError::Internal(e.to_string()))?;
        Ok(file_path.to_string_lossy().into_owned())
    }

    /// Replace full ruleset list (draft publish). Cleans orphaned local .srs files.
    pub fn update_rulesets(
        &mut self,
        rule_sets: Vec<crate::core::dto::RuleSet>,
    ) -> Result<(), crate::error::CommandError> {
        let old = self.profile.rule_sets.clone().unwrap_or_default();
        let new_sources: std::collections::HashSet<String> =
            rule_sets.iter().map(|r| r.source.clone()).collect();

        let mut next_profile = self.profile.clone();
        next_profile.rule_sets = Some(rule_sets);
        next_profile.normalize_policy_graph();
        validate_profile_dns_rules(&next_profile)?;
        self.db.save_profile(&next_profile)?;
        self.profile = next_profile;

        for rs in &old {
            if rs.kind == "local" && !new_sources.contains(rs.source.as_str()) {
                let _ = std::fs::remove_file(&rs.source);
            }
        }
        Ok(())
    }

    /// Replace the full DNS rule list (persisted, triggers pendingRestart in frontend).
    pub fn update_dns_rules(
        &mut self,
        dns_rules: Vec<crate::core::dto::DnsRule>,
    ) -> Result<(), crate::error::CommandError> {
        validate_dns_rules(&self.profile, &dns_rules)?;

        let mut next_profile = self.profile.clone();
        let dns = next_profile.dns.get_or_insert_with(Default::default);
        dns.rules = if dns_rules.is_empty() {
            None
        } else {
            Some(dns_rules)
        };
        self.db.save_profile(&next_profile)?;
        self.profile = next_profile;
        Ok(())
    }

    /// Persist `dns.final` — the resolver used for queries no DNS rule matched.
    ///
    /// `None`/empty means the built-in local resolver (the system resolver). Any
    /// other value must name a configured DNS server; `local-hosts` is rejected
    /// because a hosts table NXDOMAINs everything outside it.
    pub fn update_dns_final(
        &mut self,
        final_server: Option<String>,
    ) -> Result<(), crate::error::CommandError> {
        let final_server = final_server
            .map(|tag| tag.trim().to_string())
            .filter(|tag| !tag.is_empty() && tag != "local-dns");

        if let Some(tag) = &final_server {
            if tag == DNS_HOSTS_SERVER_TAG {
                return Err(crate::error::CommandError::InvalidInput(format!(
                    "`{DNS_HOSTS_SERVER_TAG}` only answers its own hosts entries, so it cannot resolve unmatched queries"
                )));
            }
            let known = self
                .profile
                .dns
                .as_ref()
                .and_then(|dns| dns.servers.as_deref())
                .unwrap_or_default()
                .iter()
                .any(|server| server.tag == *tag);
            if !known {
                return Err(crate::error::CommandError::InvalidInput(format!(
                    "DNS server `{tag}` is not configured"
                )));
            }
        }

        let mut next_profile = self.profile.clone();
        let dns = next_profile.dns.get_or_insert_with(Default::default);
        dns.final_server = final_server;
        dns.mode = None;
        self.db.save_profile(&next_profile)?;
        self.profile = next_profile;
        Ok(())
    }

    pub fn update_dns_hosts(
        &mut self,
        hosts: Vec<DnsHostEntry>,
    ) -> Result<(), crate::error::CommandError> {
        let hosts = normalize_dns_hosts(hosts)?;
        let mut next_profile = self.profile.clone();
        let dns = next_profile.dns.get_or_insert_with(Default::default);
        dns.hosts = (!hosts.is_empty()).then_some(hosts);
        validate_profile_dns_rules(&next_profile)?;

        self.db.save_profile(&next_profile)?;
        self.profile = next_profile;
        Ok(())
    }

    /// Persist download detour for all remote rulesets and restart core to re-fetch.
    pub fn retry_rulesets(&mut self, policy_tag: String) -> Result<(), crate::error::CommandError> {
        let policy_tag = policy_tag.trim().to_string();
        if policy_tag.is_empty() {
            return Err(crate::error::CommandError::InvalidInput(
                "policy tag required".into(),
            ));
        }
        let Some(rule_sets) = self.profile.rule_sets.as_mut() else {
            return Err(crate::error::CommandError::NotFound("no rulesets".into()));
        };
        let mut n = 0usize;
        for rs in rule_sets.iter_mut() {
            if rs.kind == "remote" {
                rs.download_policy = Some(policy_tag.clone());
                n += 1;
            }
        }
        if n == 0 {
            return Err(crate::error::CommandError::InvalidInput(
                "no remote rulesets to retry".into(),
            ));
        }
        self.db.save_profile(&self.profile)?;
        self.events
            .info("Ruleset retry", format!("{n} remote via {policy_tag}"));
        // Full restart so remote rule-set StartContext runs again.
        if self.lifecycle.is_running() {
            let _ = self.lifecycle.stop();
        }
        self.start_snapshot()?;
        Ok(())
    }

    pub fn delete_ruleset(&mut self, id: String) -> Result<(), crate::error::CommandError> {
        if let Some(mut rule_sets) = self.profile.rule_sets.clone() {
            if let Some(idx) = rule_sets.iter().position(|r| r.id == id) {
                let rs = rule_sets.remove(idx);
                let mut next_profile = self.profile.clone();
                next_profile.rule_sets = Some(rule_sets);
                next_profile.normalize_policy_graph();
                validate_profile_dns_rules(&next_profile)?;
                self.db.save_profile(&next_profile)?;
                self.profile = next_profile;

                if rs.kind == "local" {
                    let _ = std::fs::remove_file(&rs.source);
                }
            }
        }
        Ok(())
    }

    pub fn update_rules(
        &mut self,
        rules: Vec<crate::core::dto::Rule>,
    ) -> Result<(), crate::error::CommandError> {
        self.profile.rules = rules;
        self.profile.normalize_policy_graph();
        self.db.save_profile(&self.profile)?;
        // Live overlay — rewrite config for cold start, push permanent without SIGHUP.
        self.write_config_file_only()?;
        self.push_permanent_rules_overlay()?;
        self.flush_connections_after_live_rules();
        Ok(())
    }

    /// Write core config to disk without reloading process (for live_rules bake).
    fn write_config_file_only(&mut self) -> Result<(), crate::error::CommandError> {
        if !self.lifecycle.is_running() {
            return Ok(());
        }
        self.lifecycle
            .write_config_only(&self.profile)
            .map_err(crate::error::CommandError::Internal)?;
        Ok(())
    }

    /// Kill all active connections so next dial re-matches against live rules.
    fn flush_connections_after_live_rules(&self) {
        let Some(clash) = self.lifecycle.clash_api().cloned() else {
            return;
        };
        std::thread::spawn(move || {
            if let Err(e) = clash.close_all_connections() {
                crate::clog!("Failed to close all connections after live rules: {e:?}");
            }
        });
    }

    fn sync_live_policy_selections(&self, clash: &crate::adapters::clash_api::ClashApiClient) {
        let converted =
            crate::core::compiler::profile_converter::ProfileConverter::convert(&self.profile);
        for policy in &converted.proxy_policies {
            if let crate::core::models::policy::ProxyPolicyType::Manual(manual) =
                &policy.policy_type
            {
                if let Some(target) = &manual.default {
                    let _ = clash.select_proxy(&policy.tag, target);
                }
            }
        }
    }

    fn push_permanent_rules_overlay(&mut self) -> Result<(), crate::error::CommandError> {
        let Some(clash) = self.lifecycle.clash_api() else {
            return Ok(());
        };
        let (saved, revision) = self.db.profile_snapshot(&self.profile.id)?;
        self.sync_live_policy_selections(clash);
        let payload=crate::core::compiler::runtime_profile_compiler::RuntimeProfileCompiler::live_rules_payload(&self.profile,true).map_err(crate::error::CommandError::Internal)?;
        let body = payload["permanent"].to_string();
        clash.put_permanent_rules(&body).map_err(|e| {
            crate::error::CommandError::Internal(format!("permanent-rules push: {e:?}"))
        })?;
        self.lifecycle.acknowledge_live_rules(
            &self.profile,
            &saved,
            revision,
            crate::core::lifecycle::LiveRulesLayer::Permanent,
        );
        Ok(())
    }

    pub fn update_temp_rules(
        &mut self,
        temp_rules: Vec<crate::core::dto::TempRule>,
    ) -> Result<(), crate::error::CommandError> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        // Split incoming list: session rules stay volatile (no expiry, no DB),
        // everything else is a persisted temp rule (drop expired + FINAL).
        let (session, persisted): (Vec<_>, Vec<_>) =
            temp_rules.into_iter().partition(|r| r.session);
        let new_session: Vec<_> = session
            .into_iter()
            .filter(|r| !matches!(r.kind, crate::core::dto::RuleType::Final))
            .collect();

        // Detect session rules that were removed or had their outbound changed.
        // Their ask_group_key must be forgotten so the core's session cache
        // doesn't shadow the deletion/edit with a stale cached decision.
        let mut forget_keys: Vec<String> = Vec::new();
        for old in &self.profile.session_rules {
            if let Some(key) = &old.ask_group_key {
                let still_exists = new_session
                    .iter()
                    .any(|n| n.id == old.id && n.outbound == old.outbound);
                if !still_exists {
                    forget_keys.push(key.clone());
                }
            }
        }
        if !forget_keys.is_empty() {
            if let Some(clash) = self.lifecycle.clash_api() {
                if let Err(e) = clash.forget_ask_session_keys(&forget_keys) {
                    // If the forget POST fails, the core's ask session cache
                    // keeps the stale decision and ask never re-fires after
                    // the session rule is deleted — surface it instead of
                    // silently swallowing (stale core binary was the cause).
                    crate::clog!("Failed to forget ask session keys {forget_keys:?}: {e:?}");
                }
            }
        }

        self.profile.session_rules = new_session;
        self.profile.temp_rules = persisted
            .into_iter()
            .filter(|r| {
                !r.is_expired(now_ms)
                    && !matches!(r.kind, crate::core::dto::RuleType::Final)
                    && r.expires_at > now_ms
            })
            .collect();
        self.profile.normalize_policy_graph();
        self.db.save_profile(&self.profile)?;
        // Live overlay — no core reload.
        self.write_config_file_only()?;
        self.push_temp_rules_overlay()?;
        self.flush_connections_after_live_rules();
        Ok(())
    }

    /// Drop expired temp rules from DB/memory; push overlay (no reload).
    pub fn prune_expired_temp_rules(&mut self) -> Result<bool, crate::error::CommandError> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let before = self.profile.temp_rules.len();
        self.profile.temp_rules.retain(|r| !r.is_expired(now_ms));
        if self.profile.temp_rules.len() == before {
            return Ok(false);
        }
        self.db.save_profile(&self.profile)?;
        self.write_config_file_only()?;
        self.push_temp_rules_overlay()?;
        self.flush_connections_after_live_rules();
        Ok(true)
    }

    fn push_temp_rules_overlay(&mut self) -> Result<(), crate::error::CommandError> {
        let Some(clash) = self.lifecycle.clash_api() else {
            return Ok(());
        };
        let (saved, revision) = self.db.profile_snapshot(&self.profile.id)?;
        self.sync_live_policy_selections(clash);
        let payload=crate::core::compiler::runtime_profile_compiler::RuntimeProfileCompiler::live_rules_payload(&self.profile,true).map_err(crate::error::CommandError::Internal)?;
        let body = payload["temp"].to_string();
        clash
            .put_temp_rules(&body)
            .map_err(|e| crate::error::CommandError::Internal(format!("temp-rules push: {e:?}")))?;
        self.lifecycle.acknowledge_live_rules(
            &self.profile,
            &saved,
            revision,
            crate::core::lifecycle::LiveRulesLayer::Temp,
        );
        Ok(())
    }

    /// Soonest future `expires_at` on active profile, if any.
    pub fn next_temp_rule_expiry_ms(&self) -> Option<i64> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        self.profile
            .temp_rules
            .iter()
            .filter(|r| r.expires_at > now_ms)
            .map(|r| r.expires_at)
            .min()
    }

    /// Move a temp rule into permanent rules (top, before FINAL) and drop it
    /// from temp. Works for persisted temp rules and volatile session rules.
    pub fn promote_temp_rule(&mut self, id: u64) -> Result<(), crate::error::CommandError> {
        // Session rules promote from the volatile list; otherwise persisted temp.
        let from_session = self.profile.session_rules.iter().position(|r| r.id == id);
        let temp = if let Some(idx) = from_session {
            self.profile.session_rules.remove(idx)
        } else {
            let idx = self
                .profile
                .temp_rules
                .iter()
                .position(|r| r.id == id)
                .ok_or_else(|| {
                    crate::error::CommandError::NotFound(format!("temp rule `{id}` not found"))
                })?;
            self.profile.temp_rules.remove(idx)
        };
        // The promoted rule originated from an ask decision the core cached
        // in its ask session map. Once the temp/session rule is gone, that
        // entry would outlive it and silently re-apply the old decision
        // after the permanent rule is deleted — clear it on promote.
        let ask_key = temp.ask_group_key.clone();
        let permanent = temp.to_rule();

        let exists = self.profile.rules.iter().any(|r| {
            r.kind == permanent.kind
                && r.value == permanent.value
                && r.outbound == permanent.outbound
        });
        if !exists {
            let next_id = self
                .profile
                .rules
                .iter()
                .filter(|r| r.kind != crate::core::dto::RuleType::Final)
                .map(|r| r.id)
                .max()
                .unwrap_or(0)
                .saturating_add(1);
            let mut rule = permanent;
            rule.id = next_id;
            let mut rules = self.profile.rules.clone();
            let finals: Vec<_> = rules
                .iter()
                .filter(|r| r.kind == crate::core::dto::RuleType::Final)
                .cloned()
                .collect();
            rules.retain(|r| r.kind != crate::core::dto::RuleType::Final);
            rules.insert(0, rule);
            rules.extend(finals);
            self.profile.rules = rules;
        }

        self.profile.normalize_policy_graph();
        self.db.save_profile(&self.profile)?;
        // Live overlay — no SIGHUP.
        self.write_config_file_only()?;
        self.push_temp_rules_overlay()?;
        self.push_permanent_rules_overlay()?;
        // The core cached the original ask decision; with the temp/session
        // rule gone it would shadow the promoted rule after deletion.
        if let Some(key) = ask_key {
            if let Some(clash) = self.lifecycle.clash_api() {
                if let Err(e) = clash.forget_ask_session_keys(&[key]) {
                    crate::clog!("Failed to forget ask session key after promote: {e:?}");
                }
            }
        }
        self.flush_connections_after_live_rules();
        Ok(())
    }

    pub fn set_connection_ask(
        &mut self,
        config: crate::core::dto::ConnectionAskConfig,
    ) -> Result<(), crate::error::CommandError> {
        let mut cfg = config;
        if cfg.timeout_ms < 1000 {
            cfg.timeout_ms = 1000;
        }
        if cfg.group_by != "process_dest" {
            cfg.group_by = "process".to_string();
        }
        self.profile.local_proxy.connection_ask = Some(cfg);
        self.profile.normalize_policy_graph();
        self.db.save_local_proxy(&self.profile.local_proxy)?;
        // No auto-reload: persisted only. Takes effect when the user hits
        // "Restart Core" (frontend flags pendingRestart).
        Ok(())
    }

    /// Decide held unmatched connection; optionally append permanent rule.
    pub fn decide_connection_ask(
        &mut self,
        id: String,
        outbound: String,
        reject: bool,
        remember: bool,
        process_path: Option<String>,
        process_bundle: Option<String>,
        dest_host: Option<String>,
        group_by: Option<String>,
    ) -> Result<(), crate::error::CommandError> {
        let action = if reject {
            "reject"
        } else if outbound.is_empty() {
            "final"
        } else if outbound.eq_ignore_ascii_case("direct") {
            "direct"
        } else {
            "route"
        };
        let clash = self
            .lifecycle
            .clash_api()
            .ok_or_else(|| crate::error::CommandError::Internal("clash api unavailable".into()))?;
        clash
            .decide_connection_ask(&id, &outbound, action, reject)
            .map_err(|e| crate::error::CommandError::Internal(format!("{e:?}")))?;

        if reject {
            return Ok(());
        }

        let path = process_path.unwrap_or_default();
        if path.is_empty() {
            return Ok(());
        }

        // ponytail: AND process+domain when headless multi-item rules exposed.
        let _ = (dest_host, group_by);
        let (kind, value) = process_rule_kind_value(&path, process_bundle.as_deref());

        if remember {
            // ── Permanent rule (survives reload, persisted to DB) ──────────
            let rule_exists = self
                .profile
                .rules
                .iter()
                .any(|r| r.kind == kind && r.value == value && r.outbound == outbound);
            if rule_exists {
                return Ok(());
            }

            let next_id = self
                .profile
                .rules
                .iter()
                .filter(|r| r.kind != crate::core::dto::RuleType::Final)
                .map(|r| r.id)
                .max()
                .unwrap_or(0)
                .saturating_add(1);

            let new_rule = crate::core::dto::Rule {
                id: next_id,
                kind,
                value,
                outbound: if outbound.is_empty() {
                    "direct".to_string()
                } else {
                    outbound
                },
                comment: "Ask remember".to_string(),
                rule_set: None,
                orphaned: false,
                enabled: true,
            };

            let mut rules = self.profile.rules.clone();
            let final_rules: Vec<_> = rules
                .iter()
                .filter(|r| r.kind == crate::core::dto::RuleType::Final)
                .cloned()
                .collect();
            rules.retain(|r| r.kind != crate::core::dto::RuleType::Final);
            rules.insert(0, new_rule);
            rules.extend(final_rules);

            self.profile.rules = rules;
            self.profile.normalize_policy_graph();
            self.db.save_profile(&self.profile)?;
            // Push permanent overlay so the rule takes effect without reload.
            self.write_config_file_only()?;
            self.push_permanent_rules_overlay()?;
            // The core's decide cached this decision in its ask session map.
            // The permanent rule shadows the cache while it exists, but after
            // the rule is deleted the orphaned entry would silently re-apply
            // the old decision instead of re-asking. Clear it now so deleting
            // a remembered rule re-triggers ask.
            if let Some(clash) = self.lifecycle.clash_api() {
                if let Err(e) = clash.forget_ask_session_keys(std::slice::from_ref(&path)) {
                    crate::clog!("Failed to forget ask session key after remember: {e:?}");
                }
            }
        } else {
            // ── Session rule (volatile, dies on core reload/restart) ───────
            let rule_exists = self
                .profile
                .session_rules
                .iter()
                .any(|r| r.kind == kind && r.value == value && r.outbound == outbound);
            if rule_exists {
                return Ok(());
            }

            const SESSION_ID_BASE: u64 = 8_000_000_000_000_000;
            let next_id = self
                .profile
                .session_rules
                .iter()
                .map(|r| r.id)
                .min()
                .unwrap_or(SESSION_ID_BASE)
                .saturating_sub(1);

            let new_rule = crate::core::dto::TempRule {
                id: next_id,
                kind,
                value,
                outbound: if outbound.is_empty() {
                    "direct".to_string()
                } else {
                    outbound
                },
                comment: "Ask session".to_string(),
                rule_set: None,
                orphaned: false,
                expires_at: 0,
                enabled: true,
                session: true,
                // group_key = process_path when groupBy="process" (default).
                // Used to invalidate the core's ask session cache on
                // delete/edit so the next connection re-triggers ask.
                ask_group_key: Some(path.clone()),
            };

            self.profile.session_rules.insert(0, new_rule);
            self.push_temp_rules_overlay()?;
        }
        Ok(())
    }

    pub fn clash_api_client(&self) -> Option<ClashApiClient> {
        self.lifecycle.clash_api().cloned()
    }

    /// Persist a batch of buffered traffic deltas (called from the daemon flush).
    pub fn record_traffic_batch(
        &self,
        deltas: &[crate::core::dto::TrafficDelta],
    ) -> Result<(), crate::error::CommandError> {
        self.db.record_traffic_batch(deltas).map_err(Into::into)
    }

    /// Read traffic analytics for the dashboard card.
    pub fn traffic_analytics(
        &self,
        since_ts: i64,
        proxy_only: bool,
    ) -> Result<crate::core::dto::TrafficAnalytics, crate::error::CommandError> {
        self.db
            .query_traffic_analytics(since_ts, proxy_only)
            .map_err(Into::into)
    }

    /// Direct-vs-proxy totals for the summary card.
    pub fn traffic_summary(
        &self,
        period: &str,
    ) -> Result<crate::core::dto::TrafficSummary, crate::error::CommandError> {
        self.db.query_traffic_summary(period).map_err(Into::into)
    }

    pub fn paths_info(&self) -> CorePathsInfo {
        CorePathsInfo {
            app_data_dir: self.app_data_dir.to_string_lossy().into_owned(),
            resource_dir: self
                .resource_dir
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned()),
            core_dir: self.core_dir.to_string_lossy().into_owned(),
            bin_dir: self.bin_dir.to_string_lossy().into_owned(),
            platform_bin_dir: self.platform_bin_dir.to_string_lossy().into_owned(),
            runtime_dir: self.runtime_dir.to_string_lossy().into_owned(),
            config_dir: self.config_dir.to_string_lossy().into_owned(),
            profile_path: self.db.path().to_string_lossy().into_owned(),
        }
    }

    fn used_route_tags(&self) -> HashSet<String> {
        let mut used = HashSet::from([
            "direct".to_string(),
            "block".to_string(),
            "dns-out".to_string(),
        ]);
        used.extend(
            self.profile
                .policies
                .iter()
                .map(|policy| policy.tag.clone()),
        );
        used.extend(self.profile.proxies.iter().map(|proxy| proxy.tag.clone()));
        used
    }

    fn allocate_proxy_tag(&self, name_hint: &str) -> String {
        let mut used = self.used_route_tags();
        allocate_named_tag("proxy", name_hint, &mut used)
    }

    fn allocate_policy_tag(&self, name_hint: &str) -> String {
        let mut used = self.used_route_tags();
        allocate_named_tag("policy", name_hint, &mut used)
    }

    fn allocate_profile_id(&self, name_hint: &str) -> String {
        let mut used = self
            .db
            .list_profiles()
            .unwrap_or_default()
            .into_iter()
            .map(|p| p.id)
            .collect::<HashSet<_>>();
        // Never collide with the reserved default id.
        used.insert(DEFAULT_PROFILE_ID.to_string());
        allocate_named_tag("profile", name_hint, &mut used)
    }
}

fn process_rule_kind_value(
    path: &str,
    bundle: Option<&str>,
) -> (crate::core::dto::RuleType, String) {
    // macOS .app helpers: PROCESS-PATH-REGEX on bundle prefix
    if let Some(b) = bundle.filter(|s| s.contains(".app")) {
        let escaped = regex_escape(b.trim_end_matches('/'));
        return (
            crate::core::dto::RuleType::ProcessPathRegex,
            format!("^{escaped}/"),
        );
    }
    if path.contains(".app/") {
        if let Some(idx) = path.find(".app/") {
            let bundle = &path[..=idx + 3]; // includes .app
            let escaped = regex_escape(bundle);
            return (
                crate::core::dto::RuleType::ProcessPathRegex,
                format!("^{escaped}/"),
            );
        }
    }
    (crate::core::dto::RuleType::ProcessPath, path.to_string())
}

fn regex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '\\' | '^' | '$' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

fn resolve_zeytun_core_binary_path(
    bin_dir: &Path,
    platform_bin_dir: &Path,
    resource_dir: Option<&Path>,
) -> PathBuf {
    let binary_name = if cfg!(target_os = "windows") {
        "zeytun-core.exe"
    } else {
        "zeytun-core"
    };

    if let Some(resource_dir) = resource_dir {
        if let Some(candidate) = find_zeytun_core_binary_in_resource_dir(resource_dir, binary_name)
        {
            return candidate;
        }
    }

    let dev_resource_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources");
    if let Some(candidate) = find_zeytun_core_binary_in_resource_dir(&dev_resource_dir, binary_name)
    {
        return candidate;
    }

    let platform_candidate = platform_bin_dir.join(binary_name);
    if platform_candidate.exists() {
        return platform_candidate;
    }

    bin_dir.join(binary_name)
}

fn find_zeytun_core_binary_in_resource_dir(
    resource_dir: &Path,
    binary_name: &str,
) -> Option<PathBuf> {
    let candidates = [
        resource_dir
            .join("resources")
            .join("bin")
            .join(platform_key())
            .join(binary_name),
        resource_dir.join("resources").join("bin").join(binary_name),
        resource_dir
            .join("bin")
            .join(platform_key())
            .join(binary_name),
        resource_dir.join("bin").join(binary_name),
    ];

    candidates.into_iter().find(|p| p.exists())
}

fn platform_key() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

fn merge_dns_config(
    mut update: crate::core::dto::DnsConfig,
    current: Option<crate::core::dto::DnsConfig>,
) -> crate::core::dto::DnsConfig {
    // DNS rules and hosts have dedicated update commands. General DNS settings
    // updates must not erase either collection when its field is omitted.
    if update.rules.is_none() {
        update.rules = current.as_ref().and_then(|dns| dns.rules.clone());
    }
    if update.hosts.is_none() {
        update.hosts = current.and_then(|dns| dns.hosts);
    }
    update
}

pub(crate) fn normalize_dns_domain(domain: &str) -> Result<String, crate::error::CommandError> {
    let domain = domain.trim().to_ascii_lowercase();
    let domain = domain.strip_suffix('.').unwrap_or(&domain).to_string();
    let valid = !domain.is_empty()
        && domain.len() <= 253
        && domain.parse::<std::net::IpAddr>().is_err()
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphanumeric)
                && label
                    .as_bytes()
                    .last()
                    .is_some_and(u8::is_ascii_alphanumeric)
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        });

    if !valid {
        return Err(crate::error::CommandError::InvalidInput(format!(
            "invalid DNS domain `{}`",
            domain
        )));
    }
    Ok(domain)
}

fn normalize_dns_hosts(
    hosts: Vec<DnsHostEntry>,
) -> Result<Vec<DnsHostEntry>, crate::error::CommandError> {
    let mut ids = HashSet::with_capacity(hosts.len());
    hosts
        .into_iter()
        .map(|mut host| {
            host.id = host.id.trim().to_string();
            if host.id.is_empty() {
                return Err(crate::error::CommandError::InvalidInput(
                    "DNS host id must not be empty".to_string(),
                ));
            }
            if !ids.insert(host.id.clone()) {
                return Err(crate::error::CommandError::InvalidInput(format!(
                    "duplicate DNS host id `{}`",
                    host.id
                )));
            }

            host.domain = normalize_dns_domain(&host.domain)?;
            host.address = host
                .address
                .trim()
                .parse::<std::net::IpAddr>()
                .map_err(|_| {
                    crate::error::CommandError::InvalidInput(format!(
                        "DNS host `{}` address must be a valid IPv4 or IPv6 address",
                        host.id
                    ))
                })?
                .to_string();
            Ok(host)
        })
        .collect()
}

pub(crate) fn validate_profile_dns_rules(
    profile: &ZeytunCore,
) -> Result<(), crate::error::CommandError> {
    crate::core::dns_config::normalize_profile_dns(&mut profile.clone())
        .map_err(crate::error::CommandError::InvalidInput)?;
    if profile
        .dns
        .as_ref()
        .and_then(|dns| dns.servers.as_deref())
        .unwrap_or_default()
        .iter()
        .any(|server| server.tag == DNS_HOSTS_SERVER_TAG)
    {
        return Err(crate::error::CommandError::InvalidInput(format!(
            "DNS server tag `{DNS_HOSTS_SERVER_TAG}` is reserved for DNS hosts"
        )));
    }

    let dns_rules = profile
        .dns
        .as_ref()
        .and_then(|dns| dns.rules.as_deref())
        .unwrap_or_default();
    validate_dns_rules(profile, dns_rules)
}

fn validate_dns_rules(
    profile: &ZeytunCore,
    dns_rules: &[crate::core::dto::DnsRule],
) -> Result<(), crate::error::CommandError> {
    let server_tags = profile
        .dns
        .as_ref()
        .and_then(|dns| dns.servers.as_deref())
        .unwrap_or_default()
        .iter()
        .map(|server| server.tag.as_str())
        .collect::<HashSet<_>>();
    let rule_sets = profile.rule_sets.as_deref().unwrap_or_default();
    let mut ids = HashSet::with_capacity(dns_rules.len());

    for rule in dns_rules {
        if rule.id.trim().is_empty() {
            return Err(crate::error::CommandError::InvalidInput(
                "DNS rule id must not be empty".to_string(),
            ));
        }
        if !ids.insert(rule.id.as_str()) {
            return Err(crate::error::CommandError::InvalidInput(format!(
                "duplicate DNS rule id `{}`",
                rule.id
            )));
        }
        if !matches!(rule.kind.as_str(), "domain" | "domain_suffix" | "ruleset") {
            return Err(crate::error::CommandError::InvalidInput(format!(
                "invalid DNS rule kind `{}`",
                rule.kind
            )));
        }
        if rule.value.trim().is_empty() {
            return Err(crate::error::CommandError::InvalidInput(format!(
                "DNS rule `{}` value must not be empty",
                rule.id
            )));
        }
        if rule.target != "block"
            && rule.target != crate::core::zeytun_core_config::DNS_LOCAL_SERVER_TAG
            && !server_tags.contains(rule.target.as_str())
        {
            return Err(crate::error::CommandError::InvalidInput(format!(
                "DNS rule `{}` target `{}` is not a configured DNS server",
                rule.id, rule.target
            )));
        }
        if rule.kind == "ruleset" {
            let Some(rule_set) = rule_sets.iter().find(|rule_set| rule_set.tag == rule.value)
            else {
                return Err(crate::error::CommandError::InvalidInput(format!(
                    "DNS rule `{}` references unknown ruleset `{}`",
                    rule.id, rule.value
                )));
            };
            if !rule_set.enabled {
                return Err(crate::error::CommandError::InvalidInput(format!(
                    "DNS rule `{}` references disabled ruleset `{}`",
                    rule.id, rule.value
                )));
            }
        } else {
            normalize_dns_domain(rule.value.trim().trim_start_matches('.'))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod live_apply_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::dto::{DnsConfig, DnsHostEntry, DnsRule, DnsServer, RuleSet};

    #[test]
    fn system_proxy_toggle_does_not_restart_the_core() {
        let code = include_str!("manager.rs");
        let toggle = code.split("pub fn set_system_proxy(").nth(1).unwrap();
        let toggle = toggle.split("pub fn set_allow_lan(").next().unwrap();
        assert!(!toggle.contains("restart_if_running"));
        assert!(toggle.contains("lifecycle.set_system_proxy"));
    }

    /// These setters persist only; the core is re-applied by the user's manual
    /// "Restart Core" button (frontend flags pendingRestart), never automatically.
    #[test]
    fn deferred_setters_do_not_auto_reload_or_restart() {
        let code = include_str!("manager.rs");
        for (start, end) in [
            ("pub fn set_allow_lan(", "pub fn set_listen_port("),
            ("pub fn set_listen_port(", "pub fn set_log_level("),
            ("pub fn set_log_level(", "pub fn network_policies("),
            ("pub fn set_connection_ask(", "/// Decide held unmatched"),
        ] {
            let body = code.split(start).nth(1).unwrap().split(end).next().unwrap();
            assert!(
                !body.contains("reload_if_running") && !body.contains("restart_if_running"),
                "{start} must not auto reload/restart"
            );
        }
    }

    fn dns_rule(id: &str, kind: &str, value: &str, target: &str) -> DnsRule {
        DnsRule {
            id: id.to_string(),
            kind: kind.to_string(),
            value: value.to_string(),
            target: target.to_string(),
            comment: None,
            enabled: true,
        }
    }

    fn profile_for_dns_validation() -> ZeytunCore {
        let mut profile = ZeytunCore {
            dns: Some(DnsConfig {
                mode: None,
                servers: Some(vec![DnsServer {
                    tag: "primary".to_string(),
                    address: "1.1.1.1".to_string(),
                    detour: None,
                    name: "Primary DNS".to_string(),
                }]),
                final_server: Some("primary".to_string()),
                fake_ip: None,
                rules: None,
                hosts: None,
            }),
            ..ZeytunCore::default()
        };
        profile.rule_sets = Some(vec![
            RuleSet {
                id: "enabled-id".to_string(),
                tag: "enabled-ruleset".to_string(),
                kind: "local".to_string(),
                source: "/tmp/enabled.srs".to_string(),
                action: "direct".to_string(),
                comment: None,
                download_policy: None,
                enabled: true,
                name: None,
            },
            RuleSet {
                id: "disabled-id".to_string(),
                tag: "disabled-ruleset".to_string(),
                kind: "local".to_string(),
                source: "/tmp/disabled.srs".to_string(),
                action: "direct".to_string(),
                comment: None,
                download_policy: None,
                enabled: false,
                name: None,
            },
        ]);
        profile
    }

    fn assert_invalid(profile: &ZeytunCore, rules: &[DnsRule], expected: &str) {
        match validate_dns_rules(profile, rules) {
            Err(crate::error::CommandError::InvalidInput(message)) => assert!(
                message.contains(expected),
                "expected `{expected}` in validation error `{message}`"
            ),
            other => panic!("expected invalid input containing `{expected}`, got {other:?}"),
        }
    }

    #[test]
    fn general_dns_updates_preserve_existing_rules_and_hosts() {
        let mut current = profile_for_dns_validation().dns.expect("DNS config");
        current.rules = Some(vec![dns_rule(
            "existing",
            "domain",
            "example.com",
            "primary",
        )]);
        current.hosts = Some(vec![DnsHostEntry {
            id: "host".to_string(),
            domain: "host.example".to_string(),
            address: "192.0.2.10".to_string(),
            enabled: true,
        }]);
        let update = DnsConfig {
            mode: None,
            servers: None,
            final_server: None,
            fake_ip: Some(true),
            rules: None,
            hosts: None,
        };

        let merged = merge_dns_config(update, Some(current));
        assert!(merged.final_server.is_none());
        assert!(merged.servers.is_none());
        assert_eq!(merged.rules.as_deref().map(|rules| rules.len()), Some(1));
        assert_eq!(merged.hosts.as_deref().map(|hosts| hosts.len()), Some(1));
        assert_eq!(merged.fake_ip, Some(true));
    }

    #[test]
    fn dns_hosts_are_normalized_and_validate_ids_domains_and_addresses() {
        let hosts = normalize_dns_hosts(vec![
            DnsHostEntry {
                id: " v4 ".to_string(),
                domain: "Example.COM.".to_string(),
                address: " 192.0.2.1 ".to_string(),
                enabled: true,
            },
            DnsHostEntry {
                id: "v6".to_string(),
                domain: "ipv6.example".to_string(),
                address: "2001:0db8::1".to_string(),
                enabled: false,
            },
        ])
        .expect("valid DNS hosts");
        assert_eq!(hosts[0].id, "v4");
        assert_eq!(hosts[0].domain, "example.com");
        assert_eq!(hosts[0].address, "192.0.2.1");
        assert_eq!(hosts[1].address, "2001:db8::1");

        for (entries, expected) in [
            (
                vec![DnsHostEntry {
                    id: " ".to_string(),
                    domain: "example.com".to_string(),
                    address: "192.0.2.1".to_string(),
                    enabled: true,
                }],
                "id must not be empty",
            ),
            (
                vec![
                    DnsHostEntry {
                        id: "same".to_string(),
                        domain: "one.example".to_string(),
                        address: "192.0.2.1".to_string(),
                        enabled: true,
                    },
                    DnsHostEntry {
                        id: "same".to_string(),
                        domain: "two.example".to_string(),
                        address: "192.0.2.2".to_string(),
                        enabled: true,
                    },
                ],
                "duplicate DNS host id",
            ),
            (
                vec![DnsHostEntry {
                    id: "domain".to_string(),
                    domain: "bad_domain.example".to_string(),
                    address: "192.0.2.1".to_string(),
                    enabled: true,
                }],
                "invalid DNS domain",
            ),
            (
                vec![DnsHostEntry {
                    id: "address".to_string(),
                    domain: "example.com".to_string(),
                    address: "192.0.2.999".to_string(),
                    enabled: true,
                }],
                "valid IPv4 or IPv6",
            ),
        ] {
            match normalize_dns_hosts(entries) {
                Err(crate::error::CommandError::InvalidInput(message)) => {
                    assert!(message.contains(expected), "unexpected error: {message}");
                }
                other => panic!("expected invalid DNS host containing `{expected}`, got {other:?}"),
            }
        }
    }

    #[test]
    fn profile_dns_validation_rejects_reserved_hosts_server_tag() {
        let mut profile = profile_for_dns_validation();
        profile
            .dns
            .as_mut()
            .expect("DNS config")
            .servers
            .as_mut()
            .expect("DNS servers")
            .push(DnsServer {
                tag: DNS_HOSTS_SERVER_TAG.to_string(),
                address: "8.8.8.8".to_string(),
                detour: None,
                name: "Hosts DNS".to_string(),
            });

        match validate_profile_dns_rules(&profile) {
            Err(crate::error::CommandError::InvalidInput(message)) => {
                assert!(message.contains("reserved for DNS hosts"));
            }
            other => panic!("expected reserved DNS server tag error, got {other:?}"),
        }
    }

    #[test]
    fn dns_rule_validation_accepts_supported_contract() {
        let profile = profile_for_dns_validation();
        let rules = vec![
            dns_rule("domain", "domain", "example.com", "primary"),
            dns_rule("suffix", "domain_suffix", ".example.org", "block"),
            dns_rule("ruleset", "ruleset", "enabled-ruleset", "primary"),
        ];

        validate_dns_rules(&profile, &rules).expect("valid DNS rule contract");
    }

    #[test]
    fn dns_rule_validation_rejects_invalid_fields_and_references() {
        let profile = profile_for_dns_validation();

        assert_invalid(
            &profile,
            &[dns_rule(" ", "domain", "example.com", "primary")],
            "id must not be empty",
        );
        assert_invalid(
            &profile,
            &[
                dns_rule("duplicate", "domain", "one.example", "primary"),
                dns_rule("duplicate", "domain", "two.example", "primary"),
            ],
            "duplicate DNS rule id",
        );
        assert_invalid(
            &profile,
            &[dns_rule("kind", "geosite", "ir", "primary")],
            "invalid DNS rule kind",
        );
        assert_invalid(
            &profile,
            &[dns_rule("value", "domain", " ", "primary")],
            "value must not be empty",
        );
        assert_invalid(
            &profile,
            &[dns_rule("target", "domain", "example.com", "BLOCK")],
            "is not a configured DNS server",
        );
        assert_invalid(
            &profile,
            &[dns_rule("ruleset", "ruleset", "missing", "primary")],
            "unknown ruleset",
        );
        assert_invalid(
            &profile,
            &[dns_rule(
                "ruleset",
                "ruleset",
                "disabled-ruleset",
                "primary",
            )],
            "disabled ruleset",
        );
    }

    #[test]
    fn profile_dns_validation_rejects_removed_dependencies() {
        let mut profile = profile_for_dns_validation();
        profile.dns.as_mut().expect("DNS config").rules =
            Some(vec![dns_rule("domain", "domain", "example.com", "primary")]);
        profile.dns.as_mut().expect("DNS config").servers = None;
        assert_invalid(
            &profile,
            profile
                .dns
                .as_ref()
                .and_then(|dns| dns.rules.as_deref())
                .expect("DNS rules"),
            "is not a configured DNS server",
        );

        let mut profile = profile_for_dns_validation();
        profile.dns.as_mut().expect("DNS config").rules = Some(vec![dns_rule(
            "ruleset",
            "ruleset",
            "enabled-ruleset",
            "primary",
        )]);
        profile.rule_sets = None;
        match validate_profile_dns_rules(&profile) {
            Err(crate::error::CommandError::InvalidInput(message)) => {
                assert!(message.contains("unknown ruleset"));
            }
            other => panic!("expected missing ruleset validation error, got {other:?}"),
        }
    }

    #[test]
    fn resolves_bundled_and_dev_core_binary_paths() {
        let temp = std::env::temp_dir().join(format!(
            "zeytun-test-{}",
            crate::core::constants::unix_millis()
        ));
        let bundled_path = temp
            .join("resources")
            .join("bin")
            .join(platform_key())
            .join("zeytun-core");
        std::fs::create_dir_all(bundled_path.parent().unwrap()).unwrap();
        std::fs::write(&bundled_path, b"mock").unwrap();

        assert_eq!(
            find_zeytun_core_binary_in_resource_dir(&temp, "zeytun-core"),
            Some(bundled_path)
        );
        let _ = std::fs::remove_dir_all(&temp);
    }

    #[test]
    fn reapply_system_proxy_calls_lifecycle_when_enabled_and_running() {
        let code = include_str!("manager.rs");
        let reapply = code.split("pub fn reapply_system_proxy(").nth(1).unwrap();
        let reapply = reapply.split("pub fn set_allow_lan(").next().unwrap();
        assert!(reapply.contains("self.profile.local_proxy.system_proxy.unwrap_or(false)"));
        assert!(reapply.contains("self.is_running()"));
        assert!(reapply.contains("self.lifecycle.set_system_proxy(true, &self.profile)"));
    }
}

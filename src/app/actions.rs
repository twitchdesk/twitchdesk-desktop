use tracing::warn;

use crate::models::{
    AuthLoginRequest, AuthLoginResponse, AuthRegisterRequest, AuthRegisterResponse, ChannelAddRequest,
    ChannelStatus, ChannelsResponse,
    MeResponse,
    TwitchValidateResponse,
    TwitchOAuthStartResponse,
    TemplateCreateRequest, TemplateDetailResponse, TemplateDuplicateRequest, TemplateVersionCreateRequest,
    TemplateVersionResponse, TemplateVersionUpdateRequest,
    TemplatesListResponse,
    AiTokenStatusResponse, AiTokenUpsertRequest,
    AiAlertCreateRequest, AiAlertUpdateRequest,
    AiAlertsListResponse, AiAlertDetailResponse,
    AiAlertPublicStatusResponse,
    AiAlertFireRequest, AiAlertFireResponse,
};

use super::{state::TwitchDeskApp, types::Screen};

impl TwitchDeskApp {
    fn api_base_and_token(&self) -> Result<(String, String), String> {
        let token = self
            .local
            .access_token
            .clone()
            .ok_or_else(|| "Missing access token. Login first.".to_string())?;
        if token.trim().is_empty() {
            return Err("Missing access token. Login first.".to_string());
        }
        let base = self.local.api_base_url.trim().trim_end_matches('/').to_string();
        if base.is_empty() {
            return Err("Missing API base URL".to_string());
        }
        Ok((base, token))
    }

    pub(crate) fn load_user_config_from_api(&mut self) -> Result<(), anyhow::Error> {
        let (base, token) = self.api_base_and_token().map_err(anyhow::Error::msg)?;
        let url = format!("{}/v1/users/me", base);

        let me = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .get(url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            let parsed = serde_json::from_str::<MeResponse>(&body)?;
            Ok::<_, anyhow::Error>(parsed)
        })?;

        // Sync non-secret settings from API.
        self.local.user_cfg.twitch_client_id = me.twitch_client_id;
        self.local.user_cfg.public_twitch_avatar_enabled = me.public_twitch_avatar_enabled;

        // Never fetch/persist the secret; user must re-enter it when changing.
        self.local.user_cfg.twitch_client_secret.clear();

        // After syncing config, check if Twitch creds are valid in the API.
        self.check_twitch_credentials_and_maybe_alert();

        Ok(())
    }

    fn check_twitch_credentials_and_maybe_alert(&mut self) {
        let Ok((base, token)) = self.api_base_and_token() else {
            return;
        };
        let url = format!("{}/v1/twitch/validate", base);

        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .get(url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            let parsed = serde_json::from_str::<TwitchValidateResponse>(&body)?;
            Ok::<_, anyhow::Error>(parsed)
        });

        match result {
            Ok(r) => match r.status.as_str() {
                "ok" => {}
                "missing" => {
                    self.alert_popup = Some(
                        "Twitch Client ID/Secret mangler i cloud config. Gå til Settings og gem et gyldigt Client ID + Client Secret."
                            .to_string(),
                    );
                }
                "invalid" => {
                    self.alert_popup = Some(
                        "Twitch Client Secret er ugyldigt (Twitch svarer 'invalid client secret'). Gå til Settings og gem et korrekt Client Secret."
                            .to_string(),
                    );
                }
                _ => {
                    self.alert_popup = Some(
                        "Kunne ikke validere Twitch credentials. Prøv igen senere."
                            .to_string(),
                    );
                }
            },
            Err(e) => {
                warn!(error = ?e, "twitch credential validation request failed");
                // Don't spam popups on transient network issues.
            }
        }
    }

    pub(crate) fn connect_twitch_oauth(&mut self) {
        let (base, token) = match self.api_base_and_token() {
            Ok(v) => v,
            Err(msg) => {
                self.status = msg;
                return;
            }
        };

        let url = format!("{}/v1/twitch/oauth/start", base);

        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .get(url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            let parsed = serde_json::from_str::<TwitchOAuthStartResponse>(&body)?;
            Ok::<_, anyhow::Error>(parsed)
        });

        match result {
            Ok(r) => match webbrowser::open(&r.url) {
                Ok(_) => self.status = "Opened Twitch OAuth in browser.".to_string(),
                Err(e) => self.status = format!("Open browser failed: {e}"),
            },
            Err(e) => {
                warn!(error = ?e, "twitch oauth start failed");
                self.status = format!("Twitch OAuth start failed: {e:#}");
            }
        }
    }

    pub(crate) fn save_user_config_to_api(&mut self) {
        let Some(token) = self.local.access_token.clone() else {
            self.status = "Missing access token. Login first.".to_string();
            return;
        };

        let base = self.local.api_base_url.trim().trim_end_matches('/').to_string();
        let url = format!("{}/v1/users/me", base);
        let cfg = self.local.user_cfg.clone();

        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .put(url)
                .header("Authorization", format!("Bearer {}", token))
                .json(&cfg)
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            Ok::<_, anyhow::Error>(())
        });

        match result {
            Ok(()) => {
                // We never want to keep secrets around longer than needed.
                self.local.user_cfg.twitch_client_secret.clear();
                self.status = "Saved settings to cloud API.".to_string();
            }
            Err(e) => {
                warn!(error = ?e, "save user config failed");
                self.status = format!("Save to API failed: {e:#}");
            }
        }
    }

    // -------------------------------
    // AI Alerts
    // -------------------------------

    pub(crate) fn ai_token_refresh_status(&mut self) {
        let (base, token) = match self.api_base_and_token() {
            Ok(v) => v,
            Err(msg) => {
                self.ai_status = msg;
                return;
            }
        };

        let url = format!("{}/v1/ai/token", base);
        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .get(url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            let parsed = serde_json::from_str::<AiTokenStatusResponse>(&body)?;
            Ok::<_, anyhow::Error>(parsed)
        });

        match result {
            Ok(r) => {
                self.ai_token_connected = Some(r.connected);
                self.ai_status = if r.connected {
                    "OpenAI token connected.".to_string()
                } else {
                    "OpenAI token not connected.".to_string()
                };
            }
            Err(e) => {
                warn!(error = ?e, "ai token status failed");
                self.ai_status = format!("Token status failed: {e:#}");
            }
        }
    }

    pub(crate) fn ai_token_save(&mut self) {
        let token_value = self.ai_token_input.trim().to_string();
        if token_value.is_empty() {
            self.ai_status = "Missing OpenAI token".to_string();
            return;
        }

        let (base, token) = match self.api_base_and_token() {
            Ok(v) => v,
            Err(msg) => {
                self.ai_status = msg;
                return;
            }
        };

        let url = format!("{}/v1/ai/token", base);
        let req = AiTokenUpsertRequest { token: token_value };

        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .put(url)
                .header("Authorization", format!("Bearer {}", token))
                .json(&req)
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            Ok::<_, anyhow::Error>(())
        });

        match result {
            Ok(()) => {
                self.ai_token_input.clear();
                self.ai_token_connected = Some(true);
                self.ai_status = "Saved OpenAI token to cloud API.".to_string();
            }
            Err(e) => {
                warn!(error = ?e, "ai token save failed");
                self.ai_status = format!("Save token failed: {e:#}");
            }
        }
    }

    pub(crate) fn ai_token_disconnect(&mut self) {
        let (base, token) = match self.api_base_and_token() {
            Ok(v) => v,
            Err(msg) => {
                self.ai_status = msg;
                return;
            }
        };

        let url = format!("{}/v1/ai/token", base);
        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .delete(url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            Ok::<_, anyhow::Error>(())
        });

        match result {
            Ok(()) => {
                self.ai_token_connected = Some(false);
                self.ai_status = "Disconnected OpenAI token.".to_string();
            }
            Err(e) => {
                warn!(error = ?e, "ai token delete failed");
                self.ai_status = format!("Disconnect token failed: {e:#}");
            }
        }
    }

    pub(crate) fn ai_alerts_refresh_list(&mut self) {
        let (base, token) = match self.api_base_and_token() {
            Ok(v) => v,
            Err(msg) => {
                self.ai_status = msg;
                return;
            }
        };

        let url = format!("{}/v1/ai/alerts", base);
        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .get(url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            let parsed = serde_json::from_str::<AiAlertsListResponse>(&body)?;
            Ok::<_, anyhow::Error>(parsed)
        });

        match result {
            Ok(resp) => {
                self.ai_alerts_list = resp.alerts;
                self.ai_status = "Alerts refreshed.".to_string();
            }
            Err(e) => {
                warn!(error = ?e, "ai alerts list failed");
                self.ai_status = format!("Alerts refresh failed: {e:#}");
            }
        }
    }

    pub(crate) fn ai_alerts_clear_editor(&mut self) {
        self.ai_alerts_selected_id = None;
        self.ai_alerts_name.clear();
        self.ai_alerts_prompt.clear();
        self.ai_alerts_is_enabled = true;
        self.ai_alerts_cooldown_ms = 0;
        self.ai_public_enabled = false;
        self.ai_public_url.clear();
        self.ai_test_result.clear();
    }

    pub(crate) fn ai_alerts_select(&mut self, alert_id: &str) {
        let (base, token) = match self.api_base_and_token() {
            Ok(v) => v,
            Err(msg) => {
                self.ai_status = msg;
                return;
            }
        };

        let url = format!(
            "{}/v1/ai/alerts/{}",
            base,
            urlencoding::encode(alert_id.trim())
        );

        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .get(url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            let parsed = serde_json::from_str::<AiAlertDetailResponse>(&body)?;
            Ok::<_, anyhow::Error>(parsed)
        });

        match result {
            Ok(detail) => {
                self.ai_alerts_selected_id = Some(detail.id);
                self.ai_alerts_name = detail.name;
                self.ai_alerts_prompt = detail.prompt;
                self.ai_alerts_is_enabled = detail.is_enabled;
                self.ai_alerts_cooldown_ms = detail.cooldown_ms;
                self.ai_test_result.clear();
                self.ai_alert_public_refresh();
                self.ai_status = "Alert loaded.".to_string();
            }
            Err(e) => {
                warn!(error = ?e, "ai alert get failed");
                self.ai_status = format!("Load alert failed: {e:#}");
            }
        }
    }

    pub(crate) fn ai_alerts_create(&mut self) {
        let name = self.ai_alerts_name.trim().to_string();
        if name.is_empty() {
            self.ai_status = "Missing alert name".to_string();
            return;
        }
        let prompt = self.ai_alerts_prompt.trim().to_string();
        if prompt.is_empty() {
            self.ai_status = "Missing prompt".to_string();
            return;
        }

        let (base, token) = match self.api_base_and_token() {
            Ok(v) => v,
            Err(msg) => {
                self.ai_status = msg;
                return;
            }
        };

        let url = format!("{}/v1/ai/alerts", base);
        let req = AiAlertCreateRequest {
            name,
            prompt,
            is_enabled: Some(self.ai_alerts_is_enabled),
            cooldown_ms: Some(self.ai_alerts_cooldown_ms.max(0)),
        };

        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .post(url)
                .header("Authorization", format!("Bearer {}", token))
                .json(&req)
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            let parsed = serde_json::from_str::<AiAlertDetailResponse>(&body)?;
            Ok::<_, anyhow::Error>(parsed)
        });

        match result {
            Ok(created) => {
                self.ai_status = "Alert created.".to_string();
                self.ai_alerts_selected_id = Some(created.id.clone());
                self.ai_alerts_refresh_list();
                self.ai_alerts_select(&created.id);
            }
            Err(e) => {
                warn!(error = ?e, "ai alert create failed");
                self.ai_status = format!("Create alert failed: {e:#}");
            }
        }
    }

    pub(crate) fn ai_alerts_update(&mut self) {
        let Some(alert_id) = self.ai_alerts_selected_id.clone() else {
            self.ai_status = "Select an alert first".to_string();
            return;
        };

        let name = self.ai_alerts_name.trim().to_string();
        if name.is_empty() {
            self.ai_status = "Missing alert name".to_string();
            return;
        }
        let prompt = self.ai_alerts_prompt.trim().to_string();
        if prompt.is_empty() {
            self.ai_status = "Missing prompt".to_string();
            return;
        }

        let (base, token) = match self.api_base_and_token() {
            Ok(v) => v,
            Err(msg) => {
                self.ai_status = msg;
                return;
            }
        };

        let url = format!(
            "{}/v1/ai/alerts/{}",
            base,
            urlencoding::encode(alert_id.trim())
        );
        let req = AiAlertUpdateRequest {
            name: Some(name),
            prompt: Some(prompt),
            is_enabled: Some(self.ai_alerts_is_enabled),
            cooldown_ms: Some(self.ai_alerts_cooldown_ms.max(0)),
        };

        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .put(url)
                .header("Authorization", format!("Bearer {}", token))
                .json(&req)
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            let parsed = serde_json::from_str::<AiAlertDetailResponse>(&body)?;
            Ok::<_, anyhow::Error>(parsed)
        });

        match result {
            Ok(_updated) => {
                self.ai_status = "Alert updated.".to_string();
                self.ai_alerts_refresh_list();
                self.ai_alert_public_refresh();
            }
            Err(e) => {
                warn!(error = ?e, "ai alert update failed");
                self.ai_status = format!("Update alert failed: {e:#}");
            }
        }
    }

    pub(crate) fn ai_alerts_delete(&mut self) {
        let Some(alert_id) = self.ai_alerts_selected_id.clone() else {
            self.ai_status = "Select an alert first".to_string();
            return;
        };

        let (base, token) = match self.api_base_and_token() {
            Ok(v) => v,
            Err(msg) => {
                self.ai_status = msg;
                return;
            }
        };

        let url = format!(
            "{}/v1/ai/alerts/{}",
            base,
            urlencoding::encode(alert_id.trim())
        );

        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .delete(url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            Ok::<_, anyhow::Error>(())
        });

        match result {
            Ok(()) => {
                self.ai_status = "Alert deleted.".to_string();
                self.ai_alerts_clear_editor();
                self.ai_alerts_refresh_list();
            }
            Err(e) => {
                warn!(error = ?e, "ai alert delete failed");
                self.ai_status = format!("Delete alert failed: {e:#}");
            }
        }
    }

    pub(crate) fn ai_alert_public_refresh(&mut self) {
        let Some(alert_id) = self.ai_alerts_selected_id.clone() else {
            return;
        };
        let (base, token) = match self.api_base_and_token() {
            Ok(v) => v,
            Err(msg) => {
                self.ai_status = msg;
                return;
            }
        };

        let url = format!(
            "{}/v1/ai/alerts/{}/public",
            base,
            urlencoding::encode(alert_id.trim())
        );

        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .get(url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            let parsed = serde_json::from_str::<AiAlertPublicStatusResponse>(&body)?;
            Ok::<_, anyhow::Error>(parsed)
        });

        match result {
            Ok(r) => {
                self.ai_public_enabled = r.enabled;
                self.ai_public_url = r.public_url.unwrap_or_default();
            }
            Err(e) => {
                warn!(error = ?e, "ai public status failed");
                self.ai_status = format!("Public status failed: {e:#}");
            }
        }
    }

    pub(crate) fn ai_alert_public_enable(&mut self) {
        let Some(alert_id) = self.ai_alerts_selected_id.clone() else {
            self.ai_status = "Select an alert first".to_string();
            return;
        };
        let (base, token) = match self.api_base_and_token() {
            Ok(v) => v,
            Err(msg) => {
                self.ai_status = msg;
                return;
            }
        };

        let url = format!(
            "{}/v1/ai/alerts/{}/public",
            base,
            urlencoding::encode(alert_id.trim())
        );

        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .post(url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            let parsed = serde_json::from_str::<AiAlertPublicStatusResponse>(&body)?;
            Ok::<_, anyhow::Error>(parsed)
        });

        match result {
            Ok(r) => {
                self.ai_public_enabled = r.enabled;
                self.ai_public_url = r.public_url.unwrap_or_default();
                self.ai_status = "Public trigger enabled.".to_string();
            }
            Err(e) => {
                warn!(error = ?e, "ai public enable failed");
                self.ai_status = format!("Enable public failed: {e:#}");
            }
        }
    }

    pub(crate) fn ai_alert_public_disable(&mut self) {
        let Some(alert_id) = self.ai_alerts_selected_id.clone() else {
            self.ai_status = "Select an alert first".to_string();
            return;
        };
        let (base, token) = match self.api_base_and_token() {
            Ok(v) => v,
            Err(msg) => {
                self.ai_status = msg;
                return;
            }
        };

        let url = format!(
            "{}/v1/ai/alerts/{}/public",
            base,
            urlencoding::encode(alert_id.trim())
        );

        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .delete(url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            let parsed = serde_json::from_str::<AiAlertPublicStatusResponse>(&body)?;
            Ok::<_, anyhow::Error>(parsed)
        });

        match result {
            Ok(r) => {
                self.ai_public_enabled = r.enabled;
                self.ai_public_url = r.public_url.unwrap_or_default();
                self.ai_status = "Public trigger disabled.".to_string();
            }
            Err(e) => {
                warn!(error = ?e, "ai public disable failed");
                self.ai_status = format!("Disable public failed: {e:#}");
            }
        }
    }

    pub(crate) fn ai_alert_test_fire(&mut self) {
        let url = self.ai_public_url.trim().to_string();
        if url.is_empty() {
            self.ai_test_result = "Public URL missing. Enable public first.".to_string();
            return;
        }

        let event_id = self.ai_test_event_id.trim().to_string();
        if event_id.is_empty() {
            self.ai_test_result = "Missing event_id".to_string();
            return;
        }

        let username = self.ai_test_username.trim().to_string();
        let message = self.ai_test_message.trim().to_string();
        let req = AiAlertFireRequest {
            event_id,
            username: if username.is_empty() { None } else { Some(username) },
            message: if message.is_empty() { None } else { Some(message) },
        };

        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http.post(url).json(&req).send().await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            let parsed = serde_json::from_str::<AiAlertFireResponse>(&body)?;
            Ok::<_, anyhow::Error>(parsed)
        });

        match result {
            Ok(r) => {
                let mut out = format!("status: {}", r.status);
                if let Some(t) = r.text {
                    out.push_str("\n\n");
                    out.push_str(&t);
                }
                self.ai_test_result = out;
            }
            Err(e) => {
                warn!(error = ?e, "ai test fire failed");
                self.ai_test_result = format!("Fire failed: {e:#}");
            }
        }
    }
    pub(crate) fn register_user(&mut self) {
        let base = self.local.api_base_url.trim().trim_end_matches('/').to_string();
        let url = format!("{}/v1/auth/register", base);

        let req = AuthRegisterRequest {
            username: self.username.trim().to_string(),
            password: self.password.clone(),
            config: self.local.user_cfg.clone(),
        };

        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http.post(url).json(&req).send().await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            let parsed = serde_json::from_str::<AuthRegisterResponse>(&body)?;
            Ok::<_, anyhow::Error>(parsed)
        });

        match result {
            Ok(r) => {
                self.local.username = Some(self.username.trim().to_string());
                self.local.access_token = Some(r.access_token);
                self.save_local();
                if let Err(e) = self.load_user_config_from_api() {
                    warn!(error = ?e, "load user config after register failed");
                }
                // Also validate Twitch creds right after auth.
                self.check_twitch_credentials_and_maybe_alert();
                self.status = "Registered. Saved bearer token locally.".to_string();
                self.start_transition(Screen::Dashboard);
            }
            Err(e) => {
                warn!(error = ?e, "register failed");
                self.status = format!("Register failed: {e:#}");
            }
        }
    }

    pub(crate) fn login_user(&mut self) {
        let base = self.local.api_base_url.trim().trim_end_matches('/').to_string();
        let url = format!("{}/v1/auth/login", base);

        let req = AuthLoginRequest {
            username: self.username.trim().to_string(),
            password: self.password.clone(),
        };

        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http.post(url).json(&req).send().await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            let parsed = serde_json::from_str::<AuthLoginResponse>(&body)?;
            Ok::<_, anyhow::Error>(parsed)
        });

        match result {
            Ok(r) => {
                self.local.username = Some(self.username.trim().to_string());
                self.local.access_token = Some(r.access_token);
                self.save_local();
                if let Err(e) = self.load_user_config_from_api() {
                    warn!(error = ?e, "load user config after login failed");
                }
                // Also validate Twitch creds right after auth.
                self.check_twitch_credentials_and_maybe_alert();
                self.status = "Logged in. Saved bearer token locally.".to_string();
                self.start_transition(Screen::Dashboard);
            }
            Err(e) => {
                warn!(error = ?e, "login failed");
                self.status = format!("Login failed: {e:#}");
            }
        }
    }

    pub(crate) fn test_twitch_lookup(&mut self) {
        let Some(token) = self.local.access_token.clone() else {
            self.test_result = "Missing access token. Register/Login first.".to_string();
            return;
        };
        let base = self.local.api_base_url.trim().trim_end_matches('/').to_string();
        let url = format!(
            "{}/v1/twitch/users?login={}",
            base,
            urlencoding::encode(self.test_login.trim())
        );

        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .get(url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            Ok::<_, anyhow::Error>(body)
        });

        match result {
            Ok(body) => self.test_result = body,
            Err(e) => self.test_result = format!("Request failed: {e:#}"),
        }
    }

    pub(crate) fn refresh_channel_statuses(&mut self) {
        let Some(token) = self.local.access_token.clone() else {
            self.status = "Missing access token. Login first.".to_string();
            return;
        };
        let base = self.local.api_base_url.trim().trim_end_matches('/').to_string();
        let url = format!("{}/v1/channels/status", base);

        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .get(url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            let parsed = serde_json::from_str::<Vec<ChannelStatus>>(&body)?;
            Ok::<_, anyhow::Error>(parsed)
        });

        match result {
            Ok(list) => {
                self.channel_statuses = list;
                self.status = "Channels refreshed.".to_string();
            }
            Err(e) => {
                warn!(error = ?e, "refresh channels failed");
                self.status = format!("Refresh channels failed: {e:#}");
            }
        }
    }

    pub(crate) fn add_channel(&mut self) {
        let Some(token) = self.local.access_token.clone() else {
            self.status = "Missing access token. Login first.".to_string();
            return;
        };
        let base = self.local.api_base_url.trim().trim_end_matches('/').to_string();
        let url = format!("{}/v1/channels", base);

        let login = self.channel_to_add.trim().to_string();
        if login.is_empty() {
            self.status = "Missing channel login".to_string();
            return;
        }

        let req = ChannelAddRequest { login };

        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .post(url)
                .header("Authorization", format!("Bearer {}", token))
                .json(&req)
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            let parsed = serde_json::from_str::<ChannelsResponse>(&body)?;
            Ok::<_, anyhow::Error>(parsed)
        });

        match result {
            Ok(_updated) => {
                self.channel_to_add.clear();
                self.refresh_channel_statuses();
            }
            Err(e) => {
                warn!(error = ?e, "add channel failed");
                self.status = format!("Add channel failed: {e:#}");
            }
        }
    }

    pub(crate) fn remove_channel(&mut self, login: &str) {
        let Some(token) = self.local.access_token.clone() else {
            self.status = "Missing access token. Login first.".to_string();
            return;
        };
        let base = self.local.api_base_url.trim().trim_end_matches('/').to_string();
        let url = format!(
            "{}/v1/channels/{}",
            base,
            urlencoding::encode(login.trim())
        );

        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .delete(url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            let parsed = serde_json::from_str::<ChannelsResponse>(&body)?;
            Ok::<_, anyhow::Error>(parsed)
        });

        match result {
            Ok(_updated) => {
                self.refresh_channel_statuses();
            }
            Err(e) => {
                warn!(error = ?e, "remove channel failed");
                self.status = format!("Remove channel failed: {e:#}");
            }
        }
    }

    // -------------------------------
    // Templates
    // -------------------------------

    pub(crate) fn templates_refresh_list(&mut self) {
        let (base, token) = match self.api_base_and_token() {
            Ok(v) => v,
            Err(msg) => {
                self.templates_status = msg;
                return;
            }
        };

        let url = format!("{}/api/templates", base);
        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .get(url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            let parsed = serde_json::from_str::<TemplatesListResponse>(&body)?;
            Ok::<_, anyhow::Error>(parsed)
        });

        match result {
            Ok(resp) => {
                self.templates_list = resp.templates;
                self.templates_status = "Templates refreshed.".to_string();
            }
            Err(e) => {
                warn!(error = ?e, "templates list failed");
                self.templates_status = format!("Templates refresh failed: {e:#}");
            }
        }
    }

    pub(crate) fn templates_create(&mut self) {
        let name = self.templates_new_name.trim().to_string();
        if name.is_empty() {
            self.templates_status = "Missing template name".to_string();
            return;
        }

        let (base, token) = match self.api_base_and_token() {
            Ok(v) => v,
            Err(msg) => {
                self.templates_status = msg;
                return;
            }
        };

        let url = format!("{}/api/templates", base);
        let req = TemplateCreateRequest { name };

        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .post(url)
                .header("Authorization", format!("Bearer {}", token))
                .json(&req)
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            let parsed = serde_json::from_str::<TemplateDetailResponse>(&body)?;
            Ok::<_, anyhow::Error>(parsed)
        });

        match result {
            Ok(created) => {
                self.templates_new_name.clear();
                self.templates_selected_template_id = Some(created.id.clone());
                self.templates_selected_template_name = Some(created.name.clone());
                self.templates_versions = created.versions.clone();
                self.templates_selected_version = created
                    .versions
                    .first()
                    .map(|v| v.version.clone());
                self.templates_status = "Template created.".to_string();
                self.templates_refresh_list();
                if let Some(ver) = self.templates_selected_version.clone() {
                    self.templates_load_version(&created.id, &ver);
                }
            }
            Err(e) => {
                warn!(error = ?e, "template create failed");
                self.templates_status = format!("Create template failed: {e:#}");
            }
        }
    }

    pub(crate) fn templates_select_template(&mut self, template_id: &str) {
        let (base, token) = match self.api_base_and_token() {
            Ok(v) => v,
            Err(msg) => {
                self.templates_status = msg;
                return;
            }
        };

        let url = format!(
            "{}/api/templates/{}",
            base,
            urlencoding::encode(template_id.trim())
        );

        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .get(url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            let parsed = serde_json::from_str::<TemplateDetailResponse>(&body)?;
            Ok::<_, anyhow::Error>(parsed)
        });

        match result {
            Ok(detail) => {
                self.templates_selected_template_id = Some(detail.id.clone());
                self.templates_selected_template_name = Some(detail.name.clone());
                self.templates_versions = detail.versions.clone();
                self.templates_selected_version = detail.versions.first().map(|v| v.version.clone());
                self.templates_status = "Template loaded.".to_string();

                if let Some(ver) = self.templates_selected_version.clone() {
                    self.templates_load_version(&detail.id, &ver);
                } else {
                    self.templates_index_html.clear();
                    self.templates_style_css.clear();
                    self.templates_overlay_js.clear();
                }
            }
            Err(e) => {
                warn!(error = ?e, "template detail failed");
                self.templates_status = format!("Load template failed: {e:#}");
            }
        }
    }

    pub(crate) fn templates_load_version(&mut self, template_id: &str, version: &str) {
        let (base, token) = match self.api_base_and_token() {
            Ok(v) => v,
            Err(msg) => {
                self.templates_status = msg;
                return;
            }
        };

        let url = format!(
            "{}/api/templates/{}/versions/{}",
            base,
            urlencoding::encode(template_id.trim()),
            urlencoding::encode(version.trim())
        );

        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .get(url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            let parsed = serde_json::from_str::<TemplateVersionResponse>(&body)?;
            Ok::<_, anyhow::Error>(parsed)
        });

        match result {
            Ok(v) => {
                self.templates_selected_version = Some(v.version.clone());
                self.templates_index_html = v.index_html;
                self.templates_style_css = v.style_css;
                self.templates_overlay_js = v.overlay_js;
                self.templates_status = format!("Loaded version {}.", v.version);
            }
            Err(e) => {
                warn!(error = ?e, "version load failed");
                self.templates_status = format!("Load version failed: {e:#}");
            }
        }
    }

    pub(crate) fn templates_save_current_version(&mut self) {
        let Some(template_id) = self.templates_selected_template_id.clone() else {
            self.templates_status = "Select a template first".to_string();
            return;
        };
        let Some(version) = self.templates_selected_version.clone() else {
            self.templates_status = "Select a version first".to_string();
            return;
        };

        let (base, token) = match self.api_base_and_token() {
            Ok(v) => v,
            Err(msg) => {
                self.templates_status = msg;
                return;
            }
        };

        let url = format!(
            "{}/api/templates/{}/versions/{}",
            base,
            urlencoding::encode(template_id.trim()),
            urlencoding::encode(version.trim())
        );
        let req = TemplateVersionUpdateRequest {
            index_html: self.templates_index_html.clone(),
            style_css: self.templates_style_css.clone(),
            overlay_js: self.templates_overlay_js.clone(),
        };

        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .put(url)
                .header("Authorization", format!("Bearer {}", token))
                .json(&req)
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            let parsed = serde_json::from_str::<TemplateVersionResponse>(&body)?;
            Ok::<_, anyhow::Error>(parsed)
        });

        match result {
            Ok(v) => {
                self.templates_status = format!("Saved version {}.", v.version);
                self.templates_select_template(&template_id);
            }
            Err(e) => {
                warn!(error = ?e, "version save failed");
                self.templates_status = format!("Save failed: {e:#}");
            }
        }
    }

    pub(crate) fn templates_publish_current_version(&mut self) {
        let Some(template_id) = self.templates_selected_template_id.clone() else {
            self.templates_status = "Select a template first".to_string();
            return;
        };
        let Some(version) = self.templates_selected_version.clone() else {
            self.templates_status = "Select a version first".to_string();
            return;
        };

        let (base, token) = match self.api_base_and_token() {
            Ok(v) => v,
            Err(msg) => {
                self.templates_status = msg;
                return;
            }
        };

        let url = format!(
            "{}/api/templates/{}/versions/{}/publish",
            base,
            urlencoding::encode(template_id.trim()),
            urlencoding::encode(version.trim())
        );

        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .post(url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            let parsed = serde_json::from_str::<TemplateVersionResponse>(&body)?;
            Ok::<_, anyhow::Error>(parsed)
        });

        match result {
            Ok(v) => {
                self.templates_status = format!("Published version {}.", v.version);
                self.templates_select_template(&template_id);
            }
            Err(e) => {
                warn!(error = ?e, "publish failed");
                self.templates_status = format!("Publish failed: {e:#}");
            }
        }
    }

    pub(crate) fn templates_create_version_from_current(&mut self) {
        let Some(template_id) = self.templates_selected_template_id.clone() else {
            self.templates_status = "Select a template first".to_string();
            return;
        };
        let source = self.templates_selected_version.clone();
        let new_version = self.templates_new_version.trim().to_string();
        if new_version.is_empty() {
            self.templates_status = "Missing new version".to_string();
            return;
        }

        let (base, token) = match self.api_base_and_token() {
            Ok(v) => v,
            Err(msg) => {
                self.templates_status = msg;
                return;
            }
        };

        let url = format!(
            "{}/api/templates/{}/versions",
            base,
            urlencoding::encode(template_id.trim())
        );
        let req = TemplateVersionCreateRequest {
            new_version: new_version.clone(),
            source_version: source,
        };

        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .post(url)
                .header("Authorization", format!("Bearer {}", token))
                .json(&req)
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            let parsed = serde_json::from_str::<TemplateVersionResponse>(&body)?;
            Ok::<_, anyhow::Error>(parsed)
        });

        match result {
            Ok(v) => {
                self.templates_new_version.clear();
                self.templates_status = format!("Created version {}.", v.version);
                self.templates_select_template(&template_id);
                self.templates_load_version(&template_id, &v.version);
            }
            Err(e) => {
                warn!(error = ?e, "create version failed");
                self.templates_status = format!("Create version failed: {e:#}");
            }
        }
    }

    pub(crate) fn templates_duplicate_template(&mut self) {
        let Some(template_id) = self.templates_selected_template_id.clone() else {
            self.templates_status = "Select a template first".to_string();
            return;
        };
        let new_name = self.templates_duplicate_template_name.trim().to_string();
        if new_name.is_empty() {
            self.templates_status = "Missing new template name".to_string();
            return;
        }

        let (base, token) = match self.api_base_and_token() {
            Ok(v) => v,
            Err(msg) => {
                self.templates_status = msg;
                return;
            }
        };

        let url = format!(
            "{}/api/templates/{}/duplicate",
            base,
            urlencoding::encode(template_id.trim())
        );
        let req = TemplateDuplicateRequest { new_name };

        let result = self.rt.block_on(async {
            let http = reqwest::Client::new();
            let resp = http
                .post(url)
                .header("Authorization", format!("Bearer {}", token))
                .json(&req)
                .send()
                .await?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                anyhow::bail!("HTTP {}: {}", status, body);
            }
            let parsed = serde_json::from_str::<TemplateDetailResponse>(&body)?;
            Ok::<_, anyhow::Error>(parsed)
        });

        match result {
            Ok(t) => {
                self.templates_duplicate_template_name.clear();
                self.templates_status = "Template duplicated.".to_string();
                self.templates_refresh_list();
                self.templates_select_template(&t.id);
            }
            Err(e) => {
                warn!(error = ?e, "duplicate template failed");
                self.templates_status = format!("Duplicate template failed: {e:#}");
            }
        }
    }
}

use tracing::warn;

use crate::models::{
    AuthLoginRequest, AuthLoginResponse, AuthRegisterRequest, AuthRegisterResponse, ChannelAddRequest,
    ChannelStatus, ChannelsResponse,
    MeResponse,
    TemplateCreateRequest, TemplateDetailResponse, TemplateDuplicateRequest, TemplateVersionCreateRequest,
    TemplateVersionResponse, TemplateVersionUpdateRequest,
    TemplatesListResponse,
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

        Ok(())
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

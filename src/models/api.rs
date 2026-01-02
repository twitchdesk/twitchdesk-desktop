use serde::{Deserialize, Serialize};

/// Data stored in the cloud API (contains secrets). Do not log.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserConfig {
    pub twitch_client_id: String,
    pub twitch_client_secret: String,

    pub twitch_channel: Option<String>,
    #[serde(default)]
    pub twitch_channels: Vec<String>,
    pub twitch_bot_username: Option<String>,
    pub twitch_irc_oauth_token: Option<String>,

    pub twitch_user_access_token: Option<String>,
    pub twitch_refresh_token: Option<String>,

    #[serde(default)]
    pub public_twitch_avatar_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub config: UserConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterResponse {
    pub user_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRegisterRequest {
    pub username: String,
    pub password: String,
    pub config: UserConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRegisterResponse {
    pub access_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthLoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthLoginResponse {
    pub access_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelAddRequest {
    pub login: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelsResponse {
    pub channels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelStatus {
    pub login: String,
    pub is_live: bool,
}

// -------------------------------
// Overlay Templates (OBS)
// -------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateCreateRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateDuplicateRequest {
    pub new_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVersionCreateRequest {
    pub new_version: String,
    #[serde(default)]
    pub source_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVersionDuplicateRequest {
    pub new_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVersionUpdateRequest {
    pub index_html: String,
    pub style_css: String,
    pub overlay_js: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVersionSummary {
    pub version: String,
    pub is_published: bool,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateListItem {
    pub id: String,
    pub name: String,
    pub updated_at: String,
    pub versions: Vec<TemplateVersionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplatesListResponse {
    pub templates: Vec<TemplateListItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateDetailResponse {
    pub id: String,
    pub name: String,
    pub updated_at: String,
    pub versions: Vec<TemplateVersionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVersionResponse {
    pub template_id: String,
    pub version: String,
    pub is_published: bool,
    pub updated_at: String,
    pub index_html: String,
    pub style_css: String,
    pub overlay_js: String,
}

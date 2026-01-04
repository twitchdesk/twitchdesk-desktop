use serde::{Deserialize, Serialize};

use super::UserConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalClientState {
    pub api_base_url: String,
    pub username: Option<String>,
    pub access_token: Option<String>,

    // Keep config in memory only; it is stored server-side via the API.
    #[serde(skip)]
    pub user_cfg: UserConfig,
}

impl Default for LocalClientState {
    fn default() -> Self {
        // NOTE: This is not a true secret (clients must know where to connect),
        // but keeping it out of the repo avoids publishing infrastructure details.
        let api_base_url = std::env::var("TWITCHDESK_API_BASE_URL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| option_env!("TWITCHDESK_API_BASE_URL").map(|v| v.to_string()))
            .unwrap_or_else(|| "https://api.twitchdesk.com".to_string());

        Self {
            api_base_url,
            username: None,
            access_token: None,
            user_cfg: UserConfig::default(),
        }
    }
}

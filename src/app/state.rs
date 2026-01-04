use eframe::egui;
use std::time::Instant;

use crate::{
    storage,
    models::LocalClientState,
};

use super::types::{Screen, View};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TemplatesEditorTab {
    Html,
    Css,
    Js,
}

pub(crate) struct TwitchDeskApp {
    pub(crate) local: LocalClientState,
    pub(crate) status: String,

    pub(crate) username: String,
    pub(crate) password: String,

    pub(crate) screen: Screen,
    pub(crate) pending_screen: Option<Screen>,
    pub(crate) transition_started_at: Option<Instant>,
    pub(crate) active_view: View,

    pub(crate) test_login: String,
    pub(crate) test_result: String,

    pub(crate) channel_to_add: String,
    pub(crate) channel_statuses: Vec<crate::models::ChannelStatus>,

    pub(crate) api_health: Option<bool>,
    pub(crate) api_health_last_checked: Option<Instant>,
    pub(crate) api_health_task: Option<tokio::task::JoinHandle<bool>>,

    // Updates
    pub(crate) update_available: Option<String>,
    pub(crate) update_last_error: Option<String>,
    pub(crate) update_check_task: Option<tokio::task::JoinHandle<Result<Option<String>, String>>>,

    // Templates
    pub(crate) templates_list: Vec<crate::models::TemplateListItem>,
    pub(crate) templates_new_name: String,
    pub(crate) templates_selected_template_id: Option<String>,
    pub(crate) templates_selected_template_name: Option<String>,
    pub(crate) templates_selected_version: Option<String>,
    pub(crate) templates_versions: Vec<crate::models::TemplateVersionSummary>,
    pub(crate) templates_editor_tab: TemplatesEditorTab,
    pub(crate) templates_index_html: String,
    pub(crate) templates_style_css: String,
    pub(crate) templates_overlay_js: String,
    pub(crate) templates_new_version: String,
    pub(crate) templates_duplicate_template_name: String,
    pub(crate) templates_status: String,

    pub(crate) alert_popup: Option<String>,

    pub(crate) rt: tokio::runtime::Runtime,
}

impl TwitchDeskApp {
    pub(crate) fn new() -> Self {
        let local = storage::load_local_state().unwrap_or_else(|_| LocalClientState::default());
        let status = match storage::local_state_path() {
            Ok(p) => format!("Local state: {}", p.display()),
            Err(_) => "Local state: <unknown>".to_string(),
        };

        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");

        let username = local.username.clone().unwrap_or_default();
        let screen = if local
            .access_token
            .as_ref()
            .map(|t| !t.is_empty())
            .unwrap_or(false)
        {
            Screen::Dashboard
        } else {
            Screen::Login
        };

        let mut app = Self {
            local,
            status,
            username,
            password: "".to_string(),
            screen,
            pending_screen: None,
            transition_started_at: None,
            active_view: View::Home,
            test_login: "someuser".to_string(),
            test_result: "".to_string(),
            channel_to_add: "".to_string(),
            channel_statuses: vec![],

            api_health: None,
            api_health_last_checked: None,
            api_health_task: None,

            update_available: None,
            update_last_error: None,
            update_check_task: None,

            templates_list: vec![],
            templates_new_name: "".to_string(),
            templates_selected_template_id: None,
            templates_selected_template_name: None,
            templates_selected_version: None,
            templates_versions: vec![],
            templates_editor_tab: TemplatesEditorTab::Html,
            templates_index_html: "".to_string(),
            templates_style_css: "".to_string(),
            templates_overlay_js: "".to_string(),
            templates_new_version: "".to_string(),
            templates_duplicate_template_name: "".to_string(),
            templates_status: "".to_string(),

            alert_popup: None,
            rt,
        };

        // If we have a persisted bearer token, sync user config from the API.
        if app.screen == Screen::Dashboard {
            let _ = app.load_user_config_from_api();
        }

        // Check for updates once on startup (release builds only).
        app.schedule_update_check();

        app
    }

    pub(crate) fn start_transition(&mut self, next: Screen) {
        self.screen = Screen::Transition;
        self.pending_screen = Some(next);
        self.transition_started_at = Some(Instant::now());
    }

    pub(crate) fn save_local(&mut self) {
        match storage::save_local_state(&self.local) {
            Ok(path) => self.status = format!("Saved local state to {}", path.display()),
            Err(e) => self.status = format!("Save local state failed: {e:#}"),
        }
    }

    pub(crate) fn logout(&mut self) {
        self.local.access_token = None;
        self.local.user_cfg = crate::models::UserConfig::default();
        self.save_local();
        self.start_transition(Screen::Login);
        self.active_view = View::Home;
        self.status = "Logged out.".to_string();
    }
}

impl eframe::App for TwitchDeskApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.tick_api_health();

        match self.screen {
            Screen::Login => {
                self.ui_header_bar(ctx);
                egui::CentralPanel::default().show(ctx, |ui| self.ui_login(ui));
            }
            Screen::Transition => {
                self.ui_header_bar(ctx);
                egui::CentralPanel::default().show(ctx, |ui| self.ui_transition(ctx, ui));
            }
            Screen::Dashboard => {
                self.ui_header_bar(ctx);
                self.ui_sidebar(ctx);
                egui::CentralPanel::default().show(ctx, |ui| self.ui_view(ui));
            }
        }

        self.ui_alert_popup(ctx);
    }
}

impl TwitchDeskApp {
    fn ui_alert_popup(&mut self, ctx: &egui::Context) {
        let Some(msg) = self.alert_popup.as_ref() else {
            return;
        };

        let mut open = true;
        let mut close_requested = false;
        egui::Window::new("Twitch")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.label(msg);
                ui.add_space(12.0);
                if ui.button("OK").clicked() {
                    close_requested = true;
                }
            });

        if close_requested {
            open = false;
        }

        if !open {
            self.alert_popup = None;
        }
    }
}

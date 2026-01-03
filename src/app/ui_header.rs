use eframe::egui;
use std::time::{Duration, Instant};

use super::{state::TwitchDeskApp, types::Screen};
use crate::update;

impl TwitchDeskApp {
    pub(crate) fn schedule_update_check(&mut self) {
        if cfg!(debug_assertions) {
            return;
        }
        if self.update_check_task.is_some() {
            return;
        }

        self.update_check_task = Some(self.rt.spawn(async move {
            // Run potentially slow IO off the UI thread.
            match update::check_update_available() {
                Ok(Some(info)) => Ok(Some(info.latest.to_string())),
                Ok(None) => Ok(None),
                Err(e) => Err(format!("{e:#}")),
            }
        }));
    }

    pub(crate) fn tick_api_health(&mut self) {
        // 1) Harvest completed background check
        if let Some(handle) = self.api_health_task.take() {
            if handle.is_finished() {
                let ok = self.rt.block_on(async { handle.await }).unwrap_or(false);
                self.api_health = Some(ok);
                self.api_health_last_checked = Some(Instant::now());
            } else {
                // Put it back if still running
                self.api_health_task = Some(handle);
            }
        }

        // 2) Schedule next check if due
        let due = self
            .api_health_last_checked
            .map(|t| t.elapsed() > Duration::from_secs(5))
            .unwrap_or(true);

        if !due || self.api_health_task.is_some() {
            return;
        }

        let base = self.local.api_base_url.trim().trim_end_matches('/').to_string();
        let url = format!("{}/health", base);
        self.api_health_task = Some(self.rt.spawn(async move {
            let client = reqwest::Client::new();
            let req = client.get(url).send();
            let resp = tokio::time::timeout(Duration::from_millis(800), req).await;
            match resp {
                Ok(Ok(r)) => r.status().is_success(),
                _ => false,
            }
        }));
    }

    fn tick_update_check(&mut self) {
        if let Some(handle) = self.update_check_task.take() {
            if handle.is_finished() {
                match self.rt.block_on(async { handle.await }) {
                    Ok(Ok(Some(v))) => {
                        self.update_available = Some(v);
                        self.update_last_error = None;

                        if update::auto_updates_enabled() {
                            // Start update flow immediately; this will exit the current process
                            // if it successfully spawns the apply helper.
                            match update::apply_update_if_available_and_exit() {
                                Ok(()) => {
                                    // If we didn't exit, there was no update.
                                    self.update_available = None;
                                }
                                Err(e) => {
                                    self.update_last_error = Some(format!("{e:#}"));
                                    self.status = format!("Auto-update failed: {e:#}");
                                }
                            }
                        }
                    }
                    Ok(Ok(None)) => {
                        self.update_available = None;
                        self.update_last_error = None;
                    }
                    Ok(Err(e)) => {
                        self.update_last_error = Some(e);
                    }
                    Err(e) => {
                        self.update_last_error = Some(format!("{e}"));
                    }
                }
            } else {
                self.update_check_task = Some(handle);
            }
        }
    }

    pub(crate) fn ui_header_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            self.tick_update_check();
            ui.horizontal(|ui| {
                ui.heading("TwitchDesk");
                ui.separator();

                // Health indicator
                let (dot, text) = match self.api_health {
                    Some(true) => (
                        egui::RichText::new("●").color(egui::Color32::GREEN),
                        "API: Online",
                    ),
                    Some(false) => (
                        egui::RichText::new("●").color(egui::Color32::RED),
                        "API: Offline",
                    ),
                    None => (
                        egui::RichText::new("●").color(egui::Color32::GRAY),
                        "API: Checking…",
                    ),
                };
                ui.label(dot);
                ui.label(text);

                // Update indicator
                if let Some(v) = self.update_available.as_deref() {
                    ui.separator();
                    ui.label(format!("Update available: v{}", v));
                    if !update::auto_updates_enabled() {
                        if ui.button("Update now").clicked() {
                            // This will spawn the helper and exit if an update is available.
                            // If anything fails, we show it in the status.
                            match update::apply_update_if_available_and_exit() {
                                Ok(()) => {
                                    self.status = "No update available.".to_string();
                                }
                                Err(e) => {
                                    self.status = format!("Update failed: {e:#}");
                                }
                            }
                        }
                    }
                } else if let Some(err) = self.update_last_error.clone() {
                    ui.separator();
                    ui.label(egui::RichText::new("Update check failed").color(egui::Color32::YELLOW));
                    if ui.button("Retry").clicked() {
                        self.update_last_error = None;
                        self.schedule_update_check();
                    }
                    if ui.button("Details").clicked() {
                        self.status = format!("Update check failed: {}", err);
                    }
                }

                // Right side actions
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Refresh").clicked() {
                        self.api_health_last_checked = None;
                        self.tick_api_health();
                    }

                    if self.screen == Screen::Dashboard {
                        if ui.button("Logout").clicked() {
                            self.logout();
                        }
                        if let Some(u) = self.local.username.as_deref() {
                            if !u.is_empty() {
                                ui.separator();
                                ui.label(format!("User: {}", u));
                            }
                        }
                    }
                });
            });
        });
    }
}

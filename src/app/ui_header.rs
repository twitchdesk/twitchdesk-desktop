use eframe::egui;
use std::time::{Duration, Instant};

use super::{state::TwitchDeskApp, types::Screen};

impl TwitchDeskApp {
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

    pub(crate) fn ui_header_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
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

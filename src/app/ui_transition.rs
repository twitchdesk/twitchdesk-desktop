use eframe::egui;
use std::time::Duration;

use super::{state::TwitchDeskApp, types::Screen};

impl TwitchDeskApp {
    pub(crate) fn ui_transition(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        let total = Duration::from_secs(3);
        let elapsed = self
            .transition_started_at
            .map(|t| t.elapsed())
            .unwrap_or(Duration::ZERO);
        let frac = (elapsed.as_secs_f32() / total.as_secs_f32()).clamp(0.0, 1.0);

        if elapsed >= total {
            if let Some(next) = self.pending_screen.take() {
                self.screen = next;
            } else {
                self.screen = Screen::Login;
            }
            self.transition_started_at = None;
            return;
        }

        ctx.request_repaint();

        ui.vertical_centered(|ui| {
            ui.add_space(48.0);
            ui.heading("TwitchDesk");
            ui.add_space(16.0);
            ui.add(
                egui::ProgressBar::new(frac)
                    .show_percentage()
                    .desired_width(360.0),
            );
            ui.add_space(8.0);
            ui.label("Loading…");
        });
    }
}

use eframe::egui;

use super::state::TwitchDeskApp;

impl TwitchDeskApp {
    pub(crate) fn ui_login(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(32.0);
            ui.heading("TwitchDesk");
            ui.add_space(8.0);

            let card = egui::Frame::group(ui.style())
                .inner_margin(egui::Margin::same(16.0))
                .rounding(egui::Rounding::same(10.0));

            card.show(ui, |ui| {
                ui.set_max_width(360.0);
                ui.label("Sign in");
                ui.add_space(8.0);

                ui.add(
                    egui::TextEdit::singleline(&mut self.username)
                        .hint_text("Username")
                        .desired_width(f32::INFINITY),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.password)
                        .hint_text("Password")
                        .password(true)
                        .desired_width(f32::INFINITY),
                );

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Login").clicked() {
                        self.login_user();
                    }
                    if ui.button("Register").clicked() {
                        self.register_user();
                    }
                });

                if !self.status.is_empty() {
                    ui.add_space(8.0);
                    ui.label(&self.status);
                }
            });
        });
    }
}

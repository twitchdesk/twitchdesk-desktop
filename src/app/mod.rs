use anyhow::Result;

mod actions;
mod state;
mod types;
mod ui_dashboard;
mod ui_header;
mod ui_login;
mod ui_transition;

use state::TwitchDeskApp;

pub fn run_app() -> Result<()> {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "TwitchDesk",
        native_options,
        Box::new(|cc| {
            apply_theme(&cc.egui_ctx);
            Ok(Box::new(TwitchDeskApp::new()))
        }),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))
}

fn apply_theme(ctx: &egui::Context) {
    let accent = egui::Color32::from_rgb(0x7C, 0x5C, 0xFF); // #7c5cff
    let accent2 = egui::Color32::from_rgb(0x00, 0xF5, 0xD4); // #00f5d4
    let bg = egui::Color32::from_rgb(0x07, 0x0A, 0x18); // #070A18
    let panel = egui::Color32::from_rgba_premultiplied(0x12, 0x16, 0x2A, 220);

    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.panel_fill = bg;
    style.visuals.window_fill = panel;
    style.visuals.faint_bg_color = egui::Color32::from_rgb(0x0B, 0x0F, 0x22);

    style.visuals.widgets.noninteractive.bg_fill = panel;
    style.visuals.widgets.noninteractive.fg_stroke.color = egui::Color32::from_rgb(0xC9, 0xD2, 0xFF);

    style.visuals.widgets.inactive.rounding = egui::Rounding::same(16.0);
    style.visuals.widgets.hovered.rounding = egui::Rounding::same(16.0);
    style.visuals.widgets.active.rounding = egui::Rounding::same(16.0);
    style.visuals.window_rounding = egui::Rounding::same(18.0);

    style.visuals.selection.bg_fill = accent.linear_multiply(0.55);
    style.visuals.selection.stroke.color = accent;

    style.visuals.hyperlink_color = accent2;
    style.visuals.error_fg_color = egui::Color32::from_rgb(0xFF, 0x4D, 0x6D);
    style.visuals.warn_fg_color = egui::Color32::from_rgb(0xFF, 0xC1, 0x4D);

    // Subtle strokes similar to the HTML's glass border.
    let border = egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(0x7C, 0x5C, 0xFF, 60));
    style.visuals.window_stroke = border;
    style.visuals.widgets.noninteractive.bg_stroke = border;
    style.visuals.widgets.inactive.bg_stroke = border;
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgba_premultiplied(0x00, 0xF5, 0xD4, 80));
    style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, accent);

    ctx.set_style(style);
}

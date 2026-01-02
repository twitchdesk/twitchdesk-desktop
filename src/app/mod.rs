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
        Box::new(|_cc| Ok(Box::new(TwitchDeskApp::new()))),
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))
}

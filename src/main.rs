#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod models;
mod storage;
mod update;
mod preview;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive("info".parse()?),
        )
        .init();

    // Self-update on startup (release builds only).
    update::maybe_run_startup_update()?;

    app::run_app()
}

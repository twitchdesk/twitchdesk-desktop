#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod models;
mod storage;
mod update;
mod preview;
mod loading;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive("info".parse()?),
        )
        .init();

    // Internal updater apply-mode (spawned helper process).
    update::maybe_run_apply_mode_and_exit()?;

    // Show the branded HTML loading splash every launch (best effort).
    loading::show_startup_loading_splash_best_effort();

    app::run_app()
}

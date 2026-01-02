# TwitchDesk Desktop

TwitchDesk Desktop is a cross-platform desktop companion app for the TwitchDesk system.

The goal of TwitchDesk is to make it easier to be a streamer by providing a simple “control panel” for common streaming tasks (channels, settings, and overlay templates) backed by the TwitchDesk API.

## What it does

- Sign up / sign in to the TwitchDesk API
- Configure Twitch credentials (client id/secret) used by the backend
- Manage a list of Twitch channels and view live/offline status
- Create and edit overlay templates (HTML/CSS/JS) stored and served by the backend

## Downloads

Releases are published on GitHub. Download the asset matching your OS:

- Windows: `twitchdesk-desktop-windows-x86_64.zip`
- Linux: `twitchdesk-desktop-linux-x86_64.zip`
- macOS (Intel): `twitchdesk-desktop-macos-x86_64.zip`
- macOS (Apple Silicon): `twitchdesk-desktop-macos-aarch64.zip`

Each zip contains:

- `twitchdesk-desktop` (the main app)
- `twitchdesk-preview` (in-app template preview window)

## Auto-updates

Release builds check for updates on startup using GitHub Releases.

- To disable auto-updates, set `TWITCHDESK_DISABLE_UPDATES=1`.
- The updater downloads the newest release asset for your platform and restarts the app to apply it.

## Configuration

### API base URL

The app needs to know where the TwitchDesk API is running.

Priority order:

1. `TWITCHDESK_API_BASE_URL` environment variable (runtime)
2. `TWITCHDESK_API_BASE_URL` build-time environment variable embedded during CI releases
3. Fallback: `http://localhost:3000`

You can also change the API base URL in the app UI and click **Save** (it is stored locally).

> Note: The API base URL is not a “real secret” (clients must know where to connect), but we keep it out of the repository to avoid publishing infrastructure details.

## Development

Requirements:

- Rust 1.88+

Run locally:

```bash
cargo run
```

A local state file is stored in your OS user data directory (see the status line in the app).

## Releases (CI)

The GitHub Actions workflow builds and uploads release binaries when you push a tag:

```bash
git tag v0.1.0
git push origin v0.1.0
```

Optional GitHub repository secret used by the workflow:

- `TWITCHDESK_API_BASE_URL` – the default API URL embedded into release builds

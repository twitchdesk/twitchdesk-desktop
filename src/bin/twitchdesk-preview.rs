#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

#[cfg(target_os = "linux")]
use tao::platform::unix::WindowExtUnix;

use wry::WebViewBuilder;

#[cfg(target_os = "linux")]
use wry::WebViewBuilderExtUnix;

fn main() {
    let url = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: twitchdesk-preview <url>");
        std::process::exit(2);
    });

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("TwitchDesk Preview")
        .build(&event_loop)
        .expect("create window");

    let builder = WebViewBuilder::new().with_url(&url);

    #[cfg(not(target_os = "linux"))]
    let _webview = builder.build(&window).expect("build webview");

    // On Linux, using GTK build supports Wayland too.
    #[cfg(target_os = "linux")]
    let _webview = builder
        .build_gtk(window.gtk_window())
        .expect("build gtk webview");

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if let Event::WindowEvent { event, .. } = event {
            if matches!(event, WindowEvent::CloseRequested) {
                *control_flow = ControlFlow::Exit;
            }
        }
    });
}

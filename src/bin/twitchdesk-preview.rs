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
    let mut url: Option<String> = None;
    let mut auto_close_ms: Option<u64> = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--auto-close-ms" => {
                let v = args.next().unwrap_or_else(|| {
                    eprintln!("Missing value for --auto-close-ms");
                    std::process::exit(2);
                });
                auto_close_ms = Some(v.parse::<u64>().unwrap_or_else(|_| {
                    eprintln!("Invalid --auto-close-ms value: {v}");
                    std::process::exit(2);
                }));
            }
            _ => {
                if url.is_none() {
                    url = Some(arg);
                } else {
                    eprintln!("Unexpected argument: {arg}");
                    std::process::exit(2);
                }
            }
        }
    }

    let url = url.unwrap_or_else(|| {
        eprintln!("Usage: twitchdesk-preview <url> [--auto-close-ms <ms>]");
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

    let start = std::time::Instant::now();

    event_loop.run(move |event, _, control_flow| {
        if let Some(ms) = auto_close_ms {
            let deadline = start + std::time::Duration::from_millis(ms);
            *control_flow = ControlFlow::WaitUntil(deadline);
        } else {
            *control_flow = ControlFlow::Wait;
        }

        if let Some(ms) = auto_close_ms {
            if start.elapsed() >= std::time::Duration::from_millis(ms) {
                *control_flow = ControlFlow::Exit;
                return;
            }
        }

        if let Event::WindowEvent { event, .. } = event {
            if matches!(event, WindowEvent::CloseRequested) {
                *control_flow = ControlFlow::Exit;
            }
        }
    });
}

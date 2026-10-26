// SPDX-License-Identifier: MIT
// SPDX-FileCopyrightText: 2026 Benjamin Benno Falkner

//! Menu bar (tray) app for desktop users.
//!
//! The server runs on a background thread; the menu shows its address, a switch
//! for starting at login and a quit item. On macOS the app has no Dock icon, and
//! launching the bundle again opens the editor instead of a second instance.

mod autostart;

use std::{error::Error, net::TcpListener, process::Command, thread};

use edotex::config::Config;
use tao::{
    event::{Event, StartCause},
    event_loop::{ControlFlow, EventLoopBuilder},
};
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

/// Port of the desktop server.
pub const PORT: u16 = 8283;
/// Address the desktop server binds to; only reachable from this machine.
const HOST: &str = "127.0.0.1";

/// Monochrome menu bar icon (template image on macOS).
const TRAY_ICON_PNG: &[u8] = include_bytes!("../../../deploy/macos/tray-icon.png");

enum UserEvent {
    Menu(MenuEvent),
    ServerFailed(String),
}

/// Returns whether this process was started from an `.app` bundle (double-click).
pub fn launched_from_app_bundle() -> bool {
    autostart::app_bundle_path().is_some()
}

/// Starts the server in the background and runs the tray menu until quit.
/// `serve` runs the HTTP server for the given configuration and blocks.
pub fn run(
    cfg: &Config,
    serve: fn(&Config) -> Result<(), Box<dyn Error>>,
) -> Result<(), Box<dyn Error>> {
    let mut cfg = cfg.clone();
    cfg.server.server_name = HOST.to_string();
    cfg.server.port = PORT;
    let url = format!("http://localhost:{PORT}");

    #[allow(unused_mut)]
    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
        event_loop.set_activation_policy(ActivationPolicy::Accessory);
    }

    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::Menu(event));
    }));

    // A busy port usually means edotex already runs, e.g. started from a terminal.
    let port_free = TcpListener::bind((HOST, PORT)).is_ok();
    if port_free {
        let proxy = event_loop.create_proxy();
        thread::spawn(move || {
            if let Err(err) = serve(&cfg) {
                let _ = proxy.send_event(UserEvent::ServerFailed(err.to_string()));
            }
        });
    }

    let status = MenuItem::new(
        if port_free {
            format!("edotex läuft auf {url}")
        } else {
            format!("Port {PORT} ist belegt – läuft edotex schon?")
        },
        true,
        None,
    );
    let open = MenuItem::new("Editor öffnen", true, None);
    let login = CheckMenuItem::new(
        "Bei Anmeldung starten",
        autostart::is_supported(),
        autostart::is_enabled(),
        None,
    );
    let quit = MenuItem::new("edotex beenden", true, None);
    let menu = Menu::new();
    menu.append_items(&[
        &status,
        &open,
        &PredefinedMenuItem::separator(),
        &login,
        &PredefinedMenuItem::separator(),
        &quit,
    ])?;

    let icon = tray_icon()?;
    // The tray icon must be created once the event loop runs (required on macOS).
    let mut tray: Option<TrayIcon> = None;
    let mut menu = Some(menu);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::NewEvents(StartCause::Init) => {
                let builder = TrayIconBuilder::new()
                    .with_menu(Box::new(menu.take().expect("menu is built once")))
                    .with_tooltip(format!("edotex – {url}"));
                // macOS recolours a template icon to match the light or dark menu bar.
                #[cfg(target_os = "macos")]
                let builder = builder.with_icon_templated(icon.clone());
                #[cfg(not(target_os = "macos"))]
                let builder = builder.with_icon(icon.clone());
                let built = builder.build();
                match built {
                    Ok(icon) => tray = Some(icon),
                    Err(err) => {
                        eprintln!("ERROR: tray icon: {err}");
                        *control_flow = ControlFlow::Exit;
                    }
                }
            }
            // macOS: the app was launched again (double-click) while running.
            #[cfg(target_os = "macos")]
            Event::Reopen { .. } => open_url(&url),
            Event::UserEvent(UserEvent::Menu(event)) => {
                if event.id == status.id() || event.id == open.id() {
                    open_url(&url);
                } else if event.id == login.id() {
                    let enable = login.is_checked();
                    if let Err(err) = autostart::set_enabled(enable) {
                        eprintln!("ERROR: autostart: {err}");
                        login.set_checked(!enable);
                    }
                } else if event.id == quit.id() {
                    tray.take();
                    *control_flow = ControlFlow::Exit;
                }
            }
            Event::UserEvent(UserEvent::ServerFailed(message)) => {
                status.set_text(format!("Server-Fehler: {message}"));
                status.set_enabled(false);
                open.set_enabled(false);
            }
            _ => {}
        }
    })
}

/// Decodes the embedded PNG into an RGBA tray icon.
fn tray_icon() -> Result<Icon, Box<dyn Error>> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(TRAY_ICON_PNG));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info()?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("tray icon too large")?];
    let info = reader.next_frame(&mut buf)?;
    let pixels = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => pixels.to_vec(),
        png::ColorType::Rgb => pixels
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => pixels
            .chunks_exact(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => pixels.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err("indexed tray icon not expanded".into()),
    };
    Ok(Icon::from_rgba(rgba, info.width, info.height)?)
}

/// Opens `url` in the default browser.
fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let result = Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let result = Command::new("cmd").args(["/C", "start", "", url]).spawn();
    if let Err(err) = result {
        eprintln!("ERROR: open {url}: {err}");
    }
}

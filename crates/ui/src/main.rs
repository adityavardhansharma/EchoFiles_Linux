//! EchoFiles — a fast, Omarchy-native file manager.
//!
//! Flags: `echofiles [FOLDER]`, `--settings` (open Settings), `--background` (start with no
//! window; used at login), `--sync-agent-skill` (apply the saved Settings switch).
//!
//! Environment:
//! - `ECHOFILES_TIMING=1` prints startup and listing timings.
//! - `ECHOFILES_BENCH=<dir>` opens `dir`, scrolls it for 600 frames and prints frame times.
//! - `ECHOFILES_NO_FONT_PREWARM=1` restores Iced's synchronous font loading (for measurements).
//! - `WGPU_POWER_PREF`, `WGPU_BACKEND`, `VK_LOADER_DRIVERS_SELECT` override `gpu::configure`.

mod actions;
mod app;
mod drives;
mod file_list;
mod gpu;
mod indexer;
mod network;
mod kinds;
mod overlay;
mod pane;
mod phone;
mod connect;
mod phone_view;
mod preview;
mod search;
mod settings;
mod style;
mod system;
mod thumbs;
mod view;
mod widgets;

use std::sync::OnceLock;
use std::time::Instant;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

static START: OnceLock<Instant> = OnceLock::new();

pub fn since_start_ms() -> f64 {
    START.get().map_or(0.0, |s| s.elapsed().as_secs_f64() * 1000.0)
}

fn main() -> iced::Result {
    START.get_or_init(Instant::now);
    // Integrated GPU, Vulkan, and on hybrid laptops no NVIDIA driver load (see gpu.rs).
    // SAFETY: still single-threaded; nothing else reads the environment yet.
    unsafe { gpu::configure() };



    // One EchoFiles: a second launch hands its request to the running one and exits.
    let args: Vec<String> = std::env::args_os().skip(1).map(|a| a.to_string_lossy().into_owned()).collect();
    if args.iter().any(|a| a == "--sync-agent-skill") {
        if let Err(e) = system::sync_skill_from_settings() {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return Ok(());
    }
    if let Err(e) = system::sync_skill_from_settings() {
        eprintln!("{e}");
    }
    let mut owns_socket = false;
    if std::env::var_os("ECHOFILES_BENCH").is_none() {
        let request = if args.iter().any(|a| a == "--settings") {
            system::Request::Settings
        } else {
            std::env::args_os().skip(1).find(|a| !a.as_encoded_bytes().starts_with(b"--")).map(|a| system::request_for_os(&a)).unwrap_or(system::Request::Open(None))
        };
        if system::forward_to_running(&request) {
            return Ok(());
        }
        owns_socket = system::listen();
        if !owns_socket {
            for _ in 0..20 { if system::forward_to_running(&request) { return Ok(()); } std::thread::sleep(std::time::Duration::from_millis(50)); }
            eprintln!("Couldn't acquire the EchoFiles instance socket");
            return Ok(());
        }
    }

    // Iced parses every system font before it can draw text (714 files, ~450 ms here). Load
    // just the UI font now and the rest on a background thread (vendor/iced_graphics patch).
    if std::env::var_os("ECHOFILES_NO_FONT_PREWARM").is_none() {
        let ui_fonts = style::find_font_files("JetBrainsMono Nerd Font");
        if !ui_fonts.is_empty() && iced::advanced::graphics::text::set_startup_fonts(ui_fonts) {
            std::thread::spawn(|| {
                let t = Instant::now();
                iced::advanced::graphics::text::load_system_fonts_deferred();
                if std::env::var_os("ECHOFILES_TIMING").is_some() {
                    eprintln!("startup: fallback fonts ready {:.1} ms later (background)", t.elapsed().as_secs_f64() * 1000.0);
                }
            });
        }
    }

    // A daemon, so closing the window can leave EchoFiles running (Settings → General).
    let result = iced::daemon(app::App::boot, app::App::update, app::App::view)
        .title(app::App::title)
        .subscription(app::App::subscription)
        .default_font(style::FONT)
        .antialiasing(false)
        .run();
    if owns_socket {
        system::remove_own_socket();
    }
    result
}

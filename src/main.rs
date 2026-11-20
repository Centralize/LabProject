//! LabProject: a desktop project manager with Kanban boards.
//!
//! Run with `--screenshot <file.png>` to open the window without taking focus,
//! draw about 120 frames, save a picture of the window and exit.

#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod board;
mod details;
mod dialogs;
mod list;
mod model;
mod overview;
mod theme;
mod widgets;

use eframe::egui;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;

fn parse_args() -> Result<Option<PathBuf>, String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut shot = None;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--screenshot" {
            match args.get(i + 1) {
                Some(p) if !p.starts_with("--") => {
                    shot = Some(PathBuf::from(p));
                    i += 1;
                }
                _ => return Err("--screenshot needs a file name, e.g. --screenshot shot.png".into()),
            }
        } else if let Some(p) = a.strip_prefix("--screenshot=") {
            if p.is_empty() {
                return Err("--screenshot needs a file name, e.g. --screenshot shot.png".into());
            }
            shot = Some(PathBuf::from(p));
        }
        i += 1;
    }
    Ok(shot)
}

fn main() {
    let screenshot = match parse_args() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let screenshot_mode = screenshot.is_some();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("LabProject")
        .with_app_id("labproject")
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([960.0, 600.0])
        .with_icon(Arc::new(app::icon()));
    if screenshot_mode {
        // Don't steal focus from whatever the person is doing.
        viewport = viewport.with_active(false);
        #[cfg(windows)]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
    }

    #[allow(unused_mut)]
    let mut options = eframe::NativeOptions { viewport, ..Default::default() };
    #[cfg(target_os = "macos")]
    if screenshot_mode {
        options.event_loop_builder = Some(Box::new(|builder| {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            builder.with_activate_ignoring_other_apps(false);
        }));
    }

    let result = eframe::run_native(
        "LabProject",
        options,
        Box::new(move |cc| Ok(Box::new(app::App::new(cc, screenshot)))),
    );
    if let Err(e) = result {
        eprintln!("LabProject couldn't start: {e}");
        std::process::exit(1);
    }
    std::process::exit(app::EXIT_CODE.load(Ordering::SeqCst));
}

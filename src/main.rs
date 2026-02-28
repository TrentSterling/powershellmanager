#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod arrange;
mod config;
mod gui;
mod layout;
mod monitor;
mod theme;
mod tray;
mod windows;

use clap::Parser;

#[derive(Parser)]
#[command(name = "powershellmanager")]
#[command(about = "System tray app for arranging terminal windows into grid layouts")]
struct Cli {
    /// Apply a layout and exit (e.g., "2x3", "columns:4", "left-right")
    #[arg(long)]
    headless: Option<String>,
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .init();

    let cli = Cli::parse();

    if let Some(layout_str) = cli.headless {
        run_headless(&layout_str);
        return;
    }

    run_gui();
}

fn run_headless(layout_str: &str) {
    let config = config::load();

    let preset = match layout::LayoutPreset::parse(layout_str) {
        Some(p) => p,
        None => {
            eprintln!("Unknown layout: '{}'", layout_str);
            eprintln!("Examples: 2x3, columns:4, rows:3, left-right, top-bottom, main-side, focus:3");
            std::process::exit(1);
        }
    };

    let filter = crate::windows::TargetFilter::from_str(&config.defaults.target);
    let disabled = std::collections::HashSet::new();
    let result = arrange::arrange_masked(&preset, filter, &config.defaults.monitor, config.defaults.gap, &disabled);

    println!(
        "Arranged {} windows into {} layout ({} slots)",
        result.arranged,
        preset.display_name(),
        preset.slot_count()
    );
    if result.skipped > 0 {
        println!("Skipped {} windows (not enough slots)", result.skipped);
    }
    for err in &result.errors {
        eprintln!("Error: {}", err);
    }
}

fn load_window_icon() -> Option<egui::IconData> {
    static ICON_PNG: &[u8] = include_bytes!("../assets/tront-icon.png");
    let img = image::load_from_memory(ICON_PNG).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    Some(egui::IconData {
        rgba: rgba.into_raw(),
        width: w,
        height: h,
    })
}

fn run_gui() {
    let config = config::load();

    let title = format!("PowerShell Manager v{}", env!("CARGO_PKG_VERSION"));
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(&title)
        .with_inner_size([480.0, 550.0])
        .with_min_inner_size([380.0, 450.0]);

    if let Some(icon) = load_window_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        &title,
        options,
        Box::new(move |cc| Ok(Box::new(app::PsmApp::new(cc, config)))),
    ) {
        eprintln!("Failed to start GUI: {}", e);
        std::process::exit(1);
    }
}

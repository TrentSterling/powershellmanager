#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod activity;
mod app;
mod arrange;
mod branding;
mod config;
mod gui;
mod layout;
mod monitor;
#[cfg(test)]
mod native_audit;
mod order;
mod theme;
mod theme_studio;
mod tray;
#[cfg(test)]
mod ui_tests;
mod windows;

use clap::Parser;

#[derive(Parser)]
#[command(name = "powershellmanager")]
#[command(about = "Universal window manager with smart activity-based sorting")]
struct Cli {
    /// Apply a layout and exit (e.g., "2x3", "columns:4", "left-right")
    #[arg(long)]
    headless: Option<String>,
    /// Inspect the UI without moving windows, registering hotkeys or saving settings.
    #[arg(long, conflicts_with = "headless")]
    preview: bool,
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

    run_gui(cli.preview);
}

fn run_headless(layout_str: &str) {
    let config = config::load();

    let preset = match layout::LayoutPreset::parse(layout_str) {
        Some(p) => p,
        None => {
            eprintln!("Unknown layout: '{}'", layout_str);
            eprintln!(
                "Examples: 2x3, columns:4, rows:3, left-right, top-bottom, main-side, focus:3"
            );
            std::process::exit(1);
        }
    };

    let filter = crate::windows::TargetFilter::from_str(&config.defaults.target);
    let disabled = std::collections::HashSet::new();
    let extra_exclude = config.categories.excluded_lower();
    let fresh = windows::find_windows(&filter, 0, &extra_exclude);
    let queue = if config.defaults.manual_order {
        order::reconcile(fresh, &[], &config.window_order)
    } else {
        fresh
    };
    let result = arrange::arrange_ordered(
        &preset,
        &config.defaults.monitor,
        config.defaults.gap,
        &disabled,
        None,
        &queue,
        &config.pin,
    );

    println!(
        "Arranged {} windows into {} layout ({} slots)",
        result.arranged,
        preset.display_name(),
        preset.slot_count()
    );
    if result.skipped > 0 {
        println!("Skipped {} windows (not enough slots)", result.skipped);
    }
    for warning in &result.warnings {
        eprintln!("Warning: {warning}");
    }
    for err in &result.errors {
        eprintln!("Error: {}", err);
    }
    if !result.errors.is_empty() {
        std::process::exit(2);
    }
}

fn run_gui(preview: bool) {
    let config = config::load();

    let title = format!(
        "PowerShell Manager v{}{}",
        env!("CARGO_PKG_VERSION"),
        if preview { " (preview)" } else { "" }
    );
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(&title)
        .with_inner_size([1080.0, 800.0])
        .with_min_inner_size([280.0, 300.0]);

    let theme = config
        .defaults
        .theme_code
        .as_deref()
        .and_then(theme::ThemeSettings::decode)
        .unwrap_or_default();
    viewport = viewport.with_icon(std::sync::Arc::new(branding::icon(theme, 64)));

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        &title,
        options,
        Box::new(move |cc| {
            let app = if preview {
                let mut app = app::PsmApp::preview(config);
                app.managed_windows = windows::find_windows(
                    &windows::TargetFilter::from_str(&app.config.defaults.target),
                    0,
                    &app.config.categories.excluded_lower(),
                );
                app.monitors = monitor::enumerate_monitors();
                app.show_theme_studio = true;
                app.status = "Preview mode. Layout actions and saving are disabled.".into();
                app
            } else {
                app::PsmApp::new(cc, config)
            };
            Ok(Box::new(app))
        }),
    ) {
        eprintln!("Failed to start GUI: {}", e);
        std::process::exit(1);
    }
}

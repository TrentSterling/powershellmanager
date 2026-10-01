use crate::{
    app, arrange, branding, cli::Cli, config::Config, desktop::Desktop, layout, order, theme,
    windows::TargetFilter,
};
use std::collections::HashSet;

#[derive(Default, Debug, PartialEq, Eq)]
pub(crate) struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

pub(crate) struct GuiRequest {
    pub title: String,
    pub options: eframe::NativeOptions,
    pub config: Config,
    pub preview: bool,
}

impl GuiRequest {
    fn new(config: Config, preview: bool) -> Self {
        let title = format!(
            "PowerShell Manager v{}{}",
            env!("CARGO_PKG_VERSION"),
            if preview { " (preview)" } else { "" },
        );
        let theme = config
            .defaults
            .theme_code
            .as_deref()
            .and_then(theme::ThemeSettings::decode)
            .unwrap_or_default();
        let viewport = egui::ViewportBuilder::default()
            .with_title(&title)
            .with_inner_size([1080.0, 800.0])
            .with_min_inner_size([280.0, 300.0])
            .with_icon(std::sync::Arc::new(branding::icon(theme, 64)));
        Self {
            title,
            options: eframe::NativeOptions {
                viewport,
                ..Default::default()
            },
            config,
            preview,
        }
    }
}

pub(crate) trait GuiRunner {
    fn run(&mut self, request: GuiRequest) -> Result<(), String>;
}

pub(crate) struct NativeGuiRunner;

impl GuiRunner for NativeGuiRunner {
    fn run(&mut self, request: GuiRequest) -> Result<(), String> {
        eframe::run_native(
            &request.title,
            request.options,
            Box::new(move |cc| {
                Ok(Box::new(create_app(
                    cc,
                    request.config,
                    request.preview,
                    &mut crate::desktop::NativeDesktop,
                )))
            }),
        )
        .map_err(|error| error.to_string())
    }
}

pub(crate) fn create_app(
    cc: &eframe::CreationContext<'_>,
    config: Config,
    preview: bool,
    desktop: &mut dyn Desktop,
) -> app::PsmApp {
    if !preview {
        return app::PsmApp::new(cc, config);
    }
    let mut app = app::PsmApp::preview(config);
    app.managed_windows = desktop.windows(
        &TargetFilter::from_str(&app.config.defaults.target),
        0,
        &app.config.categories.excluded_lower(),
    );
    app.monitors = desktop.monitors();
    app.show_theme_studio = true;
    app.status = "Preview mode. Layout actions and saving are disabled.".into();
    app
}

pub(crate) fn dispatch(
    cli: Cli,
    config: Config,
    desktop: &mut dyn Desktop,
    gui: &mut dyn GuiRunner,
) -> CommandOutput {
    if cli.list {
        let target = cli.target.as_deref().unwrap_or(&config.defaults.target);
        let inventory = desktop.windows(
            &TargetFilter::from_str(target),
            0,
            &config.categories.excluded_lower(),
        );
        return CommandOutput {
            stdout: format!("{}\n", crate::cli::format_inventory(&inventory, cli.json)),
            ..Default::default()
        };
    }
    if let Some(layout_str) = cli.headless {
        return apply_layout(&layout_str, &config, desktop);
    }
    match gui.run(GuiRequest::new(config, cli.preview)) {
        Ok(()) => CommandOutput::default(),
        Err(error) => CommandOutput {
            stderr: format!("Failed to start GUI: {error}\n"),
            exit_code: 1,
            ..Default::default()
        },
    }
}

fn apply_layout(layout_str: &str, config: &Config, desktop: &mut dyn Desktop) -> CommandOutput {
    let Some(preset) = layout::LayoutPreset::parse(layout_str) else {
        return CommandOutput {
            stderr: format!("Unknown layout: '{layout_str}'\nExamples: 2x3, columns:4, rows:3, left-right, top-bottom, main-side, focus:3\n"),
            exit_code: 1,
            ..Default::default()
        };
    };
    let fresh = desktop.windows(
        &TargetFilter::from_str(&config.defaults.target),
        0,
        &config.categories.excluded_lower(),
    );
    let queue = if config.defaults.manual_order {
        order::reconcile(fresh, &[], &config.window_order)
    } else {
        fresh
    };
    let monitors = desktop.monitors();
    let result = arrange::arrange_with(
        desktop,
        &monitors,
        &arrange::ArrangeSettings {
            preset: &preset,
            monitor_spec: &config.defaults.monitor,
            gap: config.defaults.gap,
            disabled: &HashSet::new(),
            weights: None,
            pins: &config.pin,
            memory: None,
        },
        &queue,
    );
    let mut output = CommandOutput {
        stdout: format!(
            "Arranged {} windows into {} layout ({} slots)\n",
            result.arranged,
            preset.display_name(),
            preset.slot_count()
        ),
        exit_code: if result.errors.is_empty() { 0 } else { 2 },
        ..Default::default()
    };
    if result.skipped > 0 {
        output
            .stdout
            .push_str(&format!("Skipped {} windows\n", result.skipped));
    }
    for warning in result.warnings {
        output.stderr.push_str(&format!("Warning: {warning}\n"));
    }
    for error in result.errors {
        output.stderr.push_str(&format!("Error: {error}\n"));
    }
    output
}

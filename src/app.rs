use crate::arrange;
use crate::config::{self, Config};
use crate::gui;
use crate::layout::{LayoutPreset, builtin_presets};
use crate::theme::THEMES;
use crate::tray;
use crate::windows::{TargetFilter, find_terminal_windows, TerminalWindow};
use raw_window_handle::HasWindowHandle;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use windows::Win32::Foundation::HWND;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DividerAxis {
    Col,
    Row,
}

pub struct UpdateInfo {
    pub latest_version: String,
    pub download_url: String,
}

pub struct PsmApp {
    _tray_icon: Option<tray::TrayIcon>, // must stay alive on main thread
    pub gui_visible: bool,
    app_hwnd: isize,
    pub terminal_windows: Vec<TerminalWindow>,
    pub config: Config,
    pub presets: Vec<(String, LayoutPreset)>,
    pub selected_preset: usize,
    last_refresh: Instant,
    pub custom_cols: u32,
    pub custom_rows: u32,
    pub use_custom: bool,
    pub disabled_cells: HashSet<usize>,
    pub theme_index: usize,
    pub theme_dirty: bool,
    pub icon_texture: Option<egui::TextureHandle>,
    pub update_info: Arc<Mutex<Option<UpdateInfo>>>,
    pub col_weights: Vec<f32>,
    pub row_weights: Vec<f32>,
    pub dragging_divider: Option<(DividerAxis, usize)>,
}

impl PsmApp {
    pub fn new(cc: &eframe::CreationContext<'_>, config: Config) -> Self {
        let app_hwnd = cc
            .window_handle()
            .ok()
            .and_then(|wh| {
                if let raw_window_handle::RawWindowHandle::Win32(h) = wh.as_raw() {
                    Some(h.hwnd.get() as isize)
                } else {
                    None
                }
            })
            .unwrap_or(0);

        // Create tray icon (stays on main thread) and extract menu IDs for bg thread.
        let (tray_icon, tray_menu_ids) = match tray::create_tray(&config) {
            Some((icon, ids)) => (Some(icon), Some(ids)),
            None => {
                log::warn!("Failed to create system tray icon");
                (None, None)
            }
        };

        // Spawn tray event thread — runs independently of eframe's render loop.
        // eframe skips update() for hidden windows, so tray events must be polled here.
        if let Some(menu_ids) = tray_menu_ids {
            let ctx = cc.egui_ctx.clone();
            let hwnd = app_hwnd;
            let cfg = config.clone();
            std::thread::spawn(move || {
                tray_event_loop(menu_ids, ctx, hwnd, &cfg);
            });
        }

        let mut presets = builtin_presets();
        for layout_def in &config.layout {
            if let Some(preset) = layout_def.to_preset() {
                presets.push((layout_def.name.clone(), preset));
            }
        }

        let theme_index = config.defaults.theme.min(THEMES.len() - 1);
        let selected_preset = config.defaults.selected_preset;
        let custom_cols = config.defaults.custom_cols;
        let custom_rows = config.defaults.custom_rows;
        let use_custom = config.defaults.use_custom;

        let col_weights = if config.defaults.col_weights.len() == custom_cols as usize {
            config.defaults.col_weights.clone()
        } else {
            vec![1.0 / custom_cols as f32; custom_cols as usize]
        };
        let row_weights = if config.defaults.row_weights.len() == custom_rows as usize {
            config.defaults.row_weights.clone()
        } else {
            vec![1.0 / custom_rows as f32; custom_rows as usize]
        };

        let update_info: Arc<Mutex<Option<UpdateInfo>>> = Arc::new(Mutex::new(None));

        // Spawn background update checker
        {
            let info = Arc::clone(&update_info);
            std::thread::spawn(move || {
                check_for_updates(info);
            });
        }

        let mut app = Self {
            _tray_icon: tray_icon,
            gui_visible: true,
            app_hwnd,
            terminal_windows: Vec::new(),
            config,
            presets,
            selected_preset,
            last_refresh: Instant::now(),
            custom_cols,
            custom_rows,
            use_custom,
            disabled_cells: HashSet::new(),
            theme_index,
            theme_dirty: true,
            icon_texture: None,
            update_info,
            col_weights,
            row_weights,
            dragging_divider: None,
        };

        app.refresh_windows();
        app
    }

    pub fn active_preset(&self) -> LayoutPreset {
        if self.use_custom {
            LayoutPreset::Grid {
                cols: self.custom_cols,
                rows: self.custom_rows,
            }
        } else {
            self.presets
                .get(self.selected_preset)
                .map(|(_, p)| p.clone())
                .unwrap_or(LayoutPreset::Grid { cols: 2, rows: 2 })
        }
    }

    pub fn ensure_weights(&mut self) {
        if self.col_weights.len() != self.custom_cols as usize {
            self.col_weights = vec![1.0 / self.custom_cols as f32; self.custom_cols as usize];
        }
        if self.row_weights.len() != self.custom_rows as usize {
            self.row_weights = vec![1.0 / self.custom_rows as f32; self.custom_rows as usize];
        }
    }

    pub fn weights_are_uniform(&self) -> bool {
        let eq_col = 1.0 / self.custom_cols as f32;
        let eq_row = 1.0 / self.custom_rows as f32;
        self.col_weights.iter().all(|w| (w - eq_col).abs() < 0.001)
            && self.row_weights.iter().all(|w| (w - eq_row).abs() < 0.001)
    }

    pub fn apply_current_layout(&self) {
        let preset = self.active_preset();
        let filter = TargetFilter::from_str(&self.config.defaults.target);
        let weights = if self.use_custom && !self.weights_are_uniform() {
            Some((self.col_weights.as_slice(), self.row_weights.as_slice()))
        } else {
            None
        };
        let result = arrange::arrange_masked(
            &preset,
            filter,
            &self.config.defaults.monitor,
            self.config.defaults.gap,
            &self.disabled_cells,
            weights,
        );
        log::info!(
            "Arranged {} windows ({} skipped, {} errors)",
            result.arranged,
            result.skipped,
            result.errors.len()
        );
        for err in &result.errors {
            log::warn!("  {}", err);
        }
    }

    pub fn refresh_windows(&mut self) {
        let filter = TargetFilter::from_str(&self.config.defaults.target);
        self.terminal_windows = find_terminal_windows(filter);
        self.last_refresh = Instant::now();
    }

    pub fn set_theme(&mut self, index: usize) {
        if index < THEMES.len() {
            self.theme_index = index;
            self.theme_dirty = true;
            self.config.defaults.theme = index;
            config::save(&self.config);
        }
    }

    pub fn current_theme(&self) -> &'static crate::theme::Theme {
        &THEMES[self.theme_index]
    }

    pub fn toggle_cell(&mut self, index: usize) {
        if self.disabled_cells.contains(&index) {
            self.disabled_cells.remove(&index);
        } else {
            self.disabled_cells.insert(index);
        }
    }

    fn hide_window(&mut self) {
        crate::windows::hide_app_window(self.app_hwnd);
        self.gui_visible = false;
    }
}

impl eframe::App for PsmApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.theme_dirty {
            self.current_theme().apply_to_egui(ctx);
            self.theme_dirty = false;
        }

        // Intercept close button → hide to tray instead of closing
        let close_requested = ctx.input(|i| i.viewport().close_requested());
        if close_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.hide_window();
        }

        // Check if the tray thread restored us
        if !self.gui_visible {
            // Window might have been shown by the tray thread — detect via repaint wake
            let visible = unsafe {
                use windows::Win32::UI::WindowsAndMessaging::IsWindowVisible;
                IsWindowVisible(HWND(self.app_hwnd as *mut _)).as_bool()
            };
            if visible {
                self.gui_visible = true;
                self.refresh_windows();
            }
        }

        if self.gui_visible && self.last_refresh.elapsed().as_secs() >= 3 {
            self.refresh_windows();
        }

        if self.gui_visible {
            gui::draw(ctx, self);
        }

        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
}

/// Background thread that polls tray events independently of eframe's render loop.
/// eframe skips update() for hidden windows, so tray events must be polled here.
fn tray_event_loop(menu_ids: tray::TrayMenuIds, ctx: egui::Context, hwnd: isize, config: &Config) {
    use crate::tray::TrayAction;

    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));

        match menu_ids.poll() {
            TrayAction::ShowGui => {
                crate::windows::show_app_window(hwnd);
                ctx.request_repaint();
            }
            TrayAction::ApplyLayout(_name, preset) => {
                let filter = TargetFilter::from_str(&config.defaults.target);
                let disabled = HashSet::new();
                let result = arrange::arrange_masked(
                    &preset,
                    filter,
                    &config.defaults.monitor,
                    config.defaults.gap,
                    &disabled,
                    None,
                );
                log::info!("Tray: arranged {} windows", result.arranged);
            }
            TrayAction::Quit => {
                std::process::exit(0);
            }
            TrayAction::None => {}
        }
    }
}

fn check_for_updates(info: Arc<Mutex<Option<UpdateInfo>>>) {
    let result: Result<(), Box<dyn std::error::Error>> = (|| {
        let resp = ureq::get("https://api.github.com/repos/TrentSterling/powershellmanager/releases/latest")
            .set("User-Agent", "powershellmanager")
            .set("Accept", "application/vnd.github+json")
            .call()?;

        let json: serde_json::Value = resp.into_json()?;
        let tag = json["tag_name"].as_str().unwrap_or("");
        let url = json["html_url"].as_str().unwrap_or("");

        let latest = tag.strip_prefix('v').unwrap_or(tag);
        let current = env!("CARGO_PKG_VERSION");

        if !latest.is_empty() && latest != current && version_newer(latest, current) {
            if let Ok(mut guard) = info.lock() {
                *guard = Some(UpdateInfo {
                    latest_version: latest.to_string(),
                    download_url: url.to_string(),
                });
            }
        }
        Ok(())
    })();

    if let Err(e) = result {
        log::debug!("Update check failed (non-fatal): {}", e);
    }
}

fn version_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> {
        v.split('.').filter_map(|s| s.parse().ok()).collect()
    };
    let l = parse(latest);
    let c = parse(current);
    l > c
}

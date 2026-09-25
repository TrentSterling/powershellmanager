use crate::activity::ActivityTracker;
use crate::arrange;
use crate::config::{self, Config};
use crate::gui;
use crate::layout::{builtin_presets, LayoutPreset};
use crate::theme::{self, ThemeSettings, THEMES};
use crate::tray;
use crate::windows::{find_windows, ManagedWindow, TargetFilter};
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
    pub app_hwnd: isize,
    pub managed_windows: Vec<ManagedWindow>,
    pub config: Config,
    pub presets: Vec<(String, LayoutPreset)>,
    pub selected_preset: usize,
    last_refresh: Instant,
    pub custom_cols: u32,
    pub custom_rows: u32,
    pub use_custom: bool,
    pub disabled_cells: HashSet<usize>,
    pub theme_settings: ThemeSettings,
    pub studio: crate::theme_studio::Studio,
    pub show_theme_studio: bool,
    pub window_query: String,
    pub detail_tab: usize,
    pub status: String,
    pub native_enabled: bool,
    pub monitors: Vec<crate::monitor::MonitorInfo>,
    pub theme_dirty: bool,
    pub update_info: Arc<Mutex<Option<UpdateInfo>>>,
    pub col_weights: Vec<f32>,
    pub row_weights: Vec<f32>,
    pub dragging_divider: Option<(DividerAxis, usize)>,
    pub activity: ActivityTracker,
    pub save_grid_name: String,
    pub show_save_dialog: bool,
}

impl PsmApp {
    pub fn new(cc: &eframe::CreationContext<'_>, config: Config) -> Self {
        let app_hwnd = cc
            .window_handle()
            .ok()
            .and_then(|wh| {
                if let raw_window_handle::RawWindowHandle::Win32(h) = wh.as_raw() {
                    Some(h.hwnd.get())
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

        Self::build(config, tray_icon, app_hwnd, true)
    }

    pub fn preview(config: Config) -> Self {
        Self::build(config, None, 0, false)
    }

    fn build(
        mut config: Config,
        tray_icon: Option<tray::TrayIcon>,
        app_hwnd: isize,
        native_enabled: bool,
    ) -> Self {
        if !config.defaults.ui_scale.is_finite() {
            config.defaults.ui_scale = 1.0;
        }
        config.defaults.ui_scale = config.defaults.ui_scale.clamp(0.75, 1.5);
        config.defaults.custom_cols = config.defaults.custom_cols.clamp(1, 8);
        config.defaults.custom_rows = config.defaults.custom_rows.clamp(1, 8);
        let mut presets = builtin_presets();
        for layout_def in &config.layout {
            if let Some(preset) = layout_def.to_preset() {
                presets.push((layout_def.name.clone(), preset));
            }
        }
        for sg in &config.saved_grid {
            presets.push((
                sg.name.clone(),
                LayoutPreset::Grid {
                    cols: sg.cols,
                    rows: sg.rows,
                },
            ));
        }

        let theme_index = config.defaults.theme.min(THEMES.len() - 1);
        let selected_preset = config.defaults.selected_preset;
        let custom_cols = config.defaults.custom_cols;
        let custom_rows = config.defaults.custom_rows;
        let use_custom = config.defaults.use_custom;

        let col_weights = normalized_weights(&config.defaults.col_weights, custom_cols);
        let row_weights = normalized_weights(&config.defaults.row_weights, custom_rows);
        let disabled_cells = config.defaults.disabled_cells.iter().copied().collect();

        let update_info: Arc<Mutex<Option<UpdateInfo>>> = Arc::new(Mutex::new(None));

        // Spawn background update checker
        if native_enabled {
            let info = Arc::clone(&update_info);
            std::thread::spawn(move || {
                check_for_updates(info);
            });
        }

        let activity = if native_enabled {
            ActivityTracker::new(config.defaults.decay_half_life_days)
        } else {
            ActivityTracker::inert()
        };
        let theme_settings = config
            .defaults
            .theme_code
            .as_deref()
            .and_then(ThemeSettings::decode)
            .unwrap_or_else(|| theme::from_legacy(theme_index));

        let mut app = Self {
            _tray_icon: tray_icon,
            gui_visible: true,
            app_hwnd,
            managed_windows: Vec::new(),
            config,
            presets,
            selected_preset,
            last_refresh: Instant::now(),
            custom_cols,
            custom_rows,
            use_custom,
            disabled_cells,
            theme_settings,
            studio: Default::default(),
            show_theme_studio: false,
            window_query: String::new(),
            detail_tab: 0,
            status: "Ready. Choose a layout, then apply.".into(),
            native_enabled,
            monitors: if native_enabled {
                crate::monitor::enumerate_monitors()
            } else {
                Vec::new()
            },
            theme_dirty: true,
            update_info,
            col_weights,
            row_weights,
            dragging_divider: None,
            activity,
            save_grid_name: String::new(),
            show_save_dialog: false,
        };

        app.disabled_cells.retain(|i| {
            *i < if app.use_custom {
                (app.custom_cols * app.custom_rows) as usize
            } else {
                app.presets
                    .get(app.selected_preset)
                    .map(|(_, p)| p.slot_count())
                    .unwrap_or(4)
            }
        });
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

    pub fn apply_current_layout(&mut self) {
        if !self.native_enabled {
            self.status = "Preview only. No windows moved.".into();
            return;
        }
        let preset = self.active_preset();
        let filter = TargetFilter::from_str(&self.config.defaults.target);
        let weights = if self.use_custom && !self.weights_are_uniform() {
            Some((self.col_weights.as_slice(), self.row_weights.as_slice()))
        } else {
            None
        };
        let extra_exclude = self.config.categories.excluded_lower();
        let result = arrange::arrange_masked(
            &preset,
            &filter,
            &self.config.defaults.monitor,
            self.config.defaults.gap,
            &self.disabled_cells,
            weights,
            self.app_hwnd,
            &extra_exclude,
            self.config.defaults.smart_sort,
            Some(&self.activity),
            &self.config.pin,
        );
        log::info!(
            "Arranged {} windows ({} skipped, {} errors)",
            result.arranged,
            result.skipped,
            result.errors.len()
        );
        self.status = format!(
            "Arranged {} windows. {} skipped. {} errors.",
            result.arranged,
            result.skipped,
            result.errors.len()
        );
        for err in &result.errors {
            log::warn!("  {}", err);
        }
    }

    pub fn refresh_windows(&mut self) {
        if !self.native_enabled {
            return;
        }
        let filter = TargetFilter::from_str(&self.config.defaults.target);
        let extra_exclude = self.config.categories.excluded_lower();
        self.managed_windows = find_windows(&filter, self.app_hwnd, &extra_exclude);
        self.last_refresh = Instant::now();
    }

    pub fn save_config(&self) {
        if self.native_enabled {
            config::save(&self.config);
        }
    }

    pub fn save_layout(&mut self) {
        self.config.defaults.disabled_cells = self.disabled_cells.iter().copied().collect();
        self.config.defaults.use_custom = self.use_custom;
        self.config.defaults.selected_preset = self.selected_preset;
        self.config.defaults.custom_cols = self.custom_cols;
        self.config.defaults.custom_rows = self.custom_rows;
        self.config.defaults.col_weights = self.col_weights.clone();
        self.config.defaults.row_weights = self.row_weights.clone();
        self.save_config();
    }

    pub fn current_theme(&self) -> crate::theme::Theme {
        theme::layout_colors(self.theme_settings)
    }

    pub fn install_theme(&mut self, ctx: &egui::Context) {
        if self.theme_dirty {
            theme::install(ctx, self.theme_settings);
            ctx.set_zoom_factor(self.config.defaults.ui_scale.clamp(0.75, 1.5));
            self.theme_dirty = false;
        }
    }

    pub fn toggle_cell(&mut self, index: usize) {
        if self.disabled_cells.contains(&index) {
            self.disabled_cells.remove(&index);
        } else {
            self.disabled_cells.insert(index);
        }
        self.save_layout();
    }

    pub fn load_saved_grid(&mut self, grid: &config::SavedGrid) {
        self.use_custom = true;
        self.custom_cols = grid.cols.clamp(1, 8);
        self.custom_rows = grid.rows.clamp(1, 8);
        self.col_weights = normalized_weights(&grid.col_weights, self.custom_cols);
        self.row_weights = normalized_weights(&grid.row_weights, self.custom_rows);
        self.disabled_cells = grid
            .disabled_cells
            .iter()
            .copied()
            .filter(|i| *i < (self.custom_cols * self.custom_rows) as usize)
            .collect();
        self.dragging_divider = None;

        self.save_layout();
    }

    pub fn save_current_as_grid(&mut self, name: String) {
        let grid = config::SavedGrid {
            name: name.clone(),
            cols: self.custom_cols,
            rows: self.custom_rows,
            col_weights: self.col_weights.clone(),
            row_weights: self.row_weights.clone(),
            disabled_cells: self.disabled_cells.iter().copied().collect(),
        };
        // Upsert: replace existing with same name
        if let Some(existing) = self.config.saved_grid.iter_mut().find(|g| g.name == name) {
            *existing = grid;
        } else {
            self.config.saved_grid.push(grid);
        }
        self.save_config();
        self.rebuild_presets();
    }

    pub fn delete_saved_grid(&mut self, name: &str) {
        self.config.saved_grid.retain(|g| g.name != name);
        self.save_config();
        self.rebuild_presets();
    }

    pub fn rebuild_presets(&mut self) {
        let mut presets = builtin_presets();
        for layout_def in &self.config.layout {
            if let Some(preset) = layout_def.to_preset() {
                presets.push((layout_def.name.clone(), preset));
            }
        }
        for sg in &self.config.saved_grid {
            presets.push((
                sg.name.clone(),
                LayoutPreset::Grid {
                    cols: sg.cols,
                    rows: sg.rows,
                },
            ));
        }
        self.presets = presets;
        if self.selected_preset >= self.presets.len() {
            self.selected_preset = 0;
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
            self.install_theme(ctx);
        }

        // Intercept close button → hide to tray instead of closing
        let close_requested = ctx.input(|i| i.viewport().close_requested());
        if close_requested && self.native_enabled {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.hide_window();
        }

        // Check if the tray thread restored us
        if !self.gui_visible && self.native_enabled {
            let visible = unsafe {
                use windows::Win32::UI::WindowsAndMessaging::IsWindowVisible;
                IsWindowVisible(HWND(self.app_hwnd as *mut _)).as_bool()
            };
            if visible {
                self.gui_visible = true;
                self.refresh_windows();
            }
        }

        // Update activity tracker (drains focus events)
        self.activity.update();

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
fn tray_event_loop(menu_ids: tray::TrayMenuIds, ctx: egui::Context, hwnd: isize, config: &Config) {
    use crate::tray::TrayAction;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
    };
    use windows::Win32::UI::WindowsAndMessaging::{PeekMessageW, MSG, PM_REMOVE, WM_HOTKEY};

    const HOTKEY_ID: i32 = 1;
    // 0x47 = 'G'
    let hotkey_ok =
        unsafe { RegisterHotKey(None, HOTKEY_ID, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, 0x47) };
    if hotkey_ok.is_ok() {
        log::info!("Registered global hotkey Ctrl+Alt+G");
    } else {
        log::warn!("Failed to register global hotkey Ctrl+Alt+G (already in use?)");
    }

    // Resolve the active preset from config (snapshot at startup)
    let active_preset = if config.defaults.use_custom {
        LayoutPreset::Grid {
            cols: config.defaults.custom_cols,
            rows: config.defaults.custom_rows,
        }
    } else {
        let mut presets = builtin_presets();
        for ld in &config.layout {
            if let Some(p) = ld.to_preset() {
                presets.push((ld.name.clone(), p));
            }
        }
        presets
            .get(config.defaults.selected_preset)
            .map(|(_, p)| p.clone())
            .unwrap_or(LayoutPreset::Grid { cols: 2, rows: 2 })
    };
    let hotkey_weights = if config.defaults.use_custom
        && config.defaults.col_weights.len() == config.defaults.custom_cols as usize
        && config.defaults.row_weights.len() == config.defaults.custom_rows as usize
    {
        Some((
            config.defaults.col_weights.clone(),
            config.defaults.row_weights.clone(),
        ))
    } else {
        None
    };

    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Poll WM_HOTKEY messages (thread-level, no window needed)
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, WM_HOTKEY, WM_HOTKEY, PM_REMOVE).as_bool() {
                if msg.wParam.0 as i32 == HOTKEY_ID {
                    log::info!("Hotkey Ctrl+Alt+G pressed — applying layout");
                    let filter = TargetFilter::from_str(&config.defaults.target);
                    let disabled = HashSet::new();
                    let extra_exclude = config.categories.excluded_lower();
                    let w = hotkey_weights
                        .as_ref()
                        .map(|(c, r)| (c.as_slice(), r.as_slice()));
                    let result = arrange::arrange_masked(
                        &active_preset,
                        &filter,
                        &config.defaults.monitor,
                        config.defaults.gap,
                        &disabled,
                        w,
                        hwnd,
                        &extra_exclude,
                        false,
                        None,
                        &config.pin,
                    );
                    log::info!("Hotkey: arranged {} windows", result.arranged);
                }
            }
        }

        match menu_ids.poll() {
            TrayAction::ShowGui => {
                crate::windows::show_app_window(hwnd);
                ctx.request_repaint();
            }
            TrayAction::ApplyLayout(_name, preset, weights) => {
                let filter = TargetFilter::from_str(&config.defaults.target);
                let disabled = HashSet::new();
                let extra_exclude = config.categories.excluded_lower();
                let w = weights.as_ref().map(|(c, r)| (c.as_slice(), r.as_slice()));
                let result = arrange::arrange_masked(
                    &preset,
                    &filter,
                    &config.defaults.monitor,
                    config.defaults.gap,
                    &disabled,
                    w,
                    hwnd,
                    &extra_exclude,
                    false,
                    None,
                    &config.pin,
                );
                log::info!("Tray: arranged {} windows", result.arranged);
            }
            TrayAction::Quit => {
                unsafe {
                    let _ = UnregisterHotKey(None, HOTKEY_ID);
                }
                std::process::exit(0);
            }
            TrayAction::None => {}
        }
    }
}

fn check_for_updates(info: Arc<Mutex<Option<UpdateInfo>>>) {
    let result: Result<(), Box<dyn std::error::Error>> = (|| {
        let resp = ureq::get(
            "https://api.github.com/repos/TrentSterling/powershellmanager/releases/latest",
        )
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
    let parse = |v: &str| -> Vec<u32> { v.split('.').filter_map(|s| s.parse().ok()).collect() };
    let l = parse(latest);
    let c = parse(current);
    l > c
}

/// Recover old/manual config without changing valid custom proportions.
fn normalized_weights(values: &[f32], count: u32) -> Vec<f32> {
    let count = count.clamp(1, 8) as usize;
    let sum: f32 = values.iter().sum();
    if values.len() != count
        || !sum.is_finite()
        || sum <= 0.0
        || values.iter().any(|v| !v.is_finite() || *v <= 0.0)
    {
        vec![1.0 / count as f32; count]
    } else {
        values.iter().map(|v| v / sum).collect()
    }
}

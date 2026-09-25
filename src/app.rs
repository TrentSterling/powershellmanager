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
use std::sync::atomic::{AtomicBool, Ordering};
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

#[derive(Clone)]
struct LiveState {
    config: Config,
    windows: Vec<ManagedWindow>,
}

pub struct PsmApp {
    #[cfg(test)]
    pub capture_ui: bool,
    live: Arc<Mutex<LiveState>>,
    tray_ids: Arc<Mutex<Option<tray::TrayMenuIds>>>,
    quitting: Arc<AtomicBool>,
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
    pub activity: Arc<Mutex<ActivityTracker>>,
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

        let mut app = Self::build(config, None, app_hwnd, true);
        app.rebuild_tray();
        if app._tray_icon.is_some() {
            let (ids, live, activity, quitting) = (
                app.tray_ids.clone(),
                app.live.clone(),
                app.activity.clone(),
                app.quitting.clone(),
            );
            let ctx = cc.egui_ctx.clone();
            std::thread::spawn(move || {
                tray_event_loop(ids, ctx, app_hwnd, live, activity, quitting)
            });
        }
        app
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
                    cols: sg.cols.clamp(1, 8),
                    rows: sg.rows.clamp(1, 8),
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
            #[cfg(test)]
            capture_ui: false,
            live: Arc::new(Mutex::new(LiveState {
                config: config.clone(),
                windows: Vec::new(),
            })),
            tray_ids: Arc::new(Mutex::new(None)),
            quitting: Arc::new(AtomicBool::new(false)),
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
            activity: Arc::new(Mutex::new(activity)),
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

    pub fn actions_available(&self) -> bool {
        #[cfg(test)]
        if self.capture_ui {
            return true;
        }
        self.native_enabled
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
        self.refresh_windows();
        let preset = self.active_preset();
        let weights = if self.use_custom {
            Some((self.col_weights.as_slice(), self.row_weights.as_slice()))
        } else {
            None
        };
        let result = arrange::arrange_ordered(
            &preset,
            &self.config.defaults.monitor,
            self.config.defaults.gap,
            &self.disabled_cells,
            weights,
            &self.managed_windows,
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
        if !result.warnings.is_empty() {
            self.status
                .push_str(&format!(" {}", result.warnings.join("; ")));
        }
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
        let fresh = find_windows(&filter, self.app_hwnd, &extra_exclude);
        self.managed_windows = if self.config.defaults.manual_order {
            crate::order::reconcile(fresh, &self.managed_windows, &self.config.window_order)
        } else {
            fresh
        };
        if self.config.defaults.smart_sort && !self.config.defaults.manual_order {
            if let Ok(activity) = self.activity.lock() {
                let scores = activity.score_windows(&self.managed_windows);
                let mut scored: Vec<_> = self.managed_windows.drain(..).zip(scores).collect();
                scored.sort_by(|a, b| b.1.total_cmp(&a.1));
                self.managed_windows = scored.into_iter().map(|(w, _)| w).collect();
            }
        }
        let plan = self.assignment();
        for (rule, index) in self.config.pin.iter_mut().zip(plan.rule_windows) {
            if let Some(i) = index {
                rule.bound_hwnd = Some(self.managed_windows[i].hwnd);
                if rule.title_exact.is_some() {
                    rule.title_exact = Some(self.managed_windows[i].title.clone());
                }
            }
        }
        self.sync_live();
        self.last_refresh = Instant::now();
    }

    pub fn assignment(&self) -> crate::order::Assignment {
        crate::order::assign(
            &self.managed_windows,
            &self.config.pin,
            self.active_preset().slot_count(),
            &self.disabled_cells,
        )
    }

    pub fn toggle_pin(&mut self, hwnd: isize) {
        let Some(i) = self.managed_windows.iter().position(|w| w.hwnd == hwnd) else {
            return;
        };
        let plan = self.assignment();
        if let Some(rule) = plan.rule_windows.iter().position(|w| *w == Some(i)) {
            self.config.pin.remove(rule);
        } else {
            let slot = plan.slots.iter().position(|w| *w == Some(i)).or_else(|| {
                (0..plan.slots.len()).find(|s| {
                    !self.disabled_cells.contains(s)
                        && !self.config.pin.iter().any(|r| r.slot == *s)
                })
            });
            let Some(slot) = slot else {
                self.status = "No available slot to pin. Enable a slot first.".into();
                return;
            };
            let w = &self.managed_windows[i];
            self.config.pin.push(config::PinRule {
                process: Some(w.process_name.clone()),
                title_exact: Some(w.title.clone()),
                title_contains: None,
                bound_hwnd: Some(hwnd),
                slot,
            });
        }
        self.save_config();
    }

    pub fn reorder_window(&mut self, source: isize, target: isize, after: bool) {
        let pinned: Vec<_> = self
            .assignment()
            .rule_windows
            .into_iter()
            .map(|w| w.map(|i| self.managed_windows[i].hwnd))
            .collect();
        if crate::order::move_relative(&mut self.managed_windows, source, target, after) {
            let enabled: Vec<_> = (0..self.active_preset().slot_count())
                .filter(|s| !self.disabled_cells.contains(s))
                .collect();
            for (rule, hwnd) in self.config.pin.iter_mut().zip(pinned) {
                if let Some(i) = self
                    .managed_windows
                    .iter()
                    .position(|w| Some(w.hwnd) == hwnd)
                {
                    if let Some(slot) = enabled.get(i) {
                        rule.slot = *slot;
                    }
                }
            }
            self.config.defaults.manual_order = true;
            self.config.defaults.smart_sort = false;
            self.config.window_order = self
                .managed_windows
                .iter()
                .map(crate::order::WindowKey::from_window)
                .collect();
            self.status = "Manual order saved. Apply uses this order in enabled slots.".into();
            self.save_config();
        }
    }

    fn sync_live(&self) {
        if let Ok(mut live) = self.live.lock() {
            live.config = self.config.clone();
            live.windows = self.managed_windows.clone();
        }
    }

    fn rebuild_tray(&mut self) {
        if !self.native_enabled {
            return;
        }
        if let Some((icon, ids)) = tray::create_tray(&self.config) {
            if let Ok(mut current) = self.tray_ids.lock() {
                *current = Some(ids);
            }
            self._tray_icon = Some(icon);
        }
    }

    pub fn save_config(&self) {
        self.sync_live();
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
            if self.native_enabled {
                ctx.send_viewport_cmd(egui::ViewportCommand::Icon(Some(Arc::new(
                    crate::branding::icon(self.theme_settings, 64),
                ))));
                if let Some(tray) = &self._tray_icon {
                    tray.set_theme(self.theme_settings);
                }
            }
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
                    cols: sg.cols.clamp(1, 8),
                    rows: sg.rows.clamp(1, 8),
                },
            ));
        }
        self.presets = presets;
        self.rebuild_tray();
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
        if close_requested
            && self.native_enabled
            && self._tray_icon.is_some()
            && !self.quitting.load(Ordering::Relaxed)
        {
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
        if let Ok(mut activity) = self.activity.lock() {
            activity.update();
        }

        if self.gui_visible && self.last_refresh.elapsed().as_secs() >= 3 {
            self.refresh_windows();
        }

        if self.gui_visible {
            gui::draw(ctx, self);
        }

        if self.gui_visible {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }
}

impl Drop for PsmApp {
    fn drop(&mut self) {
        self.quitting.store(true, Ordering::Relaxed);
        self.save_config();
        if let Ok(activity) = self.activity.lock() {
            activity.save();
        }
    }
}

/// Action worker shares current settings, queue and activity with the UI.
fn tray_event_loop(
    menu_ids: Arc<Mutex<Option<tray::TrayMenuIds>>>,
    ctx: egui::Context,
    hwnd: isize,
    live: Arc<Mutex<LiveState>>,
    activity: Arc<Mutex<ActivityTracker>>,
    quitting: Arc<AtomicBool>,
) {
    use crate::tray::TrayAction;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        PeekMessageW, ShowWindow, MSG, PM_REMOVE, SW_SHOWNOACTIVATE, WM_HOTKEY,
    };
    let registered =
        unsafe { RegisterHotKey(None, 1, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, 0x47) }.is_ok();
    if !registered {
        log::warn!("Ctrl+Alt+G is already registered by another app");
    }
    while !quitting.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Ok(mut tracker) = activity.lock() {
            tracker.update();
        }
        let mut action = menu_ids
            .lock()
            .ok()
            .and_then(|ids| ids.as_ref().map(|ids| ids.poll()))
            .unwrap_or(TrayAction::None);
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, WM_HOTKEY, WM_HOTKEY, PM_REMOVE).as_bool() {
                if msg.wParam.0 == 1 {
                    action = TrayAction::ApplyCurrent;
                }
            }
        }
        match action {
            TrayAction::ShowGui => {
                crate::windows::show_app_window(hwnd);
                ctx.request_repaint();
            }
            TrayAction::Quit => {
                quitting.store(true, Ordering::Relaxed);
                if let Ok(tracker) = activity.lock() {
                    tracker.save();
                }
                // A hidden native window must receive one event to finish through eframe.
                unsafe {
                    let _ = ShowWindow(HWND(hwnd as *mut _), SW_SHOWNOACTIVATE);
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                ctx.request_repaint();
            }
            TrayAction::ApplyCurrent | TrayAction::ApplyLayout(_) => {
                let Ok(snapshot) = live.lock().map(|l| l.clone()) else {
                    continue;
                };
                let request = crate::tray::layout_request(&snapshot.config, &action);
                let Some(request) = request else {
                    continue;
                };
                let cfg = &snapshot.config;
                let fresh = find_windows(
                    &TargetFilter::from_str(&cfg.defaults.target),
                    hwnd,
                    &cfg.categories.excluded_lower(),
                );
                let mut queue = if cfg.defaults.manual_order {
                    crate::order::reconcile(fresh, &snapshot.windows, &cfg.window_order)
                } else {
                    fresh
                };
                if cfg.defaults.smart_sort && !cfg.defaults.manual_order {
                    if let Ok(tracker) = activity.lock() {
                        let scores = tracker.score_windows(&queue);
                        let mut scored: Vec<_> = queue.into_iter().zip(scores).collect();
                        scored.sort_by(|a, b| b.1.total_cmp(&a.1));
                        queue = scored.into_iter().map(|(w, _)| w).collect();
                    }
                }
                let weights = request
                    .weights
                    .as_ref()
                    .map(|(c, r)| (c.as_slice(), r.as_slice()));
                let result = arrange::arrange_ordered(
                    &request.preset,
                    &cfg.defaults.monitor,
                    cfg.defaults.gap,
                    &request.disabled,
                    weights,
                    &queue,
                    &cfg.pin,
                );
                log::info!(
                    "Tray/hotkey: {} arranged, {} skipped, {:?}, {:?}",
                    result.arranged,
                    result.skipped,
                    result.errors,
                    result.warnings
                );
            }
            TrayAction::None => {}
        }
    }
    if registered {
        unsafe {
            let _ = UnregisterHotKey(None, 1);
        }
    }
}

fn check_for_updates(info: Arc<Mutex<Option<UpdateInfo>>>) {
    let result: Result<(), Box<dyn std::error::Error>> = (|| {
        let resp = ureq::get(
            "https://api.github.com/repos/TrentSterling/powershellmanager/releases/latest",
        )
        .timeout(std::time::Duration::from_secs(10))
        .set("User-Agent", "powershellmanager")
        .set("Accept", "application/vnd.github+json")
        .call()?;

        let json: serde_json::Value = resp.into_json()?;
        let tag = json["tag_name"].as_str().unwrap_or("");
        let url = "https://github.com/TrentSterling/powershellmanager/releases/latest";

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

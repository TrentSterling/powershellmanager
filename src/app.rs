use crate::activity::ActivityTracker;
use crate::arrange;
use crate::config::{self, Config};
use crate::gui;
use crate::layout::{builtin_presets, LayoutPreset};
use crate::theme::{self, ThemeSettings, THEMES};
use crate::tray;
use crate::updates::UpdateInfo;
use crate::windows::{ManagedWindow, TargetFilter};
use raw_window_handle::HasWindowHandle;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[cfg(test)]
pub(crate) mod tests;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DividerAxis {
    Col,
    Row,
}

#[derive(Debug, Clone, Copy)]
pub struct DividerDrag {
    pub axis: DividerAxis,
    pub index: usize,
    pub grab_offset: f32,
}

#[derive(Clone)]
struct LiveState {
    config: Config,
    windows: Vec<ManagedWindow>,
}

struct AppRuntime {
    desktop: Box<dyn crate::desktop::Desktop>,
    settings_path: Option<std::path::PathBuf>,
    activity_path: Option<std::path::PathBuf>,
    update_client: Option<crate::updates::ReleaseClient>,
}

impl AppRuntime {
    fn native() -> Self {
        Self {
            desktop: Box::new(crate::desktop::NativeDesktop),
            settings_path: config::settings_path(),
            activity_path: config::activity_path(),
            update_client: Some(crate::updates::ReleaseClient::default()),
        }
    }

    fn preview() -> Self {
        Self {
            desktop: Box::new(crate::desktop::NativeDesktop),
            settings_path: None,
            activity_path: None,
            update_client: None,
        }
    }
}

pub struct PsmApp {
    live: Arc<Mutex<LiveState>>,
    tray_ids: Arc<Mutex<Option<tray::TrayMenuIds>>>,
    quitting: Arc<AtomicBool>,
    action_worker: Option<std::thread::JoinHandle<()>>,
    update_worker: Option<std::thread::JoinHandle<()>>,
    desktop: Box<dyn crate::desktop::Desktop>,
    settings_path: Option<std::path::PathBuf>,
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
    pub dragging_divider: Option<DividerDrag>,
    pub activity: Arc<Mutex<ActivityTracker>>,
    pub history: Arc<Mutex<crate::history::LayoutHistory>>,
    pub save_grid_name: String,
    pub show_save_dialog: bool,
}

impl PsmApp {
    pub fn new(cc: &eframe::CreationContext<'_>, config: Config) -> Self {
        Self::new_with_runtime(
            cc,
            config,
            AppRuntime::native(),
            Box::new(crate::hotkey::NativeHotkeys),
        )
    }

    fn new_with_runtime(
        cc: &eframe::CreationContext<'_>,
        config: Config,
        runtime: AppRuntime,
        hotkeys: Box<dyn crate::hotkey::HotkeyApi + Send>,
    ) -> Self {
        let handle = cc.window_handle().ok().map(|handle| handle.as_raw());
        let Some(app_hwnd) = native_hwnd(handle) else {
            let mut app = Self::preview(config);
            app.status = "Native window is unavailable. Layout actions are disabled.".into();
            return app;
        };

        Self::start_native(config, app_hwnd, &cc.egui_ctx, runtime, hotkeys)
    }

    fn start_native(
        config: Config,
        app_hwnd: isize,
        ctx: &egui::Context,
        runtime: AppRuntime,
        hotkeys: Box<dyn crate::hotkey::HotkeyApi + Send>,
    ) -> Self {
        let mut app = Self::build(config, app_hwnd, true, runtime, ctx);
        app.rebuild_tray();
        if app._tray_icon.is_some() {
            let ids = app.tray_ids.clone();
            let state = WorkerState {
                live: app.live.clone(),
                activity: app.activity.clone(),
                history: app.history.clone(),
                quitting: app.quitting.clone(),
            };
            let ctx = ctx.clone();
            let worker = std::thread::Builder::new()
                .name("psm-tray-actions".into())
                .spawn(move || tray_event_loop(ids, ctx, app_hwnd, state, hotkeys));
            app.set_action_worker(worker);
        }
        app
    }

    pub fn preview(config: Config) -> Self {
        Self::build(
            config,
            0,
            false,
            AppRuntime::preview(),
            &egui::Context::default(),
        )
    }

    pub(crate) fn set_action_worker(
        &mut self,
        worker: std::io::Result<std::thread::JoinHandle<()>>,
    ) {
        match worker {
            Ok(worker) => self.action_worker = Some(worker),
            Err(error) => {
                self.status = format!("Tray actions unavailable: worker could not start: {error}");
                log::warn!("{}", self.status);
            }
        }
    }

    fn set_update_worker(&mut self, worker: std::io::Result<std::thread::JoinHandle<()>>) {
        match worker {
            Ok(worker) => self.update_worker = Some(worker),
            Err(error) => log::warn!("Update checker could not start (non-fatal): {error}"),
        }
    }

    #[cfg(test)]
    pub(crate) fn quit_signal(&self) -> Arc<AtomicBool> {
        self.quitting.clone()
    }

    fn build(
        mut config: Config,
        app_hwnd: isize,
        native_enabled: bool,
        mut runtime: AppRuntime,
        ctx: &egui::Context,
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
        if config.defaults.selected_preset >= presets.len() {
            config.defaults.selected_preset = 0;
        }
        let selected_preset = config.defaults.selected_preset;
        let custom_cols = config.defaults.custom_cols;
        let custom_rows = config.defaults.custom_rows;
        let use_custom = config.defaults.use_custom;

        let col_weights = normalized_weights(&config.defaults.col_weights, custom_cols);
        let row_weights = normalized_weights(&config.defaults.row_weights, custom_rows);
        let disabled_cells = config.defaults.disabled_cells.iter().copied().collect();

        let update_info: Arc<Mutex<Option<UpdateInfo>>> = Arc::new(Mutex::new(None));

        let activity = if native_enabled {
            ActivityTracker::new(config.defaults.decay_half_life_days, runtime.activity_path)
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
            live: Arc::new(Mutex::new(LiveState {
                config: config.clone(),
                windows: Vec::new(),
            })),
            tray_ids: Arc::new(Mutex::new(None)),
            quitting: Arc::new(AtomicBool::new(false)),
            action_worker: None,
            update_worker: None,
            monitors: if native_enabled {
                runtime.desktop.monitors()
            } else {
                Vec::new()
            },
            desktop: runtime.desktop,
            settings_path: runtime.settings_path,
            _tray_icon: None,
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
            theme_dirty: true,
            update_info,
            col_weights,
            row_weights,
            dragging_divider: None,
            activity: Arc::new(Mutex::new(activity)),
            history: Arc::new(Mutex::new(crate::history::LayoutHistory::default())),
            save_grid_name: String::new(),
            show_save_dialog: false,
        };

        app.disabled_cells.retain(|i| {
            *i < if app.use_custom {
                (app.custom_cols * app.custom_rows) as usize
            } else {
                app.presets[app.selected_preset].1.slot_count()
            }
        });
        if native_enabled {
            if let Some(client) = runtime.update_client {
                app.set_update_worker(crate::updates::spawn(
                    client,
                    app.update_info.clone(),
                    app.quitting.clone(),
                    ctx.clone(),
                ));
            }
        }
        app.refresh_windows();
        app
    }

    pub fn actions_available(&self) -> bool {
        self.native_enabled
    }

    pub fn minimize_all(&mut self) {
        if !self.native_enabled {
            self.status = "Preview only. No windows minimized.".into();
            return;
        }
        for window in &self.managed_windows {
            self.desktop.minimize_window(window.hwnd);
        }
        self.refresh_windows();
    }

    pub fn restore_all(&mut self) {
        if !self.native_enabled {
            self.status = "Preview only. No windows restored.".into();
            return;
        }
        for window in &self.managed_windows {
            self.desktop.restore_window(window.hwnd);
        }
        self.desktop.focus_window(self.app_hwnd);
        self.refresh_windows();
    }

    pub fn focus_window(&mut self, hwnd: isize) {
        if !self.native_enabled {
            self.status = "Preview only. No windows focused.".into();
            return;
        }
        if !self
            .managed_windows
            .iter()
            .any(|window| window.hwnd == hwnd)
        {
            self.status = "Selected window is no longer available. Refresh the window list.".into();
            return;
        }
        self.desktop.focus_window(hwnd);
        self.refresh_windows();
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
        let Ok(mut history) = self.history.lock() else {
            self.status = "Could not access layout history. No windows moved.".into();
            return;
        };
        let preset = self.active_preset();
        let weights = if self.use_custom {
            Some((self.col_weights.as_slice(), self.row_weights.as_slice()))
        } else {
            None
        };
        let result = arrange::arrange_with(
            &mut *self.desktop,
            &self.monitors,
            &arrange::ArrangeSettings {
                preset: &preset,
                monitor_spec: &self.config.defaults.monitor,
                gap: self.config.defaults.gap,
                disabled: &self.disabled_cells,
                weights,
                pins: &self.config.pin,
            },
            &self.managed_windows,
        );
        log::info!(
            "Arranged {} windows ({} skipped, {} errors)",
            result.arranged,
            result.skipped,
            result.errors.len()
        );
        self.status = result.status();
        history.pending_status = None;
        history.record(result.snapshots);
        for err in &result.errors {
            log::warn!("  {}", err);
        }
    }

    pub fn undo_layout(&mut self) {
        if !self.native_enabled {
            self.status = "Preview only. No windows restored.".into();
            return;
        }
        self.status = match self.history.lock() {
            Ok(mut history) => {
                history.pending_status = None;
                if !history.can_undo() {
                    "No layout changes to undo.".into()
                } else {
                    history.undo(&mut *self.desktop).status()
                }
            }
            Err(_) => "Could not access layout history.".into(),
        };
        self.refresh_windows();
    }

    pub fn refresh_windows(&mut self) {
        if !self.native_enabled {
            return;
        }
        self.monitors = self.desktop.monitors();
        let filter = TargetFilter::from_str(&self.config.defaults.target);
        let extra_exclude = self.config.categories.excluded_lower();
        let fresh = self.desktop.windows(&filter, self.app_hwnd, &extra_exclude);
        self.managed_windows = crate::desktop::ordered_windows(
            fresh,
            &self.managed_windows,
            &self.config,
            &self.activity,
        );
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
        match tray::create_tray(&self.config) {
            Ok((icon, ids)) => {
                let Ok(mut current) = self.tray_ids.lock() else {
                    self.status = "Could not update tray: action state is unavailable.".into();
                    log::warn!("{}", self.status);
                    return;
                };
                *current = Some(ids);
                self._tray_icon = Some(icon);
            }
            Err(error) => {
                self.status = format!("Could not create tray: {error}");
                log::warn!("{}", self.status);
            }
        }
    }

    pub fn save_config(&mut self) {
        self.sync_live();
        if self.native_enabled {
            let result = config::save(&self.config, self.settings_path.as_deref());
            if let Err(error) = result {
                log::warn!("{error}");
                self.status = error;
            }
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
                    if let Err(error) = tray.set_theme(self.theme_settings) {
                        self.status = format!("Could not update tray icon: {error}");
                        log::warn!("{}", self.status);
                    }
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
        self.rebuild_presets();
        self.save_config();
    }

    pub fn delete_saved_grid(&mut self, name: &str) {
        self.config.saved_grid.retain(|g| g.name != name);
        self.rebuild_presets();
        self.save_config();
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
        if self.selected_preset >= self.presets.len() {
            self.selected_preset = 0;
        }
        self.config.defaults.selected_preset = self.selected_preset;
        self.sync_live();
        self.rebuild_tray();
    }

    fn hide_window(&mut self) {
        self.desktop.hide_app(self.app_hwnd);
        self.gui_visible = false;
    }

    fn sync_tray_state(&mut self) {
        let Some(icon) = &self._tray_icon else {
            return;
        };
        let layout = if self.use_custom {
            format!("Custom {}x{}", self.custom_cols, self.custom_rows)
        } else {
            self.presets
                .get(self.selected_preset)
                .map(|(name, _)| name.clone())
                .unwrap_or_else(|| self.active_preset().display_name())
        };
        let enabled_slots = (0..self.active_preset().slot_count())
            .filter(|slot| !self.disabled_cells.contains(slot))
            .count();
        let state = tray::TrayState {
            layout,
            windows: self.managed_windows.len(),
            enabled_slots,
            can_apply: self.actions_available()
                && enabled_slots > 0
                && !self.managed_windows.is_empty()
                && !self.monitors.is_empty(),
            can_undo: self.history.lock().is_ok_and(|history| history.can_undo()),
            visible: self.gui_visible,
            status: self.status.clone(),
        };
        if let Err(error) = icon.update_state(state) {
            let message = format!("Could not update tray status: {error}");
            if self.status != message {
                log::warn!("{message}");
                self.status = message;
            }
        }
    }
}

fn native_hwnd(handle: Option<raw_window_handle::RawWindowHandle>) -> Option<isize> {
    match handle {
        Some(raw_window_handle::RawWindowHandle::Win32(handle)) => Some(handle.hwnd.get()),
        _ => None,
    }
}

impl eframe::App for PsmApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.update_view(ctx);
    }
}

impl PsmApp {
    fn update_view(&mut self, ctx: &egui::Context) {
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
        if self.native_enabled {
            let visible = self.desktop.visible(self.app_hwnd);
            if visible != self.gui_visible {
                self.gui_visible = visible;
                if visible {
                    self.refresh_windows();
                }
            }
        }

        // Update activity tracker (drains focus events)
        if let Ok(mut activity) = self.activity.lock() {
            activity.update();
        }

        if self.gui_visible && self.last_refresh.elapsed().as_secs() >= 3 {
            self.refresh_windows();
        }

        let pending = match self.history.lock() {
            Ok(mut history) => history.pending_status.take(),
            Err(_) => {
                self.status = "Could not access layout history.".into();
                None
            }
        };
        if let Some(status) = pending {
            self.status = status;
            self.refresh_windows();
        }
        if self.gui_visible {
            gui::draw(ctx, self);
        }

        self.sync_tray_state();

        if self.gui_visible {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
    }
}

impl Drop for PsmApp {
    fn drop(&mut self) {
        self.quitting.store(true, Ordering::Release);
        // The optional HTTP request has a ten-second deadline. Cancel publishing
        // and detach it so network latency cannot delay closing the app.
        drop(self.update_worker.take());
        if let Some(worker) = self.action_worker.take() {
            if worker.join().is_err() {
                log::warn!("Tray action worker stopped unexpectedly during shutdown");
            }
        }
        self.save_config();
        if let Ok(activity) = self.activity.lock() {
            activity.save();
        }
    }
}

struct WorkerState {
    live: Arc<Mutex<LiveState>>,
    activity: Arc<Mutex<ActivityTracker>>,
    history: Arc<Mutex<crate::history::LayoutHistory>>,
    quitting: Arc<AtomicBool>,
}

trait ActionEvents {
    fn wait(&mut self);
    fn menu(&mut self) -> tray::TrayAction;
    fn hotkey(&mut self) -> bool;
}

struct NativeActionEvents<'a> {
    menu_ids: &'a Mutex<Option<tray::TrayMenuIds>>,
    hotkey: Option<crate::hotkey::Binding<'a>>,
}

impl<'a> NativeActionEvents<'a> {
    fn new(
        menu_ids: &'a Mutex<Option<tray::TrayMenuIds>>,
        hotkeys: &'a dyn crate::hotkey::HotkeyApi,
    ) -> Self {
        use windows::Win32::UI::Input::KeyboardAndMouse::{MOD_ALT, MOD_CONTROL, MOD_NOREPEAT};
        let hotkey = crate::hotkey::Binding::register(
            hotkeys,
            1,
            MOD_CONTROL | MOD_ALT | MOD_NOREPEAT,
            0x47,
        )
        .map_err(|error| log::warn!("Ctrl+Alt+G is unavailable: {error}"))
        .ok();
        Self { menu_ids, hotkey }
    }
}

impl ActionEvents for NativeActionEvents<'_> {
    fn wait(&mut self) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    fn menu(&mut self) -> tray::TrayAction {
        self.menu_ids
            .lock()
            .ok()
            .and_then(|ids| ids.as_ref().map(|ids| ids.poll()))
            .unwrap_or(tray::TrayAction::None)
    }
    fn hotkey(&mut self) -> bool {
        self.hotkey.as_ref().is_some_and(|binding| binding.poll())
    }
}

/// Action worker shares current settings, queue and activity with the UI.
fn tray_event_loop(
    menu_ids: Arc<Mutex<Option<tray::TrayMenuIds>>>,
    ctx: egui::Context,
    hwnd: isize,
    state: WorkerState,
    hotkeys: Box<dyn crate::hotkey::HotkeyApi + Send>,
) {
    if state.quitting.load(Ordering::Relaxed) {
        return;
    }
    let mut events = NativeActionEvents::new(&menu_ids, hotkeys.as_ref());
    run_action_worker(
        &mut events,
        &mut crate::desktop::NativeDesktop,
        &ctx,
        hwnd,
        &state,
    );
}

fn run_action_worker(
    events: &mut dyn ActionEvents,
    desktop: &mut dyn crate::desktop::Desktop,
    ctx: &egui::Context,
    hwnd: isize,
    state: &WorkerState,
) {
    let mut pending_hotkey = false;
    while !state.quitting.load(Ordering::Relaxed) {
        events.wait();
        if state.quitting.load(Ordering::Relaxed) {
            break;
        }
        if let Ok(mut tracker) = state.activity.lock() {
            tracker.update();
        }
        let action = events.menu();
        pending_hotkey |= events.hotkey();
        dispatch_tray_action(
            next_tray_action(action, &mut pending_hotkey),
            desktop,
            ctx,
            hwnd,
            state,
        );
    }
}

fn dispatch_tray_action(
    action: tray::TrayAction,
    desktop: &mut dyn crate::desktop::Desktop,
    ctx: &egui::Context,
    hwnd: isize,
    state: &WorkerState,
) {
    use crate::tray::TrayAction;
    match action {
        TrayAction::ShowGui => {
            desktop.show_app(hwnd);
            ctx.request_repaint();
        }
        TrayAction::HideGui => {
            desktop.hide_app(hwnd);
            ctx.request_repaint();
        }
        TrayAction::RefreshWindows => {
            if let Ok(mut history) = state.history.lock() {
                history.pending_status = Some("Window list refreshed.".into());
            }
            ctx.request_repaint();
        }
        TrayAction::Quit => {
            state.quitting.store(true, Ordering::Relaxed);
            if let Ok(tracker) = state.activity.lock() {
                tracker.save();
            }
            desktop.wake_for_close(hwnd);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            ctx.request_repaint();
        }
        TrayAction::ApplyCurrent | TrayAction::ApplyLayout(_) => {
            let Ok(mut history) = state.history.lock() else {
                log::warn!("Could not access layout history. No windows moved.");
                ctx.request_repaint();
                return;
            };
            let snapshot = match state.live.lock() {
                Ok(live) => live.clone(),
                Err(_) => {
                    history.pending_status =
                        Some("Could not access current settings. No windows moved.".into());
                    ctx.request_repaint();
                    return;
                }
            };
            let Some(request) = tray::layout_request(&snapshot.config, &action) else {
                history.pending_status =
                    Some("Saved layout is unavailable. Reopen the layout menu.".into());
                ctx.request_repaint();
                return;
            };
            let cfg = &snapshot.config;
            let fresh = desktop.windows(
                &TargetFilter::from_str(&cfg.defaults.target),
                hwnd,
                &cfg.categories.excluded_lower(),
            );
            let queue =
                crate::desktop::ordered_windows(fresh, &snapshot.windows, cfg, &state.activity);
            let weights = request
                .weights
                .as_ref()
                .map(|(c, r)| (c.as_slice(), r.as_slice()));
            let monitors = desktop.monitors();
            let result = arrange::arrange_with(
                desktop,
                &monitors,
                &arrange::ArrangeSettings {
                    preset: &request.preset,
                    monitor_spec: &cfg.defaults.monitor,
                    gap: cfg.defaults.gap,
                    disabled: &request.disabled,
                    weights,
                    pins: &cfg.pin,
                },
                &queue,
            );
            log::info!(
                "Tray/hotkey: {} arranged, {} skipped, {:?}, {:?}",
                result.arranged,
                result.skipped,
                result.errors,
                result.warnings
            );
            history.pending_status = Some(result.status());
            history.record(result.snapshots);
            ctx.request_repaint();
        }
        TrayAction::UndoLayout => {
            if let Ok(mut history) = state.history.lock() {
                history.pending_status = Some(if history.can_undo() {
                    history.undo(desktop).status()
                } else {
                    "No layout changes to undo.".into()
                });
            }
            ctx.request_repaint();
        }
        TrayAction::None => {}
    }
}
pub(crate) fn next_tray_action(
    action: tray::TrayAction,
    pending_hotkey: &mut bool,
) -> tray::TrayAction {
    use tray::TrayAction;
    match action {
        TrayAction::None if *pending_hotkey => {
            *pending_hotkey = false;
            TrayAction::ApplyCurrent
        }
        TrayAction::None => TrayAction::None,
        TrayAction::ShowGui => TrayAction::ShowGui,
        action => {
            // An explicit layout, Undo or Quit wins over a simultaneous Apply
            // hotkey. In particular, do not immediately reapply after Undo.
            *pending_hotkey = false;
            action
        }
    }
}

/// Recover old/manual config without changing valid custom proportions.
pub(crate) fn normalized_weights(values: &[f32], count: u32) -> Vec<f32> {
    let count = count.clamp(1, 8) as usize;
    let sum: f32 = values.iter().sum();
    if values.len() != count || !sum.is_finite() || sum <= 0.0 || values.iter().any(|v| *v <= 0.0) {
        vec![1.0 / count as f32; count]
    } else {
        // Imported ratios may be so extreme that division rounds a positive
        // value to zero. Keep divider pairs representable and safe to resize.
        values
            .iter()
            .map(|v| (v / sum).max(f32::MIN_POSITIVE))
            .collect()
    }
}

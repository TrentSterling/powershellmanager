use crate::arrange;
use crate::config::{self, Config};
use crate::gui;
use crate::layout::{LayoutPreset, builtin_presets};
use crate::theme::THEMES;
use crate::tray::{TrayAction, TrayState};
use crate::windows::{TargetFilter, find_terminal_windows, TerminalWindow};
use raw_window_handle::HasWindowHandle;
use std::collections::HashSet;
use std::time::Instant;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_SHOWDEFAULT, ShowWindow};

pub struct PsmApp {
    tray: Option<TrayState>,
    pub gui_visible: bool,
    hwnd: Option<HWND>,
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
}

impl PsmApp {
    pub fn new(cc: &eframe::CreationContext<'_>, config: Config) -> Self {
        let hwnd = cc
            .window_handle()
            .ok()
            .and_then(|wh| {
                if let raw_window_handle::RawWindowHandle::Win32(h) = wh.as_raw() {
                    Some(HWND(h.hwnd.get() as *mut _))
                } else {
                    None
                }
            });

        let tray = TrayState::new(&config);
        if tray.is_none() {
            log::warn!("Failed to create system tray icon");
        }

        let mut presets = builtin_presets();
        for layout_def in &config.layout {
            if let Some(preset) = layout_def.to_preset() {
                presets.push((layout_def.name.clone(), preset));
            }
        }

        let theme_index = config.defaults.theme.min(THEMES.len() - 1);
        let mut app = Self {
            tray,
            gui_visible: true,
            hwnd,
            terminal_windows: Vec::new(),
            config,
            presets,
            selected_preset: 0,
            last_refresh: Instant::now(),
            custom_cols: 2,
            custom_rows: 2,
            use_custom: false,
            disabled_cells: HashSet::new(),
            theme_index,
            theme_dirty: true,
            icon_texture: None,
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

    pub fn apply_current_layout(&self) {
        let preset = self.active_preset();
        let filter = TargetFilter::from_str(&self.config.defaults.target);
        let result = arrange::arrange_masked(
            &preset,
            filter,
            &self.config.defaults.monitor,
            self.config.defaults.gap,
            &self.disabled_cells,
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

    fn show_window(&mut self) {
        if let Some(hwnd) = self.hwnd {
            unsafe {
                let _ = ShowWindow(hwnd, SW_SHOWDEFAULT);
            }
        }
        self.gui_visible = true;
    }

    fn hide_window(&mut self) {
        if let Some(hwnd) = self.hwnd {
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
        }
        self.gui_visible = false;
    }

    fn toggle_gui(&mut self) {
        if self.gui_visible {
            self.hide_window();
        } else {
            self.refresh_windows();
            self.show_window();
        }
    }
}

impl eframe::App for PsmApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.theme_dirty {
            self.current_theme().apply_to_egui(ctx);
            self.theme_dirty = false;
        }

        let close_requested = ctx.input(|i| i.viewport().close_requested());
        if close_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.hide_window();
        }

        if let Some(tray) = &self.tray {
            match tray.poll() {
                TrayAction::ToggleGui => {
                    self.toggle_gui();
                }
                TrayAction::ApplyLayout(_name, preset) => {
                    let filter = TargetFilter::from_str(&self.config.defaults.target);
                    let result = arrange::arrange_masked(
                        &preset,
                        filter,
                        &self.config.defaults.monitor,
                        self.config.defaults.gap,
                        &self.disabled_cells,
                    );
                    log::info!("Tray: arranged {} windows", result.arranged);
                }
                TrayAction::Quit => {
                    std::process::exit(0);
                }
                TrayAction::None => {}
            }
        }

        if self.gui_visible && self.last_refresh.elapsed().as_secs() >= 3 {
            self.refresh_windows();
        }

        if self.gui_visible {
            gui::draw(ctx, self);
        }

        ctx.request_repaint_after(std::time::Duration::from_secs(1));
    }
}

use crate::layout::LayoutPreset;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedGrid {
    pub name: String,
    pub cols: u32,
    pub rows: u32,
    #[serde(default)]
    pub col_weights: Vec<f32>,
    #[serde(default)]
    pub row_weights: Vec<f32>,
    #[serde(default)]
    pub disabled_cells: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedTheme {
    pub name: String,
    pub code: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub window_order: Vec<crate::order::WindowKey>,
    #[serde(default)]
    pub saved_theme: Vec<NamedTheme>,
    #[serde(default)]
    pub defaults: Defaults,
    #[serde(default)]
    pub layout: Vec<LayoutDef>,
    #[serde(default)]
    pub categories: CategoryOverrides,
    #[serde(default)]
    pub pin: Vec<PinRule>,
    #[serde(default)]
    pub saved_grid: Vec<SavedGrid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Defaults {
    #[serde(default = "default_target")]
    pub target: String,
    #[serde(default = "default_monitor")]
    pub monitor: String,
    #[serde(default = "default_gap")]
    pub gap: i32,
    #[serde(default)]
    pub theme: usize,
    #[serde(default)]
    pub theme_code: Option<String>,
    #[serde(default = "default_scale")]
    pub ui_scale: f32,
    #[serde(default = "default_true")]
    pub settings_open: bool,
    #[serde(default = "default_true")]
    pub about_open: bool,
    #[serde(default)]
    pub use_custom: bool,
    #[serde(default = "default_2")]
    pub custom_cols: u32,
    #[serde(default = "default_2")]
    pub custom_rows: u32,
    #[serde(default)]
    pub selected_preset: usize,
    #[serde(default)]
    pub disabled_cells: Vec<usize>,
    #[serde(default)]
    pub col_weights: Vec<f32>,
    #[serde(default)]
    pub row_weights: Vec<f32>,
    #[serde(default)]
    pub smart_sort: bool,
    #[serde(default)]
    pub manual_order: bool,
    #[serde(default = "default_decay_half_life")]
    pub decay_half_life_days: f64,
    /// New matching windows move into the first free slot; placed windows stay put.
    #[serde(default)]
    pub auto_arrange: bool,
    /// When a window closes, later windows slide up instead of leaving the hole.
    #[serde(default)]
    pub slide_to_fill: bool,
    /// With every slot full, auto mode uses the same grid on another display.
    #[serde(default)]
    pub overflow_display: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutDef {
    pub name: String,
    #[serde(default)]
    pub grid: Option<String>,
    #[serde(default)]
    pub style: Option<String>,
    #[serde(default)]
    pub count: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CategoryOverrides {
    #[serde(default)]
    pub terminals: Vec<String>,
    #[serde(default)]
    pub editors: Vec<String>,
    #[serde(default)]
    pub browsers: Vec<String>,
    #[serde(default)]
    pub chat: Vec<String>,
    #[serde(default)]
    pub media: Vec<String>,
    #[serde(default)]
    pub games: Vec<String>,
    #[serde(default)]
    pub devtools: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinRule {
    #[serde(default)]
    pub title_exact: Option<String>,
    /// Session binding survives changing terminal titles; never persisted.
    #[serde(skip)]
    pub bound_hwnd: Option<isize>,
    #[serde(default)]
    pub process: Option<String>,
    #[serde(default)]
    pub title_contains: Option<String>,
    pub slot: usize,
}

fn default_scale() -> f32 {
    1.0
}
fn default_target() -> String {
    "all".into()
}
fn default_monitor() -> String {
    "primary".into()
}
fn default_gap() -> i32 {
    4
}
fn default_true() -> bool {
    true
}
fn default_2() -> u32 {
    2
}
fn default_decay_half_life() -> f64 {
    7.0
}

impl Default for Defaults {
    fn default() -> Self {
        Self {
            target: default_target(),
            monitor: default_monitor(),
            gap: default_gap(),
            theme: 0,
            theme_code: None,
            ui_scale: 1.0,
            settings_open: true,
            about_open: true,
            use_custom: false,
            custom_cols: 2,
            custom_rows: 2,
            selected_preset: 0,
            disabled_cells: Vec::new(),
            col_weights: Vec::new(),
            row_weights: Vec::new(),
            smart_sort: false,
            manual_order: false,
            decay_half_life_days: default_decay_half_life(),
            auto_arrange: false,
            slide_to_fill: false,
            overflow_display: false,
        }
    }
}

impl LayoutDef {
    pub fn to_preset(&self) -> Option<LayoutPreset> {
        if let Some(grid) = &self.grid {
            return LayoutPreset::parse(grid);
        }
        if let Some(style) = &self.style {
            let count = self.count.unwrap_or(2);
            if !(1..=8).contains(&count) {
                return None;
            }
            match style.as_str() {
                "columns" => return Some(LayoutPreset::Columns(count)),
                "rows" => return Some(LayoutPreset::Rows(count)),
                "left-right" => return Some(LayoutPreset::LeftRight),
                "top-bottom" => return Some(LayoutPreset::TopBottom),
                "main-side" => return Some(LayoutPreset::MainSide { side_count: count }),
                "focus" => return Some(LayoutPreset::Focus { side_count: count }),
                _ => {}
            }
        }
        None
    }
}

impl CategoryOverrides {
    /// Get the list of process names to exclude from window management.
    pub fn excluded_lower(&self) -> Vec<String> {
        self.exclude.iter().map(|s| s.to_lowercase()).collect()
    }
}

impl PinRule {
    /// Every supplied condition must match. One rule reserves one window.
    pub fn matches(&self, process_name: &str, title: &str) -> bool {
        let present =
            self.process.is_some() || self.title_exact.is_some() || self.title_contains.is_some();
        present
            && self
                .process
                .as_ref()
                .is_none_or(|p| process_name.eq_ignore_ascii_case(p))
            && self.title_exact.as_ref().is_none_or(|t| title == t)
            && self
                .title_contains
                .as_ref()
                .is_none_or(|t| !t.is_empty() && title.to_lowercase().contains(&t.to_lowercase()))
    }
}

pub fn load() -> Config {
    crate::persistence::load_toml(
        settings_path()
            .into_iter()
            .chain([PathBuf::from("powershellmanager.toml")]),
    )
}

pub fn save(config: &Config, path: Option<&std::path::Path>) -> Result<(), String> {
    let path = path.ok_or("Could not locate the settings directory")?;
    crate::persistence::save_toml(path, config)
}

pub(crate) fn settings_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".powershellmanager").join("config.toml"))
}

/// Path to the activity database file.
pub fn activity_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".powershellmanager").join("activity.toml"))
}

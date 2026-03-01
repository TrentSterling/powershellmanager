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
pub struct Config {
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
    pub col_weights: Vec<f32>,
    #[serde(default)]
    pub row_weights: Vec<f32>,
    #[serde(default)]
    pub smart_sort: bool,
    #[serde(default = "default_decay_half_life")]
    pub decay_half_life_days: f64,
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
    pub process: Option<String>,
    #[serde(default)]
    pub title_contains: Option<String>,
    pub slot: usize,
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
            settings_open: true,
            about_open: true,
            use_custom: false,
            custom_cols: 2,
            custom_rows: 2,
            selected_preset: 0,
            col_weights: Vec::new(),
            row_weights: Vec::new(),
            smart_sort: false,
            decay_half_life_days: default_decay_half_life(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            defaults: Defaults::default(),
            layout: Vec::new(),
            categories: CategoryOverrides::default(),
            pin: Vec::new(),
            saved_grid: Vec::new(),
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
    /// Check if a window matches this pin rule.
    pub fn matches(&self, process_name: &str, title: &str) -> bool {
        if let Some(proc) = &self.process {
            if process_name.to_lowercase() == proc.to_lowercase() {
                return true;
            }
        }
        if let Some(substr) = &self.title_contains {
            if title.to_lowercase().contains(&substr.to_lowercase()) {
                return true;
            }
        }
        false
    }
}

pub fn load() -> Config {
    if let Some(path) = config_path() {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(config) = toml::from_str::<Config>(&content) {
                    log::info!("Loaded config from {}", path.display());
                    return config;
                } else {
                    log::warn!("Failed to parse config at {}", path.display());
                }
            }
        }
    }

    // Try CWD
    let cwd_path = PathBuf::from("powershellmanager.toml");
    if cwd_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&cwd_path) {
            if let Ok(config) = toml::from_str::<Config>(&content) {
                log::info!("Loaded config from CWD");
                return config;
            }
        }
    }

    log::info!("Using default config");
    Config::default()
}

pub fn save(config: &Config) {
    if let Some(path) = config_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match toml::to_string_pretty(config) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&path, content) {
                    log::warn!("Failed to write config: {}", e);
                } else {
                    log::info!("Saved config to {}", path.display());
                }
            }
            Err(e) => log::warn!("Failed to serialize config: {}", e),
        }
    }
}

fn config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".powershellmanager").join("config.toml"))
}

/// Path to the activity database file.
pub fn activity_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".powershellmanager").join("activity.toml"))
}

use crate::{
    config::Config,
    layout::{builtin_presets, LayoutPreset},
    theme::ThemeSettings,
};
use std::collections::HashSet;
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, Submenu};
use tray_icon::{Icon, TrayIconBuilder, TrayIconEvent};

pub struct TrayIcon {
    pub _tray: tray_icon::TrayIcon,
}
pub struct TrayMenuIds {
    open_id: MenuId,
    quit_id: MenuId,
    current_id: MenuId,
    layout_items: Vec<(MenuId, LayoutChoice)>,
}
#[derive(Debug, Clone)]
pub enum LayoutChoice {
    Preset(LayoutPreset),
    Saved(String),
}
#[derive(Debug)]
pub enum TrayAction {
    None,
    ShowGui,
    ApplyCurrent,
    ApplyLayout(LayoutChoice),
    Quit,
}
pub struct LayoutRequest {
    pub preset: LayoutPreset,
    pub weights: Option<(Vec<f32>, Vec<f32>)>,
    pub disabled: HashSet<usize>,
}

/// Resolve at click time, using current settings rather than a startup snapshot.
pub fn layout_request(config: &Config, action: &TrayAction) -> Option<LayoutRequest> {
    match action {
        TrayAction::ApplyLayout(LayoutChoice::Saved(name)) => {
            let grid = config.saved_grid.iter().find(|g| &g.name == name)?;
            Some(LayoutRequest {
                preset: LayoutPreset::Grid {
                    cols: grid.cols.clamp(1, 8),
                    rows: grid.rows.clamp(1, 8),
                },
                weights: Some((grid.col_weights.clone(), grid.row_weights.clone())),
                disabled: grid.disabled_cells.iter().copied().collect(),
            })
        }
        TrayAction::ApplyLayout(LayoutChoice::Preset(preset)) => Some(LayoutRequest {
            preset: preset.clone(),
            weights: None,
            disabled: HashSet::new(),
        }),
        TrayAction::ApplyCurrent => {
            let d = &config.defaults;
            let preset = if d.use_custom {
                LayoutPreset::Grid {
                    cols: d.custom_cols.clamp(1, 8),
                    rows: d.custom_rows.clamp(1, 8),
                }
            } else {
                let mut presets = builtin_presets();
                presets.extend(
                    config
                        .layout
                        .iter()
                        .filter_map(|l| l.to_preset().map(|p| (l.name.clone(), p))),
                );
                if d.selected_preset >= presets.len() {
                    if let Some(g) = config.saved_grid.get(d.selected_preset - presets.len()) {
                        return layout_request(
                            config,
                            &TrayAction::ApplyLayout(LayoutChoice::Saved(g.name.clone())),
                        );
                    }
                }
                presets
                    .get(d.selected_preset)
                    .map(|(_, p)| p.clone())
                    .unwrap_or(LayoutPreset::Grid { cols: 2, rows: 2 })
            };
            Some(LayoutRequest {
                preset,
                weights: if d.use_custom {
                    Some((d.col_weights.clone(), d.row_weights.clone()))
                } else {
                    None
                },
                disabled: d.disabled_cells.iter().copied().collect(),
            })
        }
        _ => None,
    }
}

pub fn create_tray(config: &Config) -> Option<(TrayIcon, TrayMenuIds)> {
    let menu = Menu::new();
    let open = MenuItem::new("Open Window", true, None);
    let quit = MenuItem::new("Quit", true, None);
    let current = MenuItem::new("Apply current layout (Ctrl+Alt+G)", true, None);
    menu.append_items(&[&open, &current]).ok()?;
    let layouts = Submenu::new("Layouts", true);
    let mut items = Vec::new();
    let mut presets = builtin_presets();
    presets.extend(
        config
            .layout
            .iter()
            .filter_map(|l| l.to_preset().map(|p| (l.name.clone(), p))),
    );
    for (name, preset) in presets {
        let item = MenuItem::new(name, true, None);
        layouts.append(&item).ok()?;
        items.push((item.id().clone(), LayoutChoice::Preset(preset)));
    }
    for grid in &config.saved_grid {
        let item = MenuItem::new(
            format!("{} ({}x{})", grid.name, grid.cols, grid.rows),
            true,
            None,
        );
        layouts.append(&item).ok()?;
        items.push((item.id().clone(), LayoutChoice::Saved(grid.name.clone())));
    }
    menu.append_items(&[&layouts, &quit]).ok()?;
    let theme = config
        .defaults
        .theme_code
        .as_deref()
        .and_then(ThemeSettings::decode)
        .unwrap_or_else(|| {
            crate::theme::from_legacy(config.defaults.theme.min(crate::theme::THEMES.len() - 1))
        });
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("PowerShell Manager")
        .with_icon(tray_icon(theme)?)
        .build()
        .ok()?;
    Some((
        TrayIcon { _tray: tray },
        TrayMenuIds {
            open_id: open.id().clone(),
            quit_id: quit.id().clone(),
            current_id: current.id().clone(),
            layout_items: items,
        },
    ))
}
impl TrayIcon {
    pub fn set_theme(&self, theme: ThemeSettings) {
        let _ = self._tray.set_icon(tray_icon(theme));
    }
}
fn tray_icon(theme: ThemeSettings) -> Option<Icon> {
    let icon = crate::branding::icon(theme, 32);
    Icon::from_rgba(icon.rgba, icon.width, icon.height).ok()
}
impl TrayMenuIds {
    pub fn poll(&self) -> TrayAction {
        if let Ok(TrayIconEvent::Click {
            button: tray_icon::MouseButton::Left,
            button_state: tray_icon::MouseButtonState::Up,
            ..
        }) = TrayIconEvent::receiver().try_recv()
        {
            return TrayAction::ShowGui;
        }
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.open_id {
                return TrayAction::ShowGui;
            }
            if event.id == self.quit_id {
                return TrayAction::Quit;
            }
            if event.id == self.current_id {
                return TrayAction::ApplyCurrent;
            }
            for (id, choice) in &self.layout_items {
                if event.id == *id {
                    return TrayAction::ApplyLayout(choice.clone());
                }
            }
        }
        TrayAction::None
    }
}

use crate::{
    config::Config,
    layout::{builtin_presets, LayoutPreset},
    theme::ThemeSettings,
};
use std::{cell::RefCell, collections::HashSet, rc::Rc};
use tray_icon::menu::{IsMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIconBuilder, TrayIconEvent, TrayIconId};

#[cfg(test)]
mod tests;

pub struct TrayIcon {
    pub _tray: tray_icon::TrayIcon,
    api: Rc<dyn TrayApi>,
    summary: MenuItem,
    last_action: MenuItem,
    current: MenuItem,
    undo: MenuItem,
    hide: MenuItem,
    layouts: Submenu,
    last_state: RefCell<Option<TrayState>>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TrayState {
    pub layout: String,
    pub windows: usize,
    pub enabled_slots: usize,
    pub can_apply: bool,
    pub can_undo: bool,
    pub visible: bool,
    pub status: String,
}

impl TrayState {
    fn summary(&self) -> String {
        format!(
            "{} windows / {} enabled slots | {}",
            self.windows, self.enabled_slots, self.layout
        )
    }

    fn tooltip(&self) -> String {
        let visibility = if self.visible {
            "Window open"
        } else {
            "Running in tray"
        };
        bounded_text(
            &format!("PowerShell Manager\n{}\n{visibility}", self.summary()),
            127,
        )
    }
}

/// Windows notification tips have 128 UTF-16 code units, including the terminator.
fn bounded_text(text: &str, limit: usize) -> String {
    let mut units = 0;
    text.chars()
        .take_while(|ch| {
            units += ch.len_utf16();
            units <= limit
        })
        .collect()
}

trait TrayApi {
    fn append_menu(&self, menu: &Menu, items: &[&dyn IsMenuItem]) -> tray_icon::menu::Result<()>;
    fn append_layout(&self, menu: &Submenu, item: &MenuItem) -> tray_icon::menu::Result<()>;
    fn make_icon(&self, icon: egui::IconData) -> Result<Icon, tray_icon::BadIcon>;
    fn build(&self, menu: Menu, icon: Icon) -> tray_icon::Result<tray_icon::TrayIcon>;
    fn set_icon(&self, tray: &tray_icon::TrayIcon, icon: Icon) -> tray_icon::Result<()>;
    fn set_tooltip(&self, tray: &tray_icon::TrayIcon, text: &str) -> tray_icon::Result<()>;
}

struct NativeTrayApi;
impl TrayApi for NativeTrayApi {
    fn append_menu(&self, menu: &Menu, items: &[&dyn IsMenuItem]) -> tray_icon::menu::Result<()> {
        menu.append_items(items)
    }
    fn append_layout(&self, menu: &Submenu, item: &MenuItem) -> tray_icon::menu::Result<()> {
        menu.append(item)
    }
    fn make_icon(&self, icon: egui::IconData) -> Result<Icon, tray_icon::BadIcon> {
        Icon::from_rgba(icon.rgba, icon.width, icon.height)
    }
    fn build(&self, menu: Menu, icon: Icon) -> tray_icon::Result<tray_icon::TrayIcon> {
        TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("PowerShell Manager")
            .with_icon(icon)
            .build()
    }
    fn set_icon(&self, tray: &tray_icon::TrayIcon, icon: Icon) -> tray_icon::Result<()> {
        tray.set_icon(Some(icon))
    }
    fn set_tooltip(&self, tray: &tray_icon::TrayIcon, text: &str) -> tray_icon::Result<()> {
        tray.set_tooltip(Some(text))
    }
}
pub struct TrayMenuIds {
    tray_id: TrayIconId,
    open_id: MenuId,
    quit_id: MenuId,
    current_id: MenuId,
    undo_id: MenuId,
    hide_id: MenuId,
    refresh_id: MenuId,
    layout_items: Vec<(MenuId, LayoutChoice)>,
}
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutChoice {
    Preset(LayoutPreset),
    Saved { index: usize, name: String },
}
#[derive(Debug, Clone, PartialEq)]
pub enum TrayAction {
    None,
    ShowGui,
    HideGui,
    RefreshWindows,
    ApplyCurrent,
    UndoLayout,
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
        TrayAction::ApplyLayout(LayoutChoice::Saved { index, name }) => {
            let grid = config
                .saved_grid
                .get(*index)
                .filter(|grid| &grid.name == name)?;
            Some(saved_request(grid))
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
                        return Some(saved_request(g));
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

fn saved_request(grid: &crate::config::SavedGrid) -> LayoutRequest {
    LayoutRequest {
        preset: LayoutPreset::Grid {
            cols: grid.cols.clamp(1, 8),
            rows: grid.rows.clamp(1, 8),
        },
        weights: Some((grid.col_weights.clone(), grid.row_weights.clone())),
        disabled: grid.disabled_cells.iter().copied().collect(),
    }
}

pub fn create_tray(config: &Config) -> Result<(TrayIcon, TrayMenuIds), String> {
    create_tray_with_api(config, Rc::new(NativeTrayApi))
}

fn create_tray_with_api(
    config: &Config,
    api: Rc<dyn TrayApi>,
) -> Result<(TrayIcon, TrayMenuIds), String> {
    let menu = Menu::new();
    let summary = MenuItem::new("PowerShell Manager", false, None);
    let last_action = MenuItem::new("Ready", false, None);
    let open = MenuItem::new("Show PowerShell Manager", true, None);
    let hide = MenuItem::new("Hide to tray", true, None);
    let refresh = MenuItem::new("Refresh windows", true, None);
    let quit = MenuItem::new("Quit", true, None);
    let current = MenuItem::new("Apply current layout (Ctrl+Alt+G)", false, None);
    let undo = MenuItem::new("Undo last layout", false, None);
    api.append_menu(
        &menu,
        &[
            &summary,
            &last_action,
            &PredefinedMenuItem::separator(),
            &open,
            &hide,
            &refresh,
            &current,
            &undo,
        ],
    )
    .map_err(|error| error.to_string())?;
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
        api.append_layout(&layouts, &item)
            .map_err(|error| error.to_string())?;
        items.push((item.id().clone(), LayoutChoice::Preset(preset)));
    }
    for (index, grid) in config.saved_grid.iter().enumerate() {
        let item = MenuItem::new(
            format!("{} ({}x{})", grid.name, grid.cols, grid.rows),
            true,
            None,
        );
        api.append_layout(&layouts, &item)
            .map_err(|error| error.to_string())?;
        items.push((
            item.id().clone(),
            LayoutChoice::Saved {
                index,
                name: grid.name.clone(),
            },
        ));
    }
    api.append_menu(&menu, &[&layouts, &PredefinedMenuItem::separator(), &quit])
        .map_err(|error| error.to_string())?;
    let theme = config
        .defaults
        .theme_code
        .as_deref()
        .and_then(ThemeSettings::decode)
        .unwrap_or_else(|| {
            crate::theme::from_legacy(config.defaults.theme.min(crate::theme::THEMES.len() - 1))
        });
    let tray = api
        .build(menu, tray_icon(theme, api.as_ref())?)
        .map_err(|error| format!("Notification icon registration failed: {error}"))?;
    let tray_id = tray.id().clone();
    Ok((
        TrayIcon {
            _tray: tray,
            api,
            summary,
            last_action,
            current: current.clone(),
            undo: undo.clone(),
            hide: hide.clone(),
            layouts,
            last_state: RefCell::new(None),
        },
        TrayMenuIds {
            tray_id,
            open_id: open.id().clone(),
            quit_id: quit.id().clone(),
            current_id: current.id().clone(),
            undo_id: undo.id().clone(),
            hide_id: hide.id().clone(),
            refresh_id: refresh.id().clone(),
            layout_items: items,
        },
    ))
}
impl TrayIcon {
    pub(crate) fn update_state(&self, state: TrayState) -> Result<(), String> {
        if self.last_state.borrow().as_ref() == Some(&state) {
            return Ok(());
        }
        self.api
            .set_tooltip(&self._tray, &state.tooltip())
            .map_err(|error| error.to_string())?;
        // Ampersands in user layout names must be literal rather than accelerators.
        self.summary.set_text(state.summary().replace('&', "&&"));
        self.last_action
            .set_text(bounded_text(&state.status, 180).replace('&', "&&"));
        self.current.set_text(format!(
            "Apply {} (Ctrl+Alt+G)",
            state.layout.replace('&', "&&")
        ));
        self.current.set_enabled(state.can_apply);
        self.undo.set_enabled(state.can_undo);
        self.hide.set_enabled(state.visible);
        self.layouts.set_enabled(state.windows > 0);
        *self.last_state.borrow_mut() = Some(state);
        Ok(())
    }

    pub fn set_theme(&self, theme: ThemeSettings) -> Result<(), String> {
        self.api
            .set_icon(&self._tray, tray_icon(theme, self.api.as_ref())?)
            .map_err(|error| error.to_string())
    }
}
fn tray_icon(theme: ThemeSettings, api: &dyn TrayApi) -> Result<Icon, String> {
    let icon = crate::branding::icon(theme, 32);
    api.make_icon(icon).map_err(|error| error.to_string())
}
impl TrayMenuIds {
    pub fn poll(&self) -> TrayAction {
        self.poll_events(
            &mut MenuEvent::receiver().try_iter(),
            &mut TrayIconEvent::receiver().try_iter(),
        )
    }

    fn poll_events(
        &self,
        menu_events: &mut dyn Iterator<Item = MenuEvent>,
        icon_events: &mut dyn Iterator<Item = TrayIconEvent>,
    ) -> TrayAction {
        // Menu commands take priority over hover traffic. Bound each drain so a
        // producer cannot hold the worker indefinitely while shutdown is pending.
        for event in menu_events.take(64) {
            if event.id == self.open_id {
                return TrayAction::ShowGui;
            }
            if event.id == self.quit_id {
                return TrayAction::Quit;
            }
            if event.id == self.current_id {
                return TrayAction::ApplyCurrent;
            }
            if event.id == self.undo_id {
                return TrayAction::UndoLayout;
            }
            if event.id == self.hide_id {
                return TrayAction::HideGui;
            }
            if event.id == self.refresh_id {
                return TrayAction::RefreshWindows;
            }
            for (id, choice) in &self.layout_items {
                if event.id == *id {
                    return TrayAction::ApplyLayout(choice.clone());
                }
            }
        }
        for event in icon_events.take(64) {
            let owner = match event {
                TrayIconEvent::Click {
                    id,
                    button: tray_icon::MouseButton::Left,
                    button_state: tray_icon::MouseButtonState::Up,
                    ..
                }
                | TrayIconEvent::DoubleClick {
                    id,
                    button: tray_icon::MouseButton::Left,
                    ..
                } => Some(id),
                _ => None,
            };
            if let Some(id) = owner {
                if id == self.tray_id {
                    return TrayAction::ShowGui;
                }
            }
        }
        TrayAction::None
    }
}

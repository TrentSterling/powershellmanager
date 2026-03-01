use crate::config::Config;
use crate::layout::{LayoutPreset, builtin_presets};
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, Submenu};
use tray_icon::{Icon, TrayIconBuilder, TrayIconEvent};

/// Holds the tray icon (must stay alive on the main thread — not Send).
pub struct TrayIcon {
    pub _tray: tray_icon::TrayIcon,
}

/// Menu IDs extracted from tray setup — Send + Clone, safe to move to a thread.
pub struct TrayMenuIds {
    pub open_id: MenuId,
    pub quit_id: MenuId,
    pub layout_items: Vec<(MenuId, String, LayoutPreset, Option<(Vec<f32>, Vec<f32>)>)>,
}

#[derive(Debug)]
pub enum TrayAction {
    None,
    ShowGui,
    ApplyLayout(String, LayoutPreset, Option<(Vec<f32>, Vec<f32>)>),
    Quit,
}

/// Create the tray icon (stays on main thread) and return the menu IDs (move to bg thread).
pub fn create_tray(config: &Config) -> Option<(TrayIcon, TrayMenuIds)> {
    let menu = Menu::new();

    let open_item = MenuItem::new("Open Window", true, None);
    let open_id = open_item.id().clone();
    let _ = menu.append(&open_item);

    let sep_top = tray_icon::menu::PredefinedMenuItem::separator();
    let _ = menu.append(&sep_top);

    let layouts_submenu = Submenu::new("Layouts", true);
    let mut layout_items = Vec::new();

    for (name, preset) in builtin_presets() {
        let item = MenuItem::new(&name, true, None);
        let id = item.id().clone();
        let _ = layouts_submenu.append(&item);
        layout_items.push((id, name, preset, None));
    }

    for layout_def in &config.layout {
        if let Some(preset) = layout_def.to_preset() {
            let item = MenuItem::new(&layout_def.name, true, None);
            let id = item.id().clone();
            let _ = layouts_submenu.append(&item);
            layout_items.push((id, layout_def.name.clone(), preset, None));
        }
    }

    if !config.saved_grid.is_empty() {
        let _ = layouts_submenu.append(&tray_icon::menu::PredefinedMenuItem::separator());
        for sg in &config.saved_grid {
            let label = format!("{} ({}x{})", sg.name, sg.cols, sg.rows);
            let item = MenuItem::new(&label, true, None);
            let id = item.id().clone();
            let _ = layouts_submenu.append(&item);
            let weights = Some((sg.col_weights.clone(), sg.row_weights.clone()));
            let preset = LayoutPreset::Grid { cols: sg.cols, rows: sg.rows };
            layout_items.push((id, sg.name.clone(), preset, weights));
        }
    }

    let _ = menu.append(&layouts_submenu);

    let separator = tray_icon::menu::PredefinedMenuItem::separator();
    let _ = menu.append(&separator);

    let quit_item = MenuItem::new("Quit", true, None);
    let quit_id = quit_item.id().clone();
    let _ = menu.append(&quit_item);

    let icon = create_tray_icon()?;

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("PowerShell Manager")
        .with_icon(icon)
        .build()
        .ok()?;

    Some((
        TrayIcon { _tray: tray },
        TrayMenuIds { open_id, quit_id, layout_items },
    ))
}

impl TrayMenuIds {
    /// Poll tray icon clicks and menu events. Call from a background thread.
    pub fn poll(&self) -> TrayAction {
        if let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click {
                button: tray_icon::MouseButton::Left,
                button_state: tray_icon::MouseButtonState::Up,
                ..
            } = event
            {
                return TrayAction::ShowGui;
            }
        }

        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.open_id {
                return TrayAction::ShowGui;
            }
            if event.id == self.quit_id {
                return TrayAction::Quit;
            }
            for (id, name, preset, weights) in &self.layout_items {
                if event.id == *id {
                    return TrayAction::ApplyLayout(name.clone(), preset.clone(), weights.clone());
                }
            }
        }

        TrayAction::None
    }
}

fn create_tray_icon() -> Option<Icon> {
    static ICON_PNG: &[u8] = include_bytes!("../assets/tront-icon.png");

    let img = image::load_from_memory(ICON_PNG).ok()?;
    let resized = img.resize_exact(32, 32, image::imageops::FilterType::Lanczos3);
    let rgba = resized.to_rgba8();
    let (w, h) = rgba.dimensions();
    Icon::from_rgba(rgba.into_raw(), w, h).ok()
}

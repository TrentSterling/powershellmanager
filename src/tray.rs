use crate::config::Config;
use crate::layout::{LayoutPreset, builtin_presets};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, Submenu};
use tray_icon::{Icon, TrayIconBuilder, TrayIconEvent};

pub struct TrayState {
    _tray: tray_icon::TrayIcon,
    open_id: tray_icon::menu::MenuId,
    layout_items: Vec<(tray_icon::menu::MenuId, String, LayoutPreset)>,
    quit_id: tray_icon::menu::MenuId,
}

#[derive(Debug)]
pub enum TrayAction {
    None,
    ToggleGui,
    ApplyLayout(String, LayoutPreset),
    Quit,
}

impl TrayState {
    pub fn new(config: &Config) -> Option<Self> {
        let menu = Menu::new();

        let open_item = MenuItem::new("Open Window", true, None);
        let open_id = open_item.id().clone();
        let _ = menu.append(&open_item);

        let sep_top = tray_icon::menu::PredefinedMenuItem::separator();
        let _ = menu.append(&sep_top);

        let layouts_submenu = Submenu::new("Layouts", true);
        let mut layout_items = Vec::new();

        // Built-in presets
        for (name, preset) in builtin_presets() {
            let item = MenuItem::new(&name, true, None);
            let id = item.id().clone();
            let _ = layouts_submenu.append(&item);
            layout_items.push((id, name, preset));
        }

        // Custom presets from config
        for layout_def in &config.layout {
            if let Some(preset) = layout_def.to_preset() {
                let item = MenuItem::new(&layout_def.name, true, None);
                let id = item.id().clone();
                let _ = layouts_submenu.append(&item);
                layout_items.push((id, layout_def.name.clone(), preset));
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

        Some(Self {
            _tray: tray,
            open_id,
            layout_items,
            quit_id,
        })
    }

    pub fn poll(&self) -> TrayAction {
        // Check for left-click on tray icon
        if let Ok(event) = TrayIconEvent::receiver().try_recv() {
            match event {
                TrayIconEvent::Click {
                    button: tray_icon::MouseButton::Left,
                    button_state: tray_icon::MouseButtonState::Up,
                    ..
                } => return TrayAction::ToggleGui,
                _ => {}
            }
        }

        // Check menu events
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id == self.open_id {
                return TrayAction::ToggleGui;
            }
            if event.id == self.quit_id {
                return TrayAction::Quit;
            }
            for (id, name, preset) in &self.layout_items {
                if event.id == *id {
                    return TrayAction::ApplyLayout(name.clone(), preset.clone());
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

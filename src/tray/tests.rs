use super::*;
use crate::config::{LayoutDef, SavedGrid};
use tray_icon::{MouseButton, MouseButtonState};

#[derive(Clone, Copy, Default, PartialEq)]
enum Fault {
    #[default]
    None,
    Commands,
    Layout(usize),
    Tail,
    Icon,
    Build,
    SetIcon,
    SetTooltip,
}

#[derive(Default)]
struct FaultApi {
    fault: std::cell::Cell<Fault>,
    menu_calls: std::cell::Cell<usize>,
    layout_calls: std::cell::Cell<usize>,
    build_calls: std::cell::Cell<usize>,
    set_calls: std::cell::Cell<usize>,
    tip_calls: std::cell::Cell<usize>,
}
impl TrayApi for FaultApi {
    fn append_menu(&self, menu: &Menu, items: &[&dyn IsMenuItem]) -> tray_icon::menu::Result<()> {
        let call = self.menu_calls.get();
        self.menu_calls.set(call + 1);
        if (call == 0 && self.fault.get() == Fault::Commands)
            || (call == 1 && self.fault.get() == Fault::Tail)
        {
            Err(tray_icon::menu::Error::NotInitialized)
        } else {
            NativeTrayApi.append_menu(menu, items)
        }
    }
    fn append_layout(&self, menu: &Submenu, item: &MenuItem) -> tray_icon::menu::Result<()> {
        let call = self.layout_calls.get();
        self.layout_calls.set(call + 1);
        if self.fault.get() == Fault::Layout(call) {
            Err(tray_icon::menu::Error::NotInitialized)
        } else {
            NativeTrayApi.append_layout(menu, item)
        }
    }
    fn make_icon(&self, icon: egui::IconData) -> Result<Icon, tray_icon::BadIcon> {
        if self.fault.get() == Fault::Icon {
            NativeTrayApi.make_icon(egui::IconData {
                rgba: vec![],
                width: 2,
                height: 2,
            })
        } else {
            NativeTrayApi.make_icon(icon)
        }
    }
    fn build(&self, menu: Menu, icon: Icon) -> tray_icon::Result<tray_icon::TrayIcon> {
        self.build_calls.set(self.build_calls.get() + 1);
        if self.fault.get() == Fault::Build {
            Err(tray_icon::Error::OsError(std::io::Error::other(
                "owned registration failure",
            )))
        } else {
            NativeTrayApi.build(menu, icon)
        }
    }
    fn set_icon(&self, tray: &tray_icon::TrayIcon, icon: Icon) -> tray_icon::Result<()> {
        self.set_calls.set(self.set_calls.get() + 1);
        if self.fault.get() == Fault::SetIcon {
            Err(tray_icon::Error::OsError(std::io::Error::other(
                "owned icon update failure",
            )))
        } else {
            NativeTrayApi.set_icon(tray, icon)
        }
    }
    fn set_tooltip(&self, tray: &tray_icon::TrayIcon, text: &str) -> tray_icon::Result<()> {
        self.tip_calls.set(self.tip_calls.get() + 1);
        if self.fault.get() == Fault::SetTooltip {
            Err(tray_icon::Error::OsError(std::io::Error::other(
                "owned tooltip update failure",
            )))
        } else {
            NativeTrayApi.set_tooltip(tray, text)
        }
    }
}

#[test]
fn menu_and_icon_failures_abort_setup_before_notification_registration() {
    let config = Config {
        saved_grid: vec![grid("Failure audit", 2)],
        ..Default::default()
    };
    for fault in [
        Fault::Commands,
        Fault::Layout(0),
        Fault::Layout(builtin_presets().len()),
        Fault::Tail,
        Fault::Icon,
        Fault::Build,
    ] {
        let api = Rc::new(FaultApi::default());
        api.fault.set(fault);
        let error = create_tray_with_api(&config, api.clone())
            .err()
            .expect("controlled tray setup must fail");
        match fault {
            Fault::Icon => assert!(error.contains("don't match the number of pixels")),
            Fault::Build => assert_eq!(
                error,
                "Notification icon registration failed: owned registration failure"
            ),
            _ => assert_eq!(error, tray_icon::menu::Error::NotInitialized.to_string()),
        }
        assert_eq!(api.build_calls.get(), usize::from(fault == Fault::Build));
        assert_eq!(api.set_calls.get(), 0);
    }
}

fn ids() -> TrayMenuIds {
    TrayMenuIds {
        tray_id: TrayIconId::new("owned-icon"),
        open_id: MenuId::new("open"),
        quit_id: MenuId::new("quit"),
        current_id: MenuId::new("current"),
        undo_id: MenuId::new("undo"),
        hide_id: MenuId::new("hide"),
        refresh_id: MenuId::new("refresh"),
        layout_items: vec![
            (
                MenuId::new("split"),
                LayoutChoice::Preset(LayoutPreset::LeftRight),
            ),
            (
                MenuId::new("saved"),
                LayoutChoice::Saved {
                    index: 0,
                    name: "Work".into(),
                },
            ),
        ],
    }
}

fn click(id: &str, button: MouseButton, button_state: MouseButtonState) -> TrayIconEvent {
    TrayIconEvent::Click {
        id: TrayIconId::new(id),
        position: tray_icon::dpi::PhysicalPosition::new(0.0, 0.0),
        rect: Default::default(),
        button,
        button_state,
    }
}

fn poll(ids: &TrayMenuIds, menus: Vec<MenuEvent>, icons: Vec<TrayIconEvent>) -> TrayAction {
    ids.poll_events(&mut menus.into_iter(), &mut icons.into_iter())
}

#[test]
fn only_current_icon_left_button_release_opens_the_window() {
    let ids = ids();
    for owner in ["owned-icon", "retired-icon"] {
        for button in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
            for state in [MouseButtonState::Up, MouseButtonState::Down] {
                let expected = if owner == "owned-icon"
                    && button == MouseButton::Left
                    && state == MouseButtonState::Up
                {
                    TrayAction::ShowGui
                } else {
                    TrayAction::None
                };
                assert_eq!(
                    poll(&ids, vec![], vec![click(owner, button, state)]),
                    expected
                );
            }
        }
    }
    let id = ids.tray_id.clone();
    let position = tray_icon::dpi::PhysicalPosition::new(10.0, 20.0);
    let rect = Default::default();
    let passive = vec![
        TrayIconEvent::Enter {
            id: id.clone(),
            position,
            rect,
        },
        TrayIconEvent::Move {
            id: id.clone(),
            position,
            rect,
        },
        TrayIconEvent::Leave {
            id: id.clone(),
            position,
            rect,
        },
    ];
    assert_eq!(poll(&ids, vec![], passive), TrayAction::None);
    for owner in ["owned-icon", "retired-icon"] {
        for button in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
            let expected = if owner == "owned-icon" && button == MouseButton::Left {
                TrayAction::ShowGui
            } else {
                TrayAction::None
            };
            assert_eq!(
                poll(
                    &ids,
                    vec![],
                    vec![TrayIconEvent::DoubleClick {
                        id: TrayIconId::new(owner),
                        position,
                        rect,
                        button,
                    }]
                ),
                expected
            );
        }
    }
}

#[test]
fn menu_commands_keep_identity_and_take_priority_without_discarding_clicks() {
    let ids = ids();
    for (id, expected) in [
        (&ids.open_id, TrayAction::ShowGui),
        (&ids.quit_id, TrayAction::Quit),
        (&ids.current_id, TrayAction::ApplyCurrent),
        (&ids.undo_id, TrayAction::UndoLayout),
        (&ids.hide_id, TrayAction::HideGui),
        (&ids.refresh_id, TrayAction::RefreshWindows),
        (
            &ids.layout_items[0].0,
            TrayAction::ApplyLayout(ids.layout_items[0].1.clone()),
        ),
        (
            &ids.layout_items[1].0,
            TrayAction::ApplyLayout(ids.layout_items[1].1.clone()),
        ),
    ] {
        let mut menus = vec![
            MenuEvent {
                id: MenuId::new("retired-menu-item"),
            },
            MenuEvent { id: id.clone() },
        ]
        .into_iter();
        let mut icons =
            vec![click("owned-icon", MouseButton::Left, MouseButtonState::Up)].into_iter();
        assert_eq!(ids.poll_events(&mut menus, &mut icons), expected);
        assert_eq!(icons.len(), 1);
        assert_eq!(ids.poll_events(&mut menus, &mut icons), TrayAction::ShowGui);
        assert_eq!(ids.poll_events(&mut menus, &mut icons), TrayAction::None);
    }
}

#[test]
fn stale_events_are_drained_with_a_bound_and_never_starve_menu_commands() {
    let ids = ids();
    let mut icons = (0..100)
        .map(|_| click("retired-icon", MouseButton::Left, MouseButtonState::Up))
        .chain(std::iter::once(click(
            "owned-icon",
            MouseButton::Left,
            MouseButtonState::Up,
        )));
    assert_eq!(
        ids.poll_events(&mut std::iter::empty(), &mut icons),
        TrayAction::None
    );
    assert_eq!(
        ids.poll_events(&mut std::iter::empty(), &mut icons),
        TrayAction::ShowGui
    );
    let mut menus = (0..100)
        .map(|_| MenuEvent {
            id: MenuId::new("retired"),
        })
        .chain(std::iter::once(MenuEvent {
            id: ids.quit_id.clone(),
        }));
    assert_eq!(
        ids.poll_events(&mut menus, &mut std::iter::empty()),
        TrayAction::None
    );
    assert_eq!(
        ids.poll_events(&mut menus, &mut std::iter::empty()),
        TrayAction::Quit
    );
    let mut hover = std::iter::repeat_with(|| TrayIconEvent::Move {
        id: ids.tray_id.clone(),
        position: tray_icon::dpi::PhysicalPosition::new(0.0, 0.0),
        rect: Default::default(),
    });
    assert_eq!(
        ids.poll_events(&mut std::iter::empty(), &mut hover),
        TrayAction::None
    );
    assert_eq!(
        ids.poll_events(
            &mut std::iter::once(MenuEvent {
                id: ids.quit_id.clone()
            }),
            &mut hover
        ),
        TrayAction::Quit
    );
}

fn grid(name: &str, cols: u32) -> SavedGrid {
    SavedGrid {
        name: name.into(),
        cols,
        rows: 2,
        col_weights: vec![0.2, 0.8],
        row_weights: vec![0.4, 0.6],
        disabled_cells: vec![1, 3],
    }
}

#[test]
fn current_layout_uses_exact_saved_index_and_live_configuration() {
    let mut config = Config {
        layout: vec![
            LayoutDef {
                name: "invalid".into(),
                grid: Some("bad".into()),
                style: None,
                count: None,
            },
            LayoutDef {
                name: "Split".into(),
                grid: None,
                style: Some("columns".into()),
                count: Some(3),
            },
        ],
        saved_grid: vec![grid("Duplicate", 2), grid("Duplicate", 4)],
        ..Default::default()
    };
    let offset = builtin_presets().len() + 1;
    config.defaults.selected_preset = offset + 1;
    let request = layout_request(&config, &TrayAction::ApplyCurrent).unwrap();
    assert_eq!(request.preset, LayoutPreset::Grid { cols: 4, rows: 2 });
    assert_eq!(request.weights, Some((vec![0.2, 0.8], vec![0.4, 0.6])));
    assert_eq!(request.disabled, [1, 3].into_iter().collect());
    let explicit = TrayAction::ApplyLayout(LayoutChoice::Saved {
        index: 1,
        name: "Duplicate".into(),
    });
    assert_eq!(
        layout_request(&config, &explicit).unwrap().preset,
        LayoutPreset::Grid { cols: 4, rows: 2 }
    );
    assert!(layout_request(
        &config,
        &TrayAction::ApplyLayout(LayoutChoice::Saved {
            index: 1,
            name: "renamed".into()
        })
    )
    .is_none());
    config.defaults.selected_preset = offset - 1;
    assert_eq!(
        layout_request(&config, &TrayAction::ApplyCurrent)
            .unwrap()
            .preset,
        LayoutPreset::Columns(3)
    );
    config.defaults.selected_preset = usize::MAX;
    assert_eq!(
        layout_request(&config, &TrayAction::ApplyCurrent)
            .unwrap()
            .preset,
        LayoutPreset::Grid { cols: 2, rows: 2 }
    );
    config.defaults.selected_preset = 0;
    assert_eq!(
        layout_request(&config, &TrayAction::ApplyCurrent)
            .unwrap()
            .preset,
        builtin_presets()[0].1
    );
    config.defaults.use_custom = true;
    config.defaults.custom_cols = 0;
    config.defaults.custom_rows = 99;
    config.defaults.col_weights = vec![1.0];
    config.defaults.row_weights = vec![0.5, 0.5];
    config.defaults.disabled_cells = vec![2, 2, 4];
    let request = layout_request(&config, &TrayAction::ApplyCurrent).unwrap();
    assert_eq!(request.preset, LayoutPreset::Grid { cols: 1, rows: 8 });
    assert_eq!(request.weights, Some((vec![1.0], vec![0.5, 0.5])));
    assert_eq!(request.disabled, [2, 4].into_iter().collect());
}

#[test]
fn tray_tooltips_report_visibility_counts_and_preserve_utf16_boundaries() {
    let mut state = TrayState {
        layout: "Custom 4x4".into(),
        windows: 12,
        enabled_slots: 13,
        can_apply: true,
        can_undo: true,
        visible: true,
        status: "Ready".into(),
    };
    assert_eq!(
        state.summary(),
        "12 windows / 13 enabled slots | Custom 4x4"
    );
    assert!(state.tooltip().ends_with("Window open"));
    state.visible = false;
    assert!(state.tooltip().ends_with("Running in tray"));
    state.layout = "🔮日本語 & café ".repeat(40);
    let tip = state.tooltip();
    assert!(tip.encode_utf16().count() <= 127);
    assert!(tip.contains("日本語"));
    assert_eq!(bounded_text("a🔮b", 2), "a");
    assert_eq!(bounded_text("a🔮b", 3), "a🔮");
    assert_eq!(bounded_text("", 0), "");
}

#[test]
fn explicit_layouts_do_not_inherit_current_masks_and_deleted_grids_are_rejected() {
    let mut config = Config::default();
    config.defaults.disabled_cells = vec![0, 1];
    config.saved_grid = vec![grid("Work", 99)];
    let preset = TrayAction::ApplyLayout(LayoutChoice::Preset(LayoutPreset::LeftRight));
    let request = layout_request(&config, &preset).unwrap();
    assert_eq!(request.preset, LayoutPreset::LeftRight);
    assert!(request.disabled.is_empty());
    assert!(request.weights.is_none());
    let saved = TrayAction::ApplyLayout(LayoutChoice::Saved {
        index: 0,
        name: "Work".into(),
    });
    assert_eq!(
        layout_request(&config, &saved).unwrap().preset,
        LayoutPreset::Grid { cols: 8, rows: 2 }
    );
    config.saved_grid.clear();
    assert!(layout_request(&config, &saved).is_none());
    for action in [
        TrayAction::None,
        TrayAction::Quit,
        TrayAction::ShowGui,
        TrayAction::UndoLayout,
        TrayAction::HideGui,
        TrayAction::RefreshWindows,
    ] {
        assert!(layout_request(&config, &action).is_none());
    }
}

#[test]
#[ignore = "Creates and removes only test-owned notification icons; no desktop input"]
fn native_audit_creates_rebuilds_themes_and_rejects_retired_tray_resources() {
    let mut config = Config {
        layout: vec![
            LayoutDef {
                name: "valid".into(),
                grid: Some("3x2".into()),
                style: None,
                count: None,
            },
            LayoutDef {
                name: "invalid".into(),
                grid: Some("invalid".into()),
                style: None,
                count: None,
            },
        ],
        saved_grid: vec![grid("Owned audit", 3), grid("Owned audit", 4)],
        ..Default::default()
    };
    config.defaults.theme = usize::MAX;
    let (first, old_ids) = create_tray(&config).expect("test-owned legacy tray");
    assert_eq!(first._tray.id(), &old_ids.tray_id);
    assert_eq!(old_ids.layout_items.len(), builtin_presets().len() + 3);
    assert_eq!(
        old_ids.layout_items.last().unwrap().1,
        LayoutChoice::Saved {
            index: 1,
            name: "Owned audit".into()
        }
    );
    first.set_theme(crate::theme::from_legacy(0)).unwrap();
    drop(first);
    for code in [
        "invalid theme code".to_owned(),
        crate::theme::from_legacy(1).encode(),
    ] {
        config.defaults.theme_code = Some(code);
        let (current, current_ids) = create_tray(&config).expect("rebuilt test-owned tray");
        assert_ne!(old_ids.tray_id, current_ids.tray_id);
        assert_eq!(current_ids.poll(), TrayAction::None);
        assert_eq!(
            poll(
                &current_ids,
                vec![MenuEvent {
                    id: old_ids.quit_id.clone()
                }],
                vec![click(
                    &old_ids.tray_id.0,
                    MouseButton::Left,
                    MouseButtonState::Up
                )]
            ),
            TrayAction::None
        );
        current.set_theme(crate::theme::from_legacy(2)).unwrap();
    }
    let api = Rc::new(FaultApi::default());
    let (owned, _) = create_tray_with_api(&config, api.clone())
        .expect("test-owned tray for theme failure audit");
    let mut state = TrayState {
        layout: "Build & tools".into(),
        windows: 2,
        enabled_slots: 4,
        can_apply: true,
        can_undo: true,
        visible: true,
        status: "Arranged 2 windows.".into(),
    };
    owned.update_state(state.clone()).unwrap();
    assert_eq!(
        owned.summary.text(),
        "2 windows / 4 enabled slots | Build && tools"
    );
    assert!(owned.current.is_enabled());
    assert!(owned.undo.is_enabled());
    assert!(owned.hide.is_enabled());
    assert!(owned.layouts.is_enabled());
    assert_eq!(owned.last_action.text(), "Arranged 2 windows.");
    let tips = api.tip_calls.get();
    owned.update_state(state.clone()).unwrap();
    assert_eq!(
        api.tip_calls.get(),
        tips,
        "unchanged frames must not modify the notification icon"
    );
    state.visible = false;
    state.can_apply = false;
    state.can_undo = false;
    state.windows = 0;
    owned.update_state(state.clone()).unwrap();
    assert!(!owned.current.is_enabled());
    assert!(!owned.undo.is_enabled());
    assert!(!owned.hide.is_enabled());
    assert!(!owned.layouts.is_enabled());
    api.fault.set(Fault::SetTooltip);
    state.windows = 5;
    assert_eq!(
        owned.update_state(state).unwrap_err(),
        "owned tooltip update failure"
    );
    assert_eq!(owned.last_state.borrow().as_ref().unwrap().windows, 0);
    api.fault.set(Fault::SetIcon);
    assert_eq!(
        owned.set_theme(ThemeSettings::default()).unwrap_err(),
        "owned icon update failure"
    );
    let set_calls = api.set_calls.get();
    api.fault.set(Fault::Icon);
    assert!(owned
        .set_theme(ThemeSettings::default())
        .unwrap_err()
        .contains("don't match the number of pixels"));
    assert_eq!(
        api.set_calls.get(),
        set_calls,
        "invalid icon must not reach notification update"
    );
    api.fault.set(Fault::None);
    owned.set_theme(ThemeSettings::default()).unwrap();
    api.fault.set(Fault::SetIcon);
    crate::app::tests::audit_theme_update_failure_is_visible(owned);
    api.fault.set(Fault::None);
    let (owned, _) = create_tray_with_api(&config, api.clone()).unwrap();
    api.fault.set(Fault::SetTooltip);
    crate::app::tests::audit_tray_status_failure_is_visible(owned, &|| api.fault.set(Fault::None));
    println!("NATIVE TRAY PASS: owned icons created, rebuilt, themed and removed; retired events rejected; theme failures propagated; no desktop input");
}

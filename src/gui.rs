use crate::app::{DividerAxis, PsmApp};
use crate::config;
use crate::monitor::Rect;
use crate::theme::{self, Theme};
use crate::windows;
use egui::{RichText, Stroke};
mod preview;
use preview::{draw_interactive_preview, PreviewAction};

fn section(ui: &mut egui::Ui, label: &str, theme: &Theme) {
    ui.add_space(4.0);
    ui.label(RichText::new(label).strong().color(theme.accent2));
}

pub fn draw(ctx: &egui::Context, app: &mut PsmApp) {
    app.install_theme(ctx);
    theme::paint_background(ctx, app.theme_settings);
    let t = app.current_theme();
    let tokens = theme::tokens(app.theme_settings);
    egui::TopBottomPanel::top("brand")
        .frame(
            egui::Frame::NONE
                .fill(theme::panel_color(app.theme_settings))
                .inner_margin(16),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(32.0, 32.0), egui::Sense::hover());
                for row in 0..2 {
                    for col in 0..2 {
                        let cell = egui::Rect::from_min_size(
                            rect.min + egui::vec2(col as f32 * 17.0, row as f32 * 17.0),
                            egui::vec2(13.0, 13.0),
                        );
                        ui.painter().rect_filled(
                            cell,
                            3.0,
                            if row == col {
                                tokens.accent_dim
                            } else {
                                tokens.panel_raised
                            },
                        );
                        ui.painter().rect_stroke(
                            cell,
                            3.0,
                            Stroke::new(1.5, if row == col { t.accent } else { t.accent2 }),
                            egui::StrokeKind::Inside,
                        );
                    }
                }
                ui.vertical(|ui| {
                    ui.label(RichText::new("POWERSHELL MANAGER").size(20.0).strong());
                    ui.label(RichText::new("Your windows. Your grid.").color(t.text_muted));
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Theme Studio").clicked() {
                        app.show_theme_studio = !app.show_theme_studio;
                    }
                });
            });
        });
    egui::TopBottomPanel::bottom("apply-bar")
        .frame(egui::Frame::NONE.fill(tokens.panel).inner_margin(12))
        .show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                let enabled = app
                    .active_preset()
                    .slot_count()
                    .saturating_sub(app.disabled_cells.len());
                if ui
                    .add_enabled(
                        app.native_enabled && enabled > 0 && !app.managed_windows.is_empty(),
                        egui::Button::new(RichText::new("Apply layout").strong())
                            .fill(tokens.accent_dim)
                            .min_size(egui::vec2(140.0, 36.0)),
                    )
                    .clicked()
                {
                    app.apply_current_layout();
                }
                if ui
                    .add_enabled(app.native_enabled, egui::Button::new("Refresh"))
                    .clicked()
                {
                    app.refresh_windows();
                }
                ui.label(format!(
                    "{} windows / {} enabled slots",
                    app.managed_windows.len(),
                    enabled
                ));
            });
            ui.label(RichText::new(&app.status).small().color(t.text_muted));
        });
    let wide = ctx.screen_rect().width() >= 840.0;
    if wide {
        egui::SidePanel::left("layout-rail")
            .exact_width(246.0)
            .resizable(false)
            .frame(
                egui::Frame::NONE
                    .fill(theme::panel_color(app.theme_settings))
                    .inner_margin(14),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("layout-options")
                    .show(ui, |ui| {
                        controls(ui, app, &t);
                    });
            });
    }
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(theme::panel_color(app.theme_settings)).inner_margin(18)).show(ctx, |ui| {
        egui::ScrollArea::vertical().id_salt("workspace-scroll").show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Layout workspace").size(26.0).strong());
                ui.colored_label(t.accent2, if app.use_custom { "CUSTOM" } else { "PRESET" });
            });
            ui.label(RichText::new(if app.use_custom { "Drag dividers to resize. Click a slot to enable or disable it." } else { "Click a slot to enable or disable it. Switch to Custom to drag dividers." }).color(t.text_muted));
            ui.add_space(5.0);
            match draw_interactive_preview(ui, ctx, app, &t) {
                PreviewAction::ToggleCell(index) => app.toggle_cell(index),
                PreviewAction::WeightsChanged => app.save_layout(),
                PreviewAction::None => {}
            }
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(t.accent2, "Occupied");
                ui.colored_label(t.accent, "Available");
                ui.colored_label(t.text_muted, "Disabled");
                ui.label(RichText::new("Slot preview; windows move only when you apply.").small());
            });
            if !wide {
                egui::CollapsingHeader::new("Layout controls").default_open(true).show(ui, |ui| { controls(ui, app, &t); });
            }
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut app.detail_tab, 0, format!("Windows ({})", app.managed_windows.len()));
                ui.selectable_value(&mut app.detail_tab, 1, format!("Pins ({})", app.config.pin.len()));
                ui.selectable_value(&mut app.detail_tab, 2, "Activity");
                ui.selectable_value(&mut app.detail_tab, 3, "About");
            });
            ui.separator();
            match app.detail_tab {
                1 => pins(ui, app, &t),
                2 => activity(ui, app, &t),
                3 => about(ui, app),
                _ => inventory(ui, app, &t)
            }
        });
    });
    crate::theme_studio::show(ctx, app);
}

fn controls(ui: &mut egui::Ui, app: &mut PsmApp, t: &Theme) {
    section(ui, "ARRANGE", t);
    let old_target = app.config.defaults.target.clone();
    ui.label(
        RichText::new(windows::TargetFilter::from_str(&old_target).display_name())
            .small()
            .color(t.text_muted),
    );
    ui.horizontal_wrapped(|ui| {
        ui.selectable_value(&mut app.config.defaults.target, "all".into(), "All windows");
        ui.selectable_value(
            &mut app.config.defaults.target,
            "terminals".into(),
            "Terminals",
        );
    });
    if old_target != app.config.defaults.target {
        app.save_config();
        app.refresh_windows();
    }
    if ui
        .checkbox(&mut app.config.defaults.smart_sort, "Rank by activity")
        .on_hover_text("Frequently used and recently focused apps get earlier slots.")
        .changed()
    {
        app.save_config();
    }
    section(ui, "DISPLAY", t);
    let old_monitor = app.config.defaults.monitor.clone();
    egui::ComboBox::from_id_salt("monitor")
        .selected_text(if old_monitor == "primary" {
            "Primary display".into()
        } else {
            format!("Display {}", old_monitor)
        })
        .width(ui.available_width() - 8.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut app.config.defaults.monitor,
                "primary".into(),
                "Primary display",
            );
            for monitor in &app.monitors {
                ui.selectable_value(
                    &mut app.config.defaults.monitor,
                    monitor.index.to_string(),
                    format!(
                        "Display {}: {} x {}{}",
                        monitor.index,
                        monitor.work_area.w,
                        monitor.work_area.h,
                        if monitor.is_primary { " (primary)" } else { "" }
                    ),
                );
            }
        });
    if ui
        .add(
            egui::Slider::new(&mut app.config.defaults.gap, 0..=32)
                .text("Gap")
                .suffix(" px"),
        )
        .changed()
        || old_monitor != app.config.defaults.monitor
    {
        app.save_config();
    }
    section(ui, "LAYOUT", t);
    let old_custom = app.use_custom;
    ui.horizontal(|ui| {
        ui.selectable_value(&mut app.use_custom, false, "Presets");
        ui.selectable_value(&mut app.use_custom, true, "Custom");
    });
    if old_custom != app.use_custom {
        app.disabled_cells.clear();
        app.save_layout();
    }
    if app.use_custom {
        ui.horizontal(|ui| {
            ui.label("Cols");
            let a = ui.add(egui::DragValue::new(&mut app.custom_cols).range(1..=8));
            ui.label("Rows");
            let b = ui.add(egui::DragValue::new(&mut app.custom_rows).range(1..=8));
            if a.changed() || b.changed() {
                app.ensure_weights();
                app.disabled_cells.clear();
                app.dragging_divider = None;
                app.save_layout();
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui.small_button("Equalize").clicked() {
                app.col_weights.clear();
                app.row_weights.clear();
                app.ensure_weights();
                app.save_layout();
            }
            if ui.small_button("Enable all").clicked() {
                app.disabled_cells.clear();
                app.save_layout();
            }
        });
        ui.add_space(4.0);
        if ui.button("Save this grid").clicked() {
            app.show_save_dialog = !app.show_save_dialog;
        }
        if app.show_save_dialog {
            ui.add(
                egui::TextEdit::singleline(&mut app.save_grid_name)
                    .hint_text("Name this layout")
                    .desired_width(ui.available_width()),
            );
            if ui
                .add_enabled(
                    !app.save_grid_name.trim().is_empty(),
                    egui::Button::new("Save layout"),
                )
                .clicked()
            {
                app.save_current_as_grid(app.save_grid_name.trim().to_string());
                app.show_save_dialog = false;
            }
        }
    } else {
        let selected = app
            .presets
            .get(app.selected_preset)
            .map(|p| p.0.as_str())
            .unwrap_or("2x2");
        let old = app.selected_preset;
        egui::ComboBox::from_id_salt("all-presets")
            .selected_text(selected)
            .width(ui.available_width() - 8.0)
            .show_ui(ui, |ui| {
                for (i, (name, _)) in app.presets.iter().enumerate() {
                    ui.selectable_value(&mut app.selected_preset, i, name);
                }
            });
        if old != app.selected_preset {
            select_preset(app, app.selected_preset);
        }
        ui.add_space(2.0);
        let choices = [
            "2x2",
            "3x2",
            "left-right",
            "main-side",
            "focus:3",
            "columns:3",
        ];
        for pair in choices.chunks(2) {
            ui.horizontal(|ui| {
                for name in pair {
                    if let Some(preset) = crate::layout::LayoutPreset::parse(name) {
                        let selected = preset == app.active_preset();
                        if preset_button(ui, &preset, name, selected, t) {
                            if let Some(i) = app.presets.iter().position(|(_, p)| *p == preset) {
                                select_preset(app, i);
                            }
                        }
                    }
                }
            });
        }
    }
    if !app.config.saved_grid.is_empty() {
        section(ui, "SAVED GRIDS", t);
        for grid in app.config.saved_grid.clone() {
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(&grid.name).wrap()).clicked() {
                    app.load_saved_grid(&grid);
                }
                if ui
                    .small_button("x")
                    .on_hover_text("Delete saved grid")
                    .clicked()
                {
                    app.delete_saved_grid(&grid.name);
                }
            });
        }
    }
}

fn select_preset(app: &mut PsmApp, index: usize) {
    app.selected_preset = index;
    let name = app.presets[index].0.clone();
    if let Some(grid) = app
        .config
        .saved_grid
        .iter()
        .find(|g| g.name == name)
        .cloned()
    {
        app.load_saved_grid(&grid);
    } else {
        app.disabled_cells.clear();
        app.save_layout();
    }
}

fn preset_button(
    ui: &mut egui::Ui,
    preset: &crate::layout::LayoutPreset,
    name: &str,
    selected: bool,
    t: &Theme,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(98.0, 69.0), egui::Sense::click());
    ui.painter().rect_filled(
        rect,
        6.0,
        if selected || response.hovered() {
            t.cell_hover
        } else {
            t.surface
        },
    );
    ui.painter().rect_stroke(
        rect,
        6.0,
        Stroke::new(1.0, if selected { t.accent } else { t.border }),
        egui::StrokeKind::Inside,
    );
    let area = Rect {
        x: 0,
        y: 0,
        w: 100,
        h: 52,
    };
    for slot in preset.compute_slots(&area, 4) {
        let r = egui::Rect::from_min_size(
            rect.min + egui::vec2(8.0 + slot.x as f32 * 0.82, 7.0 + slot.y as f32 * 0.67),
            egui::vec2(slot.w as f32 * 0.82, slot.h as f32 * 0.67),
        );
        ui.painter().rect_filled(r, 2.0, t.cell_occupied);
        ui.painter().rect_stroke(
            r,
            2.0,
            Stroke::new(1.0, t.accent2),
            egui::StrokeKind::Inside,
        );
    }
    ui.painter().text(
        rect.center_bottom() - egui::vec2(0.0, 11.0),
        egui::Align2::CENTER_CENTER,
        name,
        egui::FontId::proportional(12.0),
        t.text,
    );
    response.on_hover_text(preset.display_name()).clicked()
}

fn inventory(ui: &mut egui::Ui, app: &mut PsmApp, t: &Theme) {
    ui.horizontal_wrapped(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.window_query)
                .hint_text("Search windows or apps")
                .desired_width(230.0),
        );
        if ui
            .add_enabled(app.native_enabled, egui::Button::new("Minimize all"))
            .clicked()
        {
            for win in &app.managed_windows {
                windows::minimize_window(win.hwnd);
            }
        }
        if ui
            .add_enabled(app.native_enabled, egui::Button::new("Restore all"))
            .clicked()
        {
            for win in &app.managed_windows {
                windows::restore_window(win.hwnd);
            }
            windows::focus_window(app.app_hwnd);
        }
    });
    let query = app.window_query.to_lowercase();
    let wins = app.managed_windows.clone();
    let mut shown = 0;
    for (i, win) in wins.iter().enumerate() {
        if !query.is_empty()
            && !win.title.to_lowercase().contains(&query)
            && !win.process_name.to_lowercase().contains(&query)
        {
            continue;
        }
        shown += 1;
        ui.push_id(i, |ui| {
            let tokens = theme::tokens(app.theme_settings);
            let fill = if i % 2 == 0 {
                ui.visuals().faint_bg_color
            } else {
                theme::panel_color(app.theme_settings)
            };
            egui::Frame::NONE
                .fill(fill)
                .inner_margin(8)
                .corner_radius(5)
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.colored_label(t.accent2, win.category.short_label());
                        ui.add(
                            egui::Label::new(RichText::new(&win.process_name).strong()).truncate(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let pinned = app
                                .config
                                .pin
                                .iter()
                                .any(|p| p.matches(&win.process_name, &win.title));
                            if ui
                                .small_button(if pinned { "Unpin" } else { "Pin" })
                                .clicked()
                            {
                                if pinned {
                                    app.config
                                        .pin
                                        .retain(|p| !p.matches(&win.process_name, &win.title));
                                } else {
                                    let next = (0..app
                                        .active_preset()
                                        .slot_count()
                                        .saturating_sub(app.disabled_cells.len()))
                                        .find(|i| !app.config.pin.iter().any(|p| p.slot == *i))
                                        .unwrap_or(0);
                                    app.config.pin.push(config::PinRule {
                                        process: Some(win.process_name.clone()),
                                        title_contains: None,
                                        slot: next,
                                    });
                                }
                                app.save_config();
                            }
                            if ui
                                .add_enabled(app.native_enabled, egui::Button::new("Focus").small())
                                .clicked()
                            {
                                windows::focus_window(win.hwnd);
                            }
                            if win.is_minimized {
                                ui.colored_label(tokens.text_muted, "Minimized");
                            }
                        });
                    });
                    ui.add(
                        egui::Label::new(RichText::new(&win.title).color(t.text_muted)).truncate(),
                    )
                    .on_hover_text(format!("{}\n{} x {}", win.title, win.rect.w, win.rect.h));
                });
        });
    }
    if shown == 0 {
        ui.label(if query.is_empty() {
            "No matching windows found. Try All windows or Refresh."
        } else {
            "No windows match your search."
        });
    }
}

fn pins(ui: &mut egui::Ui, app: &mut PsmApp, t: &Theme) {
    ui.label("Pin an app from the Windows tab, then choose its slot here.");
    let enabled_slots: Vec<_> = (0..app.active_preset().slot_count())
        .filter(|i| !app.disabled_cells.contains(i))
        .collect();
    let count = enabled_slots.len();
    ui.label("Pin rules use activity ranking. Slot order counts enabled cells.");
    if ui
        .checkbox(
            &mut app.config.defaults.smart_sort,
            "Rank by activity and apply pins",
        )
        .changed()
    {
        app.save_config();
    }
    let mut remove = None;
    let mut changed = false;
    for (i, rule) in app.config.pin.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    rule.process
                        .as_deref()
                        .or(rule.title_contains.as_deref())
                        .unwrap_or("Unnamed rule"),
                );
                let mut slot = rule.slot + 1;
                ui.label("Enabled slot");
                if ui
                    .add(egui::DragValue::new(&mut slot).range(1..=count.max(1)))
                    .changed()
                {
                    rule.slot = slot - 1;
                    changed = true;
                }
                if rule.slot >= count {
                    ui.colored_label(t.accent2, "Slot unavailable in this layout");
                }
                if ui.small_button("Remove").clicked() {
                    remove = Some(i);
                }
            });
        });
    }
    if let Some(i) = remove {
        app.config.pin.remove(i);
        changed = true;
    }
    if changed {
        app.save_config();
    }
}

fn activity(ui: &mut egui::Ui, app: &PsmApp, t: &Theme) {
    ui.label("Activity ranking uses focus time, app switches and recency. Stored locally.");
    let top = app.activity.top_apps(10);
    if top.is_empty() {
        ui.colored_label(t.text_muted, "Activity appears as you use your apps.");
    }
    for (name, score) in top {
        ui.horizontal(|ui| {
            ui.label(name);
            ui.monospace(format!("{score:.0} points"));
        });
    }
    for (name, act) in app.activity.session_stats().iter().take(5) {
        ui.label(format!(
            "{}: {:.0} min focused / {} switches / {}",
            name,
            act.focus_secs / 60.0,
            act.switch_count,
            act.category.short_label()
        ));
    }
}

fn about(ui: &mut egui::Ui, app: &PsmApp) {
    ui.heading(format!("PowerShellManager {}", env!("CARGO_PKG_VERSION")));
    ui.label("Built by Trent Sterling / tront.xyz");
    ui.label("Weighted grids, saved layouts and a workspace that looks like you.");
    ui.hyperlink_to(
        "Source & releases",
        "https://github.com/TrentSterling/powershellmanager",
    );
    if let Ok(update) = app.update_info.lock() {
        if let Some(info) = update.as_ref() {
            ui.hyperlink_to(
                format!("Download {}", info.latest_version),
                &info.download_url,
            );
        }
    }
    egui::CollapsingHeader::new("Font license").show(ui, |ui| {
        ui.label(theme::typography::LICENSE);
    });
    egui::CollapsingHeader::new("Theme engine credits").show(ui, |ui| {
        ui.label(include_str!("../assets/theme-engine-NOTICE"));
    });
}

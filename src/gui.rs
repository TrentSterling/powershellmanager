use crate::app::PsmApp;
use crate::monitor::Rect;
use crate::theme::{Theme, THEMES};
use std::collections::HashSet;

static ICON_PNG: &[u8] = include_bytes!("../assets/tront-icon.png");

fn load_icon_texture(ctx: &egui::Context) -> egui::TextureHandle {
    let img = image::load_from_memory(ICON_PNG).expect("embedded PNG is valid");
    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let pixels = rgba.into_raw();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
    ctx.load_texture("tront-icon", color_image, egui::TextureOptions::LINEAR)
}

pub fn draw(ctx: &egui::Context, app: &mut PsmApp) {
    let theme = app.current_theme().clone();

    // Load icon texture once, cache in app
    if app.icon_texture.is_none() {
        app.icon_texture = Some(load_icon_texture(ctx));
    }

    egui::CentralPanel::default().show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("PowerShell Manager");
            ui.separator();

            // Detected windows
            ui.label(format!(
                "Detected terminal windows: {}",
                app.terminal_windows.len()
            ));

            egui::ScrollArea::vertical()
                .id_salt("windows_list")
                .max_height(120.0)
                .show(ui, |ui| {
                    for win in &app.terminal_windows {
                        ui.horizontal(|ui| {
                            ui.monospace(&win.process_name);
                            ui.label("\u{2014}");
                            let title = if win.title.len() > 50 {
                                format!("{}...", &win.title[..47])
                            } else {
                                win.title.clone()
                            };
                            ui.label(title);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(format!("{}x{}", win.rect.w, win.rect.h));
                                },
                            );
                        });
                    }
                    if app.terminal_windows.is_empty() {
                        ui.colored_label(theme.text_muted, "No terminal windows found.");
                    }
                });

            ui.separator();

            // Layout mode toggle
            ui.horizontal(|ui| {
                ui.selectable_value(&mut app.use_custom, false, "Preset");
                ui.selectable_value(&mut app.use_custom, true, "Custom Grid");
            });

            ui.add_space(4.0);

            if app.use_custom {
                ui.horizontal(|ui| {
                    ui.label("Columns:");
                    let old_cols = app.custom_cols;
                    ui.add(egui::DragValue::new(&mut app.custom_cols).range(1..=8));
                    ui.label("Rows:");
                    let old_rows = app.custom_rows;
                    ui.add(egui::DragValue::new(&mut app.custom_rows).range(1..=8));

                    if app.custom_cols != old_cols || app.custom_rows != old_rows {
                        app.disabled_cells.clear();
                    }
                });
            } else {
                ui.horizontal(|ui| {
                    ui.label("Layout:");
                    let current_name = app
                        .presets
                        .get(app.selected_preset)
                        .map(|(n, _)| n.as_str())
                        .unwrap_or("None");

                    let old_preset = app.selected_preset;
                    egui::ComboBox::from_id_salt("layout_picker")
                        .selected_text(current_name)
                        .show_ui(ui, |ui| {
                            for (i, (name, _)) in app.presets.iter().enumerate() {
                                ui.selectable_value(&mut app.selected_preset, i, name);
                            }
                        });

                    if app.selected_preset != old_preset {
                        app.disabled_cells.clear();
                    }
                });
            }

            ui.horizontal(|ui| {
                if ui.button("Apply").clicked() {
                    app.apply_current_layout();
                }
                if ui.button("Refresh").clicked() {
                    app.refresh_windows();
                }
                let enabled_count =
                    app.active_preset().slot_count() - app.disabled_cells.len();
                ui.colored_label(
                    theme.text_muted,
                    format!(
                        "{} enabled / {} total slots",
                        enabled_count,
                        app.active_preset().slot_count()
                    ),
                );
            });

            ui.separator();

            // Interactive layout preview
            let preset = app.active_preset();
            ui.label("Preview (click cells to toggle):");
            let toggle = draw_interactive_preview(
                ui,
                &preset,
                app.terminal_windows.len(),
                &app.disabled_cells,
                &theme,
            );
            if let Some(cell_idx) = toggle {
                app.toggle_cell(cell_idx);
            }

            ui.separator();

            // Settings
            ui.collapsing("Settings", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Theme:");
                    let current_theme_name = THEMES[app.theme_index].name;
                    egui::ComboBox::from_id_salt("theme_picker")
                        .selected_text(current_theme_name)
                        .show_ui(ui, |ui| {
                            for (i, t) in THEMES.iter().enumerate() {
                                if ui
                                    .selectable_value(&mut app.theme_index, i, t.name)
                                    .changed()
                                {
                                    app.set_theme(i);
                                }
                            }
                        });
                });
                ui.horizontal(|ui| {
                    ui.label("Target:");
                    ui.colored_label(theme.text_muted, &app.config.defaults.target);
                });
                ui.horizontal(|ui| {
                    ui.label("Monitor:");
                    ui.colored_label(theme.text_muted, &app.config.defaults.monitor);
                });
                ui.horizontal(|ui| {
                    ui.label("Gap:");
                    ui.colored_label(
                        theme.text_muted,
                        format!("{}px", app.config.defaults.gap),
                    );
                });
            });

            // About
            ui.collapsing("About", |ui| {
                ui.horizontal(|ui| {
                    if let Some(tex) = &app.icon_texture {
                        ui.image(egui::load::SizedTexture::new(
                            tex.id(),
                            egui::vec2(64.0, 64.0),
                        ));
                    }
                    ui.vertical(|ui| {
                        ui.label(format!(
                            "PowerShell Manager v{}",
                            env!("CARGO_PKG_VERSION")
                        ));
                        ui.colored_label(theme.text_muted, "by Trent Sterling (Tront)");
                        ui.add_space(2.0);
                        ui.hyperlink_to("tront.xyz", "https://tront.xyz");
                    });
                });
                ui.add_space(4.0);
                ui.colored_label(
                    theme.text_muted,
                    "System tray tool for arranging terminal windows into grid layouts.",
                );
            });
        });
    });
}

/// Draws interactive preview. Returns Some(index) if a cell was clicked.
fn draw_interactive_preview(
    ui: &mut egui::Ui,
    preset: &crate::layout::LayoutPreset,
    window_count: usize,
    disabled: &HashSet<usize>,
    theme: &Theme,
) -> Option<usize> {
    let preview_width = ui.available_width().min(400.0).max(200.0);
    let preview_height = preview_width * 9.0 / 16.0;
    let preview_size = egui::vec2(preview_width, preview_height);

    let (response, painter) = ui.allocate_painter(preview_size, egui::Sense::click());
    let rect = response.rect;

    // Background (monitor)
    painter.rect_filled(rect, 4.0, theme.surface);
    painter.rect_stroke(
        rect,
        4.0,
        egui::Stroke::new(1.0, theme.border),
        egui::StrokeKind::Outside,
    );

    // Compute slots in a virtual work area
    let area = Rect {
        x: 0,
        y: 0,
        w: 1920,
        h: 1080,
    };
    let slots = preset.compute_slots(&area, 12);

    let pad = 6.0;
    let scale_x = (preview_size.x - pad * 2.0) / area.w as f32;
    let scale_y = (preview_size.y - pad * 2.0) / area.h as f32;
    let offset = rect.min + egui::vec2(pad, pad);

    let mut clicked_cell = None;
    let click_pos = if response.clicked() {
        response.interact_pointer_pos()
    } else {
        None
    };
    let hover_pos = response.hover_pos();

    let mut enabled_idx = 0;

    for (i, slot) in slots.iter().enumerate() {
        let slot_rect = egui::Rect::from_min_size(
            offset + egui::vec2(slot.x as f32 * scale_x, slot.y as f32 * scale_y),
            egui::vec2(slot.w as f32 * scale_x, slot.h as f32 * scale_y),
        );

        let is_disabled = disabled.contains(&i);
        let is_hovered = hover_pos.map_or(false, |p| slot_rect.contains(p));

        if let Some(pos) = click_pos {
            if slot_rect.contains(pos) {
                clicked_cell = Some(i);
            }
        }

        let has_window = if is_disabled {
            false
        } else {
            let has = enabled_idx < window_count;
            enabled_idx += 1;
            has
        };

        let color = if is_disabled {
            theme.cell_disabled
        } else if is_hovered {
            theme.cell_hover
        } else if has_window {
            theme.cell_occupied
        } else {
            theme.cell_enabled
        };

        painter.rect_filled(slot_rect, 3.0, color);
        painter.rect_stroke(
            slot_rect,
            3.0,
            egui::Stroke::new(
                1.0,
                if is_disabled {
                    theme.border
                } else {
                    theme.accent.linear_multiply(0.5)
                },
            ),
            egui::StrokeKind::Outside,
        );

        if is_disabled {
            painter.text(
                slot_rect.center(),
                egui::Align2::CENTER_CENTER,
                "X",
                egui::FontId::proportional(16.0),
                theme.text_muted,
            );
        } else {
            let label = format!("{}", i + 1);
            painter.text(
                slot_rect.center(),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(14.0),
                theme.text,
            );
        }
    }

    clicked_cell
}

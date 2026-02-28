use crate::app::{DividerAxis, PsmApp};
use crate::config;
use crate::monitor::Rect;
use crate::theme::{Theme, THEMES};
use crate::windows;


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
            ui.horizontal(|ui| {
                ui.label(format!(
                    "Detected terminal windows: {}",
                    app.terminal_windows.len()
                ));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !app.terminal_windows.is_empty() {
                        if ui.small_button("Minimize All").clicked() {
                            for win in &app.terminal_windows {
                                windows::minimize_window(win.hwnd);
                            }
                        }
                        if ui.small_button("Restore All").clicked() {
                            for win in &app.terminal_windows {
                                windows::restore_window(win.hwnd);
                            }
                        }
                    }
                });
            });

            egui::ScrollArea::vertical()
                .id_salt("windows_list")
                .max_height(160.0)
                .show(ui, |ui| {
                    // Clone to avoid borrow issues with button clicks
                    let wins: Vec<_> = app.terminal_windows.iter().cloned().collect();
                    for win in &wins {
                        ui.horizontal(|ui| {
                            if ui.small_button("Focus").clicked() {
                                windows::focus_window(win.hwnd);
                            }
                            if win.is_minimized {
                                ui.colored_label(theme.text_muted, "[min]");
                            }
                            ui.monospace(&win.process_name);
                            ui.label("\u{2014}");
                            let title = if win.title.len() > 40 {
                                format!("{}...", &win.title[..37])
                            } else {
                                win.title.clone()
                            };
                            ui.label(title);
                        });
                    }
                    if app.terminal_windows.is_empty() {
                        ui.colored_label(theme.text_muted, "No terminal windows found.");
                    }
                });

            ui.separator();

            // Layout mode toggle
            let old_use_custom = app.use_custom;
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
                        app.col_weights = vec![1.0 / app.custom_cols as f32; app.custom_cols as usize];
                        app.row_weights = vec![1.0 / app.custom_rows as f32; app.custom_rows as usize];
                        app.config.defaults.custom_cols = app.custom_cols;
                        app.config.defaults.custom_rows = app.custom_rows;
                        app.config.defaults.col_weights = app.col_weights.clone();
                        app.config.defaults.row_weights = app.row_weights.clone();
                        config::save(&app.config);
                    }

                    if !app.weights_are_uniform() {
                        if ui.small_button("Reset Sizes").clicked() {
                            app.col_weights = vec![1.0 / app.custom_cols as f32; app.custom_cols as usize];
                            app.row_weights = vec![1.0 / app.custom_rows as f32; app.custom_rows as usize];
                            app.config.defaults.col_weights = app.col_weights.clone();
                            app.config.defaults.row_weights = app.row_weights.clone();
                            config::save(&app.config);
                        }
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
                        app.config.defaults.selected_preset = app.selected_preset;
                        config::save(&app.config);
                    }
                });
            }

            if app.use_custom != old_use_custom {
                app.config.defaults.use_custom = app.use_custom;
                config::save(&app.config);
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
            app.ensure_weights();
            ui.label("Preview (click cells to toggle):");
            let action = draw_interactive_preview(ui, app, &theme);
            match action {
                PreviewAction::ToggleCell(cell_idx) => {
                    app.toggle_cell(cell_idx);
                }
                PreviewAction::WeightsChanged => {
                    app.config.defaults.col_weights = app.col_weights.clone();
                    app.config.defaults.row_weights = app.row_weights.clone();
                    config::save(&app.config);
                }
                PreviewAction::None => {}
            }

            ui.separator();

            // Settings
            let settings_resp = egui::CollapsingHeader::new("Settings")
                .default_open(app.config.defaults.settings_open)
                .show(ui, |ui| {
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
            if settings_resp.header_response.clicked() {
                app.config.defaults.settings_open = !app.config.defaults.settings_open;
                config::save(&app.config);
            }

            // About
            let about_resp = egui::CollapsingHeader::new("About")
                .default_open(app.config.defaults.about_open)
                .show(ui, |ui| {
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

                    // Update banner
                    if let Ok(guard) = app.update_info.lock() {
                        if let Some(info) = guard.as_ref() {
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.colored_label(
                                    theme.accent,
                                    format!("Update available: v{}", info.latest_version),
                                );
                                ui.hyperlink_to("Download", &info.download_url);
                            });
                        }
                    }
                });
            if about_resp.header_response.clicked() {
                app.config.defaults.about_open = !app.config.defaults.about_open;
                config::save(&app.config);
            }
        });
    });
}

enum PreviewAction {
    None,
    ToggleCell(usize),
    WeightsChanged,
}

impl PreviewAction {
    fn is_none(&self) -> bool {
        matches!(self, PreviewAction::None)
    }
}

/// Draws interactive preview with optional draggable dividers (custom grid mode).
fn draw_interactive_preview(
    ui: &mut egui::Ui,
    app: &mut PsmApp,
    theme: &Theme,
) -> PreviewAction {
    let preset = app.active_preset();
    let window_count = app.terminal_windows.len();
    let show_dividers = app.use_custom && app.custom_cols > 0 && app.custom_rows > 0;
    let non_uniform = show_dividers && !app.weights_are_uniform();

    let preview_width = ui.available_width().min(400.0).max(200.0);
    let preview_height = preview_width * 9.0 / 16.0;
    let preview_size = egui::vec2(preview_width, preview_height);

    let (response, painter) = ui.allocate_painter(preview_size, egui::Sense::click_and_drag());
    let rect = response.rect;

    // Background (monitor)
    painter.rect_filled(rect, 4.0, theme.surface);
    painter.rect_stroke(
        rect,
        4.0,
        egui::Stroke::new(1.0, theme.border),
        egui::StrokeKind::Outside,
    );

    let pad = 6.0;
    let inner_w = preview_size.x - pad * 2.0;
    let inner_h = preview_size.y - pad * 2.0;
    let offset = rect.min + egui::vec2(pad, pad);

    // Compute slots using weights for custom grid, or preset for non-custom
    let area = Rect { x: 0, y: 0, w: 1920, h: 1080 };
    let gap_virtual = 12;

    let slots = if show_dividers {
        crate::layout::compute_weighted_grid(
            app.custom_cols, app.custom_rows, &area, gap_virtual,
            &app.col_weights, &app.row_weights,
        )
    } else {
        preset.compute_slots(&area, gap_virtual)
    };

    let scale_x = inner_w / area.w as f32;
    let scale_y = inner_h / area.h as f32;

    // Divider hit detection and rendering
    let divider_hit_px = 5.0;
    let hover_pos = response.hover_pos();
    let mut hovered_divider: Option<(DividerAxis, usize)> = None;

    // Compute divider positions in screen coords (only for custom grid)
    let mut col_divider_x = Vec::new();
    let mut row_divider_y = Vec::new();

    if show_dividers {
        let cols = app.custom_cols as usize;
        let rows = app.custom_rows as usize;
        let usable_w_virtual = area.w - gap_virtual * (cols as i32 - 1);
        let usable_h_virtual = area.h - gap_virtual * (rows as i32 - 1);

        // Column divider x positions (between columns)
        let mut cx = 0.0_f32;
        for c in 0..cols - 1 {
            let col_w = usable_w_virtual as f32 * app.col_weights[c];
            cx += col_w;
            let screen_x = offset.x + (cx + gap_virtual as f32 * c as f32 + gap_virtual as f32 * 0.5) * scale_x;
            col_divider_x.push(screen_x);
        }

        // Row divider y positions (between rows)
        let mut ry = 0.0_f32;
        for r in 0..rows - 1 {
            let row_h = usable_h_virtual as f32 * app.row_weights[r];
            ry += row_h;
            let screen_y = offset.y + (ry + gap_virtual as f32 * r as f32 + gap_virtual as f32 * 0.5) * scale_y;
            row_divider_y.push(screen_y);
        }

        // Check hover on dividers
        if let Some(hp) = hover_pos {
            for (i, &dx) in col_divider_x.iter().enumerate() {
                if (hp.x - dx).abs() < divider_hit_px {
                    hovered_divider = Some((DividerAxis::Col, i));
                    break;
                }
            }
            if hovered_divider.is_none() {
                for (i, &dy) in row_divider_y.iter().enumerate() {
                    if (hp.y - dy).abs() < divider_hit_px {
                        hovered_divider = Some((DividerAxis::Row, i));
                        break;
                    }
                }
            }
        }
    }

    // Drag interaction for dividers
    let mut action = PreviewAction::None;

    if show_dividers {
        if response.drag_started() {
            if let Some(div) = hovered_divider {
                app.dragging_divider = Some(div);
            }
        }

        if response.dragged() {
            if let Some((axis, idx)) = app.dragging_divider {
                let delta = response.drag_delta();
                let min_weight = 0.05;

                match axis {
                    DividerAxis::Col => {
                        let total_w = inner_w;
                        let weight_delta = delta.x / total_w;
                        let w0 = (app.col_weights[idx] + weight_delta).max(min_weight);
                        let w1 = (app.col_weights[idx + 1] - weight_delta).max(min_weight);
                        let sum = w0 + w1;
                        let old_sum = app.col_weights[idx] + app.col_weights[idx + 1];
                        app.col_weights[idx] = w0 / sum * old_sum;
                        app.col_weights[idx + 1] = w1 / sum * old_sum;
                    }
                    DividerAxis::Row => {
                        let total_h = inner_h;
                        let weight_delta = delta.y / total_h;
                        let w0 = (app.row_weights[idx] + weight_delta).max(min_weight);
                        let w1 = (app.row_weights[idx + 1] - weight_delta).max(min_weight);
                        let sum = w0 + w1;
                        let old_sum = app.row_weights[idx] + app.row_weights[idx + 1];
                        app.row_weights[idx] = w0 / sum * old_sum;
                        app.row_weights[idx + 1] = w1 / sum * old_sum;
                    }
                }
            }
        }

        if response.drag_stopped() {
            if app.dragging_divider.is_some() {
                app.dragging_divider = None;
                // Normalize weights
                let col_sum: f32 = app.col_weights.iter().sum();
                if col_sum > 0.0 {
                    for w in &mut app.col_weights { *w /= col_sum; }
                }
                let row_sum: f32 = app.row_weights.iter().sum();
                if row_sum > 0.0 {
                    for w in &mut app.row_weights { *w /= row_sum; }
                }
                action = PreviewAction::WeightsChanged;
            }
        }
    }

    // Set cursor based on hover/drag state
    if app.dragging_divider.is_some() || hovered_divider.is_some() {
        let axis = app.dragging_divider.map(|(a, _)| a)
            .or(hovered_divider.map(|(a, _)| a));
        if let Some(a) = axis {
            ui.ctx().set_cursor_icon(match a {
                DividerAxis::Col => egui::CursorIcon::ResizeHorizontal,
                DividerAxis::Row => egui::CursorIcon::ResizeVertical,
            });
        }
    }

    // Draw cells
    let mut enabled_idx = 0;
    let mut clicked_cell = None;
    let click_pos = if response.clicked() && app.dragging_divider.is_none() {
        response.interact_pointer_pos()
    } else {
        None
    };

    for (i, slot) in slots.iter().enumerate() {
        let slot_rect = egui::Rect::from_min_size(
            offset + egui::vec2(slot.x as f32 * scale_x, slot.y as f32 * scale_y),
            egui::vec2(slot.w as f32 * scale_x, slot.h as f32 * scale_y),
        );

        let is_disabled = app.disabled_cells.contains(&i);
        let is_hovered = hover_pos.map_or(false, |p| slot_rect.contains(p))
            && hovered_divider.is_none()
            && app.dragging_divider.is_none();

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
                if is_disabled { theme.border } else { theme.accent.linear_multiply(0.5) },
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
            if non_uniform && slot_rect.width() > 30.0 && slot_rect.height() > 30.0 {
                // Show cell number above center and percentage below
                let cols = app.custom_cols as usize;
                let row = i / cols;
                let col = i % cols;
                let w_pct = (app.col_weights[col] * 100.0).round() as u32;
                let h_pct = (app.row_weights[row] * 100.0).round() as u32;
                let pct_label = format!("{}%x{}%", w_pct, h_pct);

                painter.text(
                    slot_rect.center() - egui::vec2(0.0, 7.0),
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(14.0),
                    theme.text,
                );
                painter.text(
                    slot_rect.center() + egui::vec2(0.0, 7.0),
                    egui::Align2::CENTER_CENTER,
                    pct_label,
                    egui::FontId::proportional(9.0),
                    theme.text_muted,
                );
            } else {
                painter.text(
                    slot_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(14.0),
                    theme.text,
                );
            }
        }
    }

    // Draw divider lines on top
    if show_dividers {
        let top = offset.y;
        let bottom = offset.y + inner_h;
        let left = offset.x;
        let right = offset.x + inner_w;

        for (i, &dx) in col_divider_x.iter().enumerate() {
            let is_active = app.dragging_divider == Some((DividerAxis::Col, i))
                || hovered_divider == Some((DividerAxis::Col, i));
            let stroke_w = if is_active { 2.5 } else { 1.0 };
            let color = if is_active {
                theme.accent
            } else {
                theme.accent.linear_multiply(0.4)
            };
            painter.line_segment(
                [egui::pos2(dx, top), egui::pos2(dx, bottom)],
                egui::Stroke::new(stroke_w, color),
            );
        }

        for (i, &dy) in row_divider_y.iter().enumerate() {
            let is_active = app.dragging_divider == Some((DividerAxis::Row, i))
                || hovered_divider == Some((DividerAxis::Row, i));
            let stroke_w = if is_active { 2.5 } else { 1.0 };
            let color = if is_active {
                theme.accent
            } else {
                theme.accent.linear_multiply(0.4)
            };
            painter.line_segment(
                [egui::pos2(left, dy), egui::pos2(right, dy)],
                egui::Stroke::new(stroke_w, color),
            );
        }
    }

    if let Some(cell_idx) = clicked_cell {
        if action.is_none() {
            return PreviewAction::ToggleCell(cell_idx);
        }
    }
    action
}

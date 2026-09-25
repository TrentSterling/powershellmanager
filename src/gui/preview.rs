use super::*;
pub(super) enum PreviewAction {
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
pub(super) fn draw_interactive_preview(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    app: &mut PsmApp,
    theme: &Theme,
) -> PreviewAction {
    let preset = app.active_preset();
    let window_count = app.managed_windows.len();
    let show_dividers = app.use_custom && app.custom_cols > 0 && app.custom_rows > 0;
    let non_uniform = show_dividers && !app.weights_are_uniform();

    let preview_width = ui.available_width().max(60.0);
    let panel_height = ctx.screen_rect().height();
    let max_preview_h = (panel_height * 0.4).max(40.0);
    let preview_height = (preview_width * 9.0 / 16.0).min(max_preview_h);
    let preview_size = egui::vec2(preview_width, preview_height);

    let (response, painter) = ui.allocate_painter(preview_size, egui::Sense::click_and_drag());
    let rect = response.rect;
    #[cfg(test)]
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("test-preview-rect"), rect));

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
    let area = Rect {
        x: 0,
        y: 0,
        w: 1920,
        h: 1080,
    };
    let gap_virtual = app.config.defaults.gap.clamp(0, 64);

    let slots = if show_dividers {
        crate::layout::compute_weighted_grid(
            app.custom_cols,
            app.custom_rows,
            &area,
            gap_virtual,
            &app.col_weights,
            &app.row_weights,
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
            let screen_x = offset.x
                + (cx + gap_virtual as f32 * c as f32 + gap_virtual as f32 * 0.5) * scale_x;
            col_divider_x.push(screen_x);
        }

        // Row divider y positions (between rows)
        let mut ry = 0.0_f32;
        for r in 0..rows - 1 {
            let row_h = usable_h_virtual as f32 * app.row_weights[r];
            ry += row_h;
            let screen_y = offset.y
                + (ry + gap_virtual as f32 * r as f32 + gap_virtual as f32 * 0.5) * scale_y;
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
            // Hit-test where the press began. A fast first movement may already
            // be outside the five-pixel handle by the time egui starts dragging.
            if let Some(origin) = ctx.input(|i| i.pointer.press_origin()) {
                if rect.contains(origin) {
                    app.dragging_divider = col_divider_x
                        .iter()
                        .position(|x| (origin.x - x).abs() <= divider_hit_px)
                        .map(|i| (DividerAxis::Col, i))
                        .or_else(|| {
                            row_divider_y
                                .iter()
                                .position(|y| (origin.y - y).abs() <= divider_hit_px)
                                .map(|i| (DividerAxis::Row, i))
                        });
                }
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

        if response.drag_stopped() && app.dragging_divider.is_some() {
            app.dragging_divider = None;
            // Normalize weights
            let col_sum: f32 = app.col_weights.iter().sum();
            if col_sum > 0.0 {
                for w in &mut app.col_weights {
                    *w /= col_sum;
                }
            }
            let row_sum: f32 = app.row_weights.iter().sum();
            if row_sum > 0.0 {
                for w in &mut app.row_weights {
                    *w /= row_sum;
                }
            }
            action = PreviewAction::WeightsChanged;
        }
    }

    // Set cursor based on hover/drag state
    if app.dragging_divider.is_some() || hovered_divider.is_some() {
        let axis = app
            .dragging_divider
            .map(|(a, _)| a)
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
    let click_pos =
        if response.clicked() && app.dragging_divider.is_none() && hovered_divider.is_none() {
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
        let is_hovered = hover_pos.is_some_and(|p| slot_rect.contains(p))
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
                if is_disabled {
                    theme.border
                } else {
                    theme.accent
                },
            ),
            egui::StrokeKind::Outside,
        );

        let cell_too_small = slot_rect.width() < 20.0 || slot_rect.height() < 20.0;
        if !cell_too_small {
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
                if non_uniform && slot_rect.width() > 140.0 && slot_rect.height() > 80.0 {
                    // Show cell number above center and percentage below
                    let cols = app.custom_cols as usize;
                    let row = i / cols;
                    let col = i % cols;
                    let w_pct = (app.col_weights[col] * 100.0).round() as u32;
                    let h_pct = (app.row_weights[row] * 100.0).round() as u32;
                    let pct_label = format!("{}% wide / {}% tall", w_pct, h_pct);

                    painter.text(
                        slot_rect.center() - egui::vec2(0.0, 12.0),
                        egui::Align2::CENTER_CENTER,
                        label,
                        egui::FontId::proportional(
                            if slot_rect.width() > 100.0 && slot_rect.height() > 80.0 {
                                32.0
                            } else {
                                14.0
                            },
                        ),
                        theme.text,
                    );
                    painter.text(
                        slot_rect.center() + egui::vec2(0.0, 15.0),
                        egui::Align2::CENTER_CENTER,
                        pct_label,
                        egui::FontId::monospace(11.0),
                        theme.text_muted,
                    );
                } else {
                    painter.text(
                        slot_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        label,
                        egui::FontId::proportional(
                            if slot_rect.width() > 100.0 && slot_rect.height() > 80.0 {
                                32.0
                            } else {
                                14.0
                            },
                        ),
                        theme.text,
                    );
                }
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

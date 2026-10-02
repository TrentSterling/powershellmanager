use super::*;
#[derive(Debug, PartialEq)]
pub(super) enum PreviewAction {
    None,
    ToggleCell(usize),
    WeightsChanged,
}

impl PreviewAction {
    fn with_clicked_cell(self, cell: Option<usize>) -> Self {
        if matches!(self, Self::None) {
            cell.map_or(self, Self::ToggleCell)
        } else {
            self
        }
    }
}

/// Draws interactive preview with optional draggable dividers (custom grid mode).
pub(super) fn draw_interactive_preview(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    app: &mut PsmApp,
    theme: &Theme,
    height_limit: f32,
) -> PreviewAction {
    let preset = app.active_preset();
    let show_dividers = app.use_custom;
    let non_uniform = show_dividers && !app.weights_are_uniform();

    let area = if app.monitors.is_empty() {
        Rect {
            x: 0,
            y: 0,
            w: 1920,
            h: 1080,
        }
    } else {
        let work_area =
            crate::monitor::resolve_monitor(&app.monitors, &app.config.defaults.monitor).work_area;
        Rect {
            x: 0,
            y: 0,
            w: work_area.w.max(1),
            h: work_area.h.max(1),
        }
    };
    let pad = 6.0;
    let preview_width = ui.available_width().max(60.0);
    let panel_height = ctx.screen_rect().height();
    let max_preview_h = (panel_height * 0.4).min(height_limit).max(24.0);
    let scale = ((preview_width - pad * 2.0) / area.w as f32)
        .min((max_preview_h - pad * 2.0) / area.h as f32);
    let preview_size =
        egui::vec2(area.w as f32 * scale, area.h as f32 * scale) + egui::vec2(pad * 2.0, pad * 2.0);
    let (region, _) = ui.allocate_exact_size(
        egui::vec2(preview_width, preview_size.y),
        egui::Sense::hover(),
    );
    let rect = egui::Rect::from_center_size(region.center(), preview_size);
    let response = ui.interact(
        rect,
        ui.id().with("layout-preview"),
        egui::Sense::click_and_drag(),
    );
    let painter = ui.painter_at(rect);
    #[cfg(test)]
    ctx.data_mut(|d| {
        d.insert_temp(egui::Id::new("test-preview-rect"), rect);
        d.insert_temp(egui::Id::new("test-preview-id"), response.id);
    });

    // Background (monitor)
    painter.rect_filled(rect, 4.0, theme.surface);
    painter.rect_stroke(
        rect,
        4.0,
        egui::Stroke::new(1.0_f32, theme.border),
        egui::StrokeKind::Outside,
    );

    let inner_w = preview_size.x - pad * 2.0;
    let inner_h = preview_size.y - pad * 2.0;
    let offset = rect.min + egui::vec2(pad, pad);

    // Compute slots using weights for custom grid, or preset for non-custom
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

    if slots.is_empty() {
        app.dragging_divider = None;
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "No space for this grid",
            egui::FontId::proportional(14.0),
            theme.text_muted,
        );
        return PreviewAction::None;
    }

    let scale_x = inner_w / area.w as f32;
    let scale_y = inner_h / area.h as f32;

    // Divider hit detection and rendering
    let divider_hit_px = 5.0;
    let hover_pos = response.hover_pos();

    // Compute divider positions in screen coords (only for custom grid)
    let mut col_divider_x = Vec::new();
    let mut row_divider_y = Vec::new();

    if show_dividers {
        let cols = app.custom_cols as usize;
        // Use the same rounded boundaries and clamped gaps as Apply.
        for pair in slots.iter().take(cols).collect::<Vec<_>>().windows(2) {
            col_divider_x
                .push(offset.x + (pair[0].x + pair[0].w + pair[1].x) as f32 * 0.5 * scale_x);
        }
        for pair in slots.iter().step_by(cols).collect::<Vec<_>>().windows(2) {
            row_divider_y
                .push(offset.y + (pair[0].y + pair[0].h + pair[1].y) as f32 * 0.5 * scale_y);
        }
    }
    let hovered_divider = divider_at(
        hover_pos,
        rect,
        &col_divider_x,
        &row_divider_y,
        divider_hit_px,
    );

    // Drag interaction for dividers
    let mut action = PreviewAction::None;

    if show_dividers {
        if response.drag_started_by(egui::PointerButton::Primary) {
            // Hit-test where the press began. A fast first movement may already
            // be outside the five-pixel handle by the time egui starts dragging.
            let origin = ctx.input(|i| i.pointer.press_origin());
            app.dragging_divider = origin
                .zip(divider_at(
                    origin,
                    rect,
                    &col_divider_x,
                    &row_divider_y,
                    divider_hit_px,
                ))
                .map(|(origin, (axis, index))| crate::app::DividerDrag {
                    axis,
                    index,
                    grab_offset: match axis {
                        DividerAxis::Col => origin.x - col_divider_x[index],
                        DividerAxis::Row => origin.y - row_divider_y[index],
                    },
                });
        }

        if response.dragged_by(egui::PointerButton::Primary) {
            if let (Some(drag), Some(pointer)) =
                (app.dragging_divider, response.interact_pointer_pos())
            {
                match drag.axis {
                    DividerAxis::Col => {
                        let gap = (slots[1].x - slots[0].x - slots[0].w) as f32;
                        let usable = area.w as f32 - gap * (app.custom_cols - 1) as f32;
                        let boundary = ((pointer.x - drag.grab_offset - offset.x) / scale_x
                            - gap * (drag.index as f32 + 0.5))
                            / usable;
                        resize_pair(&mut app.col_weights, drag.index, boundary);
                    }
                    DividerAxis::Row => {
                        let gap =
                            (slots[app.custom_cols as usize].y - slots[0].y - slots[0].h) as f32;
                        let usable = area.h as f32 - gap * (app.custom_rows - 1) as f32;
                        let boundary = ((pointer.y - drag.grab_offset - offset.y) / scale_y
                            - gap * (drag.index as f32 + 0.5))
                            / usable;
                        resize_pair(&mut app.row_weights, drag.index, boundary);
                    }
                }
            }
        }
    }

    // A release can arrive while hidden. Reconcile with the current button state
    // so a restored preview never retains a stale drag or loses changed widths.
    if app.dragging_divider.is_some()
        && (!show_dividers || !ctx.input(|i| i.pointer.button_down(egui::PointerButton::Primary)))
    {
        app.dragging_divider = None;
        app.col_weights = crate::app::normalized_weights(&app.col_weights, app.custom_cols);
        app.row_weights = crate::app::normalized_weights(&app.row_weights, app.custom_rows);
        action = PreviewAction::WeightsChanged;
    }

    // Set cursor based on hover/drag state
    if let Some(axis) = app
        .dragging_divider
        .map(|drag| drag.axis)
        .or(hovered_divider.map(|(axis, _)| axis))
    {
        ui.ctx().set_cursor_icon(match axis {
            DividerAxis::Col => egui::CursorIcon::ResizeHorizontal,
            DividerAxis::Row => egui::CursorIcon::ResizeVertical,
        });
    }

    // Draw cells
    let assignment = app.assignment();
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
        #[cfg(test)]
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(("test-preview-cell", i)), slot_rect));

        let is_disabled = app.disabled_cells.contains(&i);
        let is_hovered = hover_pos.is_some_and(|p| slot_rect.contains(p))
            && hovered_divider.is_none()
            && app.dragging_divider.is_none();

        if let Some(pos) = click_pos {
            if slot_rect.contains(pos) {
                clicked_cell = Some(i);
            }
        }

        let assigned = assignment.slots.get(i).copied().flatten();
        let manual_title = assigned.map(|w| app.managed_windows[w].title.as_str());
        let has_window = assigned.is_some();

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
                1.0_f32,
                if is_disabled {
                    theme.border
                } else {
                    theme.accent
                },
            ),
            egui::StrokeKind::Outside,
        );

        if let Some(title) = manual_title {
            if slot_rect.width() > 100.0 && slot_rect.height() > 105.0 {
                let mut job = egui::text::LayoutJob::simple_singleline(
                    title.into(),
                    egui::FontId::proportional(12.0),
                    theme.text,
                );
                job.wrap.max_width = slot_rect.width() - 16.0;
                job.wrap.max_rows = 1;
                let galley = ui.fonts(|f| f.layout_job(job));
                painter.galley(
                    egui::pos2(
                        slot_rect.center().x - galley.size().x / 2.0,
                        slot_rect.bottom() - 28.0,
                    ),
                    galley,
                    theme.text,
                );
            }
        }
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
                        egui::FontId::proportional(32.0),
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
            let is_active = app
                .dragging_divider
                .is_some_and(|drag| drag.axis == DividerAxis::Col && drag.index == i)
                || hovered_divider == Some((DividerAxis::Col, i));
            let stroke_w: f32 = if is_active { 2.5 } else { 1.0 };
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
            let is_active = app
                .dragging_divider
                .is_some_and(|drag| drag.axis == DividerAxis::Row && drag.index == i)
                || hovered_divider == Some((DividerAxis::Row, i));
            let stroke_w: f32 = if is_active { 2.5 } else { 1.0 };
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

    action.with_clicked_cell(clicked_cell)
}

/// Hover and press use identical inclusive handle boundaries and axis priority.
fn divider_at(
    pointer: Option<egui::Pos2>,
    rect: egui::Rect,
    columns: &[f32],
    rows: &[f32],
    hit: f32,
) -> Option<(DividerAxis, usize)> {
    let pointer = pointer.filter(|point| rect.contains(*point))?;
    columns
        .iter()
        .position(|x| (pointer.x - x).abs() <= hit)
        .map(|index| (DividerAxis::Col, index))
        .or_else(|| {
            rows.iter()
                .position(|y| (pointer.y - y).abs() <= hit)
                .map(|index| (DividerAxis::Row, index))
        })
}

fn resize_pair(weights: &mut [f32], index: usize, boundary: f32) {
    let prefix: f32 = weights[..index].iter().sum();
    let pair_sum = weights[index] + weights[index + 1];
    // Small imported pairs still need a valid interval while keeping their total.
    let minimum = 0.05_f32.min(pair_sum * 0.5);
    weights[index] = (boundary - prefix).clamp(minimum, pair_sum - minimum);
    weights[index + 1] = pair_sum - weights[index];
}

#[cfg(test)]
#[path = "preview/tests.rs"]
mod tests;

//! Custom window chrome, matching Trontop: PSM runs `.with_decorations(false)`, so
//! the brand header is the title bar. This module owns what the OS caption did:
//! minimize / maximize / close, moving the window and resizing it from the edges.
//!
//! Given up with the OS caption: the Win11 hover-maximize snap flyout and system
//! rounded corners. Drag-to-edge Aero Snap still works because `StartDrag` hands the
//! move to the OS. Close sends `ViewportCommand::Close`, so the existing
//! close-intercept still hides PSM to the tray.
use crate::theme::Tokens;
use egui::{viewport::ResizeDirection as Dir, CursorIcon, Sense, Stroke, StrokeKind};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Caption {
    Minimize,
    Maximize,
    Restore,
    Close,
}

impl Caption {
    pub fn label(self) -> &'static str {
        match self {
            Caption::Minimize => "Minimize",
            Caption::Maximize => "Maximize",
            Caption::Restore => "Restore",
            Caption::Close => "Hide to tray",
        }
    }
}

pub fn is_maximized(ctx: &egui::Context) -> bool {
    ctx.input(|i| i.viewport().maximized.unwrap_or(false))
}

/// One raised caption button with a hand-painted glyph (line segments, never font
/// glyphs, so no font can tofu them). Close is tinted with the danger color.
fn button(ui: &mut egui::Ui, kind: Caption, t: &Tokens, compact: bool) -> egui::Response {
    let size = if compact {
        egui::vec2(22.0, 20.0)
    } else {
        egui::vec2(34.0, 27.0)
    };
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hovered = response.hovered();
    let danger = kind == Caption::Close;
    let fill = match (hovered, danger) {
        (true, true) => t.danger,
        (true, false) => t.row_hover,
        _ => t.panel_raised,
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 6.0, fill);
    painter.rect_stroke(
        rect,
        6.0,
        Stroke::new(1.0_f32, t.border),
        StrokeKind::Inside,
    );
    let ink = match (hovered, danger) {
        (true, true) => egui::Color32::WHITE,
        (false, true) => t.danger,
        _ => t.text,
    };
    let stroke = Stroke::new(1.4_f32, ink);
    let c = rect.center();
    let line = |a: egui::Vec2, b: egui::Vec2| {
        painter.line_segment([c + a, c + b], stroke);
    };
    match kind {
        Caption::Minimize => line(egui::vec2(-5.0, 0.0), egui::vec2(5.0, 0.0)),
        Caption::Maximize => {
            let square = egui::Rect::from_center_size(c, egui::vec2(10.0, 10.0));
            painter.rect_stroke(square, 1.0, stroke, StrokeKind::Middle);
        }
        Caption::Restore => {
            for offset in [egui::vec2(2.5, -2.5), egui::vec2(-2.0, 2.0)] {
                let square = egui::Rect::from_center_size(c + offset, egui::vec2(8.0, 8.0));
                painter.rect_stroke(square, 1.0, stroke, StrokeKind::Middle);
            }
        }
        Caption::Close => {
            line(egui::vec2(-5.0, -5.0), egui::vec2(5.0, 5.0));
            line(egui::vec2(-5.0, 5.0), egui::vec2(5.0, -5.0));
        }
    }
    #[cfg(test)]
    ui.ctx()
        .data_mut(|data| data.insert_temp(egui::Id::new("test-caption").with(kind.label()), rect));
    response.on_hover_text(kind.label())
}

/// Close, maximize/restore and minimize, laid out inside the header's existing
/// right-to-left block (a second right-to-left block would get zero width).
pub fn caption_buttons(ui: &mut egui::Ui, t: &Tokens, compact: bool) {
    let ctx = ui.ctx().clone();
    let maximized = is_maximized(&ctx);
    ui.spacing_mut().item_spacing.x = if compact { 2.0 } else { 4.0 };
    if button(ui, Caption::Close, t, compact).clicked() {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
    let resize = if maximized {
        Caption::Restore
    } else {
        Caption::Maximize
    };
    if button(ui, resize, t, compact).clicked() {
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    }
    if button(ui, Caption::Minimize, t, compact).clicked() {
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    }
}

/// Drag moves the window; double-click toggles maximize.
pub fn drag_window(ctx: &egui::Context, response: &egui::Response) {
    if response.double_clicked() {
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized(ctx)));
    } else if response.drag_started() {
        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
}

/// Make the measured gap between the brand and the header buttons a move handle.
/// `interact` on a known rect, never an `available_size()` filler, which would
/// push the right-hand buttons off-screen.
pub fn drag_region(ui: &mut egui::Ui, rect: egui::Rect) {
    let response = ui.interact(rect, ui.id().with("window-drag"), Sense::click_and_drag());
    drag_window(ui.ctx(), &response);
}

const EDGE: f32 = 6.0;
const CORNER: f32 = 10.0;

/// Resize handles for the border that `.with_decorations(false)` removed: four
/// edges and four corners, each a foreground area above the content.
pub fn edge_resize(ctx: &egui::Context) {
    let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
    if is_maximized(ctx) || fullscreen {
        return;
    }
    let s = ctx.screen_rect();
    let (l, r, t, b) = (s.left(), s.right(), s.top(), s.bottom());
    let rect = |x0, y0, x1, y1| egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1));
    let strips = [
        (
            rect(l + CORNER, t, r - CORNER, t + EDGE),
            Dir::North,
            CursorIcon::ResizeVertical,
        ),
        (
            rect(l + CORNER, b - EDGE, r - CORNER, b),
            Dir::South,
            CursorIcon::ResizeVertical,
        ),
        (
            rect(l, t + CORNER, l + EDGE, b - CORNER),
            Dir::West,
            CursorIcon::ResizeHorizontal,
        ),
        (
            rect(r - EDGE, t + CORNER, r, b - CORNER),
            Dir::East,
            CursorIcon::ResizeHorizontal,
        ),
        (
            rect(l, t, l + CORNER, t + CORNER),
            Dir::NorthWest,
            CursorIcon::ResizeNwSe,
        ),
        (
            rect(r - CORNER, t, r, t + CORNER),
            Dir::NorthEast,
            CursorIcon::ResizeNeSw,
        ),
        (
            rect(l, b - CORNER, l + CORNER, b),
            Dir::SouthWest,
            CursorIcon::ResizeNeSw,
        ),
        (
            rect(r - CORNER, b - CORNER, r, b),
            Dir::SouthEast,
            CursorIcon::ResizeNwSe,
        ),
    ];
    for (i, (strip, direction, cursor)) in strips.into_iter().enumerate() {
        egui::Area::new(egui::Id::new("psm-resize-edge").with(i))
            .order(egui::Order::Foreground)
            .fixed_pos(strip.min)
            .show(ctx, |ui| {
                let (_, response) = ui.allocate_exact_size(strip.size(), Sense::drag());
                let response = response.on_hover_cursor(cursor);
                if response.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
                }
            });
    }
}

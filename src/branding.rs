//! Four tiles, theme color, and two contrasting edges for arbitrary taskbars.
use crate::theme::ThemeSettings;
pub fn icon(theme: ThemeSettings, size: u32) -> egui::IconData {
    let high = size * 4;
    let edge = (40.0 / size as f32).max(0.85);
    let mut image = image::RgbaImage::new(high, high);
    for y in 0..high {
        for x in 0..high {
            let px = x as f32 / high as f32 * 32.0;
            let py = y as f32 / high as f32 * 32.0;
            for row in 0..2 {
                for col in 0..2 {
                    let left = 1.0 + 16.0 * col as f32;
                    let top = 1.0 + 16.0 * row as f32;
                    let dx = (px - (left + 6.5)).abs() - 4.5;
                    let dy = (py - (top + 6.5)).abs() - 4.5;
                    let distance = dx.max(0.0).hypot(dy.max(0.0)) + dx.max(dy).min(0.0) - 2.0;
                    if distance <= 0.0 {
                        let color = if distance > -edge {
                            [10, 12, 18]
                        } else if distance > -2.0 * edge {
                            [245, 248, 255]
                        } else if row == col {
                            theme.accent
                        } else {
                            theme.secondary
                        };
                        image.put_pixel(x, y, image::Rgba([color[0], color[1], color[2], 255]));
                    }
                }
            }
        }
    }
    let rgba = image::imageops::resize(&image, size, size, image::imageops::FilterType::Lanczos3)
        .into_raw();
    egui::IconData {
        rgba,
        width: size,
        height: size,
    }
}
pub fn paint(ui: &mut egui::Ui, rect: egui::Rect, theme: ThemeSettings) {
    for row in 0..2 {
        for col in 0..2 {
            let cell = egui::Rect::from_min_size(
                rect.min + egui::vec2(1.0 + col as f32 * 16.0, 1.0 + row as f32 * 16.0),
                egui::vec2(13.0, 13.0),
            );
            let rgb = if row == col {
                theme.accent
            } else {
                theme.secondary
            };
            ui.painter()
                .rect_filled(cell, 3.0, egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
            ui.painter().rect_stroke(
                cell,
                3.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(10, 12, 18)),
                egui::StrokeKind::Inside,
            );
            ui.painter().rect_stroke(
                cell.shrink(1.0),
                2.0,
                egui::Stroke::new(0.8, egui::Color32::from_rgb(245, 248, 255)),
                egui::StrokeKind::Inside,
            );
        }
    }
}

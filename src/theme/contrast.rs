//! Text-bearing surfaces have a bounded brightness envelope. Raw palette colors
//! remain untouched for editing, artwork and filled actions with adaptive text.
use super::{mix, Color32};
use std::sync::LazyLock;

static LINEAR: LazyLock<[f32; 256]> = LazyLock::new(|| {
    std::array::from_fn(|i| {
        let value = i as f32 / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    })
});

fn luminance(color: Color32) -> f32 {
    0.2126 * LINEAR[color.r() as usize]
        + 0.7152 * LINEAR[color.g() as usize]
        + 0.0722 * LINEAR[color.b() as usize]
}

/// Same worst-case luminance as existing surface/ink helpers, without imposing
/// their per-channel cap on colorful artwork beneath translucent panels.
pub(super) fn in_envelope(color: Color32, dark: bool) -> bool {
    if dark {
        luminance(color) <= luminance(Color32::from_gray(64))
    } else {
        // A linear sRGB half-space is closed under interpolation. Checking only
        // luminance here lets two safe light endpoints mix into an unsafe middle.
        0.2126 * color.r() as f32 + 0.7152 * color.g() as f32 + 0.0722 * color.b() as f32 >= 205.0
    }
}

/// Opaque sRGB colors, before glyph anti-aliasing.
pub(crate) fn ratio(a: Color32, b: Color32) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// Retain a preferred hue when possible, then lift/darken only enough to pass.
/// Black or white always supplies an attainable fallback for an opaque sRGB fill.
pub fn readable_text(preferred: Color32, background: Color32) -> Color32 {
    if ratio(preferred, background) >= 4.5 {
        return preferred;
    }
    let target = if ratio(Color32::WHITE, background) > ratio(Color32::BLACK, background) {
        Color32::WHITE
    } else {
        Color32::BLACK
    };
    let (mut low, mut high) = (0.0, 1.0);
    let mut result = target;
    for _ in 0..8 {
        let middle = (low + high) * 0.5;
        let candidate = mix(preferred, target, middle);
        if ratio(candidate, background) >= 4.5 {
            high = middle;
            result = candidate;
        } else {
            low = middle;
        }
    }
    result
}

pub fn surface(color: Color32, dark: bool) -> Color32 {
    let color = color.to_opaque();
    if in_envelope(color, dark) {
        return color;
    }
    let anchor = if dark { Color32::BLACK } else { Color32::WHITE };
    let (mut low, mut high) = (0.0, 1.0);
    let mut result = anchor;
    for _ in 0..12 {
        let amount = (low + high) * 0.5;
        let candidate = mix(color, anchor, amount);
        if in_envelope(candidate, dark) {
            high = amount;
            result = candidate;
        } else {
            low = amount;
        }
    }
    result
}

/// Safe on every surface in the envelope, including hover and composed bands.
pub fn ink(color: Color32, dark: bool) -> Color32 {
    readable_text(color, Color32::from_gray(if dark { 64 } else { 205 }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{tokens, ThemeSettings};

    #[test]
    fn contrast_reference_pairs_and_existing_failures() {
        assert!((ratio(Color32::WHITE, Color32::BLACK) - 21.0).abs() < 0.001);
        assert_eq!(ratio(Color32::GRAY, Color32::GRAY), 1.0);
        // These were actual alpha.22 choices, not hypothetical extra styles.
        assert!(ratio(Color32::WHITE, tokens(ThemeSettings::default()).danger) < 4.5);
        let light = tokens(ThemeSettings {
            dark: false,
            ..ThemeSettings::monke_portal()
        });
        assert!(ratio(light.accent, light.panel_raised) < 4.5);
    }

    #[test]
    fn arbitrary_colors_keep_surface_ink_and_action_text_readable() {
        for r in (0..=255).step_by(17) {
            for g in (0..=255).step_by(17) {
                for b in (0..=255).step_by(17) {
                    let raw = Color32::from_rgb(r, g, b);
                    for dark in [false, true] {
                        let t = tokens(ThemeSettings {
                            dark,
                            ..Default::default()
                        });
                        let bg = surface(raw, dark);
                        for foreground in [t.text, t.text_muted, ink(raw, dark)] {
                            assert!(ratio(foreground, bg) >= 4.5, "{foreground:?} on {bg:?}");
                        }
                        let fg = readable_text(t.text, raw);
                        assert!(ratio(fg, raw) >= 4.5, "{fg:?} on raw {raw:?}");
                        assert_eq!(readable_text(fg, raw), fg);
                    }
                }
            }
        }
    }
}

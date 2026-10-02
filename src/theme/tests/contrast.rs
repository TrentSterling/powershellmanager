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

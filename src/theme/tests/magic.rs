use super::*;
#[test]
fn random_rolls_are_repeatable_diverse_readable_and_preserve_preferences() {
    let mut palettes = std::collections::HashSet::new();
    for seed in 0..512 {
        for flavor in Flavor::ALL {
            for dark in [true, false] {
                let original = ThemeSettings {
                    dark,
                    roundness: 13.0,
                    frost: 0.93,
                    gradient_strength: 0.17,
                    gradient_enabled: false,
                    ..Default::default()
                };
                let mut s = original;
                let kind = randomize(&mut s, seed, flavor);
                assert_ne!(kind, Flavor::Auto);
                let mut again = original;
                randomize(&mut again, seed, flavor);
                assert_eq!(s, again);
                assert_eq!(s, s.normalized());
                assert!(s.stops.windows(2).all(|w| w[0].color != w[1].color));
                palettes.insert(s.stops.map(|stop| stop.color));
                let t = crate::theme::tokens(s);
                for bg in [
                    t.bg,
                    t.panel,
                    t.panel_raised,
                    t.row_hover,
                    t.accent_dim,
                    t.graph_bg,
                ] {
                    for ink in [t.text, t.text_muted, t.ink(t.accent), t.ink(t.secondary)] {
                        assert!(crate::theme::contrast_ratio(ink, bg) >= 4.5);
                    }
                }
                Palette::capture(&original).apply(&mut s);
                assert_eq!(s, original, "randomization touched unrelated preferences");
            }
        }
    }
    assert!(
        palettes.len() > 2800,
        "too many duplicate palettes: {}",
        palettes.len()
    );
}

#[test]
fn palette_undo_preserves_later_appearance_and_position_edits() {
    let original = ThemeSettings::default();
    let undo = Palette::capture(&original);
    let mut s = original;
    randomize(&mut s, 42, Flavor::Neon);
    s.dark = false;
    s.roundness = 2.0;
    s.stops[1].position = 0.25;
    undo.apply(&mut s);
    assert_eq!(s.accent, original.accent);
    assert_eq!(
        s.stops.map(|stop| stop.color),
        original.stops.map(|stop| stop.color)
    );
    assert!(!s.dark);
    assert_eq!(s.roundness, 2.0);
    assert_eq!(s.stops[1].position, 0.25);
}

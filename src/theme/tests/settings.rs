use super::*;

#[test]
fn settings_round_trip() {
    let settings = ThemeSettings::monke_portal();
    assert_eq!(ThemeSettings::decode(&settings.encode()), Some(settings));
}

#[test]
fn malformed_settings_are_rejected() {
    assert!(ThemeSettings::decode("broken").is_none());
}

#[test]
fn empty_screens_use_the_flat_background_and_valid_screens_use_the_gradient_mesh() {
    let settings = ThemeSettings::default();
    for size in [egui::vec2(0.0, 100.0), egui::vec2(100.0, 0.0)] {
        let ctx = egui::Context::default();
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                ..Default::default()
            },
            |ctx| paint_background(ctx, settings),
        );
        assert!(output
            .shapes
            .iter()
            .all(|shape| !matches!(shape.shape, egui::Shape::Mesh(_))));
    }
    let ctx = egui::Context::default();
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(100.0, 100.0),
            )),
            ..Default::default()
        },
        |ctx| paint_background(ctx, settings),
    );
    assert!(output.shapes.iter().any(|shape| matches!(
        &shape.shape,
        egui::Shape::Mesh(mesh) if mesh.is_valid() && !mesh.vertices.is_empty()
    )));
}

#[test]
fn arbitrary_gradients_keep_composed_panel_text_readable_at_every_frost() {
    for dark in [false, true] {
        for frost in [0.0, 0.10, 0.45, 0.8, 1.0] {
            let s = ThemeSettings {
                dark,
                gradient_strength: 1.0,
                frost,
                frost_light: frost,
                surface_tint: 1.0,
                ..Default::default()
            };
            let t = tokens(s);
            for red in [0, 64, 128, 192, 255] {
                for green in [0, 64, 128, 192, 255] {
                    for blue in [0, 64, 128, 192, 255] {
                        let bg = composed_panel(s, Color32::from_rgb(red, green, blue));
                        for fg in [t.text, t.text_muted, t.ink(t.accent), t.ink(t.secondary)] {
                            assert!(
                                contrast_ratio(fg, bg) >= 4.5,
                                "{dark} {frost} {fg:?} on {bg:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn full_range_controls_preserve_vivid_colors_and_frost_modes() {
    let mut s = ThemeSettings {
        gradient_strength: 1.0,
        frost: 0.0,
        frost_light: 1.0,
        ..Default::default()
    }
    .normalized();
    assert_eq!(s.gradient_strength, 1.0);
    assert_eq!(s.active_frost(), 0.0);
    assert_eq!(panel_color(s).a(), 0);
    let blue = backdrop(s, Color32::BLUE);
    assert!(
        blue.b() > 200,
        "blue was crushed by the old per-channel cap: {blue:?}"
    );
    s.dark = false;
    assert_eq!(s.active_frost(), 1.0);
    assert_eq!(panel_color(s).a(), 255);
    *s.active_frost_mut() = 0.3;
    s.dark = true;
    assert_eq!(s.active_frost(), 0.0);
    s.frost = 1.0;
    assert_eq!(backdrop(s, Color32::RED), Color32::RED);
    assert_eq!(composed_panel(s, Color32::RED), tokens(s).panel);
}

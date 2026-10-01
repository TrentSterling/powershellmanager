use super::*;

#[test]
fn all_four_pegs_are_exact_and_edges_extend() {
    let mut s = ThemeSettings {
        stops: Stop::palette([[255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 0]]),
        ..Default::default()
    };
    s.stops[0].position = 0.1;
    s.stops[3].position = 0.9;
    for peg in s.stops {
        assert_eq!(s.gradient_color(peg.position), rgb(peg.color));
    }
    assert_eq!(s.gradient_color(0.0), rgb(s.stops[0].color));
    assert_eq!(s.gradient_color(1.0), rgb(s.stops[3].color));
    let original = s;
    s.reverse_gradient();
    for i in 0..101 {
        let a = s.gradient_color(i as f32 / 100.0);
        let b = original.gradient_color(1.0 - i as f32 / 100.0);
        for (a, b) in a.to_array().iter().zip(b.to_array()) {
            assert!((*a as i16 - b as i16).abs() <= 1);
        }
    }
}

#[test]
fn degenerate_inputs_cannot_break_peg_order_or_mesh() {
    let mut s = ThemeSettings::default();
    for (i, v) in [f32::NAN, 1.0, 1.0, f32::INFINITY].into_iter().enumerate() {
        s.stops[i].position = v;
    }
    s = s.normalized();
    for i in 0..4 {
        s.move_stop(i, -1.0);
        s.move_stop(i, 5.0);
    }
    for pair in s.stops.windows(2) {
        assert!(pair[1].position > pair[0].position);
    }
    let rect = Rect::from_min_size(egui::pos2(12.0, 5.0), egui::vec2(913.0, 417.0));
    for angle in [0.0, 45.0, 90.0, 132.0, 180.0, 270.0, 359.0, f32::NAN] {
        let m = mesh(rect, s.stops, angle);
        assert!(m.is_valid());
        assert!(m.vertices.len() <= 36);
        let mut area = 0.0;
        for triangle in m.indices.chunks_exact(3) {
            let [a, b, c] = std::array::from_fn(|i| m.vertices[triangle[i] as usize].pos);
            let (ab, ac) = (b - a, c - a);
            area += (ab.x * ac.y - ab.y * ac.x).abs() * 0.5;
        }
        assert!((area - rect.area()).abs() < 1.0, "{angle}: {area}");
        assert!(m
            .vertices
            .iter()
            .all(|v| rect.expand(0.001).contains(v.pos)));
    }
}

#[test]
fn nonfinite_phases_invalid_peg_edits_and_empty_rectangles_are_safe() {
    let mut s = ThemeSettings::default();
    for phase in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(s.gradient_color(phase), s.gradient_color(0.0));
    }
    let original = s;
    for (index, position) in [
        (4, 0.5),
        (usize::MAX, 0.5),
        (1, f32::NAN),
        (2, f32::INFINITY),
    ] {
        s.move_stop(index, position);
        assert_eq!(s, original);
    }
    for rect in [
        Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(0.0, 100.0)),
        Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(100.0, 0.0)),
        Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(-10.0, 100.0)),
        Rect::from_min_size(egui::pos2(f32::NAN, 0.0), egui::vec2(100.0, 100.0)),
        Rect::from_min_size(egui::pos2(0.0, f32::INFINITY), egui::vec2(100.0, 100.0)),
        Rect::from_min_max(
            egui::pos2(-f32::MAX, -f32::MAX),
            egui::pos2(f32::MAX, f32::MAX),
        ),
        Rect::from_min_max(
            egui::pos2(f32::MAX * 0.75, 0.0),
            egui::pos2(f32::MAX, 100.0),
        ),
        Rect::from_min_max(egui::pos2(-1.5e38, -1.5e38), egui::pos2(1.5e38, 1.5e38)),
    ] {
        let mesh = mesh(rect, s.stops, 45.0);
        assert!(mesh.vertices.is_empty(), "unsafe rectangle {rect:?}");
        assert!(mesh.indices.is_empty());
        assert!(mesh.is_valid());
    }
}

#[test]
fn translated_rectangles_keep_complete_gradient_coverage_at_float_precision_limits() {
    let rect = Rect::from_min_size(
        egui::pos2(100_000_008.0, 100_000_008.0),
        egui::vec2(8.0, 8.0),
    );
    for angle in [0.0, 45.0, 90.0, 135.0, 180.0, 225.0, 270.0, 315.0] {
        let mesh = mesh(rect, ThemeSettings::default().stops, angle);
        assert!(mesh.is_valid());
        let mut area = 0.0;
        for triangle in mesh.indices.chunks_exact(3) {
            let [a, b, c] = std::array::from_fn(|i| mesh.vertices[triangle[i] as usize].pos);
            let (ab, ac) = (b - a, c - a);
            area += (ab.x * ac.y - ab.y * ac.x).abs() * 0.5;
        }
        assert_eq!(area, rect.area(), "angle {angle}");
        assert!(mesh.vertices.iter().all(|vertex| rect.contains(vertex.pos)));
    }
}

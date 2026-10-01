use super::*;
use egui::{Pos2, Rect};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stop {
    pub position: f32,
    pub color: [u8; 3],
}

impl Stop {
    pub fn palette(colors: [[u8; 3]; 4]) -> [Self; 4] {
        std::array::from_fn(|i| Self {
            position: i as f32 / 3.0,
            color: colors[i],
        })
    }
}

pub(super) fn normalize(stops: &mut [Stop; 4]) {
    for (i, stop) in stops.iter_mut().enumerate() {
        stop.position = if stop.position.is_finite() {
            stop.position.clamp(0.0, 1.0)
        } else {
            i as f32 / 3.0
        };
    }
    stops.sort_by(|a, b| a.position.total_cmp(&b.position));
    for i in 0..4 {
        stops[i].position = stops[i].position.clamp(
            if i == 0 {
                0.0
            } else {
                stops[i - 1].position + 0.01
            },
            1.0 - (3 - i) as f32 * 0.01,
        );
    }
}

impl ThemeSettings {
    pub fn gradient_color(self, phase: f32) -> Color32 {
        let phase = if phase.is_finite() {
            phase.clamp(0.0, 1.0)
        } else {
            0.0
        };
        for pair in self.stops.windows(2) {
            if phase <= pair[1].position {
                let span = (pair[1].position - pair[0].position).max(0.0001);
                return mix(
                    rgb(pair[0].color),
                    rgb(pair[1].color),
                    (phase - pair[0].position) / span,
                );
            }
        }
        rgb(self.stops[3].color)
    }

    pub fn move_stop(&mut self, index: usize, position: f32) {
        if index >= 4 || !position.is_finite() {
            return;
        }
        let min = if index == 0 {
            0.0
        } else {
            self.stops[index - 1].position + 0.01
        };
        let max = if index == 3 {
            1.0
        } else {
            self.stops[index + 1].position - 0.01
        };
        self.stops[index].position = position.clamp(min, max);
    }

    pub fn reverse_gradient(&mut self) {
        self.stops.reverse();
        for stop in &mut self.stops {
            stop.position = 1.0 - stop.position;
        }
    }

    pub fn evenly_space(&mut self) {
        for (i, stop) in self.stops.iter_mut().enumerate() {
            stop.position = i as f32 / 3.0;
        }
    }
}

/// Clip each linear-color band at its actual peg planes. This keeps close pegs
/// exact at every angle, with a small bounded mesh instead of a sampled grid.
pub fn paint_gradient(
    painter: &egui::Painter,
    rect: Rect,
    stops: [Stop; 4],
    angle: f32,
    color: impl Fn(Color32) -> Color32,
) {
    let mut mesh = mesh(rect, stops, angle);
    for vertex in &mut mesh.vertices {
        vertex.color = color(vertex.color);
    }
    painter.add(egui::Shape::mesh(mesh));
}

fn mesh(rect: Rect, stops: [Stop; 4], angle: f32) -> egui::Mesh {
    let mut mesh = egui::Mesh::default();
    if !rect.is_finite()
        || !rect.size().is_finite()
        || !rect.center().is_finite()
        || rect.width() <= 0.0
        || rect.height() <= 0.0
    {
        return mesh;
    }
    let settings = ThemeSettings {
        stops,
        gradient_angle: angle,
        ..Default::default()
    }
    .normalized();
    let angle = settings.gradient_angle.to_radians();
    let direction = egui::vec2(angle.cos(), angle.sin());
    let span = (rect.width() * direction.x.abs() + rect.height() * direction.y.abs()).max(0.001);
    if !span.is_finite() {
        return mesh;
    }
    // Clip in local coordinates; absolute midpoints and interpolation can lose
    // the rectangle or make neighboring bands overlap at large coordinates.
    let half_size = rect.size() * 0.5;
    let phase = |p: Pos2| 0.5 + (p.to_vec2() - half_size).dot(direction) / span;
    let mut bounds = vec![0.0];
    bounds.extend(settings.stops.iter().map(|s| s.position));
    bounds.push(1.0);
    for pair in bounds.windows(2) {
        if (pair[1] - pair[0]).abs() < f32::EPSILON {
            continue;
        }
        let mut polygon = vec![
            Pos2::ZERO,
            egui::pos2(rect.width(), 0.0),
            egui::pos2(rect.width(), rect.height()),
            egui::pos2(0.0, rect.height()),
        ];
        for (boundary, keep_above) in [(pair[0], true), (pair[1], false)] {
            let input = std::mem::take(&mut polygon);
            let inside = |v: f32| {
                if keep_above {
                    v >= boundary
                } else {
                    v <= boundary
                }
            };
            for (&previous, &next) in input.last().into_iter().chain(&input).zip(&input) {
                let (a, b) = (phase(previous), phase(next));
                if inside(a) != inside(b) {
                    polygon.push(previous.lerp(next, ((boundary - a) / (b - a)).clamp(0.0, 1.0)));
                }
                if inside(b) {
                    polygon.push(next);
                }
            }
        }
        let first = mesh.vertices.len() as u32;
        for &p in &polygon {
            mesh.colored_vertex(rect.min + p.to_vec2(), settings.gradient_color(phase(p)));
        }
        for i in 1..polygon.len().saturating_sub(1) {
            mesh.add_triangle(first, first + i as u32, first + i as u32 + 1);
        }
    }
    mesh
}

#[cfg(test)]
#[path = "tests/gradient.rs"]
mod tests;

use crate::monitor::Rect;

#[derive(Debug, Clone)]
pub struct Slot {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LayoutPreset {
    Grid { cols: u32, rows: u32 },
    Columns(u32),
    Rows(u32),
    LeftRight,
    TopBottom,
    MainSide { side_count: u32 },
    Focus { side_count: u32 },
}

impl LayoutPreset {
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().to_lowercase();

        // "2x3", "3x2", etc.
        if let Some((c, r)) = s.split_once('x') {
            let cols = c.trim().parse::<u32>().ok()?;
            let rows = r.trim().parse::<u32>().ok()?;
            if (1..=8).contains(&cols) && (1..=8).contains(&rows) {
                return Some(Self::Grid { cols, rows });
            }
        }

        // "columns:4" or "columns 4"
        if let Some(rest) = s.strip_prefix("columns") {
            return parse_count(rest, 0).map(Self::Columns);
        }

        // "rows:3" or "rows 3"
        if let Some(rest) = s.strip_prefix("rows") {
            return parse_count(rest, 0).map(Self::Rows);
        }

        if s == "left-right" || s == "leftright" || s == "split" {
            return Some(Self::LeftRight);
        }

        if s == "top-bottom" || s == "topbottom" {
            return Some(Self::TopBottom);
        }

        if let Some(rest) = s
            .strip_prefix("main-side")
            .or_else(|| s.strip_prefix("mainside"))
        {
            return parse_count(rest, 2).map(|side_count| Self::MainSide { side_count });
        }

        if let Some(rest) = s.strip_prefix("focus") {
            return parse_count(rest, 3).map(|side_count| Self::Focus { side_count });
        }

        None
    }

    pub fn valid(&self) -> bool {
        match self {
            Self::Grid { cols, rows } => (1..=8).contains(cols) && (1..=8).contains(rows),
            Self::Columns(n) | Self::Rows(n) => (1..=8).contains(n),
            Self::MainSide { side_count } | Self::Focus { side_count } => {
                (1..=8).contains(side_count)
            }
            _ => true,
        }
    }
    pub fn slot_count(&self) -> usize {
        if !self.valid() {
            return 0;
        }

        match self {
            Self::Grid { cols, rows } => (*cols as usize) * (*rows as usize),
            Self::Columns(n) => *n as usize,
            Self::Rows(n) => *n as usize,
            Self::LeftRight => 2,
            Self::TopBottom => 2,
            Self::MainSide { side_count } => 1 + *side_count as usize,
            Self::Focus { side_count } => 1 + *side_count as usize,
        }
    }

    pub fn compute_slots(&self, area: &Rect, gap: i32) -> Vec<Slot> {
        if !self.valid() || area.w < 8 || area.h < 8 {
            return Vec::new();
        }
        let gap = gap.clamp(0, 64).min((area.w.min(area.h) / 8 - 1).max(0));
        match self {
            Self::Grid { cols, rows } => compute_weighted_grid(*cols, *rows, area, gap, &[], &[]),
            Self::Columns(n) => compute_weighted_grid(*n, 1, area, gap, &[], &[]),
            Self::Rows(n) => compute_weighted_grid(1, *n, area, gap, &[], &[]),
            Self::LeftRight => compute_weighted_grid(2, 1, area, gap, &[], &[]),
            Self::TopBottom => compute_weighted_grid(1, 2, area, gap, &[], &[]),
            Self::MainSide { side_count } | Self::Focus { side_count } => {
                let (numerator, denominator) = if matches!(self, Self::Focus { .. }) {
                    (3, 4)
                } else {
                    (2, 3)
                };
                let main_w = ((area.w - gap) as i64 * numerator / denominator) as i32;
                let side_area = Rect {
                    x: area.x + main_w + gap,
                    y: area.y,
                    w: area.w - main_w - gap,
                    h: area.h,
                };
                let mut slots = vec![Slot {
                    x: area.x,
                    y: area.y,
                    w: main_w,
                    h: area.h,
                }];
                slots.extend(compute_weighted_grid(
                    1,
                    *side_count,
                    &side_area,
                    gap,
                    &[],
                    &[],
                ));
                slots
            }
        }
    }
    pub fn display_name(&self) -> String {
        match self {
            Self::Grid { cols, rows } => format!("{}x{} Grid", cols, rows),
            Self::Columns(n) => format!("{} Columns", n),
            Self::Rows(n) => format!("{} Rows", n),
            Self::LeftRight => "Left / Right".to_string(),
            Self::TopBottom => "Top / Bottom".to_string(),
            Self::MainSide { side_count } => format!("Main + {} Side", side_count),
            Self::Focus { side_count } => format!("Focus + {} Side", side_count),
        }
    }
}

fn parse_count(rest: &str, default: u32) -> Option<u32> {
    let rest = rest.trim();
    let rest = rest.strip_prefix(':').unwrap_or(rest).trim();
    let count = if rest.is_empty() {
        default
    } else {
        rest.parse().ok()?
    };
    (1..=8).contains(&count).then_some(count)
}

/// Compute grid slots with per-column and per-row weight fractions.
/// Weights are normalized fractions that sum to 1.0.
pub fn compute_weighted_grid(
    cols: u32,
    rows: u32,
    area: &Rect,
    gap: i32,
    col_weights: &[f32],
    row_weights: &[f32],
) -> Vec<Slot> {
    if !(1..=8).contains(&cols)
        || !(1..=8).contains(&rows)
        || area.w < cols as i32
        || area.h < rows as i32
    {
        return Vec::new();
    }
    let gap = gap
        .clamp(0, 64)
        .min(((area.w - cols as i32) / cols as i32).min((area.h - rows as i32) / rows as i32));
    let cols = cols as usize;
    let rows = rows as usize;
    // Rounded cumulative boundaries distribute remainder pixels without seams.
    fn axis(
        origin: i32,
        extent: i32,
        gap: i32,
        count: usize,
        values: &[f32],
    ) -> (Vec<i32>, Vec<i32>) {
        let sum: f64 = values.iter().map(|v| *v as f64).sum();
        let valid = values.len() == count
            && sum.is_finite()
            && sum > 0.0
            && values.iter().all(|v| *v > 0.0);
        let available = extent - gap * (count as i32 - 1);
        let mut positions = Vec::new();
        let mut sizes = Vec::new();
        let mut cumulative = 0.0;
        let mut previous = 0;
        for i in 0..count {
            cumulative += if valid {
                values.get(i).copied().unwrap_or(1.0) as f64 / sum
            } else {
                1.0 / count as f64
            };
            let end = if i + 1 == count {
                available
            } else {
                (cumulative * available as f64).round() as i32
            }
            .clamp(previous + 1, available - (count - i - 1) as i32);
            positions.push(origin + previous + gap * i as i32);
            sizes.push(end - previous);
            previous = end;
        }
        (positions, sizes)
    }
    let (col_x, col_widths) = axis(area.x, area.w, gap, cols, col_weights);
    let (row_y, row_heights) = axis(area.y, area.h, gap, rows, row_weights);

    let mut slots = Vec::with_capacity(cols * rows);
    for r in 0..rows {
        for c in 0..cols {
            slots.push(Slot {
                x: col_x[c],
                y: row_y[r],
                w: col_widths[c],
                h: row_heights[r],
            });
        }
    }
    slots
}

pub fn builtin_presets() -> Vec<(String, LayoutPreset)> {
    vec![
        ("1x2 Grid".into(), LayoutPreset::Grid { cols: 1, rows: 2 }),
        ("2x1 Grid".into(), LayoutPreset::Grid { cols: 2, rows: 1 }),
        ("2x2 Grid".into(), LayoutPreset::Grid { cols: 2, rows: 2 }),
        ("2x3 Grid".into(), LayoutPreset::Grid { cols: 2, rows: 3 }),
        ("3x2 Grid".into(), LayoutPreset::Grid { cols: 3, rows: 2 }),
        ("3x3 Grid".into(), LayoutPreset::Grid { cols: 3, rows: 3 }),
        ("4x2 Grid".into(), LayoutPreset::Grid { cols: 4, rows: 2 }),
        ("4x3 Grid".into(), LayoutPreset::Grid { cols: 4, rows: 3 }),
        ("4x4 Grid".into(), LayoutPreset::Grid { cols: 4, rows: 4 }),
        ("Left / Right".into(), LayoutPreset::LeftRight),
        ("Top / Bottom".into(), LayoutPreset::TopBottom),
        (
            "Main + 2 Side".into(),
            LayoutPreset::MainSide { side_count: 2 },
        ),
        (
            "Main + 3 Side".into(),
            LayoutPreset::MainSide { side_count: 3 },
        ),
        (
            "Main + 4 Side".into(),
            LayoutPreset::MainSide { side_count: 4 },
        ),
        (
            "Focus + 3 Side".into(),
            LayoutPreset::Focus { side_count: 3 },
        ),
        (
            "Focus + 4 Side".into(),
            LayoutPreset::Focus { side_count: 4 },
        ),
        ("2 Columns".into(), LayoutPreset::Columns(2)),
        ("3 Columns".into(), LayoutPreset::Columns(3)),
        ("4 Columns".into(), LayoutPreset::Columns(4)),
        ("2 Rows".into(), LayoutPreset::Rows(2)),
        ("3 Rows".into(), LayoutPreset::Rows(3)),
    ]
}

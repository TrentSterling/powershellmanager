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
            if cols > 0 && rows > 0 {
                return Some(Self::Grid { cols, rows });
            }
        }

        // "columns:4" or "columns 4"
        if let Some(rest) = s.strip_prefix("columns") {
            let n = rest.trim_start_matches(':').trim().parse::<u32>().ok()?;
            if n > 0 {
                return Some(Self::Columns(n));
            }
        }

        // "rows:3" or "rows 3"
        if let Some(rest) = s.strip_prefix("rows") {
            let n = rest.trim_start_matches(':').trim().parse::<u32>().ok()?;
            if n > 0 {
                return Some(Self::Rows(n));
            }
        }

        if s == "left-right" || s == "leftright" || s == "split" {
            return Some(Self::LeftRight);
        }

        if s == "top-bottom" || s == "topbottom" {
            return Some(Self::TopBottom);
        }

        if s.starts_with("main-side") || s.starts_with("mainside") {
            let rest = s
                .trim_start_matches("main-side")
                .trim_start_matches("mainside")
                .trim_start_matches(':')
                .trim();
            let n = rest.parse::<u32>().unwrap_or(2);
            return Some(Self::MainSide {
                side_count: n.max(1),
            });
        }

        if s.starts_with("focus") {
            let rest = s
                .trim_start_matches("focus")
                .trim_start_matches(':')
                .trim();
            let n = rest.parse::<u32>().unwrap_or(3);
            return Some(Self::Focus {
                side_count: n.max(1),
            });
        }

        None
    }

    pub fn slot_count(&self) -> usize {
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
        match self {
            Self::Grid { cols, rows } => {
                let cols = *cols as i32;
                let rows = *rows as i32;
                let total_gap_x = gap * (cols - 1);
                let total_gap_y = gap * (rows - 1);
                let cell_w = (area.w - total_gap_x) / cols;
                let cell_h = (area.h - total_gap_y) / rows;

                let mut slots = Vec::with_capacity((cols * rows) as usize);
                for r in 0..rows {
                    for c in 0..cols {
                        slots.push(Slot {
                            x: area.x + c * (cell_w + gap),
                            y: area.y + r * (cell_h + gap),
                            w: cell_w,
                            h: cell_h,
                        });
                    }
                }
                slots
            }
            Self::Columns(n) => {
                let n = *n as i32;
                let total_gap = gap * (n - 1);
                let col_w = (area.w - total_gap) / n;

                (0..n)
                    .map(|i| Slot {
                        x: area.x + i * (col_w + gap),
                        y: area.y,
                        w: col_w,
                        h: area.h,
                    })
                    .collect()
            }
            Self::Rows(n) => {
                let n = *n as i32;
                let total_gap = gap * (n - 1);
                let row_h = (area.h - total_gap) / n;

                (0..n)
                    .map(|i| Slot {
                        x: area.x,
                        y: area.y + i * (row_h + gap),
                        w: area.w,
                        h: row_h,
                    })
                    .collect()
            }
            Self::LeftRight => {
                let half_w = (area.w - gap) / 2;
                vec![
                    Slot {
                        x: area.x,
                        y: area.y,
                        w: half_w,
                        h: area.h,
                    },
                    Slot {
                        x: area.x + half_w + gap,
                        y: area.y,
                        w: half_w,
                        h: area.h,
                    },
                ]
            }
            Self::TopBottom => {
                let half_h = (area.h - gap) / 2;
                vec![
                    Slot {
                        x: area.x,
                        y: area.y,
                        w: area.w,
                        h: half_h,
                    },
                    Slot {
                        x: area.x,
                        y: area.y + half_h + gap,
                        w: area.w,
                        h: half_h,
                    },
                ]
            }
            Self::MainSide { side_count } => {
                let side_count = *side_count as i32;
                let main_w = (area.w - gap) * 2 / 3;
                let side_w = area.w - main_w - gap;
                let total_gap_y = gap * (side_count - 1);
                let side_h = (area.h - total_gap_y) / side_count;

                let mut slots = Vec::with_capacity(1 + side_count as usize);
                slots.push(Slot {
                    x: area.x,
                    y: area.y,
                    w: main_w,
                    h: area.h,
                });
                for i in 0..side_count {
                    slots.push(Slot {
                        x: area.x + main_w + gap,
                        y: area.y + i * (side_h + gap),
                        w: side_w,
                        h: side_h,
                    });
                }
                slots
            }
            Self::Focus { side_count } => {
                let side_count = *side_count as i32;
                let main_w = (area.w - gap) * 3 / 4;
                let side_w = area.w - main_w - gap;
                let total_gap_y = gap * (side_count - 1);
                let side_h = (area.h - total_gap_y) / side_count;

                let mut slots = Vec::with_capacity(1 + side_count as usize);
                slots.push(Slot {
                    x: area.x,
                    y: area.y,
                    w: main_w,
                    h: area.h,
                });
                for i in 0..side_count {
                    slots.push(Slot {
                        x: area.x + main_w + gap,
                        y: area.y + i * (side_h + gap),
                        w: side_w,
                        h: side_h,
                    });
                }
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
    let cols = cols as usize;
    let rows = rows as usize;

    let usable_w = area.w - gap * (cols as i32 - 1);
    let usable_h = area.h - gap * (rows as i32 - 1);

    // Compute column widths from weights
    let mut col_widths: Vec<i32> = col_weights
        .iter()
        .map(|w| (usable_w as f32 * w) as i32)
        .collect();
    // Give leftover pixels to last column
    let col_sum: i32 = col_widths.iter().sum();
    if let Some(last) = col_widths.last_mut() {
        *last += usable_w - col_sum;
    }

    // Compute row heights from weights
    let mut row_heights: Vec<i32> = row_weights
        .iter()
        .map(|w| (usable_h as f32 * w) as i32)
        .collect();
    let row_sum: i32 = row_heights.iter().sum();
    if let Some(last) = row_heights.last_mut() {
        *last += usable_h - row_sum;
    }

    // Cumulative x positions
    let mut col_x = Vec::with_capacity(cols);
    let mut cx = area.x;
    for (i, &cw) in col_widths.iter().enumerate() {
        col_x.push(cx);
        if i + 1 < cols {
            cx += cw + gap;
        }
    }

    // Cumulative y positions
    let mut row_y = Vec::with_capacity(rows);
    let mut cy = area.y;
    for (i, &rh) in row_heights.iter().enumerate() {
        row_y.push(cy);
        if i + 1 < rows {
            cy += rh + gap;
        }
    }

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

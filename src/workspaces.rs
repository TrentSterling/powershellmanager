//! Named arrangements resolve editable process/title rules, never stale handles.
use crate::{layout::Slot, monitor::MonitorInfo, order::WindowKey, windows::ManagedWindow};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub name: String,
    pub windows: Vec<Placement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Placement {
    pub window: WindowKey,
    #[serde(default)]
    pub title_rule: TitleRule,
    #[serde(default)]
    pub match_text: Option<String>,
    pub monitor: usize,
    /// Fractions of the monitor's usable work area, excluding the taskbar.
    pub rect: [f64; 4],
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TitleRule {
    #[default]
    Exact,
    Contains,
    ProcessOnly,
}
impl TitleRule {
    fn label(self) -> &'static str {
        match self {
            Self::Exact => "Exact title",
            Self::Contains => "Title contains",
            Self::ProcessOnly => "Process only",
        }
    }
}
fn matches(p: &Placement, window: &ManagedWindow) -> bool {
    let title = p.match_text.as_deref().unwrap_or(&p.window.title);
    !p.window.process.trim().is_empty()
        && window.process_name.eq_ignore_ascii_case(&p.window.process)
        && match p.title_rule {
            TitleRule::Exact => window.title == title,
            TitleRule::Contains => {
                !title.trim().is_empty()
                    && window.title.to_lowercase().contains(&title.to_lowercase())
            }
            TitleRule::ProcessOnly => true,
        }
}

/// The same resolution drives the preview and the actual restore.
pub fn match_rows<'a>(
    workspace: &Workspace,
    windows: &'a [ManagedWindow],
) -> Vec<(Option<&'a ManagedWindow>, String)> {
    let candidates: Vec<Vec<_>> = workspace
        .windows
        .iter()
        .map(|p| windows.iter().filter(|w| matches(p, w)).collect())
        .collect();
    candidates
        .iter()
        .map(|row| match row.as_slice() {
            [] => (None, "Missing".into()),
            [window]
                if candidates
                    .iter()
                    .filter(|other| other.len() == 1 && other[0].hwnd == window.hwnd)
                    .count()
                    == 1 =>
            {
                (Some(*window), format!("Matched: {}", window.title))
            }
            [_] => (None, "Conflict: another rule matches this window".into()),
            _ => (None, format!("Ambiguous: {} windows match", row.len())),
        })
        .collect()
}

pub fn capture(name: String, windows: &[ManagedWindow], monitors: &[MonitorInfo]) -> Workspace {
    let windows = windows
        .iter()
        .filter_map(|window| {
            if window.is_minimized || window.rect.w <= 0 || window.rect.h <= 0 {
                return None;
            }
            let r = window.rect;
            let monitor = monitors
                .iter()
                .filter(|m| m.work_area.w > 0 && m.work_area.h > 0)
                .max_by_key(|m| {
                    let a = m.work_area;
                    let overlap_x = ((r.x as i64 + r.w as i64).min(a.x as i64 + a.w as i64)
                        - (r.x as i64).max(a.x as i64))
                    .max(0);
                    let overlap_y = ((r.y as i64 + r.h as i64).min(a.y as i64 + a.h as i64)
                        - (r.y as i64).max(a.y as i64))
                    .max(0);
                    (overlap_x * overlap_y, m.is_primary)
                })?;
            let a = monitor.work_area;
            Some(Placement {
                window: WindowKey::from_window(window),
                title_rule: TitleRule::Exact,
                match_text: None,
                monitor: monitor.index,
                rect: [
                    (r.x as f64 - a.x as f64) / a.w as f64,
                    (r.y as f64 - a.y as f64) / a.h as f64,
                    r.w as f64 / a.w as f64,
                    r.h as f64 / a.h as f64,
                ],
            })
        })
        .collect();
    Workspace { name, windows }
}

/// Ambiguous titles are skipped. Each live window can be restored only once.
pub fn plan(
    workspace: &Workspace,
    windows: &[ManagedWindow],
    monitors: &[MonitorInfo],
) -> Vec<(isize, Slot)> {
    let mut plan = Vec::new();
    let rows = match_rows(workspace, windows);
    for (p, (window, _)) in workspace.windows.iter().zip(rows) {
        if p.rect.iter().any(|v| !v.is_finite()) || p.rect[2] <= 0.0 || p.rect[3] <= 0.0 {
            continue;
        }
        let Some(window) = window else {
            continue;
        };
        let hwnd = window.hwnd;
        let valid = |m: &&MonitorInfo| m.work_area.w > 0 && m.work_area.h > 0;
        let monitor = monitors
            .iter()
            .filter(valid)
            .find(|m| m.index == p.monitor)
            .or_else(|| monitors.iter().filter(valid).find(|m| m.is_primary))
            .or_else(|| monitors.iter().find(valid));
        let Some(monitor) = monitor else {
            continue;
        };
        let a = monitor.work_area;
        let w = (p.rect[2].clamp(0.0, 1.0) * a.w as f64).round() as i32;
        let h = (p.rect[3].clamp(0.0, 1.0) * a.h as f64).round() as i32;
        let w = w.clamp(160.min(a.w), a.w);
        let h = h.clamp(100.min(a.h), a.h);
        let dx = (p.rect[0].clamp(0.0, 1.0) * a.w as f64).round() as i32;
        let dy = (p.rect[1].clamp(0.0, 1.0) * a.h as f64).round() as i32;
        plan.push((
            hwnd,
            Slot {
                x: a.x.saturating_add(dx.clamp(0, a.w - w)),
                y: a.y.saturating_add(dy.clamp(0, a.h - h)),
                w,
                h,
            },
        ));
    }
    plan
}

pub fn panel(ui: &mut egui::Ui, app: &mut crate::app::PsmApp) {
    ui.heading("Saved workspaces");
    ui.label("Save window positions. Preview matches and adjust rules before restoring.");
    ui.small("Disconnected monitors fall back to your primary display. Restore pauses Auto arrange. Undo layout puts windows back.");
    ui.horizontal_wrapped(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut app.workspace_name)
                .hint_text("Workspace name")
                .desired_width(170.0)
                .char_limit(80),
        );
        let exists = app
            .config
            .workspace
            .iter()
            .any(|w| w.name == app.workspace_name.trim());
        if ui
            .add_enabled(
                app.actions_available() && !app.workspace_name.trim().is_empty(),
                egui::Button::new(if exists {
                    "Replace saved positions"
                } else {
                    "Save positions"
                }),
            )
            .clicked()
        {
            app.save_workspace();
        }
    });
    let mut restore = None;
    let mut remove = None;
    let actions_available = app.actions_available();
    let mut edited = false;
    for (index, workspace) in app.config.workspace.iter_mut().enumerate() {
        ui.push_id(index, |ui| {
            ui.group(|ui| {
                ui.label(egui::RichText::new(&workspace.name).strong());
                let matching = plan(workspace, &app.managed_windows, &app.monitors).len();
                ui.small(format!(
                    "{matching} of {} windows ready",
                    workspace.windows.len()
                ));
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            actions_available && matching > 0,
                            egui::Button::new("Restore workspace"),
                        )
                        .clicked()
                    {
                        restore = Some(index);
                    }
                    if ui.small_button("Use name").clicked() {
                        app.workspace_name = workspace.name.clone();
                    }
                    ui.menu_button("More", |ui| {
                        if ui.button("Delete saved workspace").clicked() {
                            remove = Some(index);
                            ui.close_menu();
                        }
                    });
                });
                egui::CollapsingHeader::new("Match preview and rules").default_open(true).show(ui, |ui| {
                    let rows = match_rows(workspace, &app.managed_windows);
                    preview(ui, workspace, &app.managed_windows, &app.monitors);
                    for (i, (placement, (matched, status))) in workspace.windows.iter_mut().zip(rows).enumerate() {
                        ui.push_id(i, |ui| {
                            ui.colored_label(if matched.is_some() { ui.visuals().text_color() } else { egui::Color32::from_rgb(210,145,60) }, format!("{}. {status}", i+1));
                            egui::CollapsingHeader::new(format!("Rule: {} · {}", placement.window.process, placement.title_rule.label())).show(ui, |ui| {
                                edited |= ui.add(egui::TextEdit::singleline(&mut placement.window.process).hint_text("Process name").desired_width(180.0)).changed();
                                egui::ComboBox::from_id_salt("title-rule").selected_text(placement.title_rule.label()).show_ui(ui, |ui| {
                                    for rule in [TitleRule::Exact, TitleRule::Contains, TitleRule::ProcessOnly] {
                                        edited |= ui.selectable_value(&mut placement.title_rule, rule, rule.label()).changed();
                                    }
                                });
                                if placement.title_rule != TitleRule::ProcessOnly {
                                    let mut text = placement.match_text.clone().unwrap_or_else(|| placement.window.title.clone());
                                    if ui.add(egui::TextEdit::singleline(&mut text).hint_text("Stable part of the title").desired_width(ui.available_width())).changed() {
                                        placement.match_text = Some(text); edited = true;
                                    }
                                }
                                ui.small("Process only and title fragments still require exactly one match. Conflicting rules are skipped.");
                            });
                        });
                    }
                });
            });
        });
    }
    if let Some(index) = restore {
        app.restore_workspace(index);
    }
    if let Some(index) = remove {
        app.config.workspace.remove(index);
        app.save_config();
    }
    if edited {
        app.save_config();
    }
}

fn preview(
    ui: &mut egui::Ui,
    workspace: &Workspace,
    windows: &[ManagedWindow],
    monitors: &[MonitorInfo],
) {
    let slots = plan(workspace, windows, monitors);
    let valid: Vec<_> = monitors
        .iter()
        .filter(|m| m.work_area.w > 0 && m.work_area.h > 0)
        .collect();
    if valid.is_empty() {
        ui.small("No displays available");
        return;
    }
    let left = valid
        .iter()
        .map(|m| m.work_area.x as f32)
        .fold(f32::INFINITY, f32::min);
    let top = valid
        .iter()
        .map(|m| m.work_area.y as f32)
        .fold(f32::INFINITY, f32::min);
    let right = valid
        .iter()
        .map(|m| m.work_area.x as f32 + m.work_area.w as f32)
        .fold(f32::NEG_INFINITY, f32::max);
    let bottom = valid
        .iter()
        .map(|m| m.work_area.y as f32 + m.work_area.h as f32)
        .fold(f32::NEG_INFINITY, f32::max);
    let width = ui.available_width().clamp(120.0, 600.0);
    let scale = (width / (right - left)).min(140.0 / (bottom - top));
    let (area, _) = ui.allocate_exact_size(
        egui::vec2(width, (bottom - top) * scale + 8.0),
        egui::Sense::hover(),
    );
    let rect = |x: f32, y: f32, w: f32, h: f32| {
        egui::Rect::from_min_size(
            area.min + egui::vec2((x - left) * scale, (y - top) * scale),
            egui::vec2(w * scale, h * scale),
        )
    };
    for m in valid {
        let a = m.work_area;
        ui.painter().rect_stroke(
            rect(a.x as f32, a.y as f32, a.w as f32, a.h as f32),
            3,
            egui::Stroke::new(1.0, ui.visuals().weak_text_color()),
            egui::StrokeKind::Inside,
        );
    }
    for (hwnd, slot) in slots {
        let r = rect(slot.x as f32, slot.y as f32, slot.w as f32, slot.h as f32).shrink(2.0);
        ui.painter()
            .rect_filled(r, 2, ui.visuals().selection.bg_fill.gamma_multiply(0.5));
        ui.painter().rect_stroke(
            r,
            2,
            ui.visuals().selection.stroke,
            egui::StrokeKind::Inside,
        );
        if let Some(i) = match_rows(workspace, windows)
            .iter()
            .position(|(w, _)| w.is_some_and(|w| w.hwnd == hwnd))
        {
            ui.painter().text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                format!("{}", i + 1),
                egui::FontId::proportional(13.0),
                ui.visuals().text_color(),
            );
        }
    }
}

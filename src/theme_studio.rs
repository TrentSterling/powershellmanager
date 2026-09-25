use crate::{
    app::PsmApp,
    config::NamedTheme,
    theme::{
        self,
        magic::{self, Flavor, Palette},
        typography::FontChoice,
        ThemeSettings,
    },
};
use egui::{Color32, RichText, Sense, Stroke};

#[derive(Default)]
pub struct Studio {
    flavor: Flavor,
    undo: Option<Palette>,
    selected_stop: usize,
    name: String,
    code: String,
    message: String,
    save_pending: bool,
}

impl Studio {
    pub fn roll(&mut self, settings: &mut ThemeSettings, seed: u64) {
        self.undo = Some(Palette::capture(settings));
        let flavor = magic::randomize(settings, seed, self.flavor);
        self.message = format!(
            "{} palette. Your frost, font and peg positions stay put.",
            flavor.label()
        );
    }

    pub fn undo(&mut self, settings: &mut ThemeSettings) {
        if let Some(palette) = self.undo.take() {
            palette.apply(settings);
        }
    }
}

pub fn show(ctx: &egui::Context, app: &mut PsmApp) {
    if !app.show_theme_studio {
        return;
    }
    let saved_before = serde_json::to_string(&app.config.saved_theme).unwrap_or_default();
    let before = app.theme_settings;
    let scale_before = app.config.defaults.ui_scale;
    let mut studio = std::mem::take(&mut app.studio);
    let mut open = true;
    let t = theme::tokens(before);
    egui::Window::new("Theme Studio")
        .id(egui::Id::new("psm-theme-studio"))
        .open(&mut open)
        .default_width(410.0)
        .min_width(300.0)
        .default_height(780.0)
        .default_pos(egui::pos2(
            (ctx.screen_rect().width() - 440.0).max(8.0),
            45.0,
        ))
        .resizable(true)
        .collapsible(false)
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(t.panel)
                .stroke(Stroke::new(1.5, t.ink(t.accent))),
        )
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .max_height((ctx.screen_rect().height() - 145.0).max(220.0))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("MAKE IT YOURS")
                            .color(t.ink(t.secondary))
                            .strong(),
                    );
                    ui.label("Tront colors. Live preview. Your layout stays yours.");
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        if ui.button(RichText::new("ColorMagic").strong()).clicked() {
                            let seed = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_nanos() as u64;
                            studio.roll(&mut app.theme_settings, seed);
                        }
                        egui::ComboBox::from_id_salt("magic-flavor")
                            .selected_text(studio.flavor.label())
                            .width(110.0)
                            .show_ui(ui, |ui| {
                                for f in Flavor::ALL {
                                    ui.selectable_value(&mut studio.flavor, f, f.label());
                                }
                            });
                        if ui
                            .add_enabled(studio.undo.is_some(), egui::Button::new("Undo"))
                            .clicked()
                        {
                            studio.undo(&mut app.theme_settings);
                        }
                    });
                    let s = &mut app.theme_settings;
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut s.dark, true, "Dark");
                        ui.selectable_value(&mut s.dark, false, "Light");
                        ui.checkbox(&mut s.high_contrast, "Bright outlines");
                    });
                    ui.separator();
                    ui.label(RichText::new("Gradient").strong());
                    ui.checkbox(&mut s.gradient_enabled, "Color the workspace");
                    gradient_editor(ui, s, &mut studio.selected_stop);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("Peg {}", studio.selected_stop + 1));
                        ui.color_edit_button_srgb(&mut s.stops[studio.selected_stop].color);
                        let mut position = s.stops[studio.selected_stop].position * 100.0;
                        if ui
                            .add(
                                egui::DragValue::new(&mut position)
                                    .range(0.0..=100.0)
                                    .suffix("%"),
                            )
                            .changed()
                        {
                            s.move_stop(studio.selected_stop, position / 100.0);
                        }
                        if ui.small_button("Reverse").clicked() {
                            s.reverse_gradient();
                        }
                        if ui.small_button("Space evenly").clicked() {
                            s.evenly_space();
                        }
                    });
                    ui.add(
                        egui::Slider::new(&mut s.gradient_angle, 0.0..=360.0)
                            .text("Direction")
                            .suffix("°"),
                    );
                    ui.add(
                        egui::Slider::new(&mut s.gradient_strength, 0.0..=1.0).text("Intensity"),
                    );
                    ui.horizontal(|ui| {
                        ui.label("Accent");
                        ui.color_edit_button_srgb(&mut s.accent);
                        ui.label("Secondary");
                        ui.color_edit_button_srgb(&mut s.secondary);
                    });
                    ui.separator();
                    ui.label(RichText::new("Surface & readability").strong());
                    ui.add(
                        egui::Slider::new(s.active_frost_mut(), 0.0..=1.0).text("Frost / opacity"),
                    );
                    ui.add(egui::Slider::new(&mut s.surface_tint, 0.0..=1.0).text("Surface tint"));
                    ui.add(
                        egui::Slider::new(&mut s.text_strength, 0.0..=1.0).text("Text contrast"),
                    );
                    ui.add(egui::Slider::new(&mut s.roundness, 0.0..=16.0).text("Corners"));
                    ui.add(egui::Slider::new(&mut s.zebra_strength, 0.0..=1.0).text("Row tint"));
                    ui.add(egui::Slider::new(&mut s.hover_strength, 0.0..=1.0).text("Hover tint"));
                    ui.horizontal(|ui| {
                        ui.label("Font");
                        egui::ComboBox::from_id_salt("theme-font")
                            .selected_text(s.font.label())
                            .show_ui(ui, |ui| {
                                for font in FontChoice::ALL {
                                    ui.selectable_value(&mut s.font, font, font.label());
                                }
                            });
                    });
                    ui.add(
                        egui::Slider::new(&mut app.config.defaults.ui_scale, 0.75..=1.5)
                            .text("UI scale"),
                    );
                    ui.separator();
                    egui::CollapsingHeader::new("Tront presets").show(ui, |ui| {
                        for (name, preset) in ThemeSettings::presets() {
                            ui.horizontal(|ui| {
                                let (rect, _) =
                                    ui.allocate_exact_size(egui::vec2(55.0, 16.0), Sense::hover());
                                theme::paint_gradient(ui.painter(), rect, preset.stops, 0.0, |c| c);
                                if ui.button(name).clicked() {
                                    *s = preset;
                                }
                            });
                        }
                    });
                    egui::CollapsingHeader::new("Saved themes & sharing").show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut studio.name)
                                    .hint_text("Theme name")
                                    .desired_width(180.0),
                            );
                            if ui
                                .add_enabled(
                                    !studio.name.trim().is_empty(),
                                    egui::Button::new("Save"),
                                )
                                .clicked()
                            {
                                let name = studio.name.trim().to_owned();
                                let saved = NamedTheme {
                                    name: name.clone(),
                                    code: s.encode(),
                                };
                                if let Some(old) =
                                    app.config.saved_theme.iter_mut().find(|v| v.name == name)
                                {
                                    *old = saved;
                                } else {
                                    app.config.saved_theme.push(saved);
                                }
                                studio.message = format!("Saved {name}");
                            }
                        });
                        let mut remove = None;
                        for (i, saved) in app.config.saved_theme.iter().enumerate() {
                            ui.horizontal(|ui| {
                                if ui.button(&saved.name).clicked() {
                                    if let Some(theme) = ThemeSettings::decode(&saved.code) {
                                        *s = theme;
                                    }
                                }
                                if ui.small_button("Delete").clicked() {
                                    remove = Some(i);
                                }
                            });
                        }
                        if let Some(i) = remove {
                            app.config.saved_theme.remove(i);
                        }
                        ui.label("Paste a Trontop or PowerShellManager theme here.");
                        ui.add(
                            egui::TextEdit::multiline(&mut studio.code)
                                .desired_rows(3)
                                .desired_width(f32::INFINITY)
                                .char_limit(16384),
                        );
                        ui.horizontal(|ui| {
                            if ui.button("Copy theme").clicked() {
                                studio.code = s.encode();
                                ctx.copy_text(studio.code.clone());
                                studio.message = "Theme copied".into();
                            }
                            if ui.button("Import").clicked() {
                                if let Some(theme) = ThemeSettings::decode(&studio.code) {
                                    *s = theme;
                                    studio.message = "Theme imported".into();
                                } else {
                                    studio.message =
                                        "Invalid theme. Your current theme was kept.".into();
                                }
                            }
                        });
                    });
                    if !studio.message.is_empty() {
                        ui.label(&studio.message);
                    }
                });
        });
    app.show_theme_studio = open;
    if before != app.theme_settings || scale_before != app.config.defaults.ui_scale {
        studio.save_pending = true;
        app.theme_dirty = true;
        app.config.defaults.theme_code = Some(app.theme_settings.encode());
    }
    // Save on release/close, not for every step of a slider drag.
    studio.save_pending |=
        saved_before != serde_json::to_string(&app.config.saved_theme).unwrap_or_default();
    if studio.save_pending && (!ctx.input(|i| i.pointer.any_down()) || !open) {
        app.save_config();
        studio.save_pending = false;
    }
    app.studio = studio;
}

fn gradient_editor(ui: &mut egui::Ui, s: &mut ThemeSettings, selected: &mut usize) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 42.0), Sense::hover());
    let strip = rect.shrink2(egui::vec2(9.0, 12.0));
    theme::paint_gradient(ui.painter(), strip, s.stops, 0.0, |c| c);
    for i in 0..4 {
        let center = egui::pos2(
            egui::lerp(strip.x_range(), s.stops[i].position),
            strip.center().y,
        );
        let response = ui.interact(
            egui::Rect::from_center_size(center, egui::vec2(18.0, 34.0)),
            ui.id().with(("peg", i)),
            Sense::click_and_drag(),
        );
        if response.clicked() || response.drag_started() {
            *selected = i;
        }
        if response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                s.move_stop(i, (pos.x - strip.left()) / strip.width());
            }
        }
        let [r, g, b] = s.stops[i].color;
        ui.painter()
            .circle_filled(center, 7.0, Color32::from_rgb(r, g, b));
        ui.painter().circle_stroke(
            center,
            8.0,
            Stroke::new(if *selected == i { 2.5 } else { 1.0 }, Color32::WHITE),
        );
        ui.painter()
            .circle_stroke(center, 9.5, Stroke::new(1.0, Color32::BLACK));
    }
}

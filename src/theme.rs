//! ColorMagic, gradient, contrast and theme JSON adapted from Trent's Trontop.
//! Attribution and terms: assets/theme-engine-NOTICE and assets/theme-engine-LICENSE.
use eframe::egui;
use egui::{Color32, FontFamily, FontId, Stroke, TextStyle, Theme as EguiTheme, Visuals};

mod legacy;
pub use legacy::{Theme, THEMES};

mod contrast;
mod gradient;
pub(crate) mod magic;
mod storage;
pub mod typography;
#[cfg(test)]
pub(crate) use contrast::ratio as contrast_ratio;
pub use contrast::{ink, surface as text_surface};
pub use gradient::{paint_gradient, Stop};

/// Shared spacing scale. Every new layout uses these instead of ad-hoc numbers.
/// Not every step is wired into a page yet; later polish-gauntlet packages
/// pick up the rest as they rebuild each page's layout.
#[allow(dead_code)]
pub mod space {
    pub const XS: f32 = 4.0;
    pub const S: f32 = 6.0;
    pub const M: f32 = 8.0;
    pub const L: f32 = 12.0;
    pub const XL: f32 = 16.0;
    pub const GAP: f32 = 8.0;
}

/// Standard inner padding for compact cards (KPI tiles, gap rows, etc.). Used by
/// Used by compact application cards.
#[allow(dead_code)]
pub const CARD_PAD: egui::Margin = egui::Margin::symmetric(10, 8);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThemeSettings {
    pub dark: bool,
    pub accent: [u8; 3],
    pub secondary: [u8; 3],
    pub stops: [Stop; 4],
    pub gradient_enabled: bool,
    pub gradient_angle: f32,
    pub gradient_strength: f32,
    pub frost: f32,
    pub frost_light: f32,
    pub surface_tint: f32,
    pub text_strength: f32,
    pub font: typography::FontChoice,
    pub roundness: f32,
    pub zebra_strength: f32,
    pub column_strength: f32,
    pub hover_strength: f32,
    pub high_contrast: bool,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        Self::tront_stack()
    }
}

impl ThemeSettings {
    pub fn tront_stack() -> Self {
        Self {
            dark: true,
            accent: [168, 85, 247],
            secondary: [46, 230, 215],
            stops: Stop::palette([
                [168, 85, 247],
                [96, 119, 250],
                [46, 230, 215],
                [43, 127, 185],
            ]),
            gradient_enabled: true,
            gradient_angle: 132.0,
            gradient_strength: 0.34,
            frost: 0.80,
            frost_light: 0.59,
            surface_tint: 0.08,
            text_strength: 0.0,
            font: typography::FontChoice::Sans,
            roundness: 8.0,
            zebra_strength: 0.075,
            column_strength: 0.05,
            hover_strength: 0.18,
            high_contrast: false,
        }
    }

    pub fn demigod() -> Self {
        Self {
            accent: [245, 91, 48],
            secondary: [255, 181, 66],
            stops: Stop::palette([[245, 91, 48], [174, 63, 86], [220, 131, 55], [255, 181, 66]]),
            gradient_angle: 158.0,
            ..Self::tront_stack()
        }
    }

    pub fn monke_portal() -> Self {
        Self {
            accent: [46, 230, 215],
            secondary: [111, 78, 255],
            stops: Stop::palette([
                [46, 230, 215],
                [32, 133, 190],
                [93, 77, 188],
                [111, 78, 255],
            ]),
            gradient_angle: 28.0,
            ..Self::tront_stack()
        }
    }

    pub fn copper_legacy() -> Self {
        Self {
            accent: [224, 104, 52],
            secondary: [200, 146, 78],
            stops: Stop::palette([
                [224, 104, 52],
                [129, 75, 54],
                [174, 109, 54],
                [200, 146, 78],
            ]),
            gradient_angle: 116.0,
            ..Self::tront_stack()
        }
    }

    pub fn presets() -> [(&'static str, Self); 8] {
        [
            ("TrontStack", Self::tront_stack()),
            ("Demigod", Self::demigod()),
            ("Monke Portal", Self::monke_portal()),
            ("Copper Legacy", Self::copper_legacy()),
            (
                "Porcelain",
                Self {
                    dark: false,
                    accent: [25, 101, 226],
                    secondary: [90, 67, 185],
                    stops: Stop::palette([
                        [124, 169, 226],
                        [162, 189, 222],
                        [190, 175, 218],
                        [145, 204, 211],
                    ]),
                    gradient_strength: 0.15,
                    frost: 0.94,
                    frost_light: 0.94,
                    ..Self::default()
                },
            ),
            (
                "Carbon",
                Self {
                    accent: [238, 175, 89],
                    secondary: [196, 147, 90],
                    stops: Stop::palette([
                        [128, 96, 58],
                        [84, 79, 70],
                        [111, 89, 66],
                        [156, 113, 68],
                    ]),
                    gradient_strength: 0.20,
                    frost: 0.92,
                    ..Self::default()
                },
            ),
            (
                "Phosphor",
                Self {
                    accent: [69, 221, 152],
                    secondary: [120, 239, 181],
                    stops: Stop::palette([[20, 93, 67], [23, 70, 60], [44, 114, 70], [24, 86, 57]]),
                    roundness: 3.0,
                    high_contrast: true,
                    ..Self::default()
                },
            ),
            (
                "Vector",
                Self {
                    accent: [47, 217, 235],
                    secondary: [240, 100, 146],
                    stops: Stop::palette([
                        [25, 131, 166],
                        [70, 55, 148],
                        [125, 50, 130],
                        [206, 75, 111],
                    ]),
                    roundness: 5.0,
                    ..Self::default()
                },
            ),
        ]
    }

    pub fn normalized(mut self) -> Self {
        let bounded = |v: f32, fallback, min, max| {
            if v.is_finite() {
                v.clamp(min, max)
            } else {
                fallback
            }
        };
        self.gradient_angle =
            bounded(self.gradient_angle, 132.0, -36000.0, 36000.0).rem_euclid(360.0);
        self.gradient_strength = bounded(self.gradient_strength, 0.34, 0.0, 1.0);
        self.frost = bounded(self.frost, 0.80, 0.0, 1.0);
        self.frost_light = bounded(self.frost_light, 0.59, 0.0, 1.0);
        self.surface_tint = bounded(self.surface_tint, 0.08, 0.0, 1.0);
        self.text_strength = bounded(self.text_strength, 0.0, 0.0, 1.0);
        self.roundness = bounded(self.roundness, 8.0, 0.0, 18.0);
        self.zebra_strength = bounded(self.zebra_strength, 0.075, 0.0, 0.18);
        self.column_strength = bounded(self.column_strength, 0.05, 0.0, 0.16);
        self.hover_strength = bounded(self.hover_strength, 0.18, 0.06, 0.30);
        gradient::normalize(&mut self.stops);
        self
    }

    pub fn active_frost(self) -> f32 {
        if self.dark {
            self.frost
        } else {
            self.frost_light
        }
    }

    pub fn active_frost_mut(&mut self) -> &mut f32 {
        if self.dark {
            &mut self.frost
        } else {
            &mut self.frost_light
        }
    }

    fn frost_alpha(self) -> f32 {
        (self.active_frost().clamp(0.0, 1.0) * 255.0).round() / 255.0
    }
}

#[derive(Clone, Copy)]
pub struct Tokens {
    pub dark: bool,
    pub bg: Color32,
    pub panel: Color32,
    pub panel_raised: Color32,
    pub row_hover: Color32,
    pub accent: Color32,
    pub secondary: Color32,
    pub accent_dim: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub border: Color32,
    pub danger: Color32,
    pub graph_bg: Color32,
}

impl Tokens {
    pub fn surface(self, color: Color32) -> Color32 {
        text_surface(color, self.dark)
    }

    pub fn ink(self, color: Color32) -> Color32 {
        ink(color, self.dark)
    }
}

pub fn tokens(settings: ThemeSettings) -> Tokens {
    let accent = rgb(settings.accent);
    let secondary = rgb(settings.secondary);
    let mut result = if settings.dark {
        Tokens {
            dark: true,
            bg: Color32::from_rgb(9, 9, 13),
            panel: Color32::from_rgb(16, 16, 23),
            panel_raised: Color32::from_rgb(25, 24, 34),
            row_hover: mix(Color32::from_rgb(30, 29, 42), accent, 0.14),
            accent,
            secondary,
            accent_dim: mix(Color32::from_rgb(22, 19, 31), accent, 0.38),
            text: Color32::from_rgb(240, 238, 247),
            text_muted: Color32::from_rgb(184, 179, 198),
            border: Color32::from_rgb(63, 59, 78),
            danger: Color32::from_rgb(232, 75, 85),
            graph_bg: Color32::from_rgb(8, 8, 13),
        }
    } else {
        Tokens {
            dark: false,
            bg: Color32::from_rgb(236, 234, 242),
            panel: Color32::from_rgb(247, 246, 250),
            panel_raised: Color32::WHITE,
            row_hover: mix(Color32::WHITE, accent, 0.10),
            accent,
            secondary,
            accent_dim: mix(Color32::WHITE, accent, 0.24),
            text: Color32::from_rgb(28, 25, 35),
            text_muted: Color32::from_rgb(78, 72, 91),
            border: Color32::from_rgb(185, 179, 196),
            danger: Color32::from_rgb(199, 47, 59),
            graph_bg: Color32::from_rgb(229, 226, 236),
        }
    };
    // Coordinate neutral surfaces with the user's palette, without turning
    // labels into raw-accent text or changing the brightness safety envelope.
    let ground = mix(accent, secondary, 0.30);
    let tinted = |color: Color32, amount| text_surface(mix(color, ground, amount), settings.dark);
    result.bg = tinted(result.bg, settings.surface_tint * 0.5625);
    result.panel = tinted(result.panel, settings.surface_tint * 0.75);
    result.panel_raised = tinted(result.panel_raised, settings.surface_tint);
    result.graph_bg = tinted(result.graph_bg, settings.surface_tint * 0.3125);
    result.text_muted = mix(result.text_muted, result.text, settings.text_strength);
    if settings.high_contrast {
        result.text = if settings.dark {
            Color32::WHITE
        } else {
            Color32::from_rgb(12, 12, 18)
        };
        result.text_muted = result.text;
        result.border = mix(result.border, result.text, 0.25);
    }
    result.row_hover = result.surface(result.row_hover);
    result.accent_dim = result.surface(result.accent_dim);
    result
}

pub fn install(ctx: &egui::Context, settings: ThemeSettings) {
    let settings = settings.normalized();
    typography::install(ctx, settings.font);
    let t = tokens(settings);
    let theme = if settings.dark {
        EguiTheme::Dark
    } else {
        EguiTheme::Light
    };
    ctx.set_theme(theme);
    let mut style = (*ctx.style_of(theme)).clone();
    style.visuals = if settings.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    style.visuals.panel_fill = Color32::TRANSPARENT;
    style.visuals.window_fill = t.panel;
    style.visuals.override_text_color = Some(t.text);
    style.visuals.extreme_bg_color = t.graph_bg;
    let signal_blend = mix(t.accent, t.secondary, 0.48);
    style.visuals.faint_bg_color =
        t.surface(mix(t.panel_raised, signal_blend, settings.zebra_strength));
    style.visuals.selection.bg_fill = t.surface(mix(t.accent_dim, signal_blend, 0.24));
    style.visuals.selection.stroke = Stroke::new(1.0, t.text);
    style.visuals.widgets.noninteractive.fg_stroke.color = t.text;
    // Strong fill is used by slider rails and handles; keep it visible on cards.
    style.visuals.widgets.inactive.bg_fill = t.border;
    style.visuals.slider_trailing_fill = true;
    style.visuals.widgets.inactive.fg_stroke.color = t.text_muted;
    style.visuals.widgets.inactive.weak_bg_fill = t.panel_raised;
    style.visuals.widgets.hovered.bg_fill = t.row_hover;
    style.visuals.widgets.hovered.weak_bg_fill =
        t.surface(mix(t.row_hover, signal_blend, settings.hover_strength));
    style.visuals.widgets.hovered.fg_stroke.color = t.text;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, t.ink(t.secondary));
    style.visuals.widgets.active.bg_fill = t.accent_dim;
    style.visuals.widgets.active.weak_bg_fill = t.accent_dim;
    style.visuals.widgets.active.fg_stroke.color = t.text;
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, t.ink(t.accent));
    style.visuals.widgets.open = style.visuals.widgets.active;
    style.visuals.hyperlink_color = t.ink(t.secondary);
    style.visuals.error_fg_color = t.ink(t.danger);
    style.visuals.warn_fg_color = t.ink(Color32::from_rgb(230, 160, 50));
    style.visuals.window_stroke = Stroke::new(1.0, t.border);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, t.border);
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, t.border);
    style.visuals.window_corner_radius = settings.roundness.into();
    style.visuals.menu_corner_radius = settings.roundness.into();
    for widgets in [
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widgets.corner_radius = (settings.roundness * 0.65).into();
    }
    style.interaction.selectable_labels = false;
    style.spacing.item_spacing = egui::vec2(8.0, 8.0);
    style.spacing.button_padding = egui::vec2(11.0, 6.0);
    style.spacing.scroll.floating = false;
    style.text_styles = [
        (
            TextStyle::Heading,
            // Dialog titles (egui title bars) share the 16 px dialog header size.
            FontId::new(16.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(15.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(14.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(12.0, FontFamily::Monospace),
        ),
    ]
    .into();
    ctx.set_style_of(theme, style);
}

pub fn paint_background(ctx: &egui::Context, settings: ThemeSettings) {
    let settings = settings.normalized();
    let t = tokens(settings);
    let rect = ctx.screen_rect();
    if !settings.gradient_enabled || rect.width() <= 0.0 || rect.height() <= 0.0 {
        ctx.layer_painter(egui::LayerId::background())
            .rect_filled(rect, 0.0, t.bg);
        return;
    }
    paint_gradient(
        &ctx.layer_painter(egui::LayerId::background()),
        rect,
        settings.stops,
        settings.gradient_angle,
        |color| backdrop(settings, color),
    );
}

/// Preserve saturated colors until the *composed panel* exceeds the shared
/// text-surface luminance envelope. Frost is accounted for once, not twice.
pub fn backdrop(settings: ThemeSettings, color: Color32) -> Color32 {
    let t = tokens(settings);
    let raw = mix(t.bg, color, settings.gradient_strength);
    let frost = settings.frost_alpha();
    let safe = |candidate| contrast::in_envelope(mix(candidate, t.panel, frost), settings.dark);
    if safe(raw) {
        return raw;
    }
    let anchor = if settings.dark {
        Color32::BLACK
    } else {
        Color32::WHITE
    };
    let (mut low, mut high) = (0.0, 1.0);
    let mut result = anchor;
    for _ in 0..12 {
        let amount = (low + high) * 0.5;
        let candidate = mix(raw, anchor, amount);
        if safe(candidate) {
            high = amount;
            result = candidate;
        } else {
            low = amount;
        }
    }
    result
}

/// Opaque preview of the exact background plus one main-panel layer.
#[cfg(test)]
pub fn composed_panel(settings: ThemeSettings, color: Color32) -> Color32 {
    let t = tokens(settings);
    let wash = if settings.gradient_enabled {
        backdrop(settings, color)
    } else {
        t.bg
    };
    mix(wash, t.panel, settings.frost_alpha())
}

pub fn panel_color(settings: ThemeSettings) -> Color32 {
    let t = tokens(settings);
    let alpha = (settings.active_frost().clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(t.panel.r(), t.panel.g(), t.panel.b(), alpha)
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let channel = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        channel(a.r(), b.r()),
        channel(a.g(), b.g()),
        channel(a.b(), b.b()),
        channel(a.a(), b.a()),
    )
}

fn rgb(value: [u8; 3]) -> Color32 {
    Color32::from_rgb(value[0], value[1], value[2])
}

pub fn layout_colors(settings: ThemeSettings) -> Theme {
    let t = tokens(settings);
    Theme {
        surface: t.graph_bg,
        text: t.text,
        text_muted: t.text_muted,
        accent: t.ink(t.accent),
        accent2: t.ink(t.secondary),
        cell_enabled: t.surface(mix(t.panel_raised, t.accent, 0.20)),
        cell_occupied: t.surface(mix(t.panel_raised, t.secondary, 0.30)),
        cell_disabled: t.panel,
        cell_hover: t.surface(mix(t.panel_raised, t.accent, 0.42)),
        border: t.border,
    }
}

pub fn from_legacy(index: usize) -> ThemeSettings {
    let old = &THEMES[index.min(THEMES.len() - 1)];
    let [r, g, b, _] = old.accent.to_array();
    let [a, c, d, _] = old.accent2.to_array();
    ThemeSettings {
        accent: [r, g, b],
        secondary: [a, c, d],
        stops: Stop::palette([[r, g, b], [88, 88, 158], [a, c, d], [32, 112, 153]]),
        gradient_strength: 0.78,
        frost: 0.38,
        surface_tint: 0.16,
        text_strength: 0.65,
        font: typography::FontChoice::Rajdhani,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
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
}

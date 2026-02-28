use egui::Color32;

#[derive(Clone)]
pub struct Theme {
    pub name: &'static str,
    pub bg: Color32,
    pub surface: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub accent: Color32,
    pub accent2: Color32,
    pub cell_enabled: Color32,
    pub cell_occupied: Color32,
    pub cell_disabled: Color32,
    pub cell_hover: Color32,
    pub border: Color32,
}

pub const THEMES: [Theme; 3] = [
    // Dark — tront.xyz brand
    Theme {
        name: "Dark",
        bg: Color32::from_rgb(0x12, 0x12, 0x12),
        surface: Color32::from_rgb(0x1a, 0x1a, 0x1a),
        text: Color32::from_rgb(0xe0, 0xe0, 0xe0),
        text_muted: Color32::from_rgb(0x80, 0x80, 0x80),
        accent: Color32::from_rgb(0xa8, 0x55, 0xf7),
        accent2: Color32::from_rgb(0x2e, 0xe6, 0xd7),
        cell_enabled: Color32::from_rgb(0x3d, 0x28, 0x6b),
        cell_occupied: Color32::from_rgb(0x1a, 0x6b, 0x63),
        cell_disabled: Color32::from_rgb(0x28, 0x28, 0x28),
        cell_hover: Color32::from_rgb(0xc0, 0x7a, 0xff),
        border: Color32::from_rgb(0x30, 0x36, 0x3d),
    },
    // Mid — softer variant
    Theme {
        name: "Mid",
        bg: Color32::from_rgb(0x2a, 0x2a, 0x2e),
        surface: Color32::from_rgb(0x35, 0x35, 0x3a),
        text: Color32::from_rgb(0xf0, 0xf0, 0xf0),
        text_muted: Color32::from_rgb(0x90, 0x90, 0x90),
        accent: Color32::from_rgb(0xb8, 0x7a, 0xef),
        accent2: Color32::from_rgb(0x5a, 0xeb, 0xd4),
        cell_enabled: Color32::from_rgb(0x4a, 0x38, 0x78),
        cell_occupied: Color32::from_rgb(0x28, 0x78, 0x70),
        cell_disabled: Color32::from_rgb(0x38, 0x38, 0x3c),
        cell_hover: Color32::from_rgb(0xd0, 0x90, 0xff),
        border: Color32::from_rgb(0x50, 0x50, 0x5a),
    },
    // Neon — high contrast
    Theme {
        name: "Neon",
        bg: Color32::from_rgb(0x0a, 0x0a, 0x0f),
        surface: Color32::from_rgb(0x12, 0x12, 0x18),
        text: Color32::from_rgb(0xe8, 0xe8, 0xe8),
        text_muted: Color32::from_rgb(0x70, 0x70, 0x70),
        accent: Color32::from_rgb(0x39, 0xff, 0x14),
        accent2: Color32::from_rgb(0xff, 0x00, 0xff),
        cell_enabled: Color32::from_rgb(0x14, 0x40, 0x0a),
        cell_occupied: Color32::from_rgb(0x40, 0x0a, 0x40),
        cell_disabled: Color32::from_rgb(0x1a, 0x1a, 0x1f),
        cell_hover: Color32::from_rgb(0x50, 0xff, 0x30),
        border: Color32::from_rgb(0x1a, 0x1a, 0x2a),
    },
];

impl Theme {
    pub fn apply_to_egui(&self, ctx: &egui::Context) {
        let mut visuals = egui::Visuals::dark();

        visuals.panel_fill = self.bg;
        visuals.window_fill = self.surface;
        visuals.extreme_bg_color = self.surface;
        visuals.faint_bg_color = self.surface;

        visuals.selection.bg_fill = self.accent.linear_multiply(0.4);
        visuals.selection.stroke = egui::Stroke::new(1.0, self.accent);
        visuals.hyperlink_color = self.accent2;

        visuals.window_stroke = egui::Stroke::new(1.0, self.border);

        // Widget styling
        let widget_bg = self.surface;
        let widget_stroke = egui::Stroke::new(1.0, self.border);
        let hover_stroke = egui::Stroke::new(1.0, self.accent);
        let active_stroke = egui::Stroke::new(2.0, self.accent);

        visuals.widgets.noninteractive.bg_fill = widget_bg;
        visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, self.text_muted);
        visuals.widgets.noninteractive.bg_stroke = widget_stroke;

        visuals.widgets.inactive.bg_fill = widget_bg;
        visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, self.text);
        visuals.widgets.inactive.bg_stroke = widget_stroke;

        visuals.widgets.hovered.bg_fill = self.accent.linear_multiply(0.15);
        visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, self.text);
        visuals.widgets.hovered.bg_stroke = hover_stroke;

        visuals.widgets.active.bg_fill = self.accent.linear_multiply(0.3);
        visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, self.text);
        visuals.widgets.active.bg_stroke = active_stroke;

        visuals.widgets.open.bg_fill = self.surface;
        visuals.widgets.open.fg_stroke = egui::Stroke::new(1.0, self.text);
        visuals.widgets.open.bg_stroke = hover_stroke;

        visuals.override_text_color = Some(self.text);

        ctx.set_visuals(visuals);
    }
}

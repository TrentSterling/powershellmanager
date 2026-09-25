use egui::Color32;

#[derive(Clone)]
pub struct Theme {
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
    Theme {
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
    Theme {
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
    Theme {
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

use eframe::egui::{self, FontData, FontDefinitions, FontFamily};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FontChoice {
    #[default]
    Sans,
    Rajdhani,
    RajdhaniBold,
    Mono,
}

impl FontChoice {
    pub const ALL: [Self; 4] = [Self::Sans, Self::Rajdhani, Self::RajdhaniBold, Self::Mono];
    pub fn key(self) -> &'static str {
        match self {
            Self::Sans => "sans",
            Self::Rajdhani => "rajdhani",
            Self::RajdhaniBold => "rajdhani-bold",
            Self::Mono => "mono",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Sans => "Sans",
            Self::Rajdhani => "Rajdhani",
            Self::RajdhaniBold => "Rajdhani SemiBold",
            Self::Mono => "Monospace",
        }
    }
    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|font| font.key() == key)
    }
}

pub const LICENSE: &str = include_str!("../../assets/fonts/OFL.txt");

pub fn install(ctx: &egui::Context, choice: FontChoice) {
    let id = egui::Id::new("psm.font-installed");
    if ctx.data(|data| data.get_temp::<FontChoice>(id)) == Some(choice) {
        return;
    }
    let mut fonts = FontDefinitions::default();
    match choice {
        FontChoice::Sans => {}
        FontChoice::Rajdhani | FontChoice::RajdhaniBold => {
            let bytes: &'static [u8] = if choice == FontChoice::Rajdhani {
                include_bytes!("../../assets/fonts/Rajdhani-Medium.ttf")
            } else {
                include_bytes!("../../assets/fonts/Rajdhani-SemiBold.ttf")
            };
            fonts.font_data.insert(
                "trontop-rajdhani".into(),
                FontData::from_static(bytes).into(),
            );
            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, "trontop-rajdhani".into());
        }
        FontChoice::Mono => {
            let mut preferred = fonts.families[&FontFamily::Monospace].clone();
            preferred.extend(fonts.families[&FontFamily::Proportional].clone());
            fonts.families.insert(FontFamily::Proportional, preferred);
        }
    }
    // Explicit monospace data columns and missing-glyph fallbacks stay intact.
    ctx.set_fonts(fonts);
    ctx.data_mut(|data| data.insert_temp(id, choice));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn font_choice_changes_interface_metrics_but_preserves_numeric_font() {
        let ctx = egui::Context::default();
        let mut interface_widths = Vec::new();
        let mut numeric_widths = Vec::new();
        for choice in FontChoice::ALL {
            install(&ctx, choice);
            let _ = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    interface_widths.push(
                        ui.painter()
                            .layout_no_wrap(
                                "Trontop processes".into(),
                                egui::FontId::proportional(14.0),
                                egui::Color32::WHITE,
                            )
                            .size()
                            .x,
                    );
                    numeric_widths.push(
                        ui.painter()
                            .layout_no_wrap(
                                "1234.56 MiB".into(),
                                egui::FontId::monospace(14.0),
                                egui::Color32::WHITE,
                            )
                            .size()
                            .x,
                    );
                });
            });
        }
        assert!(interface_widths.windows(2).all(|pair| pair[0] != pair[1]));
        assert!(numeric_widths.windows(2).all(|pair| pair[0] == pair[1]));
    }
}

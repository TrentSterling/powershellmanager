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

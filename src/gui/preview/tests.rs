use super::*;

#[test]
fn divider_hits_share_boundaries_and_column_priority_for_hover_and_press() {
    let rect = egui::Rect::from_min_max(egui::pos2(10.0, 20.0), egui::pos2(210.0, 220.0));
    let at = |point| divider_at(point, rect, &[60.0, 160.0], &[80.0, 180.0], 5.0);
    assert_eq!(at(None), None);
    assert_eq!(at(Some(egui::pos2(60.0, 19.0))), None);
    assert_eq!(at(Some(egui::pos2(211.0, 80.0))), None);
    assert_eq!(at(Some(egui::pos2(100.0, 120.0))), None);
    assert_eq!(
        at(Some(egui::pos2(55.0, 120.0))),
        Some((DividerAxis::Col, 0))
    );
    assert_eq!(
        at(Some(egui::pos2(165.0, 120.0))),
        Some((DividerAxis::Col, 1))
    );
    assert_eq!(at(Some(egui::pos2(165.01, 120.0))), None);
    assert_eq!(
        at(Some(egui::pos2(100.0, 75.0))),
        Some((DividerAxis::Row, 0))
    );
    assert_eq!(
        at(Some(egui::pos2(100.0, 185.0))),
        Some((DividerAxis::Row, 1))
    );
    assert_eq!(
        at(Some(egui::pos2(60.0, 80.0))),
        Some((DividerAxis::Col, 0))
    );
}

#[test]
fn committing_weights_takes_priority_over_a_simultaneous_cell_click() {
    assert_eq!(
        PreviewAction::None.with_clicked_cell(None),
        PreviewAction::None
    );
    assert_eq!(
        PreviewAction::None.with_clicked_cell(Some(2)),
        PreviewAction::ToggleCell(2)
    );
    assert_eq!(
        PreviewAction::WeightsChanged.with_clicked_cell(Some(2)),
        PreviewAction::WeightsChanged
    );
    assert_eq!(
        PreviewAction::ToggleCell(1).with_clicked_cell(None),
        PreviewAction::ToggleCell(1)
    );
}

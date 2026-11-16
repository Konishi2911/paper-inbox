use i_slint_backend_testing::ElementHandle;
use paper_desktop::{DetailView, DiffResolution, MainWindow, ProposalRow};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};

fn control(w: &MainWindow, label: &str) -> ElementHandle {
    ElementHandle::find_by_accessible_label(w, label)
        .next()
        .unwrap_or_else(|| panic!("missing {label}"))
}
fn values(w: &MainWindow) -> Vec<ElementHandle> {
    ElementHandle::find_by_element_type_name(w, "ComparedValue").collect()
}
#[test]
fn comparison_reflows_expands_provenance_and_preserves_resolution_contract() {
    i_slint_backend_testing::init_no_event_loop();
    let w = MainWindow::new().unwrap();
    w.set_ready(true);
    w.set_busy(false);
    w.set_detail_view(DetailView::MetadataReview);
    let row = ProposalRow {
        id: "proposal-1".into(),
        label: "タイトル".into(),
        current: "Metadata management for researchers — 現在の手動入力".into(),
        proposed: "Metadata management for researchers: a provenance-aware approach — 提案値"
            .into(),
        current_preview: "Metadata management for researchers — 現在の手動入力".into(),
        proposed_preview:
            "Metadata management for researchers: a provenance-aware approach — 提案値".into(),
        change_label: "変更".into(),
        reason: "手動値への上書き".into(),
        conflict: true,
        current_source: "手動".into(),
        source: "Crossref".into(),
        current_details: "改訂: current-id\n情報源: 手動".into(),
        proposed_details: "候補改訂: proposed-id\n取得日時: 2026-11-15\n情報源更新日時: 2026-11-14"
            .into(),
        ..Default::default()
    };
    w.set_proposal_rows(ModelRc::new(VecModel::from(vec![row.clone()])));
    let resolutions = Rc::new(RefCell::new(Vec::new()));
    let output = resolutions.clone();
    w.on_choose_metadata(move |id, decision| output.borrow_mut().push((id.to_string(), decision)));
    w.show().unwrap();
    w.window().set_size(slint::LogicalSize::new(1600., 1100.));
    let columns = values(&w);
    assert_eq!(columns.len(), 2);
    assert!((columns[0].absolute_position().y - columns[1].absolute_position().y).abs() < 2.);
    assert!(columns[1].absolute_position().x > columns[0].absolute_position().x + 100.);
    control(&w, "詳細").invoke_accessible_default_action();
    assert!(
        ElementHandle::find_by_accessible_label(&w, "詳細を閉じる")
            .next()
            .is_some()
    );
    assert!(
        resolutions.borrow().is_empty(),
        "expansion never resolves a proposal"
    );
    control(&w, "詳細を閉じる").invoke_accessible_default_action();
    w.window().set_size(slint::LogicalSize::new(1080., 1100.));
    let stacked = values(&w);
    assert_eq!(stacked.len(), 2);
    assert!((stacked[0].absolute_position().x - stacked[1].absolute_position().x).abs() < 2.);
    assert!(stacked[1].absolute_position().y > stacked[0].absolute_position().y + 30.);
    let card = ElementHandle::find_by_element_type_name(&w, "MetadataDiffCard")
        .next()
        .unwrap();
    let heading = control(&w, "取得した候補");
    assert!(
        heading.absolute_position().y >= card.absolute_position().y + card.size().height,
        "next header y={} vs card bottom={}",
        heading.absolute_position().y,
        card.absolute_position().y + card.size().height
    );
    control(&w, "提案値を採用").invoke_accessible_default_action();
    assert_eq!(
        *resolutions.borrow(),
        vec![("proposal-1".into(), DiffResolution::AcceptProposed)]
    );
    control(&w, "現在値を維持").invoke_accessible_default_action();
    assert_eq!(
        resolutions.borrow().last(),
        Some(&("proposal-1".into(), DiffResolution::KeepCurrent))
    );
    w.set_busy(true);
    assert_eq!(
        control(&w, "提案値を採用").accessible_enabled(),
        Some(false)
    );
    assert_eq!(
        control(&w, "現在値を維持").accessible_enabled(),
        Some(false)
    );
    w.set_busy(false);
    let mut stale = row.clone();
    stale.stale = true;
    w.set_proposal_rows(ModelRc::new(VecModel::from(vec![stale])));
    assert_eq!(
        control(&w, "提案値を採用").accessible_enabled(),
        Some(false)
    );
    assert_eq!(control(&w, "現在値を維持").accessible_enabled(), Some(true));
    assert!(
        ElementHandle::find_by_accessible_label(&w, "判断を取り消す")
            .next()
            .is_none(),
        "an undecided card offers no undo"
    );
    let mut decided = row.clone();
    decided.decision = DiffResolution::AcceptProposed;
    w.set_proposal_rows(ModelRc::new(VecModel::from(vec![decided])));
    control(&w, "判断を取り消す").invoke_accessible_default_action();
    assert_eq!(
        resolutions.borrow().last(),
        Some(&("proposal-1".into(), DiffResolution::Unresolved))
    );
    let mut long = row;
    long.current = "日本語 α²\n".repeat(1000).into();
    long.current_preview = "日本語のプレビュー".into();
    long.long_text = true;
    w.set_proposal_rows(ModelRc::new(VecModel::from(vec![long])));
    control(&w, "全文を表示").invoke_accessible_default_action();
    assert!(
        ElementHandle::find_by_accessible_label(&w, "長文を折りたたむ")
            .next()
            .is_some()
    );
    control(&w, "長文を折りたたむ").invoke_accessible_default_action();
    assert!(
        ElementHandle::find_by_accessible_label(&w, "全文を表示")
            .next()
            .is_some()
    );
    w.hide().unwrap();
}

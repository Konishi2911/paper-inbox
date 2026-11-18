use i_slint_backend_testing::ElementHandle;
use paper_desktop::{DetailView, MainWindow, NavigationAction, config::Config, ui};
use slint::{
    ComponentHandle,
    platform::{PointerEventButton, WindowEvent},
};
use std::time::{Duration, Instant};
#[track_caller]
fn wait(window: &MainWindow) {
    let caller = std::panic::Location::caller();
    let weak = window.as_weak();
    let start = Instant::now();
    let timer = slint::Timer::default();
    let completed = std::rc::Rc::new(std::cell::Cell::new(false));
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(5),
        move || {
            let window = weak.upgrade().unwrap();
            assert!(
                start.elapsed() < Duration::from_secs(30),
                "wait from {caller}, view {:?}: {}",
                window.get_detail_view(),
                window.get_status()
            );
            if !window.get_busy() && !completed.replace(true) {
                slint::quit_event_loop().unwrap();
            }
        },
    );
    slint::run_event_loop_until_quit().unwrap();
}

fn element(window: &MainWindow, label: &str) -> ElementHandle {
    let elements: Vec<_> = ElementHandle::find_by_accessible_label(window, label).collect();
    assert_eq!(elements.len(), 1, "unique visible control: {label}");
    elements.into_iter().next().unwrap()
}

fn click(window: &MainWindow, label: &str) {
    click_element(window, &element(window, label));
}

fn click_element(window: &MainWindow, element: &ElementHandle) {
    let origin = element.absolute_position();
    let size = element.size();
    let position =
        slint::LogicalPosition::new(origin.x + size.width / 2., origin.y + size.height / 2.);
    window
        .window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    window.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    window
        .window()
        .dispatch_event(WindowEvent::PointerReleased {
            position,
            button: PointerEventButton::Left,
        });
}

#[track_caller]
fn screen(w: &MainWindow, expected: DetailView) {
    assert_eq!(w.get_detail_view(), expected);
    let names = [
        "PaperDetail",
        "PaperEditor",
        "ImportView",
        "MetadataView",
        "ExportView",
        "ExportDialog",
        "CleanupView",
    ];
    let visible = names
        .into_iter()
        .filter(|name| {
            ElementHandle::find_by_element_type_name(w, name)
                .next()
                .is_some()
        })
        .collect::<Vec<_>>();
    let expected_name = match expected {
        DetailView::Empty => None,
        DetailView::PaperDetail => Some("PaperDetail"),
        DetailView::PaperEditor => Some("PaperEditor"),
        DetailView::Import => Some("ImportView"),
        DetailView::MetadataReview | DetailView::MetadataCandidateEditor => Some("MetadataView"),
        DetailView::ExportSettings => Some("ExportView"),
        DetailView::ExportPreview => Some("ExportDialog"),
        DetailView::AttachmentCleanup => Some("CleanupView"),
    };
    assert_eq!(
        visible,
        expected_name.into_iter().collect::<Vec<_>>(),
        "exactly one right-hand screen"
    );
    assert_eq!(
        ElementHandle::find_by_element_type_name(w, "CandidateEditor")
            .next()
            .is_some(),
        expected == DetailView::MetadataCandidateEditor
    );
}

#[test]
fn enum_transitions_restore_nested_drafts_and_do_not_leave_the_source_on_failed_loads() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let path =
        std::env::temp_dir().join(format!("paper-detail-view-{}.sqlite", uuid::Uuid::new_v4()));
    let output = path.with_extension("bib");
    let w = MainWindow::new().unwrap();
    screen(&w, DetailView::Empty);
    w.show().unwrap();
    w.window().set_size(slint::LogicalSize::new(1360., 900.));
    let controller = ui::attach_with_lookup_provider(
        &w,
        Config {
            database: path.clone(),
        },
        Box::new(|_| Err(paper_metadata::LookupError::Network)),
    );
    wait(&w);
    // The app opens on the neutral screen; manual entry is an explicit choice.
    screen(&w, DetailView::Empty);
    // An empty library is welcomed with the way to add the first paper.
    element(&w, "文献を追加する");
    click(&w, "識別子なしで手動入力する");
    wait(&w);
    screen(&w, DetailView::PaperEditor);
    w.set_citation_key("View2026".into());
    element(&w, "文献タイトル").set_accessible_value("Saved");
    click(&w, "文献を登録");
    wait(&w);
    let id = w.get_editing_id();
    screen(&w, DetailView::PaperDetail);
    w.invoke_select_paper(id.clone(), true);
    w.set_export_path(output.to_string_lossy().as_ref().into());
    click(&w, "編集");
    element(&w, "文献タイトル").set_accessible_value("Editor draft");
    click(&w, "BibTeX 出力");
    screen(&w, DetailView::ExportSettings);
    assert!(w.get_dirty());
    click(&w, "文献を追加");
    assert!(w.get_confirm_discard());
    screen(&w, DetailView::ExportSettings);
    click(&w, "編集を続ける");
    screen(&w, DetailView::ExportSettings);
    // The cleanup screen is also guarded by the unsaved-input confirmation.
    click(&w, "未使用PDFの整理");
    assert!(w.get_confirm_discard());
    screen(&w, DetailView::ExportSettings);
    click(&w, "編集を続ける");
    screen(&w, DetailView::ExportSettings);
    click(&w, "詳細・入力へ戻る");
    screen(&w, DetailView::PaperEditor);
    assert_eq!(w.get_paper_title(), "Editor draft");
    assert!(w.get_dirty());
    // Direct preview (also used by application callbacks) returns to its true origin.
    w.invoke_prepare_export();
    wait(&w);
    screen(&w, DetailView::ExportPreview);
    assert!(w.get_dirty());
    w.invoke_cancel_export();
    wait(&w);
    screen(&w, DetailView::PaperEditor);
    assert!(w.get_dirty());
    element(&w, "編集をキャンセル").invoke_accessible_default_action();
    click(&w, "破棄して移動");
    wait(&w);
    screen(&w, DetailView::PaperDetail);
    w.invoke_request_navigation(NavigationAction::MetadataReview, id.clone());
    wait(&w);
    screen(&w, DetailView::MetadataReview);
    w.invoke_begin_candidate();
    screen(&w, DetailView::MetadataCandidateEditor);
    let candidate_id = w.get_candidate_id();
    w.set_candidate_title("Candidate draft".into());
    w.set_candidate_dirty(true);
    click(&w, "BibTeX 出力");
    screen(&w, DetailView::ExportSettings);
    click(&w, "出力内容を確認");
    wait(&w);
    screen(&w, DetailView::ExportPreview);
    click(&w, "キャンセル");
    wait(&w);
    screen(&w, DetailView::ExportSettings);
    click(&w, "詳細・入力へ戻る");
    screen(&w, DetailView::MetadataCandidateEditor);
    assert_eq!(w.get_candidate_id(), candidate_id);
    assert_eq!(w.get_candidate_title(), "Candidate draft");
    assert!(w.get_candidate_dirty());
    element(&w, "入力を破棄して戻る").invoke_accessible_default_action();
    screen(&w, DetailView::MetadataReview);
    assert!(!w.get_candidate_dirty());
    w.invoke_begin_lookup();
    wait(&w);
    screen(&w, DetailView::Import);
    w.set_lookup_identifier("10.1000/test".into());
    w.set_lookup_dirty(true);
    w.invoke_fetch_lookup();
    wait(&w);
    screen(&w, DetailView::Import);
    click(&w, "BibTeX 出力");
    screen(&w, DetailView::ExportSettings);
    click(&w, "詳細・入力へ戻る");
    screen(&w, DetailView::Import);
    assert_eq!(w.get_lookup_identifier(), "10.1000/test");
    assert!(w.get_lookup_dirty());
    // Discard approval does not erase source data when the requested paper cannot be loaded.
    w.invoke_request_navigation(
        NavigationAction::OpenPaper,
        uuid::Uuid::new_v4().to_string().into(),
    );
    assert!(w.get_confirm_discard());
    click(&w, "破棄して移動");
    wait(&w);
    screen(&w, DetailView::Import);
    assert!(w.get_lookup_dirty());
    assert_eq!(w.get_lookup_identifier(), "10.1000/test");
    w.invoke_cancel_lookup();
    wait(&w);
    screen(&w, DetailView::MetadataReview);
    w.invoke_request_navigation(NavigationAction::OpenPaper, id);
    wait(&w);
    screen(&w, DetailView::PaperDetail);
    assert!(!output.exists(), "cancelled previews never publish");
    w.hide().unwrap();
    drop(controller);
    drop(w);
    std::fs::remove_file(path).unwrap();
}

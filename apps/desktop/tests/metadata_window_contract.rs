use paper_desktop::NavigationAction;
use paper_desktop::{MainWindow, config::Config, ui};
use slint::{ComponentHandle, Model};
use std::time::{Duration, Instant};
fn wait(ui: &MainWindow) {
    let weak = ui.as_weak();
    let start = Instant::now();
    let timer = slint::Timer::default();
    let completed = std::rc::Rc::new(std::cell::Cell::new(false));
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(5),
        move || {
            let ui = weak.upgrade().unwrap();
            assert!(
                start.elapsed() < Duration::from_secs(30),
                "{}",
                ui.get_status()
            );
            if !ui.get_busy() && !completed.replace(true) {
                slint::quit_event_loop().unwrap();
            }
        },
    );
    slint::run_event_loop_until_quit().unwrap();
}
fn proposal(ui: &MainWindow, label: &str) -> slint::SharedString {
    ui.get_proposal_rows()
        .iter()
        .find(|r| r.label == label)
        .unwrap_or_else(|| panic!("Missing {label}: {}", ui.get_status()))
        .id
}
#[test]
fn candidate_window_protects_input_manual_values_and_separate_canonical_approval() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let path = std::env::temp_dir().join(format!(
        "paper-metadata-window-{}.sqlite",
        paper_core::model::PaperId::new()
    ));
    let window = MainWindow::new().unwrap();
    let controller = ui::attach(
        &window,
        Config {
            database: path.clone(),
        },
    );
    wait(&window);
    window.set_citation_key("Metadata2026".into());
    window.set_paper_title("My title".into());
    window.set_doi("10.1000/a".into());
    window.invoke_save();
    wait(&window);
    assert!(window.get_saved_paper(), "{}", window.get_status());
    let id = window.get_editing_id();
    window.invoke_request_navigation(NavigationAction::MetadataReview, id.clone());
    wait(&window);
    assert!(window.get_metadata_open());
    window.invoke_begin_candidate();
    let candidate_id = window.get_candidate_id();
    window.set_candidate_doi("invalid".into());
    window.set_candidate_dirty(true);
    window.invoke_submit_candidate();
    wait(&window);
    assert!(window.get_candidate_form_open());
    assert!(window.get_candidate_dirty());
    assert_eq!(window.get_candidate_id(), candidate_id);
    assert_eq!(window.get_candidate_doi(), "invalid");
    window.invoke_request_navigation(NavigationAction::OpenPaper, id.clone());
    assert!(window.get_confirm_discard());
    assert!(window.get_metadata_open());
    window.set_confirm_discard(false);
    window.set_candidate_doi("https://doi.org/10.1000/A".into());
    window.set_candidate_kind(1);
    window.set_candidate_use_title(true);
    window.set_candidate_title("Publisher title".into());
    window.set_candidate_use_year(true);
    window.set_candidate_year("2026".into());
    window.set_candidate_updated("2026-01-01T00:00:00Z".into());
    window.invoke_submit_candidate();
    wait(&window);
    assert!(!window.get_candidate_form_open(), "{}", window.get_status());
    assert!(!window.get_candidate_dirty());
    assert_eq!(window.get_candidate_rows().row_count(), 1);
    assert_eq!(window.get_version_rows().row_count(), 1);
    assert_eq!(window.get_paper_title(), "My title");
    assert_eq!(window.get_year(), "2026");
    assert_eq!(window.get_proposal_rows().row_count(), 2);
    window.invoke_resolve_metadata(proposal(&window, "正規版の切替"), true);
    wait(&window);
    assert!(window.get_version_rows().row_data(0).unwrap().canonical);
    assert_eq!(window.get_paper_title(), "My title");
    assert_eq!(window.get_proposal_rows().row_count(), 1);
    window.invoke_resolve_metadata(proposal(&window, "タイトル"), true);
    wait(&window);
    assert_eq!(window.get_paper_title(), "Publisher title");
    assert_eq!(window.get_proposal_rows().row_count(), 0);
    // A candidate without identity evidence stays held, then explicit association
    // records a separate version while retaining even previously empty manual fields.
    window.invoke_begin_candidate();
    window.set_candidate_use_title(true);
    window.set_candidate_title("Related draft".into());
    window.set_candidate_use_publisher(true);
    window.set_candidate_publisher("Draft source".into());
    window.set_candidate_kind(4);
    window.invoke_submit_candidate();
    wait(&window);
    let held = window.get_candidate_rows().iter().find(|r| r.held).unwrap();
    assert_eq!(window.get_version_rows().row_count(), 1);
    window.invoke_associate_candidate(held.id);
    wait(&window);
    assert_eq!(window.get_version_rows().row_count(), 2);
    assert_eq!(window.get_proposal_rows().row_count(), 2);
    window.invoke_resolve_metadata(proposal(&window, "タイトル"), false);
    wait(&window);
    assert_eq!(window.get_paper_title(), "Publisher title");
    window.invoke_resolve_metadata(proposal(&window, "出版社"), true);
    wait(&window);
    assert_eq!(window.get_proposal_rows().row_count(), 0);
    // A discarded candidate form must not reappear on refresh.
    window.invoke_begin_candidate();
    window.set_candidate_dirty(false);
    window.invoke_request_navigation(NavigationAction::MetadataReview, id.clone());
    wait(&window);
    assert!(!window.get_candidate_form_open());
    window.invoke_request_navigation(NavigationAction::OpenPaper, id.clone());
    wait(&window);
    assert!(!window.get_metadata_open());
    drop(controller);
    drop(window);
    let window = MainWindow::new().unwrap();
    let controller = ui::attach(
        &window,
        Config {
            database: path.clone(),
        },
    );
    wait(&window);
    window.invoke_navigate(NavigationAction::MetadataReview, id);
    wait(&window);
    assert_eq!(window.get_version_rows().row_count(), 2);
    assert_eq!(window.get_candidate_rows().row_count(), 2);
    assert_eq!(window.get_proposal_rows().row_count(), 0);
    drop(controller);
    drop(window);
    std::fs::remove_file(path).unwrap();
}

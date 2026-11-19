use paper_core::model::*;
use paper_desktop::NavigationAction;
use paper_desktop::{MainWindow, config::Config, ui};
use slint::ComponentHandle;
use std::time::{Duration, Instant};
fn wait(ui: &MainWindow) {
    let weak = ui.as_weak();
    let start = Instant::now();
    let done = std::rc::Rc::new(std::cell::Cell::new(false));
    let timer = slint::Timer::default();
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
            if !ui.get_busy() && !done.replace(true) {
                slint::quit_event_loop().unwrap();
            }
        },
    );
    slint::run_event_loop_until_quit().unwrap();
}
fn candidate() -> MetadataCandidate {
    MetadataCandidate {
        id: CandidateId::new(),
        bibliography: Bibliography {
            title: Some("Graph methods".into()),
            authors: vec![Contributor {
                name: ContributorName::Person {
                    family: "Smith".into(),
                    given: Some("Ada".into()),
                },
                orcid: None,
            }],
            identifiers: Identifiers {
                doi: Some(Doi::parse("10.1000/a").unwrap()),
                ..Default::default()
            },
            publication: Publication {
                year: Some(2024),
                ..Default::default()
            },
            ..Default::default()
        },
        present_fields: vec![
            BibliographyField::Title,
            BibliographyField::Authors,
            BibliographyField::Identifiers,
            BibliographyField::Year,
        ],
        provenance: Provenance {
            source: MetadataSource::Crossref,
            source_record_id: Some("10.1000/a".into()),
            observed_at: chrono::Utc::now(),
            source_updated_at: None,
        },
        version_kind: VersionKind::VersionOfRecord,
    }
}
fn press_enter(window: &MainWindow) {
    let key: slint::SharedString = slint::platform::Key::Return.into();
    window
        .window()
        .dispatch_event(slint::platform::WindowEvent::KeyPressed { text: key.clone() });
    window
        .window()
        .dispatch_event(slint::platform::WindowEvent::KeyReleased { text: key });
}
#[test]
fn unified_add_screen_fetches_and_registers_with_enter_only() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let path = std::env::temp_dir().join(format!("paper-unified-add-{}.sqlite", PaperId::new()));
    let window = MainWindow::new().unwrap();
    let controller = ui::attach_with_lookup_provider(
        &window,
        Config {
            database: path.clone(),
        },
        Box::new(|_| Ok(vec![candidate()])),
    );
    wait(&window);
    let labels = |label: &str| {
        i_slint_backend_testing::ElementHandle::find_by_accessible_label(&window, label).count()
    };
    // One entry point replaces the separate lookup/manual buttons.
    assert_eq!(labels("文献を追加"), 1);
    assert_eq!(labels("DOI / arXiv から登録"), 0);
    assert_eq!(labels("手動で登録"), 0);
    i_slint_backend_testing::ElementHandle::find_by_accessible_label(&window, "文献を追加")
        .next()
        .unwrap()
        .invoke_accessible_default_action();
    wait(&window);
    assert_eq!(window.get_detail_view(), paper_desktop::DetailView::Import);
    // Closing an untouched add screen returns to the neutral view.
    window.invoke_cancel_lookup();
    wait(&window);
    assert_eq!(window.get_detail_view(), paper_desktop::DetailView::Empty);
    window.invoke_request_navigation(NavigationAction::ImportPaper, "".into());
    wait(&window);
    // The identifier field is focused on open, so typing and Enter fetch.
    window
        .window()
        .dispatch_event(slint::platform::WindowEvent::KeyPressed { text: "1".into() });
    window
        .window()
        .dispatch_event(slint::platform::WindowEvent::KeyReleased { text: "1".into() });
    assert_eq!(window.get_lookup_identifier(), "1");
    window.set_lookup_identifier("10.1000/a".into());
    press_enter(&window);
    wait(&window);
    assert!(window.get_lookup_has_result(), "{}", window.get_status());
    assert_eq!(labels("登録する引用キー"), 1);
    // The citation key field takes focus with the result; Enter registers.
    press_enter(&window);
    wait(&window);
    assert!(!window.get_lookup_open(), "{}", window.get_status());
    assert_eq!(window.get_total(), 1);
    assert_eq!(
        window.get_detail_view(),
        paper_desktop::DetailView::PaperDetail
    );
    // Manual entry stays reachable from the add screen and can switch back.
    window.invoke_request_navigation(NavigationAction::ImportPaper, "".into());
    wait(&window);
    i_slint_backend_testing::ElementHandle::find_by_accessible_label(
        &window,
        "識別子なしで手動入力",
    )
    .next()
    .unwrap()
    .invoke_accessible_default_action();
    wait(&window);
    assert_eq!(
        window.get_detail_view(),
        paper_desktop::DetailView::PaperEditor
    );
    assert!(!window.get_saved_paper());
    drop(controller);
    drop(window);
    std::fs::remove_file(path).unwrap();
}

use paper_core::{model::*, ports::PaperRepository};
use paper_desktop::NavigationAction;
use paper_desktop::{MainWindow, config::Config, ui};
use slint::{ComponentHandle, Model};
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
#[test]
fn lookup_window_covers_preview_confirmation_errors_existing_updates_and_key_proposals() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let path = std::env::temp_dir().join(format!("paper-lookup-window-{}.sqlite", PaperId::new()));
    let window = MainWindow::new().unwrap();
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = calls.clone();
    let controller = ui::attach_with_lookup_provider(
        &window,
        Config {
            database: path.clone(),
        },
        Box::new(move |id| {
            observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if matches!(id,PaperIdentifier::Doi(d) if d.as_str()=="10.1000/offline") {
                return Err(paper_metadata::LookupError::Network);
            }
            Ok(vec![candidate()])
        }),
    );
    wait(&window);
    window.invoke_request_navigation(NavigationAction::ImportPaper, "".into());
    wait(&window);
    assert!(window.get_lookup_open());
    assert!(window.get_lookup_new());
    assert!(!window.get_saved_paper());
    i_slint_backend_testing::ElementHandle::find_by_accessible_label(&window, "取得する識別子")
        .next()
        .unwrap()
        .set_accessible_value("invalid");
    assert_eq!(window.get_lookup_identifier(), "invalid");
    assert!(window.get_lookup_dirty());
    window.invoke_fetch_lookup();
    wait(&window);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(!window.get_lookup_has_result());
    window.set_lookup_identifier("10.1000/a".into());
    window.invoke_fetch_lookup();
    wait(&window);
    assert!(window.get_lookup_has_result());
    assert_eq!(window.get_lookup_key(), "Smith2024");
    assert!(
        window
            .get_lookup_fields()
            .iter()
            .any(|f| f.value.contains("Ada"))
    );
    assert!(window.get_lookup_source().contains("Crossref"));
    assert_eq!(window.get_total(), 0);
    window.set_lookup_identifier("10.1000/changed".into());
    window.invoke_confirm_lookup();
    wait(&window);
    assert!(window.get_lookup_open());
    assert_eq!(window.get_total(), 0);
    window.set_lookup_identifier("10.1000/a".into());
    window.set_lookup_key("bad key".into());
    window.invoke_confirm_lookup();
    wait(&window);
    assert!(window.get_lookup_has_result());
    assert_eq!(window.get_lookup_key(), "bad key");
    window.invoke_request_navigation(NavigationAction::CreatePaper, "".into());
    assert!(window.get_confirm_discard());
    window.set_confirm_discard(false);
    window.set_lookup_key("Smith2024".into());
    window.set_lookup_notes("from lookup".into());
    window.set_lookup_reading_status(2);
    window.set_lookup_favorite(true);
    window.invoke_confirm_lookup();
    wait(&window);
    assert!(!window.get_lookup_open(), "{}", window.get_status());
    assert_eq!(window.get_total(), 1);
    assert_eq!(window.get_citation_key(), "Smith2024");
    assert_eq!(
        window.get_detail_view(),
        paper_desktop::DetailView::PaperDetail
    );
    assert_eq!(window.get_notes(), "from lookup");
    assert_eq!(window.get_reading_status(), 2);
    assert!(window.get_favorite());
    let id = window.get_editing_id();
    window.set_paper_title("My title".into());
    window.set_dirty(true);
    window.invoke_save();
    wait(&window);
    window.invoke_suggest_key();
    wait(&window);
    assert_eq!(window.get_citation_key(), "Smith2024");
    window.invoke_save();
    wait(&window);
    window.invoke_request_navigation(NavigationAction::MetadataReview, id.clone());
    wait(&window);
    window.invoke_begin_lookup();
    wait(&window);
    assert!(!window.get_lookup_new());
    window.set_lookup_identifier("10.1000/offline".into());
    window.invoke_fetch_lookup();
    wait(&window);
    assert!(window.get_lookup_open());
    assert_eq!(window.get_lookup_identifier(), "10.1000/offline");
    assert!(!window.get_lookup_has_result());
    window.set_lookup_identifier("10.1000/a".into());
    window.invoke_fetch_lookup();
    wait(&window);
    window.invoke_confirm_lookup();
    wait(&window);
    assert!(!window.get_lookup_open());
    assert!(window.get_metadata_open());
    assert_eq!(window.get_paper_title(), "My title");
    assert_eq!(window.get_citation_key(), "Smith2024");
    assert!(
        window
            .get_proposal_rows()
            .iter()
            .any(|r| r.label == "タイトル")
    );
    // Starting a new lookup clears the old editing session; cancel cannot turn
    // the previous paper into a new-paper save by accident.
    window.invoke_request_navigation(NavigationAction::OpenPaper, id);
    wait(&window);
    window.invoke_request_navigation(NavigationAction::ImportPaper, "".into());
    wait(&window);
    assert!(!window.get_saved_paper());
    window.set_lookup_identifier("10.1000/a".into());
    window.invoke_fetch_lookup();
    wait(&window);
    assert_eq!(window.get_lookup_key(), "Smith2024Graph");
    window.invoke_cancel_lookup();
    wait(&window);
    assert!(!window.get_lookup_open());
    assert!(!window.get_saved_paper());
    assert_eq!(window.get_citation_key(), "");
    drop(controller);
    drop(window);
    let store = paper_storage::SqliteLibrary::open_local(&path).unwrap();
    let result = store.search(&Default::default()).unwrap();
    assert_eq!(result.total_count, 1);
    drop(store);
    std::fs::remove_file(path).unwrap();
}

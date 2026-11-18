use i_slint_backend_testing::ElementHandle;
use paper_core::model::*;
use paper_desktop::{MainWindow, NavigationAction, config::Config, ui};
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
fn fetch(w: &MainWindow) {
    w.invoke_request_navigation(NavigationAction::ImportPaper, "".into());
    wait(w);
    w.set_lookup_identifier("10.1000/a".into());
    w.invoke_fetch_lookup();
    wait(w);
    assert!(w.get_lookup_has_result(), "{}", w.get_status());
}
fn labels(w: &MainWindow, label: &str) -> usize {
    ElementHandle::find_by_accessible_label(w, label).count()
}

#[test]
fn registering_the_same_doi_again_warns_but_stays_possible() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let path = std::env::temp_dir().join(format!("paper-dup-{}.sqlite", PaperId::new()));
    let w = MainWindow::new().unwrap();
    w.show().unwrap();
    w.window().set_size(slint::LogicalSize::new(1360., 900.));
    let controller = ui::attach_with_lookup_provider(
        &w,
        Config {
            database: path.clone(),
        },
        Box::new(|_| Ok(vec![candidate()])),
    );
    wait(&w);

    // The first registration has nothing to warn about.
    fetch(&w);
    assert_eq!(w.get_lookup_duplicates().row_count(), 0);
    assert_eq!(labels(&w, "登録"), 1);
    assert_eq!(labels(&w, "重複を承知で登録"), 0);
    w.invoke_confirm_lookup();
    wait(&w);
    assert_eq!(w.get_total(), 1, "{}", w.get_status());
    let first = w.get_editing_id();

    // The same work again: the existing paper is named, with a way to open it.
    fetch(&w);
    assert_eq!(w.get_lookup_duplicates().row_count(), 1);
    let row = w.get_lookup_duplicates().row_data(0).unwrap();
    assert_eq!(row.id, first);
    assert!(row.label.contains("Smith2024"), "{}", row.label);
    assert!(!row.deleted);
    assert_eq!(labels(&w, "重複を承知で登録"), 1);
    assert_eq!(labels(&w, "登録"), 0);
    assert_eq!(labels(&w, &format!("{}を開く", row.label)), 1);
    // The warning stays in view above the form, whatever the form scrolls to when it gets focus.
    let open = ElementHandle::find_by_accessible_label(&w, &format!("{}を開く", row.label))
        .next()
        .unwrap();
    let key = ElementHandle::find_by_accessible_label(&w, "登録する引用キー")
        .next()
        .unwrap();
    assert!(open.absolute_position().y >= 0.);
    assert!(open.absolute_position().y + open.size().height <= key.absolute_position().y);

    // A new fetch starts clean, and registering anyway is still allowed.
    w.invoke_fetch_lookup();
    assert_eq!(
        w.get_lookup_duplicates().row_count(),
        1,
        "kept until the new result"
    );
    wait(&w);
    w.invoke_confirm_lookup();
    wait(&w);
    assert_eq!(w.get_total(), 2, "{}", w.get_status());
    assert_eq!(
        w.get_lookup_duplicates().row_count(),
        0,
        "cleared after use"
    );

    // Opening the existing paper from the warning goes there (after the usual discard check).
    fetch(&w);
    assert_eq!(w.get_lookup_duplicates().row_count(), 2);
    let existing = w.get_lookup_duplicates().row_data(0).unwrap();
    w.invoke_open_paper(existing.id.clone());
    assert!(w.get_confirm_discard(), "unsaved add screen asks first");
    drop(controller);
    let _ = std::fs::remove_file(path);
}

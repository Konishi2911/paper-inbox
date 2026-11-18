use chrono::Utc;
use paper_core::{command::LibraryCommand, model::*, ports::PaperRepository};
use paper_desktop::{MainWindow, NavigationAction, config::Config, ui};
use paper_storage::SqliteLibrary;
use slint::{ComponentHandle, Model};
use std::{
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};

fn wait(window: &MainWindow) {
    let weak = window.as_weak();
    let start = Instant::now();
    let timer = slint::Timer::default();
    let completed = Rc::new(std::cell::Cell::new(false));
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(5),
        move || {
            let window = weak.upgrade().unwrap();
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "{}",
                window.get_status()
            );
            if !window.get_busy() && !completed.replace(true) {
                slint::quit_event_loop().unwrap();
            }
        },
    );
    slint::run_event_loop_until_quit().unwrap();
}
fn stored(db: &Path, id: PaperId) -> usize {
    SqliteLibrary::open_local(db)
        .unwrap()
        .attachments(id)
        .unwrap()
        .len()
}

struct Sandbox(PathBuf);
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn dropped_pdfs_attach_only_to_the_paper_on_screen() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let sandbox =
        Sandbox(std::env::temp_dir().join(format!("paper-inbox-drop-{}", PaperId::new())));
    std::fs::create_dir_all(&sandbox.0).unwrap();
    let database = sandbox.0.join("library.sqlite");
    let paper = Paper::new(
        PaperId::new(),
        CitationKey::parse("Drop2026").unwrap(),
        Bibliography {
            title: Some("Dropped".into()),
            ..Default::default()
        },
        Utc::now(),
    )
    .unwrap();
    let id = paper.id();
    SqliteLibrary::open_local(&database)
        .unwrap()
        .commit_at(LibraryCommand::CreatePaper { paper }, Utc::now())
        .unwrap();
    let good = sandbox.0.join("Upper.PDF");
    std::fs::write(&good, "%PDF-1.7\nsome pages").unwrap();
    let other = sandbox.0.join("other.pdf");
    std::fs::write(
        &other,
        "%PDF-1.7
other",
    )
    .unwrap();
    let text = sandbox.0.join("notes.txt");
    std::fs::write(&text, "%PDF-1.7").unwrap();

    let w = MainWindow::new().unwrap();
    w.show().unwrap();
    w.window().set_size(slint::LogicalSize::new(1360., 900.));
    let _controller = ui::attach_with_providers(
        &w,
        Config {
            database: database.clone(),
        },
        Box::new(|_| Ok(vec![candidate()])),
        Box::new(|| None),
    );
    wait(&w);
    let drop_file = |path: &Path| w.invoke_file_dropped(path.to_string_lossy().as_ref().into());

    // Nothing is open yet: the drop starts the add screen with the PDF waiting on it.
    assert!(!w.get_can_drop_attach());
    assert!(
        w.get_can_drop_new(),
        "{:?} ready={} dirty={}",
        w.get_detail_view(),
        w.get_ready(),
        w.get_dirty()
    );
    drop_file(&good);
    wait(&w);
    assert!(
        w.get_lookup_open() && w.get_lookup_new(),
        "{}",
        w.get_status()
    );
    assert_eq!(w.get_lookup_pdf_name(), "Upper.PDF");
    assert_eq!(stored(&database, id), 0, "not attached to another paper");
    // It can be taken back, and dropping another one on the add screen replaces it.
    w.set_lookup_pdf("".into());
    w.set_lookup_pdf_name("".into());
    drop_file(&other);
    assert_eq!(w.get_lookup_pdf_name(), "other.pdf");
    // Registering attaches the PDF to the new paper, not to any existing one.
    w.set_lookup_identifier("10.1000/a".into());
    w.invoke_fetch_lookup();
    wait(&w);
    assert!(w.get_lookup_has_result(), "{}", w.get_status());
    w.invoke_confirm_lookup();
    wait(&w);
    assert!(!w.get_lookup_open(), "{}", w.get_status());
    assert!(
        w.get_status().contains("PDFを添付しました"),
        "{}",
        w.get_status()
    );
    assert_eq!(
        w.get_attachment_rows().row_data(0).unwrap().name,
        "other.pdf"
    );
    assert_eq!(w.get_lookup_pdf_name(), "", "cleared after use");
    assert_eq!(stored(&database, id), 0);

    w.invoke_request_navigation(NavigationAction::OpenPaper, id.to_string().into());
    wait(&w);
    assert!(w.get_can_drop_attach());
    // The extension decides first, whatever the content is.
    drop_file(&text);
    assert!(w.get_status().contains(".pdf"), "{}", w.get_status());
    assert_eq!(stored(&database, id), 0);

    // The extension is matched without regard to case, and the hover frame is cleared.
    w.set_drop_hover(true);
    drop_file(&good);
    assert!(!w.get_drop_hover());
    wait(&w);
    assert_eq!(stored(&database, id), 1, "{}", w.get_status());
    assert_eq!(
        w.get_attachment_rows().row_data(0).unwrap().name,
        "Upper.PDF"
    );

    // An unsaved edit blocks the drop, like the attach button.
    w.set_dirty(true);
    assert!(!w.get_can_drop_attach());
    drop_file(&good);
    assert!(!w.get_busy());
    assert_eq!(stored(&database, id), 1);
}

fn candidate() -> MetadataCandidate {
    MetadataCandidate {
        id: CandidateId::new(),
        bibliography: Bibliography {
            title: Some("Graph methods".into()),
            identifiers: Identifiers {
                doi: Some(Doi::parse("10.1000/a").unwrap()),
                ..Default::default()
            },
            publication: Publication {
                year: Some(2024),
                ..Default::default()
            },
            authors: vec![Contributor {
                name: ContributorName::Person {
                    family: "Smith".into(),
                    given: Some("Ada".into()),
                },
                orcid: None,
            }],
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

use paper_core::{
    command::LibraryCommand,
    model::*,
    ports::LibraryStore,
    service::{BibtexOptions, BibtexWarning},
};
use paper_desktop::{
    application::{AppError, Application, PaperForm},
    export::{self, ExportError},
};
use paper_storage::SqliteLibrary;
struct Fixture {
    dir: std::path::PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("paper-export-integration-{}", PaperId::new()));
        std::fs::create_dir(&dir).unwrap();
        Self { dir }
    }
    fn db(&self) -> std::path::PathBuf {
        self.dir.join("library.sqlite")
    }
    fn open(&self) -> Application {
        Application {
            store: SqliteLibrary::open_local(self.db()).unwrap(),
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
#[test]
fn sqlite_selection_exports_to_file_without_changing_library() {
    let fixture = Fixture::new();
    let app = fixture.open();
    let form = PaperForm {
        citation_key: "Export2026".into(),
        title: "保存済みの題名".into(),
        doi: "https://doi.org/10.1000/ABC".into(),
        arxiv_id: "arXiv:2401.12345v2".into(),
        ..Default::default()
    };
    let id = app.save(None, &form).unwrap();
    let events = app.store.pending_events(10000).unwrap().len();
    let path = fixture.dir.join("selected.bib");
    let prepared = export::prepare(
        &app,
        &[id, id],
        &BibtexOptions::default(),
        &path,
        &fixture.db(),
    )
    .unwrap();
    assert_eq!(prepared.count, 1);
    assert!(!prepared.replaces_file());
    assert!(!path.exists());
    assert!(prepared.output.content.contains("10.1000/abc"));
    assert!(prepared.output.content.contains("2401.12345v2"));
    assert!(prepared.output.content.contains("保存済みの題名"));
    prepared.publish().unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        prepared.output.content
    );
    assert_eq!(app.store.pending_events(10000).unwrap().len(), events);
    let replaced =
        export::prepare(&app, &[id], &BibtexOptions::default(), &path, &fixture.db()).unwrap();
    assert!(replaced.replaces_file());
    let before = std::fs::read(&path).unwrap();
    drop(replaced);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(matches!(
        export::prepare(&app, &[], &BibtexOptions::default(), &path, &fixture.db()),
        Err(ExportError::EmptySelection)
    ));
    assert!(matches!(
        export::prepare(
            &app,
            &[id],
            &BibtexOptions::default(),
            &fixture.dir.join("wrong.sqlite"),
            &fixture.db()
        ),
        Err(ExportError::InvalidPath)
    ));
}
#[test]
fn legacy_identifiers_can_be_repaired_without_losing_unrelated_data() {
    let fixture = Fixture::new();
    let app = fixture.open();
    let now = chrono::Utc::now();
    let mut paper = Paper::new(
        PaperId::new(),
        CitationKey::parse("Legacy").unwrap(),
        Bibliography::default(),
        now,
    )
    .unwrap();
    app.store
        .commit(LibraryCommand::CreatePaper {
            paper: paper.clone(),
        })
        .unwrap();
    // Seed a legacy DB value without bypassing the new-input API.
    let conn = rusqlite::Connection::open(fixture.db()).unwrap();
    conn.execute("UPDATE bibliographies SET doi='old invalid DOI',arxiv_id='old invalid arxiv' WHERE id=(SELECT bibliography_id FROM paper_snapshots WHERE id=(SELECT snapshot_id FROM papers WHERE id=?1))",[paper.id().to_string()]).unwrap();
    let state = app.open(paper.id()).unwrap();
    let mut input = PaperForm::from_paper(&state.paper);
    assert_eq!(input.doi, "old invalid DOI");
    input.notes = "unrelated note".into();
    app.save(Some(&state), &input).unwrap();
    let state = app.open(paper.id()).unwrap();
    assert_eq!(
        state.paper.bibliography().identifiers.raw_doi.as_deref(),
        Some("old invalid DOI")
    );
    let path = fixture.dir.join("legacy.bib");
    let prepared = export::prepare(
        &app,
        &[paper.id()],
        &BibtexOptions::default(),
        &path,
        &fixture.db(),
    )
    .unwrap();
    assert_eq!(
        prepared
            .output
            .warnings
            .iter()
            .filter(|w| matches!(w, BibtexWarning::InvalidStoredIdentifier { .. }))
            .count(),
        2
    );
    assert!(!prepared.output.content.contains("old invalid"));
    let mut input = PaperForm::from_paper(&state.paper);
    input.doi = "still invalid".into();
    assert!(matches!(
        app.save(Some(&state), &input),
        Err(AppError::Input(_))
    ));
    input.doi = "DOI:10.1000/REPAIRED".into();
    input.arxiv_id = "https://arxiv.org/abs/2401.12345v2".into();
    app.save(Some(&state), &input).unwrap();
    paper = app.open(paper.id()).unwrap().paper;
    assert_eq!(
        paper
            .bibliography()
            .identifiers
            .doi
            .as_ref()
            .unwrap()
            .as_str(),
        "10.1000/repaired"
    );
    assert!(paper.bibliography().identifiers.raw_doi.is_none());
    assert_eq!(paper.metadata().notes, "unrelated note");
}
#[cfg(unix)]
#[test]
fn database_aliases_and_symbolic_links_are_not_replaced() {
    let fixture = Fixture::new();
    let app = fixture.open();
    let id = app
        .save(
            None,
            &PaperForm {
                citation_key: "Protected".into(),
                ..Default::default()
            },
        )
        .unwrap();
    let alias = fixture.dir.join("database-alias.bib");
    std::fs::hard_link(fixture.db(), &alias).unwrap();
    assert!(matches!(
        export::prepare(
            &app,
            &[id],
            &BibtexOptions::default(),
            &alias,
            &fixture.db()
        ),
        Err(ExportError::LibraryTarget)
    ));
    let real = fixture.dir.join("original.bib");
    std::fs::write(&real, "original").unwrap();
    let link = fixture.dir.join("link.bib");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    assert!(matches!(
        export::prepare(&app, &[id], &BibtexOptions::default(), &link, &fixture.db()),
        Err(ExportError::NotRegular)
    ));
    assert_eq!(std::fs::read_to_string(real).unwrap(), "original");
}

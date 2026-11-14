use paper_core::{
    command::LibraryCommand,
    model::*,
    ports::{LibraryStore, PaperRepository},
    query::*,
};
use paper_desktop::application::{AppError, Application, AuthorInput, PaperForm};
use paper_storage::SqliteLibrary;
struct Database(std::path::PathBuf);
impl Database {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("paper-ui-app-{}.sqlite", PaperId::new())))
    }
    fn open(&self) -> Application {
        Application {
            store: SqliteLibrary::open_local(&self.0).unwrap(),
        }
    }
}
impl Drop for Database {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn form(key: &str) -> PaperForm {
    PaperForm {
        citation_key: key.into(),
        title: "Original title".into(),
        year: "2026".into(),
        publication_type: 1,
        authors: vec![
            AuthorInput {
                kind: 0,
                name: "Lovelace".into(),
                given: "Ada".into(),
                orcid: "original-orcid".into(),
            },
            AuthorInput {
                kind: 1,
                name: "Research Group".into(),
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}
#[test]
fn registration_edit_and_restart_preserve_order_and_hidden_fields() {
    let db = Database::new();
    let app = db.open();
    let id = app.save(None, &form("original")).unwrap();
    let state = app.open(id).unwrap();
    let key_before = state.paper.citation_key().clone();
    app.store
        .commit(LibraryCommand::EditBibliography {
            paper_id: id,
            version_id: None,
            value: BibliographyValue::Keywords(vec!["keep keyword".into()]),
            expected_heads: vec![],
        })
        .unwrap();
    let state = app.open(id).unwrap();
    let mut input = PaperForm::from_paper(&state.paper);
    input.authors.swap(0, 1);
    input.title = "Edited".into();
    input.notes = "my notes".into();
    input.favorite = true;
    app.save(Some(&state), &input).unwrap();
    drop(app);
    let app = db.open();
    let loaded = app.open(id).unwrap();
    assert_eq!(loaded.paper.bibliography().title.as_deref(), Some("Edited"));
    assert_eq!(loaded.paper.bibliography().keywords, vec!["keep keyword"]);
    assert_eq!(loaded.paper.citation_key(), &key_before);
    assert_eq!(loaded.paper.metadata().notes, "my notes");
    assert!(loaded.paper.metadata().favorite);
    assert!(matches!(
        loaded.paper.bibliography().authors[0].name,
        ContributorName::Organization(_)
    ));
    assert_eq!(
        loaded.paper.bibliography().authors[1].orcid.as_deref(),
        Some("original-orcid")
    );
}
#[test]
fn invalid_form_and_duplicate_key_leave_input_and_storage_unchanged() {
    let db = Database::new();
    let app = db.open();
    let original = form("valid");
    let id = app.save(None, &original).unwrap();
    for bad_year in ["zero", "0", "65536", "-1"] {
        let mut input = form("new");
        input.year = bad_year.into();
        let copy = input.clone();
        assert!(matches!(app.save(None, &input), Err(AppError::Input(_))));
        assert_eq!(input, copy);
    }
    let state = app.open(id).unwrap();
    let events = app.store.pending_events(10000).unwrap().len();
    let mut bad = PaperForm::from_paper(&state.paper);
    bad.year = "bad".into();
    bad.title = "must not save".into();
    assert!(app.save(Some(&state), &bad).is_err());
    assert_eq!(app.open(id).unwrap(), state);
    assert_eq!(app.store.pending_events(10000).unwrap().len(), events);
    assert_eq!(
        app.save(None, &original).unwrap_err(),
        AppError::DuplicateKey
    );
    assert_eq!(app.list(&PaperQuery::default()).unwrap().total_count, 1);
    let mut bad = form("invalid key");
    assert!(matches!(app.save(None, &bad), Err(AppError::Input(_))));
    bad.citation_key = "good".into();
    bad.authors[0].name = " ".into();
    assert!(matches!(app.save(None, &bad), Err(AppError::Input(_))));
}
#[test]
fn stale_session_and_explicit_clears_keep_their_contracts() {
    let db = Database::new();
    let app = db.open();
    let id = app.save(None, &form("conflict")).unwrap();
    let stale = app.open(id).unwrap();
    let other = db.open();
    let state = other.open(id).unwrap();
    let mut input = PaperForm::from_paper(&state.paper);
    input.notes = "other edit".into();
    other.save(Some(&state), &input).unwrap();
    let mut input = PaperForm::from_paper(&stale.paper);
    input.title = "stale title".into();
    assert_eq!(
        app.save(Some(&stale), &input).unwrap_err(),
        AppError::Conflict
    );
    assert_eq!(
        app.open(id).unwrap().paper.bibliography().title.as_deref(),
        Some("Original title")
    );
    let state = app.open(id).unwrap();
    let mut input = PaperForm::from_paper(&state.paper);
    input.title.clear();
    app.save(Some(&state), &input).unwrap();
    let target = FieldTarget {
        paper_id: id,
        version_id: None,
        field: BibliographyField::Title,
    };
    let current = app.store.active_revisions(&target).unwrap();
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].value, BibliographyValue::Title(None));
    assert_eq!(current[0].provenance.source, MetadataSource::Manual);
}
#[test]
fn delete_and_restore_roundtrip_and_deleted_key_stays_reserved() {
    let db = Database::new();
    let app = db.open();
    let input = form("deleted");
    let id = app.save(None, &input).unwrap();
    let state = app.open(id).unwrap();
    app.set_deleted(&state, true).unwrap();
    assert_eq!(app.list(&PaperQuery::default()).unwrap().total_count, 0);
    assert_eq!(app.save(None, &input).unwrap_err(), AppError::DuplicateKey);
    let mut query = PaperQuery::default();
    query.filter.deletion = DeletionFilter::Deleted;
    assert_eq!(app.list(&query).unwrap().total_count, 1);
    let state = app.open(id).unwrap();
    app.set_deleted(&state, false).unwrap();
    assert_eq!(
        app.open(id).unwrap().paper.bibliography(),
        state.paper.bibliography()
    );
    assert_eq!(app.list(&PaperQuery::default()).unwrap().total_count, 1);
}
#[test]
fn save_without_changes_does_not_create_events_or_rewrite_blank_fields() {
    let db = Database::new();
    let app = db.open();
    let id = app.save(None, &form("nochange")).unwrap();
    let state = app.open(id).unwrap();
    let before = app.store.pending_events(10000).unwrap().len();
    app.save(Some(&state), &PaperForm::from_paper(&state.paper))
        .unwrap();
    assert_eq!(app.store.pending_events(10000).unwrap().len(), before);
    assert_eq!(app.open(id).unwrap(), state);
}

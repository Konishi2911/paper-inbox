use paper_core::{
    model::*,
    ports::{LibraryStore, PaperRepository},
};
use paper_desktop::{
    application::{AppError, Application, AuthorInput, PaperForm, QuickChange, parse_author_list},
    detail, lookup,
};
use paper_storage::SqliteLibrary;

struct Database(std::path::PathBuf);
impl Database {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("paper-form-{}.sqlite", PaperId::new())))
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

fn fetched() -> MetadataCandidate {
    let mut b = Bibliography {
        title: Some("Graph methods".into()),
        abstract_text: Some("Fetched abstract".into()),
        keywords: vec!["graphs".into(), "a, b".into()],
        ..Default::default()
    };
    b.authors = vec![Contributor {
        name: ContributorName::Person {
            given: Some("Ada".into()),
            family: "Smith".into(),
        },
        orcid: None,
    }];
    b.identifiers.doi = Some(Doi::parse("10.1000/a").unwrap());
    b.identifiers.url = Some("https://example.org/paper".into());
    b.publication = Publication {
        kind: PublicationType::JournalArticle,
        container_title: Some("Journal".into()),
        publisher: Some("Press".into()),
        year: Some(2024),
        volume: Some("30".into()),
        issue: Some("2".into()),
        pages: Some("1-10".into()),
        ..Default::default()
    };
    MetadataCandidate {
        id: CandidateId::new(),
        bibliography: b,
        present_fields: vec![
            BibliographyField::Title,
            BibliographyField::Authors,
            BibliographyField::PublicationType,
            BibliographyField::ContainerTitle,
            BibliographyField::Publisher,
            BibliographyField::Year,
            BibliographyField::Volume,
            BibliographyField::Issue,
            BibliographyField::Pages,
            BibliographyField::Identifiers,
            BibliographyField::Abstract,
            BibliographyField::Keywords,
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

fn source(app: &Application, id: PaperId, field: BibliographyField) -> MetadataSource {
    let target = FieldTarget {
        paper_id: id,
        version_id: None,
        field,
    };
    app.store.active_revisions(&target).unwrap()[0]
        .provenance
        .source
        .clone()
}

#[test]
fn form_shows_fetched_fields_and_saves_only_the_fields_that_changed() {
    let db = Database::new();
    let app = db.open();
    let id = PaperId::new();
    app.register_lookup(id, &fetched(), "Smith2024", &Default::default())
        .unwrap();
    let state = app.open(id).unwrap();
    let form = PaperForm::from_paper(&state.paper);
    assert_eq!(form.publisher, "Press");
    assert_eq!(
        (
            form.volume.as_str(),
            form.issue.as_str(),
            form.pages.as_str()
        ),
        ("30", "2", "1-10")
    );
    assert_eq!(form.url, "https://example.org/paper");
    assert_eq!(form.abstract_text, "Fetched abstract");
    assert_eq!(form.keywords, "graphs; a, b");

    // An untouched form, even with surrounding whitespace, writes nothing.
    let events = app.store.pending_events(1000).unwrap().len();
    let mut untouched = form.clone();
    untouched.publisher = " Press ".into();
    untouched.keywords = "graphs ;a, b; ".into();
    app.save(Some(&state), &untouched).unwrap();
    assert_eq!(app.store.pending_events(1000).unwrap().len(), events);

    // Only the edited field becomes a manual value; fetched provenance stays.
    let state = app.open(id).unwrap();
    let mut edited = PaperForm::from_paper(&state.paper);
    edited.volume = "31".into();
    edited.keywords = String::new();
    app.save(Some(&state), &edited).unwrap();
    let saved = app.open(id).unwrap();
    let b = saved.paper.bibliography();
    assert_eq!(b.publication.volume.as_deref(), Some("31"));
    assert!(b.keywords.is_empty(), "clearing is an explicit change");
    assert_eq!(b.publication.publisher.as_deref(), Some("Press"));
    assert_eq!(
        b.identifiers.url.as_deref(),
        Some("https://example.org/paper")
    );
    assert_eq!(
        source(&app, id, BibliographyField::Volume),
        MetadataSource::Manual
    );
    assert_eq!(
        source(&app, id, BibliographyField::Keywords),
        MetadataSource::Manual
    );
    assert_eq!(
        source(&app, id, BibliographyField::Publisher),
        MetadataSource::Crossref
    );
    assert_eq!(
        source(&app, id, BibliographyField::Abstract),
        MetadataSource::Crossref
    );
    assert_eq!(
        source(&app, id, BibliographyField::Identifiers),
        MetadataSource::Crossref
    );

    // Changing only the URL edits identifiers and keeps the DOI.
    let state = app.open(id).unwrap();
    let mut url = PaperForm::from_paper(&state.paper);
    url.url = "https://example.org/v2".into();
    app.save(Some(&state), &url).unwrap();
    let ids = app
        .open(id)
        .unwrap()
        .paper
        .bibliography()
        .identifiers
        .clone();
    assert_eq!(ids.url.as_deref(), Some("https://example.org/v2"));
    assert_eq!(ids.doi_text(), Some("10.1000/a"));
}

#[test]
fn quick_changes_apply_one_value_and_reject_stale_or_deleted_papers() {
    let db = Database::new();
    let app = db.open();
    let id = app
        .save(
            None,
            &PaperForm {
                citation_key: "Quick".into(),
                title: "Quick".into(),
                ..Default::default()
            },
        )
        .unwrap();
    let state = app.open(id).unwrap();
    app.quick_update(&state, QuickChange::Favorite(true))
        .unwrap();
    let state = app.open(id).unwrap();
    assert!(state.paper.metadata().favorite);
    app.quick_update(&state, QuickChange::ReadingStatus(ReadingStatus::Read))
        .unwrap();
    // The previous state is stale now and must not overwrite the newer value.
    assert_eq!(
        app.quick_update(&state, QuickChange::Notes("late".into())),
        Err(AppError::Conflict)
    );
    let state = app.open(id).unwrap();
    assert_eq!(state.paper.metadata().reading_status, ReadingStatus::Read);
    assert_eq!(state.paper.metadata().notes, "");
    app.quick_update(&state, QuickChange::Notes("memo".into()))
        .unwrap();
    let state = app.open(id).unwrap();
    app.quick_update(&state, QuickChange::Archived(true))
        .unwrap();
    let state = app.open(id).unwrap();
    assert!(state.paper.metadata().archived);
    assert_eq!(state.paper.metadata().notes, "memo");
    app.set_deleted(&state, true).unwrap();
    let deleted = app.open(id).unwrap();
    assert!(matches!(
        app.quick_update(&deleted, QuickChange::Favorite(false)),
        Err(AppError::Input(_))
    ));
}

#[test]
fn pasted_authors_are_split_in_order_with_explicit_and_inferred_names() {
    let authors = parse_author_list("Smith, Ada; Grace Brewster Hopper\n山田 太郎;\n\n Plato ;, ");
    let names: Vec<_> = authors
        .iter()
        .map(|a| (a.name.as_str(), a.given.as_str(), a.kind))
        .collect();
    assert_eq!(
        names,
        [
            ("Smith", "Ada", 0),
            ("Hopper", "Grace Brewster", 0),
            ("山田", "太郎", 0),
            ("Plato", "", 0),
        ]
    );
    assert!(parse_author_list(" ; \n").is_empty());
}

#[test]
fn detail_presentation_and_links_are_built_only_from_valid_values() {
    let form = PaperForm {
        container_title: "Journal".into(),
        volume: "30".into(),
        issue: "2".into(),
        pages: "1-10".into(),
        year: "2024".into(),
        publication_type: 1,
        publisher: "Press".into(),
        keywords: "graphs; a, b".into(),
        authors: (1..=5)
            .map(|i| AuthorInput {
                kind: 0,
                name: format!("F{i}"),
                given: "G".into(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    assert_eq!(
        detail::publication_summary(&form),
        "Journal 30(2), pp. 1-10, 2024"
    );
    assert_eq!(detail::publication_type(&form), "学術誌論文");
    assert_eq!(detail::publication_summary(&PaperForm::default()), "");
    assert_eq!(detail::authors_short(&form), "G F1, G F2, G F3 他2名");
    assert_eq!(detail::authors_full(&form).matches(", ").count(), 4);
    assert_eq!(detail::keywords_line(&form), "graphs · a, b");

    let long = "あ".repeat(301);
    let (preview, cut) = detail::abstract_preview(&long);
    assert!(cut);
    assert_eq!(
        preview.chars().count(),
        301,
        "300 characters and an ellipsis"
    );
    assert_eq!(detail::abstract_preview(" short "), ("short".into(), false));

    assert_eq!(
        detail::doi_url("https://doi.org/10.1000/A<B>").as_deref(),
        Some("https://doi.org/10.1000/a%3Cb%3E")
    );
    assert_eq!(detail::doi_url("not a doi"), None);
    assert_eq!(
        detail::arxiv_url("arXiv:2401.00001v2").as_deref(),
        Some("https://arxiv.org/abs/2401.00001v2")
    );
    assert_eq!(detail::arxiv_url("bad"), None);
    assert_eq!(
        detail::web_url(" https://example.org/x?q=1 ").as_deref(),
        Some("https://example.org/x?q=1")
    );
    for rejected in [
        "javascript:alert(1)",
        "file:///etc/passwd",
        "https://",
        "https://exa mple.org",
        "ftp://example.org",
        "example.org",
    ] {
        assert_eq!(detail::web_url(rejected), None, "{rejected}");
    }
    let preview = lookup::preview(&fetched());
    assert!(preview.fields.iter().any(|(_, v)| v.contains("Press")));
}

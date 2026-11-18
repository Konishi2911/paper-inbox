use paper_core::{
    command::LibraryCommand,
    model::*,
    ports::{LibraryStore, PaperRepository},
};
use paper_desktop::{
    application::{Application, PaperForm},
    lookup,
};
use paper_storage::SqliteLibrary;
fn candidate() -> MetadataCandidate {
    let mut b = Bibliography {
        title: Some("Graph methods".into()),
        ..Default::default()
    };
    b.identifiers.doi = Some(Doi::parse("10.1000/a").unwrap());
    b.publication.year = Some(2024);
    b.authors = vec![Contributor {
        name: ContributorName::Person {
            given: Some("Ada".into()),
            family: "Smith".into(),
        },
        orcid: None,
    }];
    MetadataCandidate {
        id: CandidateId::new(),
        bibliography: b,
        present_fields: vec![
            BibliographyField::Title,
            BibliographyField::Identifiers,
            BibliographyField::Year,
            BibliographyField::Authors,
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
fn confirmed_import_is_atomic_preserves_source_and_reserves_deleted_keys() {
    let path = std::env::temp_dir().join(format!("paper-lookup-app-{}.sqlite", PaperId::new()));
    let app = Application {
        store: SqliteLibrary::open_local(&path).unwrap(),
    };
    let c = candidate();
    let id = PaperId::new();
    assert_eq!(
        app.suggest_key_for(&c.bibliography, None).unwrap(),
        "Smith2024"
    );
    app.register_lookup(id, &c, "Smith2024", &Default::default())
        .unwrap();
    let state = app.open(id).unwrap();
    assert_eq!(state.paper.bibliography().title, c.bibliography.title);
    assert_eq!(state.paper.canonical_version(), None);
    let target = FieldTarget {
        paper_id: id,
        version_id: None,
        field: BibliographyField::Title,
    };
    assert_eq!(
        app.store.active_revisions(&target).unwrap()[0]
            .provenance
            .source,
        MetadataSource::Crossref
    );
    assert!(
        app.store
            .pending_proposals(id)
            .unwrap()
            .iter()
            .all(|p| matches!(p.kind, ProposalKind::CanonicalVersionChange { .. }))
    );
    let before = app.store.pending_events(1000).unwrap().len();
    app.register_lookup(id, &c, "Smith2024", &Default::default())
        .unwrap();
    assert_eq!(app.store.pending_events(1000).unwrap().len(), before);
    let other = PaperId::new();
    assert!(
        app.register_lookup(other, &candidate(), "Smith2024", &Default::default())
            .is_err()
    );
    assert!(app.store.get(other).unwrap().is_none());
    app.store
        .commit_at(
            LibraryCommand::DeletePaper { paper_id: id },
            chrono::Utc::now(),
        )
        .unwrap();
    assert_eq!(
        app.suggest_key_for(&c.bibliography, None).unwrap(),
        "Smith2024Graph"
    );
    let form = PaperForm::from_paper(&app.open(id).unwrap().paper);
    assert_eq!(app.suggest_key(&form, Some(id)).unwrap(), "Smith2024");
    // Failure after field adoption must undo the document and every event.
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("CREATE TRIGGER fail_lookup BEFORE UPDATE OF status ON update_proposals WHEN NEW.status='Accepted' BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    let failed = PaperId::new();
    assert!(
        app.register_lookup(failed, &candidate(), "Smith2024Graph", &Default::default())
            .is_err()
    );
    assert!(app.store.get(failed).unwrap().is_none());
    assert_eq!(app.store.pending_events(1000).unwrap().len(), before + 1);
    let preview = lookup::preview(&c);
    assert!(preview.source.contains("Crossref"));
    assert!(
        preview
            .fields
            .iter()
            .any(|(_, value)| value.contains("Ada"))
    );
    drop(db);
    drop(app);
    std::fs::remove_file(path).unwrap();
}
#[test]
fn unified_add_registers_personal_values_with_fetched_provenance_in_one_commit() {
    let path = std::env::temp_dir().join(format!("paper-lookup-user-{}.sqlite", PaperId::new()));
    let app = Application {
        store: SqliteLibrary::open_local(&path).unwrap(),
    };
    let input = lookup::LookupUserInput {
        notes: "Read for chapter 2".into(),
        reading_status: ReadingStatus::Reading,
        favorite: true,
        tags: vec![],
    };
    let rejected = PaperId::new();
    assert!(
        app.register_lookup(rejected, &candidate(), "bad key", &input)
            .is_err()
    );
    assert!(app.store.get(rejected).unwrap().is_none());
    assert!(app.store.pending_events(1000).unwrap().is_empty());

    let id = PaperId::new();
    app.register_lookup(id, &candidate(), "Smith2024", &input)
        .unwrap();
    let paper = app.store.get(id).unwrap().unwrap();
    assert_eq!(paper.metadata().notes, "Read for chapter 2");
    assert_eq!(paper.metadata().reading_status, ReadingStatus::Reading);
    assert!(paper.metadata().favorite);
    let target = FieldTarget {
        paper_id: id,
        version_id: None,
        field: BibliographyField::Title,
    };
    // Personal values never turn fetched bibliography into manual values.
    assert_eq!(
        app.store.active_revisions(&target).unwrap()[0]
            .provenance
            .source,
        MetadataSource::Crossref
    );
    drop(app);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn registration_creates_and_reuses_tags_in_the_same_commit() {
    let path = std::env::temp_dir().join(format!("paper-lookup-tags-{}.sqlite", PaperId::new()));
    let app = Application {
        store: SqliteLibrary::open_local(&path).unwrap(),
    };
    let first = PaperId::new();
    let input = lookup::LookupUserInput {
        // The same tag in another spelling is one tag, not two.
        tags: vec!["ML".into(), " ml ".into(), "graphs".into()],
        ..Default::default()
    };
    app.register_lookup(first, &candidate(), "Smith2024", &input)
        .unwrap();
    let names: Vec<_> = app
        .paper_tags(first)
        .unwrap()
        .into_iter()
        .map(|t| t.name)
        .collect();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"ML".to_string()) && names.contains(&"graphs".to_string()));

    // A later registration reuses the existing tag instead of creating a duplicate.
    let second = PaperId::new();
    let input = lookup::LookupUserInput {
        tags: vec!["ｍｌ".into()],
        ..Default::default()
    };
    app.register_lookup(second, &candidate(), "Smith2024Graph", &input)
        .unwrap();
    let counts = app.tag_counts(false).unwrap();
    assert_eq!(counts.len(), 2);
    assert_eq!(
        counts
            .iter()
            .find(|c| c.tag.name == "ML")
            .unwrap()
            .paper_count,
        2
    );

    // An unusable name rejects the whole registration, leaving no paper behind.
    let bad = PaperId::new();
    let input = lookup::LookupUserInput {
        tags: vec!["fine".into(), "  ".into()],
        ..Default::default()
    };
    assert!(
        app.register_lookup(bad, &candidate(), "Other2024", &input)
            .is_err()
    );
    assert!(app.store.get(bad).unwrap().is_none());
    assert_eq!(app.tag_counts(true).unwrap().len(), 2);
    drop(app);
    std::fs::remove_file(path).unwrap();
}

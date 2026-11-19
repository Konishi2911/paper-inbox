use chrono::{TimeZone, Utc};
use paper_core::{
    command::{EditError, LibraryCommand},
    model::*,
    ports::{LibraryStore, PaperRepository},
    query::*,
};
use paper_storage::{SqliteLibrary, StorageError};
use rusqlite::Connection;

struct Database {
    path: std::path::PathBuf,
    device: DeviceId,
}
impl Database {
    fn new() -> Self {
        Self {
            path: std::env::temp_dir().join(format!("paper-inbox-test-{}.sqlite", PaperId::new())),
            device: DeviceId::new(),
        }
    }
    fn open(&self) -> SqliteLibrary {
        SqliteLibrary::open(&self.path, self.device).unwrap()
    }
    fn raw(&self) -> Connection {
        Connection::open(&self.path).unwrap()
    }
    fn counts(&self) -> Vec<i64> {
        let db = self.raw();
        [
            "papers",
            "paper_snapshots",
            "bibliographies",
            "contributors",
            "keywords",
            "revisions",
            "field_heads",
            "events",
        ]
        .iter()
        .map(|t| {
            db.query_row(&format!("SELECT count(*) FROM {t}"), [], |r| r.get(0))
                .unwrap()
        })
        .collect()
    }
}
impl Drop for Database {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
fn now(n: i64) -> chrono::DateTime<Utc> {
    Utc.timestamp_opt(1_000 + n, 0).unwrap()
}
fn paper(key: &str) -> Paper {
    let bibliography = Bibliography {
        title: Some("A study of SQL: 100%_ coverage".into()),
        authors: vec![
            Contributor {
                name: ContributorName::Person {
                    given: Some("Ada".into()),
                    family: "Lovelace".into(),
                },
                orcid: Some("example-orcid".into()),
            },
            Contributor {
                name: ContributorName::Organization("Research Group".into()),
                orcid: None,
            },
        ],
        editors: vec![Contributor {
            name: ContributorName::Literal("Editorial board".into()),
            orcid: None,
        }],
        publication: Publication {
            kind: PublicationType::JournalArticle,
            container_title: Some("Journal".into()),
            publisher: Some("Publisher".into()),
            year: Some(2026),
            month: Some(11),
            volume: Some("12".into()),
            issue: Some("3".into()),
            pages: Some("1--10".into()),
            article_number: Some("e42".into()),
        },
        identifiers: Identifiers {
            doi: Some(
                serde::Deserialize::deserialize(serde::de::value::StringDeserializer::<
                    serde::de::value::Error,
                >::new("10.1000/example".into()))
                .unwrap(),
            ),
            arxiv_id: Some(
                serde::Deserialize::deserialize(serde::de::value::StringDeserializer::<
                    serde::de::value::Error,
                >::new("2401.12345v2".into()))
                .unwrap(),
            ),
            isbn: Some("isbn".into()),
            issn: Some("issn".into()),
            url: Some("https://example.test/paper".into()),
            ..Default::default()
        },
        abstract_text: Some("日本語の概要".into()),
        keywords: vec!["Rust".into(), "SQL".into(), "Rust".into()],
    };
    Paper::new(
        PaperId::new(),
        CitationKey::parse(key).unwrap(),
        bibliography,
        now(0),
    )
    .unwrap()
}
fn create(store: &SqliteLibrary, p: &Paper) {
    store
        .commit_at(LibraryCommand::CreatePaper { paper: p.clone() }, now(0))
        .unwrap();
}
fn target(p: &Paper) -> FieldTarget {
    FieldTarget {
        paper_id: p.id(),
        version_id: None,
        field: BibliographyField::Title,
    }
}
fn edit(
    store: &SqliteLibrary,
    p: &Paper,
    heads: Vec<RevisionId>,
    text: &str,
    n: i64,
) -> paper_storage::Result<paper_core::command::CommitResult> {
    store.commit_at(
        LibraryCommand::EditBibliography {
            paper_id: p.id(),
            version_id: None,
            value: BibliographyValue::Title(Some(text.into())),
            expected_heads: heads,
        },
        now(n),
    )
}
#[test]
fn normalized_roundtrip_survives_reopen_and_preserves_order() {
    let db = Database::new();
    let p = paper("Ada2026");
    {
        let store = db.open();
        create(&store, &p);
    }
    let store = db.open();
    let loaded = store.get(p.id()).unwrap().unwrap();
    assert_eq!(loaded.bibliography(), p.bibliography());
    assert_eq!(loaded.metadata(), p.metadata());
    assert_eq!(loaded.citation_key(), p.citation_key());
    assert_eq!(loaded.created_at(), p.created_at());
    let conn = db.raw();
    let people:i64=conn.query_row("SELECT count(*) FROM contributors WHERE bibliography_id=(SELECT bibliography_id FROM paper_snapshots WHERE id=(SELECT snapshot_id FROM papers WHERE id=?1))",[p.id().to_string()],|r|r.get(0)).unwrap();
    assert_eq!(people, 3);
    let foreign_keys: i64 = conn
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(foreign_keys, 0);
    assert_eq!(store.active_revisions(&target(&p)).unwrap().len(), 1);
}
#[test]
fn stale_edits_are_rejected_across_independent_connections() {
    let db = Database::new();
    let a = db.open();
    let b = db.open();
    let p = paper("one");
    create(&a, &p);
    let head = a.active_revisions(&target(&p)).unwrap()[0].id;
    let result = edit(&a, &p, vec![head], "new", 1).unwrap();
    let counts = db.counts();
    assert!(matches!(
        edit(&b, &p, vec![head], "stale", 2),
        Err(StorageError::Edit(EditError::Conflict { .. }))
    ));
    assert_eq!(db.counts(), counts);
    let current = b.active_revisions(&target(&p)).unwrap();
    assert_eq!(current[0].id, result.created_revisions[0]);
    assert_eq!(current[0].parents, vec![head]);
    assert_eq!(b.revisions(&target(&p)).unwrap().len(), 2);
    assert_eq!(
        b.get(p.id())
            .unwrap()
            .unwrap()
            .bibliography()
            .title
            .as_deref(),
        Some("new")
    );
}
#[test]
fn deleted_keys_remain_reserved_and_restore_is_lossless() {
    let db = Database::new();
    let store = db.open();
    let p = paper("reserved");
    create(&store, &p);
    store
        .commit_at(LibraryCommand::DeletePaper { paper_id: p.id() }, now(1))
        .unwrap();
    assert_eq!(store.search(&PaperQuery::default()).unwrap().total_count, 0);
    let counts = db.counts();
    assert!(matches!(
        store.commit_at(
            LibraryCommand::CreatePaper {
                paper: paper("reserved")
            },
            now(2)
        ),
        Err(StorageError::DuplicateCitationKey(_))
    ));
    assert_eq!(db.counts(), counts);
    let heads = store
        .active_revisions(&target(&p))
        .unwrap()
        .iter()
        .map(|r| r.id)
        .collect();
    assert!(edit(&store, &p, heads, "deleted edit", 2).is_err());
    store
        .commit_at(LibraryCommand::RestorePaper { paper_id: p.id() }, now(3))
        .unwrap();
    assert_eq!(
        store.get(p.id()).unwrap().unwrap().bibliography(),
        p.bibliography()
    );
    assert_eq!(store.search(&PaperQuery::default()).unwrap().total_count, 1);
}
#[test]
fn late_sql_failure_rolls_back_paper_history_and_outbox() {
    let db = Database::new();
    let store = db.open();
    let p = paper("rollback");
    create(&store, &p);
    let head = store.active_revisions(&target(&p)).unwrap()[0].id;
    let counts = db.counts();
    db.raw().execute_batch("CREATE TRIGGER reject_event BEFORE INSERT ON events BEGIN SELECT RAISE(ABORT,'injected failure'); END;").unwrap();
    assert!(matches!(
        edit(&store, &p, vec![head], "must roll back", 1),
        Err(StorageError::Sql(_))
    ));
    assert_eq!(db.counts(), counts);
    assert_eq!(
        store.get(p.id()).unwrap().unwrap().bibliography(),
        p.bibliography()
    );
    assert_eq!(store.active_revisions(&target(&p)).unwrap()[0].id, head);
    db.raw()
        .execute_batch("DROP TRIGGER reject_event;")
        .unwrap();
    edit(&store, &p, vec![head], "succeeds", 2).unwrap();
    let events = store.pending_events(1000).unwrap();
    assert!(
        events
            .windows(2)
            .all(|pair| pair[1].sequence == pair[0].sequence + 1)
    );
}
#[test]
fn event_payloads_are_immutable_and_upload_ack_is_idempotent() {
    let db = Database::new();
    let store = db.open();
    let p = paper("events");
    create(&store, &p);
    let original = store.pending_events(1000).unwrap();
    let head = store.active_revisions(&target(&p)).unwrap()[0].id;
    edit(&store, &p, vec![head], "changed", 1).unwrap();
    let events = store.pending_events(1000).unwrap();
    match &events[0].operation {
        SyncOperation::PaperCreated(saved) => assert_eq!(saved.bibliography(), p.bibliography()),
        _ => panic!("wrong operation"),
    }
    for pair in events.windows(2) {
        assert_eq!(pair[1].parents, vec![pair[0].id]);
        assert_eq!(pair[1].sequence, pair[0].sequence + 1);
    }
    let ids: Vec<_> = original.iter().map(|e| e.id).collect();
    store.mark_events_uploaded(&ids).unwrap();
    store.mark_events_uploaded(&ids).unwrap();
    assert_eq!(store.pending_events(1000).unwrap().len(), 2);
    drop(store);
    let reopened = db.open();
    assert_eq!(reopened.pending_events(1).unwrap().len(), 1);
}
#[test]
fn query_count_pagination_and_filters_agree() {
    let db = Database::new();
    let store = db.open();
    let a = paper("A");
    let b = paper("B");
    create(&store, &a);
    create(&store, &b);
    let query = PaperQuery {
        text: Some("lovelace".into()),
        pagination: Pagination {
            limit: 1,
            offset: 1,
        },
        ..Default::default()
    };
    let result = store.search(&query).unwrap();
    assert_eq!(result.total_count, 2);
    assert_eq!(result.papers.len(), 1);
    let mut query = PaperQuery {
        text: Some("100%_".into()),
        ..Default::default()
    };
    assert_eq!(store.search(&query).unwrap().total_count, 2);
    query.filter.year_from = Some(2027);
    assert_eq!(store.search(&query).unwrap().total_count, 0);
    store
        .commit_at(
            LibraryCommand::SetArchived {
                paper_id: a.id(),
                archived: true,
            },
            now(1),
        )
        .unwrap();
    assert_eq!(store.search(&PaperQuery::default()).unwrap().total_count, 1);
    let mut query = PaperQuery::default();
    query.filter.include_archived = true;
    query.pagination.limit = 0;
    let result = store.search(&query).unwrap();
    assert_eq!(result.total_count, 2);
    assert!(result.papers.is_empty());
}

#[test]
fn every_bibliography_field_roundtrips_with_revision_history() {
    let db = Database::new();
    let store = db.open();
    let p = paper("fields");
    create(&store, &p);
    let original = p.bibliography().clone();
    let values = vec![
        BibliographyValue::Title(None),
        BibliographyValue::Authors(vec![]),
        BibliographyValue::Editors(vec![]),
        BibliographyValue::PublicationType(PublicationType::Book),
        BibliographyValue::ContainerTitle(None),
        BibliographyValue::Publisher(None),
        BibliographyValue::Year(None),
        BibliographyValue::Month(None),
        BibliographyValue::Volume(None),
        BibliographyValue::Issue(None),
        BibliographyValue::Pages(None),
        BibliographyValue::ArticleNumber(None),
        BibliographyValue::Identifiers(Identifiers::default()),
        BibliographyValue::Abstract(None),
        BibliographyValue::Keywords(vec!["new".into()]),
    ];
    let mut expected = original.clone();
    for (i, value) in values.into_iter().enumerate() {
        let target = FieldTarget {
            paper_id: p.id(),
            version_id: None,
            field: value.field(),
        };
        let old = store.active_revisions(&target).unwrap();
        assert_eq!(old.len(), 1);
        expected.apply(&value);
        let result = store
            .commit_at(
                LibraryCommand::EditBibliography {
                    paper_id: p.id(),
                    version_id: None,
                    value: value.clone(),
                    expected_heads: vec![old[0].id],
                },
                now(i as i64 + 1),
            )
            .unwrap();
        let loaded = store.get(p.id()).unwrap().unwrap();
        assert_eq!(loaded.bibliography(), &expected);
        let active = store.active_revisions(&target).unwrap();
        assert_eq!(active[0].value, value);
        assert_eq!(active[0].id, result.created_revisions[0]);
        assert_eq!(active[0].parents, vec![old[0].id]);
        let history = store.revisions(&target).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0], old[0]);
    }
    let events = store.pending_events(1000).unwrap();
    let SyncOperation::PaperCreated(first) = &events[0].operation else {
        panic!("not creation")
    };
    assert_eq!(first.bibliography(), &original);
}
#[test]
fn metadata_and_key_changes_persist_and_duplicate_key_changes_are_atomic() {
    let db = Database::new();
    let store = db.open();
    let p = paper("first");
    let other = paper("second");
    create(&store, &p);
    create(&store, &other);
    let counts = db.counts();
    assert!(matches!(
        store.commit_at(
            LibraryCommand::ChangeCitationKey {
                paper_id: p.id(),
                citation_key: other.citation_key().clone()
            },
            now(1)
        ),
        Err(StorageError::DuplicateCitationKey(_))
    ));
    assert_eq!(db.counts(), counts);
    for command in [
        LibraryCommand::UpdateNotes {
            paper_id: p.id(),
            notes: "local memo".into(),
        },
        LibraryCommand::SetReadingStatus {
            paper_id: p.id(),
            status: ReadingStatus::Read,
        },
        LibraryCommand::SetFavorite {
            paper_id: p.id(),
            favorite: true,
        },
        LibraryCommand::SetArchived {
            paper_id: p.id(),
            archived: true,
        },
        LibraryCommand::ChangeCitationKey {
            paper_id: p.id(),
            citation_key: CitationKey::parse("renamed").unwrap(),
        },
    ] {
        store.commit_at(command, now(2)).unwrap();
    }
    drop(store);
    let store = db.open();
    let loaded = store.get(p.id()).unwrap().unwrap();
    assert_eq!(loaded.metadata().notes, "local memo");
    assert_eq!(loaded.metadata().reading_status, ReadingStatus::Read);
    assert!(loaded.metadata().favorite && loaded.metadata().archived);
    assert_eq!(loaded.citation_key().as_str(), "renamed");
    assert_eq!(loaded.bibliography(), p.bibliography());
    let events = store.pending_events(1000).unwrap();
    assert!(events.iter().any(
        |e| matches!(&e.operation,SyncOperation::NotesChanged { notes,.. } if notes=="local memo")
    ));
}
#[test]
fn invalid_or_unsupported_commands_leave_no_partial_data() {
    let db = Database::new();
    let store = db.open();
    let p = paper("invalid");
    create(&store, &p);
    let counts = db.counts();
    let target = FieldTarget {
        paper_id: p.id(),
        version_id: None,
        field: BibliographyField::Month,
    };
    let head = store.active_revisions(&target).unwrap()[0].id;
    let bad = LibraryCommand::EditBibliography {
        paper_id: p.id(),
        version_id: None,
        value: BibliographyValue::Month(Some(13)),
        expected_heads: vec![head],
    };
    assert!(store.commit_at(bad, now(1)).is_err());
    assert_eq!(db.counts(), counts);
    let tagged = Paper::from_parts(
        PaperId::new(),
        CitationKey::parse("tagged").unwrap(),
        Bibliography::default(),
        None,
        UserMetadata {
            tags: vec![TagId::new()],
            ..Default::default()
        },
        now(0),
        now(0),
        None,
    )
    .unwrap();
    assert!(matches!(
        store.commit_at(LibraryCommand::CreatePaper { paper: tagged }, now(1)),
        Err(StorageError::Unsupported(_))
    ));
    assert_eq!(db.counts(), counts);
}

#[test]
fn simultaneous_writers_cannot_both_replace_the_same_head() {
    let db = Database::new();
    let store = db.open();
    let p = paper("race");
    create(&store, &p);
    let head = store.active_revisions(&target(&p)).unwrap()[0].id;
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|i| {
            let other = db.open();
            let p = p.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                edit(&other, &p, vec![head], &format!("writer-{i}"), 1)
            })
        })
        .collect();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Err(StorageError::Edit(EditError::Conflict { .. }))))
            .count(),
        1
    );
    assert_eq!(store.revisions(&target(&p)).unwrap().len(), 2);
}

#[test]
fn unset_initial_fields_can_be_completed_but_explicit_clears_are_protected() {
    use paper_core::service::{UpdateDecision, evaluate_update};
    let db = Database::new();
    let store = db.open();
    let p = Paper::new(
        PaperId::new(),
        CitationKey::parse("unset").unwrap(),
        Bibliography::default(),
        now(0),
    )
    .unwrap();
    let result = store
        .commit_at(LibraryCommand::CreatePaper { paper: p.clone() }, now(0))
        .unwrap();
    assert!(result.created_revisions.is_empty());
    assert_eq!(result.created_events.len(), 1);
    let fields = vec![
        BibliographyValue::Title(Some("external title".into())),
        BibliographyValue::Authors(paper("sample").bibliography().authors.clone()),
        BibliographyValue::Editors(paper("sample").bibliography().editors.clone()),
        BibliographyValue::PublicationType(PublicationType::JournalArticle),
        BibliographyValue::ContainerTitle(Some("journal".into())),
        BibliographyValue::Publisher(Some("publisher".into())),
        BibliographyValue::Year(Some(2026)),
        BibliographyValue::Month(Some(11)),
        BibliographyValue::Volume(Some("1".into())),
        BibliographyValue::Issue(Some("2".into())),
        BibliographyValue::Pages(Some("1--3".into())),
        BibliographyValue::ArticleNumber(Some("42".into())),
        BibliographyValue::Identifiers(Identifiers {
            url: Some("https://example.test".into()),
            ..Default::default()
        }),
        BibliographyValue::Abstract(Some("abstract".into())),
        BibliographyValue::Keywords(vec!["SQL".into()]),
    ];
    for value in fields {
        let target = FieldTarget {
            paper_id: p.id(),
            version_id: None,
            field: value.field(),
        };
        let mut candidate = FieldRevision {
            id: RevisionId::new(),
            paper_id: p.id(),
            version_id: None,
            parents: vec![],
            value: value.clone(),
            provenance: Provenance {
                source: MetadataSource::Crossref,
                source_record_id: None,
                observed_at: now(2),
                source_updated_at: None,
            },
            created_at: now(2),
        };
        assert!(store.active_revisions(&target).unwrap().is_empty());
        assert_eq!(
            evaluate_update(None, &candidate).unwrap(),
            UpdateDecision::Apply
        );
        // Explicitly writing the currently empty value counts as a manual clear.
        let cleared = match value {
            BibliographyValue::Title(_) => BibliographyValue::Title(None),
            BibliographyValue::Authors(_) => BibliographyValue::Authors(vec![]),
            BibliographyValue::Editors(_) => BibliographyValue::Editors(vec![]),
            BibliographyValue::PublicationType(_) => {
                BibliographyValue::PublicationType(PublicationType::Unknown)
            }
            BibliographyValue::ContainerTitle(_) => BibliographyValue::ContainerTitle(None),
            BibliographyValue::Publisher(_) => BibliographyValue::Publisher(None),
            BibliographyValue::Year(_) => BibliographyValue::Year(None),
            BibliographyValue::Month(_) => BibliographyValue::Month(None),
            BibliographyValue::Volume(_) => BibliographyValue::Volume(None),
            BibliographyValue::Issue(_) => BibliographyValue::Issue(None),
            BibliographyValue::Pages(_) => BibliographyValue::Pages(None),
            BibliographyValue::ArticleNumber(_) => BibliographyValue::ArticleNumber(None),
            BibliographyValue::Identifiers(_) => {
                BibliographyValue::Identifiers(Identifiers::default())
            }
            BibliographyValue::Abstract(_) => BibliographyValue::Abstract(None),
            BibliographyValue::Keywords(_) => BibliographyValue::Keywords(vec![]),
        };
        store
            .commit_at(
                LibraryCommand::EditBibliography {
                    paper_id: p.id(),
                    version_id: None,
                    value: cleared,
                    expected_heads: vec![],
                },
                now(1),
            )
            .unwrap();
        let current = store.active_revisions(&target).unwrap();
        assert_eq!(current.len(), 1);
        candidate.parents = vec![current[0].id];
        assert_eq!(
            evaluate_update(Some(&current[0]), &candidate).unwrap(),
            UpdateDecision::Propose(ProposalReason::OverwriteManualValue)
        );
    }
    drop(store);
    let store = db.open();
    let current = store.active_revisions(&target(&p)).unwrap();
    assert_eq!(current.len(), 1);
    assert_eq!(current[0].value, BibliographyValue::Title(None));
    assert_eq!(store.revisions(&target(&p)).unwrap().len(), 1);
}

#[test]
fn clearing_a_supplied_value_keeps_its_history_and_blocks_external_overwrite() {
    use paper_core::service::{UpdateDecision, evaluate_update};
    let db = Database::new();
    let store = db.open();
    let p = paper("cleared");
    create(&store, &p);
    let head = store.active_revisions(&target(&p)).unwrap()[0].id;
    let result = store
        .commit_at(
            LibraryCommand::EditBibliography {
                paper_id: p.id(),
                version_id: None,
                value: BibliographyValue::Title(None),
                expected_heads: vec![head],
            },
            now(1),
        )
        .unwrap();
    drop(store);
    let store = db.open();
    let current = store.active_revisions(&target(&p)).unwrap();
    assert_eq!(current[0].id, result.created_revisions[0]);
    assert_eq!(current[0].parents, vec![head]);
    assert_eq!(current[0].value, BibliographyValue::Title(None));
    assert_eq!(store.revisions(&target(&p)).unwrap().len(), 2);
    let mut external = current[0].clone();
    external.provenance.source = MetadataSource::Publisher;
    external.value = BibliographyValue::Title(Some("refill".into()));
    assert_eq!(
        evaluate_update(Some(&current[0]), &external).unwrap(),
        UpdateDecision::Propose(ProposalReason::OverwriteManualValue)
    );
}

#[test]
fn batch_save_is_atomic_and_detects_document_and_head_changes() {
    let db = Database::new();
    let store = db.open();
    let p = paper("batch");
    let other = paper("taken");
    create(&store, &p);
    create(&store, &other);
    let state = store.load_edit_state(p.id()).unwrap();
    let head = store.active_revisions(&target(&p)).unwrap()[0].id;
    let counts = db.counts();
    let commands = vec![
        LibraryCommand::EditBibliography {
            paper_id: p.id(),
            version_id: None,
            value: BibliographyValue::Title(Some("partial".into())),
            expected_heads: vec![head],
        },
        LibraryCommand::ChangeCitationKey {
            paper_id: p.id(),
            citation_key: other.citation_key().clone(),
        },
    ];
    assert!(matches!(
        store.commit_batch_at(commands, std::slice::from_ref(&state), now(1)),
        Err(StorageError::DuplicateCitationKey(_))
    ));
    assert_eq!(db.counts(), counts);
    assert_eq!(store.get(p.id()).unwrap().unwrap(), state.paper);
    // A new revision with identical content/time must still invalidate the edit state.
    store
        .commit_at(
            LibraryCommand::EditBibliography {
                paper_id: p.id(),
                version_id: None,
                value: BibliographyValue::Title(p.bibliography().title.clone()),
                expected_heads: vec![head],
            },
            now(0),
        )
        .unwrap();
    assert_eq!(store.get(p.id()).unwrap().unwrap(), state.paper);
    assert!(matches!(
        store.commit_batch_at(vec![], &[state], now(1)),
        Err(StorageError::Conflict(_))
    ));
    let state = store.load_edit_state(p.id()).unwrap();
    store
        .commit_at(
            LibraryCommand::UpdateNotes {
                paper_id: p.id(),
                notes: "other writer".into(),
            },
            now(2),
        )
        .unwrap();
    assert!(matches!(
        store.commit_batch_at(
            vec![LibraryCommand::SetFavorite {
                paper_id: p.id(),
                favorite: true
            }],
            &[state],
            now(3)
        ),
        Err(StorageError::Conflict(_))
    ));
    assert!(!store.get(p.id()).unwrap().unwrap().metadata().favorite);
}
#[test]
fn local_identity_persists_and_v1_upgrade_keeps_data() {
    let db = Database::new();
    let p = paper("migration");
    {
        let store = db.open();
        create(&store, &p);
    }
    db.raw()
        .execute_batch("DROP TABLE local_settings; PRAGMA user_version=1;")
        .unwrap();
    let local = SqliteLibrary::open_local(&db.path).unwrap();
    let device = local.device_id();
    assert_eq!(
        local.get(p.id()).unwrap().unwrap().bibliography(),
        p.bibliography()
    );
    assert!(!local.pending_events(1000).unwrap().is_empty());
    drop(local);
    let reopened = SqliteLibrary::open_local(&db.path).unwrap();
    assert_eq!(reopened.device_id(), device);
    let version: i64 = db
        .raw()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 6);
    drop(reopened);
    db.raw().pragma_update(None, "user_version", 99).unwrap();
    assert!(SqliteLibrary::open_local(&db.path).is_err());
    let version: i64 = db
        .raw()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 99);
}
#[test]
fn deleted_only_and_all_queries_respect_their_contract() {
    let db = Database::new();
    let store = db.open();
    let p = paper("trash");
    let other = paper("active");
    create(&store, &p);
    create(&store, &other);
    store
        .commit_at(LibraryCommand::DeletePaper { paper_id: p.id() }, now(1))
        .unwrap();
    let mut query = PaperQuery::default();
    query.filter.deletion = DeletionFilter::Deleted;
    let result = store.search(&query).unwrap();
    assert_eq!(result.total_count, 1);
    assert_eq!(result.papers[0].id, p.id());
    query.filter.deletion = DeletionFilter::All;
    assert_eq!(store.search(&query).unwrap().total_count, 2);
}

#[test]
fn invalid_legacy_identifiers_remain_readable_and_survive_unrelated_edits() {
    let db = Database::new();
    let store = db.open();
    let p = paper("legacy-identifiers");
    create(&store, &p);
    let conn = db.raw();
    let bibliography:String=conn.query_row("SELECT bibliography_id FROM paper_snapshots WHERE id=(SELECT snapshot_id FROM papers WHERE id=?1)",[p.id().to_string()],|r|r.get(0)).unwrap();
    conn.execute(
        "UPDATE bibliographies SET doi='bad DOI',arxiv_id='bad arxiv' WHERE id=?1",
        [&bibliography],
    )
    .unwrap();
    let state = store.load_edit_state(p.id()).unwrap();
    let ids = &state.paper.bibliography().identifiers;
    assert!(ids.doi.is_none() && ids.arxiv_id.is_none());
    assert_eq!(ids.raw_doi.as_deref(), Some("bad DOI"));
    assert_eq!(ids.raw_arxiv_id.as_deref(), Some("bad arxiv"));
    assert!(!store.pending_events(1000).unwrap().is_empty());
    store
        .commit_at(
            LibraryCommand::UpdateNotes {
                paper_id: p.id(),
                notes: "keep legacy values".into(),
            },
            now(1),
        )
        .unwrap();
    let loaded = store.get(p.id()).unwrap().unwrap();
    assert_eq!(
        loaded.bibliography().identifiers.raw_doi.as_deref(),
        Some("bad DOI")
    );
    let current:Option<String>=conn.query_row("SELECT doi FROM bibliographies WHERE id=(SELECT bibliography_id FROM paper_snapshots WHERE id=(SELECT snapshot_id FROM papers WHERE id=?1))",[p.id().to_string()],|r|r.get(0)).unwrap();
    assert_eq!(current.as_deref(), Some("bad DOI"));
    let target = FieldTarget {
        paper_id: p.id(),
        version_id: None,
        field: BibliographyField::Identifiers,
    };
    let head = store.active_revisions(&target).unwrap()[0].id;
    assert!(
        store
            .commit_at(
                LibraryCommand::EditBibliography {
                    paper_id: p.id(),
                    version_id: None,
                    value: BibliographyValue::Identifiers(
                        loaded.bibliography().identifiers.clone()
                    ),
                    expected_heads: vec![head]
                },
                now(2)
            )
            .is_err()
    );
    let mut repaired = loaded.bibliography().identifiers.clone();
    repaired.raw_doi = None;
    repaired.raw_arxiv_id = None;
    repaired.doi = Some(Doi::parse(" DOI:10.1000/REPAIRED ").unwrap());
    store
        .commit_at(
            LibraryCommand::EditBibliography {
                paper_id: p.id(),
                version_id: None,
                value: BibliographyValue::Identifiers(repaired),
                expected_heads: vec![head],
            },
            now(2),
        )
        .unwrap();
    assert_eq!(
        store
            .get(p.id())
            .unwrap()
            .unwrap()
            .bibliography()
            .identifiers
            .doi
            .as_ref()
            .unwrap()
            .as_str(),
        "10.1000/repaired"
    );
    let old: String = conn
        .query_row(
            "SELECT doi FROM bibliographies WHERE id=?1",
            [&bibliography],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(old, "bad DOI");
    let previous = store.revisions(&target).unwrap();
    assert_eq!(
        previous[0].value,
        BibliographyValue::Identifiers(ids.clone())
    );
}

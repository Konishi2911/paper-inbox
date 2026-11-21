use chrono::{DateTime, TimeZone, Utc};
use paper_core::{
    command::LibraryCommand,
    model::*,
    ports::{LibraryStore, PaperRepository},
    query::*,
};
use paper_storage::{SqliteLibrary, StorageError};

fn now(n: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(1_000 + n, 0).unwrap()
}
fn paper(key: &str) -> Paper {
    Paper::new(
        PaperId::new(),
        CitationKey::parse(key).unwrap(),
        Bibliography {
            title: Some(key.into()),
            ..Default::default()
        },
        now(0),
    )
    .unwrap()
}
fn create(store: &SqliteLibrary, p: &Paper) {
    store
        .commit_at(LibraryCommand::CreatePaper { paper: p.clone() }, now(0))
        .unwrap();
}
fn tag(store: &SqliteLibrary, name: &str) -> TagId {
    let tag = Tag::new(TagId::new(), name).unwrap();
    let id = tag.id;
    store
        .commit_at(LibraryCommand::CreateTag { tag }, now(1))
        .unwrap();
    id
}
fn add(store: &SqliteLibrary, p: &Paper, t: TagId) -> Result<(), StorageError> {
    store
        .commit_at(
            LibraryCommand::AddTag {
                paper_id: p.id(),
                tag_id: t,
            },
            now(2),
        )
        .map(|_| ())
}
fn find(store: &SqliteLibrary, tags: Vec<TagId>, tag_match: TagMatch) -> Vec<PaperId> {
    let mut found: Vec<_> = store
        .search(&PaperQuery {
            filter: PaperFilter {
                tags,
                tag_match,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap()
        .papers
        .into_iter()
        .map(|p| p.id)
        .collect();
    found.sort();
    found
}
fn sorted(mut ids: Vec<PaperId>) -> Vec<PaperId> {
    ids.sort();
    ids
}

#[test]
fn names_are_unique_after_folding_and_listed_by_name() {
    let store = SqliteLibrary::in_memory(DeviceId::new()).unwrap();
    let ml = tag(&store, "  ML ");
    for same in ["ml", "ＭＬ", "M L".replace(' ', "").as_str()] {
        let duplicate = store.commit_at(
            LibraryCommand::CreateTag {
                tag: Tag::new(TagId::new(), same).unwrap(),
            },
            now(1),
        );
        assert!(
            matches!(duplicate, Err(StorageError::InvalidOperation(_))),
            "{same}"
        );
    }
    tag(&store, "graphs");
    let names: Vec<_> = store
        .tags(false)
        .unwrap()
        .into_iter()
        .map(|t| t.tag.name)
        .collect();
    assert_eq!(
        names,
        ["graphs", "ML"],
        "display name keeps the first spelling"
    );
    assert_eq!(store.tags(false).unwrap()[1].tag.id, ml);
    // A raw command with an unusable name is refused too, not just the helper.
    let bad = store.commit_at(
        LibraryCommand::CreateTag {
            tag: Tag {
                id: TagId::new(),
                name: " ".into(),
            },
        },
        now(1),
    );
    assert!(matches!(bad, Err(StorageError::InvalidOperation(_))));
    assert_eq!(store.tags(false).unwrap().len(), 2);
}

#[test]
fn assigning_removing_and_refusing_repeats_are_recorded_as_events() {
    let store = SqliteLibrary::in_memory(DeviceId::new()).unwrap();
    let p = paper("Tagged2026");
    create(&store, &p);
    let t = tag(&store, "ml");
    add(&store, &p, t).unwrap();
    assert_eq!(store.paper_tags(p.id()).unwrap()[0].id, t);
    assert!(matches!(
        add(&store, &p, t),
        Err(StorageError::InvalidOperation(_))
    ));
    assert!(matches!(
        add(&store, &p, TagId::new()),
        Err(StorageError::InvalidOperation(_))
    ));
    let summary = &store.search(&PaperQuery::default()).unwrap().papers[0];
    assert_eq!(summary.tags, vec![t]);

    store
        .commit_at(
            LibraryCommand::RemoveTag {
                paper_id: p.id(),
                tag_id: t,
            },
            now(3),
        )
        .unwrap();
    assert!(store.paper_tags(p.id()).unwrap().is_empty());
    assert!(
        store
            .commit_at(
                LibraryCommand::RemoveTag {
                    paper_id: p.id(),
                    tag_id: t
                },
                now(3)
            )
            .is_err(),
        "removing twice is refused"
    );
    assert_eq!(
        store.tags(false).unwrap()[0].paper_count,
        0,
        "an unused tag stays but counts nobody"
    );

    let kinds: Vec<_> = store
        .pending_events(100)
        .unwrap()
        .into_iter()
        .map(|e| e.operation)
        .collect();
    assert!(
        kinds
            .iter()
            .any(|o| matches!(o, SyncOperation::TagCreated(c) if c.id == t))
    );
    assert!(kinds.iter().any(
        |o| matches!(o, SyncOperation::TagAdded { paper_id, tag_id } if *paper_id == p.id() && *tag_id == t)
    ));
    assert!(kinds.iter().any(
        |o| matches!(o, SyncOperation::TagRemoved { paper_id, tag_id } if *paper_id == p.id() && *tag_id == t)
    ));
}

#[test]
fn filters_combine_all_or_any_and_follow_deletion_and_archive() {
    let store = SqliteLibrary::in_memory(DeviceId::new()).unwrap();
    let (a, b, c) = (paper("A2026"), paper("B2026"), paper("C2026"));
    for p in [&a, &b, &c] {
        create(&store, p);
    }
    let (ml, graph) = (tag(&store, "ml"), tag(&store, "graph"));
    add(&store, &a, ml).unwrap();
    add(&store, &a, graph).unwrap();
    add(&store, &b, ml).unwrap();

    assert_eq!(
        find(&store, vec![ml], TagMatch::All),
        sorted(vec![a.id(), b.id()])
    );
    assert_eq!(find(&store, vec![ml, graph], TagMatch::All), vec![a.id()]);
    assert_eq!(
        find(&store, vec![ml, graph], TagMatch::Any),
        sorted(vec![a.id(), b.id()])
    );
    assert!(find(&store, vec![TagId::new()], TagMatch::Any).is_empty());

    // Counts follow what is shown: archived papers only when asked, deleted never.
    let count = |include_archived, id| {
        store
            .tags(include_archived)
            .unwrap()
            .into_iter()
            .find(|t| t.tag.id == id)
            .unwrap()
            .paper_count
    };
    assert_eq!(count(false, ml), 2);
    store
        .commit_at(
            LibraryCommand::SetArchived {
                paper_id: b.id(),
                archived: true,
            },
            now(4),
        )
        .unwrap();
    assert_eq!((count(false, ml), count(true, ml)), (1, 2));
    store
        .commit_at(LibraryCommand::DeletePaper { paper_id: a.id() }, now(5))
        .unwrap();
    assert_eq!((count(false, ml), count(true, ml)), (0, 1));
    assert!(find(&store, vec![ml], TagMatch::All).is_empty());

    // The assignment survives deletion, so restoring brings the tags back; new ones are refused.
    assert_eq!(store.paper_tags(a.id()).unwrap().len(), 2);
    assert!(add(&store, &a, tag(&store, "later")).is_err());
    store
        .commit_at(LibraryCommand::RestorePaper { paper_id: a.id() }, now(6))
        .unwrap();
    assert_eq!(count(false, ml), 1);
    assert_eq!(find(&store, vec![ml, graph], TagMatch::All), vec![a.id()]);
}

#[test]
fn a_failed_assignment_leaves_nothing_behind() {
    let store = SqliteLibrary::in_memory(DeviceId::new()).unwrap();
    let t = tag(&store, "ml");
    let missing = PaperId::new();
    let before = store.pending_events(1000).unwrap().len();
    let result = store.commit_at(
        LibraryCommand::AddTag {
            paper_id: missing,
            tag_id: t,
        },
        now(2),
    );
    assert!(matches!(result, Err(StorageError::NotFound(id)) if id == missing));
    assert_eq!(store.pending_events(1000).unwrap().len(), before);
    assert_eq!(store.tags(true).unwrap()[0].paper_count, 0);
}

fn rename(store: &SqliteLibrary, id: TagId, name: &str) -> Result<(), StorageError> {
    store
        .commit_at(
            LibraryCommand::RenameTag {
                tag_id: id,
                name: name.into(),
            },
            now(4),
        )
        .map(|_| ())
}

#[test]
fn renaming_keeps_the_tag_its_papers_and_the_uniqueness_of_names() {
    let store = SqliteLibrary::in_memory(DeviceId::new()).unwrap();
    let p = paper("Renamed2026");
    create(&store, &p);
    let ml = tag(&store, "ml");
    let nlp = tag(&store, "NLP");
    add(&store, &p, ml).unwrap();

    // The same tag (id, papers, count) under a new name, cleaned like a new one.
    rename(&store, ml, "  Machine   Learning ").unwrap();
    let all = store.tags(false).unwrap();
    let renamed = all.iter().find(|c| c.tag.id == ml).unwrap();
    assert_eq!(renamed.tag.name, "Machine Learning");
    assert_eq!(renamed.paper_count, 1);
    assert_eq!(
        store.paper_tags(p.id()).unwrap()[0].name,
        "Machine Learning"
    );
    assert_eq!(find(&store, vec![ml], TagMatch::All), vec![p.id()]);

    // Only case, width or spacing of its own name may change; another tag's name may not be taken.
    rename(&store, ml, "machine learning").unwrap();
    assert!(matches!(
        rename(&store, ml, "ｎｌｐ"),
        Err(StorageError::InvalidOperation(_))
    ));
    assert!(matches!(
        rename(&store, ml, "machine learning"),
        Err(StorageError::InvalidOperation(_))
    ));
    assert!(matches!(
        rename(&store, ml, "  "),
        Err(StorageError::InvalidOperation(_))
    ));
    assert!(matches!(
        rename(&store, TagId::new(), "unknown"),
        Err(StorageError::InvalidOperation(_))
    ));
    let names: Vec<_> = store
        .tags(true)
        .unwrap()
        .into_iter()
        .map(|c| c.tag.name)
        .collect();
    assert!(names.contains(&"machine learning".to_string()) && names.contains(&"NLP".to_string()));
    assert_eq!(names.len(), 2);

    // The old name is free again, and each rename is an event.
    let again = tag(&store, "ml");
    assert_ne!(again, ml);
    let _ = nlp;
    let renames: Vec<_> = store
        .pending_events(100)
        .unwrap()
        .into_iter()
        .filter_map(|e| match e.operation {
            SyncOperation::TagRenamed(t) => Some(t),
            _ => None,
        })
        .collect();
    assert_eq!(renames.len(), 2);
    assert!(renames.iter().all(|t| t.id == ml));
}

use paper_core::{command::LibraryCommand, model::*};
use paper_desktop::application::{AppError, Application, BulkChange, BulkOutcome};
use paper_storage::SqliteLibrary;

fn setup(count: usize) -> (Application, Vec<PaperId>) {
    let store = SqliteLibrary::in_memory(DeviceId::new()).unwrap();
    let mut ids = vec![];
    for index in 0..count {
        let paper = Paper::new(
            PaperId::new(),
            CitationKey::parse(&format!("Bulk{index}")).unwrap(),
            Bibliography {
                title: Some(format!("title {index}")),
                ..Default::default()
            },
            chrono::Utc::now(),
        )
        .unwrap();
        ids.push(paper.id());
        paper_core::ports::LibraryStore::commit(&store, LibraryCommand::CreatePaper { paper })
            .unwrap();
    }
    (Application { store }, ids)
}

fn metadata(app: &Application, id: PaperId) -> UserMetadata {
    app.open(id).unwrap().paper.metadata().clone()
}

#[test]
fn one_change_reaches_every_paper_and_counts_those_that_already_matched() {
    let (app, ids) = setup(3);
    let outcome = app
        .bulk_update(&ids, BulkChange::ReadingStatus(ReadingStatus::Read))
        .unwrap();
    assert_eq!(
        outcome,
        BulkOutcome {
            changed: 3,
            unchanged: 0
        }
    );
    assert!(
        ids.iter()
            .all(|id| metadata(&app, *id).reading_status == ReadingStatus::Read)
    );
    // Repeating it changes nothing and writes nothing.
    let again = app
        .bulk_update(&ids, BulkChange::ReadingStatus(ReadingStatus::Read))
        .unwrap();
    assert_eq!(
        again,
        BulkOutcome {
            changed: 0,
            unchanged: 3
        }
    );
    // Duplicate ids are applied once.
    let favorite = app
        .bulk_update(&[ids[0], ids[0]], BulkChange::Favorite(true))
        .unwrap();
    assert_eq!(
        favorite,
        BulkOutcome {
            changed: 1,
            unchanged: 0
        }
    );
    assert!(metadata(&app, ids[0]).favorite);
    assert!(!metadata(&app, ids[1]).favorite);
}

#[test]
fn a_tag_is_created_once_and_papers_that_have_it_are_skipped() {
    let (app, ids) = setup(3);
    app.bulk_update(&ids[..1], BulkChange::AddTag("ML".into()))
        .unwrap();
    let outcome = app
        .bulk_update(&ids, BulkChange::AddTag(" ml ".into()))
        .unwrap();
    assert_eq!(
        outcome,
        BulkOutcome {
            changed: 2,
            unchanged: 1
        }
    );
    let counts = app.tag_counts(false).unwrap();
    assert_eq!(counts.len(), 1);
    assert_eq!(counts[0].paper_count, 3);
    assert!(matches!(
        app.bulk_update(&ids, BulkChange::AddTag("  ".into())),
        Err(AppError::Input(_))
    ));
}

#[test]
fn a_paper_that_cannot_take_the_change_leaves_every_paper_untouched() {
    let (app, ids) = setup(3);
    app.bulk_update(&ids[2..], BulkChange::Delete).unwrap();
    // The deleted paper rejects the whole change, so the other two keep their old value.
    assert!(matches!(
        app.bulk_update(&ids, BulkChange::Favorite(true)),
        Err(AppError::Input(m)) if m.contains("削除済み")
    ));
    assert!(ids[..2].iter().all(|id| !metadata(&app, *id).favorite));
    // A tag that would be created is not created either.
    assert!(
        app.bulk_update(&ids, BulkChange::AddTag("new".into()))
            .is_err()
    );
    assert!(app.tag_counts(true).unwrap().is_empty());
    // Restore needs deleted papers only.
    assert!(app.bulk_update(&ids, BulkChange::Restore).is_err());
    assert_eq!(
        app.bulk_update(&ids[2..], BulkChange::Restore)
            .unwrap()
            .changed,
        1
    );
    assert!(app.bulk_update(&[], BulkChange::Delete).is_err());
}

use super::*;
use crate::{
    command::{EditError, prepare_bibliography_edit},
    model::*,
};
use chrono::TimeZone;
fn clock(n: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(n, 0).unwrap()
}
fn example() -> Paper {
    Paper::new(
        PaperId::new(),
        CitationKey::parse("test").unwrap(),
        Bibliography::default(),
        clock(100),
    )
    .unwrap()
}
#[test]
fn invalid_edit_does_not_mutate_a_paper() {
    let mut p = example();
    assert!(matches!(
        p.edit_bibliography(&BibliographyValue::Month(Some(13)), clock(101)),
        Err(PaperError::InvalidMonth)
    ));
    assert_eq!(p.bibliography().publication.month, None);
    assert_eq!(p.updated_at(), clock(100));
    assert!(matches!(
        p.edit_bibliography(&BibliographyValue::Year(Some(0)), clock(101)),
        Err(PaperError::InvalidYear)
    ));
    assert!(matches!(
        p.edit_bibliography(&BibliographyValue::Title(None), clock(99)),
        Err(PaperError::InvalidTimestamp)
    ));
    p.edit_bibliography(&BibliographyValue::Month(Some(12)), clock(101))
        .unwrap();
    assert_eq!(p.bibliography().publication.month, Some(12));
}
#[test]
fn deletion_blocks_edits_and_restore_preserves_identity() {
    let mut p = example();
    let id = p.id();
    p.delete(clock(101)).unwrap();
    assert!(matches!(
        p.change_citation_key(CitationKey::parse("next").unwrap(), clock(102)),
        Err(PaperError::Deleted)
    ));
    assert!(matches!(
        p.update_metadata(UserMetadata::default(), clock(102)),
        Err(PaperError::Deleted)
    ));
    p.restore(clock(102)).unwrap();
    assert_eq!(p.id(), id);
    assert_eq!(p.deleted_at(), None);
    assert_eq!(p.citation_key().as_str(), "test");
    assert_eq!(p.created_at(), clock(100));
}
#[test]
fn head_order_is_irrelevant_but_duplicate_heads_are_rejected() {
    let p = example();
    let a = RevisionId::new();
    let b = RevisionId::new();
    let value = BibliographyValue::Title(Some("new".into()));
    let (next, r) =
        prepare_bibliography_edit(&p, value.clone(), &[a, b], &[b, a], clock(101)).unwrap();
    assert_eq!(r.parents.len(), 2);
    assert_eq!(r.target().paper_id, p.id());
    assert_eq!(r.provenance.source, MetadataSource::Manual);
    assert_eq!(next.bibliography().title.as_deref(), Some("new"));
    assert!(matches!(
        prepare_bibliography_edit(&p, value, &[a, a], &[a], clock(101)),
        Err(EditError::DuplicateHeads)
    ));
    assert!(p.bibliography().title.is_none());
}
#[test]
fn reconstruction_rejects_inconsistent_dates_and_publication() {
    let mut b = Bibliography::default();
    b.publication.month = Some(0);
    assert!(
        Paper::new(
            PaperId::new(),
            CitationKey::parse("bad").unwrap(),
            b,
            clock(100)
        )
        .is_err()
    );
    assert!(
        Paper::from_parts(
            PaperId::new(),
            CitationKey::parse("bad").unwrap(),
            Bibliography::default(),
            None,
            UserMetadata::default(),
            clock(100),
            clock(99),
            None
        )
        .is_err()
    );
}

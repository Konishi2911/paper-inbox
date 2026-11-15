use paper_core::{
    model::{BibliographyValue, FieldRevision, MetadataSource, PaperId, Provenance, RevisionId},
    service::{MergeResult, RevisionComparisonError, compare_three_way_revisions},
};
fn revision(paper: PaperId, text: &str, parents: Vec<RevisionId>) -> FieldRevision {
    FieldRevision {
        id: RevisionId::new(),
        paper_id: paper,
        version_id: None,
        parents,
        value: BibliographyValue::Title(Some(text.into())),
        provenance: Provenance {
            source: MetadataSource::Manual,
            source_record_id: None,
            observed_at: chrono::Utc::now(),
            source_updated_at: None,
        },
        created_at: chrono::Utc::now(),
    }
}
#[test]
fn verified_common_ancestor_supports_all_scalar_comparison_outcomes() {
    let id = PaperId::new();
    let base = revision(id, "base", vec![]);
    for (a, b, kind) in [
        ("base", "base", 0),
        ("local", "base", 1),
        ("base", "remote", 2),
        ("same", "same", 3),
        ("local", "remote", 4),
    ] {
        let local = revision(id, a, vec![base.id]);
        let remote = revision(id, b, vec![base.id]);
        let result = compare_three_way_revisions(&base, &local, &remote, &[]).unwrap();
        assert_eq!(
            match result.values {
                MergeResult::Unchanged(_) => 0,
                MergeResult::TakeLocal(_) => 1,
                MergeResult::TakeRemote(_) => 2,
                MergeResult::Converged(_) => 3,
                MergeResult::Conflict { .. } => 4,
            },
            kind
        );
        assert_eq!(result.base.id, base.id);
        assert!(!result.requires_field_policy);
    }
}
#[test]
fn incomplete_unrelated_or_colliding_history_is_rejected() {
    let id = PaperId::new();
    let base = revision(id, "base", vec![]);
    let intermediate = revision(id, "middle", vec![base.id]);
    let local = revision(id, "local", vec![intermediate.id]);
    let remote = revision(id, "remote", vec![base.id]);
    assert_eq!(
        compare_three_way_revisions(&base, &local, &remote, &[]).unwrap_err(),
        RevisionComparisonError::UnverifiedAncestry
    );
    assert!(
        compare_three_way_revisions(&base, &local, &remote, std::slice::from_ref(&intermediate))
            .is_ok()
    );
    let mut bad = remote.clone();
    bad.paper_id = PaperId::new();
    assert_eq!(
        compare_three_way_revisions(&base, &local, &bad, &[]).unwrap_err(),
        RevisionComparisonError::TargetMismatch
    );
    let mut collision = base.clone();
    collision.value = BibliographyValue::Title(None);
    assert!(matches!(
        compare_three_way_revisions(&base, &remote, &remote, &[collision]),
        Err(RevisionComparisonError::RevisionCollision(_))
    ));
}

use chrono::{TimeZone, Utc};
use paper_core::{model::*, service::*};

fn revision(source: MetadataSource) -> FieldRevision {
    let now = Utc.timestamp_opt(100, 0).unwrap();
    FieldRevision {
        id: RevisionId::new(),
        paper_id: PaperId::new(),
        version_id: None,
        parents: vec![],
        value: BibliographyValue::Title(Some("original".into())),
        provenance: Provenance {
            source,
            source_record_id: Some("record".into()),
            observed_at: now,
            source_updated_at: Some(now),
        },
        created_at: now,
    }
}

#[test]
fn manual_values_require_approval_and_candidates_must_be_external() {
    let current = revision(MetadataSource::Manual);
    let mut candidate = current.clone();
    candidate.provenance.source = MetadataSource::Crossref;
    candidate.value = BibliographyValue::Title(Some("external".into()));
    assert_eq!(
        evaluate_update(Some(&current), &candidate).unwrap(),
        UpdateDecision::Propose(ProposalReason::OverwriteManualValue)
    );
    assert!(matches!(
        evaluate_update(None, &current),
        Err(UpdateError::InvalidCandidateSource)
    ));
}

#[test]
fn comparison_rejects_other_papers_versions_and_fields() {
    let current = revision(MetadataSource::Crossref);
    let mut candidate = current.clone();
    candidate.paper_id = PaperId::new();
    assert!(matches!(
        evaluate_update(Some(&current), &candidate),
        Err(UpdateError::TargetMismatch { .. })
    ));
    candidate = current.clone();
    candidate.version_id = Some(VersionId::new());
    assert!(matches!(
        evaluate_update(Some(&current), &candidate),
        Err(UpdateError::TargetMismatch { .. })
    ));
    candidate = current.clone();
    candidate.value = BibliographyValue::Year(Some(2026));
    assert!(matches!(
        evaluate_update(Some(&current), &candidate),
        Err(UpdateError::TargetMismatch { .. })
    ));
}

#[test]
fn freshness_uses_source_revision_not_fetch_time() {
    let current = revision(MetadataSource::Crossref);
    let mut candidate = current.clone();
    candidate.value = BibliographyValue::Title(Some("changed".into()));
    candidate.provenance.observed_at = Utc.timestamp_opt(1000, 0).unwrap();
    candidate.provenance.source_updated_at = Some(Utc.timestamp_opt(99, 0).unwrap());
    assert_eq!(
        evaluate_update(Some(&current), &candidate).unwrap(),
        UpdateDecision::IgnoreStale
    );
    candidate.provenance.source_updated_at = Some(Utc.timestamp_opt(101, 0).unwrap());
    assert_eq!(
        evaluate_update(Some(&current), &candidate).unwrap(),
        UpdateDecision::Apply
    );
    candidate.provenance.source_record_id = Some("other record".into());
    assert_eq!(
        evaluate_update(Some(&current), &candidate).unwrap(),
        UpdateDecision::Propose(ProposalReason::UnknownFreshness)
    );
    candidate.value = current.value.clone();
    assert_eq!(
        evaluate_update(Some(&current), &candidate).unwrap(),
        UpdateDecision::NoChange
    );
}

#[test]
fn concurrent_changes_remain_explicit_conflicts() {
    assert!(matches!(
        three_way_merge(&0, &1, &2),
        MergeResult::Conflict {
            base: 0,
            local: 1,
            remote: 2
        }
    ));
    assert_eq!(three_way_merge(&0, &1, &0), MergeResult::TakeLocal(1));
    assert_eq!(three_way_merge(&0, &0, &2), MergeResult::TakeRemote(2));
    assert_eq!(three_way_merge(&0, &1, &1), MergeResult::Converged(1));
    assert_eq!(three_way_merge(&0, &0, &0), MergeResult::Unchanged(0));
}

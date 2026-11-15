use chrono::{TimeZone, Utc};
use paper_core::{model::*, service::*};
fn time(n: i64) -> chrono::DateTime<Utc> {
    Utc.timestamp_opt(1000 + n, 0).unwrap()
}
fn paper() -> Paper {
    Paper::new(
        PaperId::new(),
        CitationKey::parse("Plan").unwrap(),
        Bibliography {
            title: Some("manual".into()),
            identifiers: Identifiers {
                doi: Some(Doi::parse("10.1000/a").unwrap()),
                ..Default::default()
            },
            ..Default::default()
        },
        time(0),
    )
    .unwrap()
}
fn candidate() -> MetadataCandidate {
    MetadataCandidate {
        id: CandidateId::new(),
        present_fields: vec![BibliographyField::Title, BibliographyField::Year],
        bibliography: Bibliography {
            title: Some("external".into()),
            publication: Publication {
                year: Some(2026),
                ..Default::default()
            },
            identifiers: Identifiers {
                doi: Some(Doi::parse("10.1000/a").unwrap()),
                ..Default::default()
            },
            ..Default::default()
        },
        provenance: Provenance {
            source: MetadataSource::Crossref,
            source_record_id: Some("record".into()),
            observed_at: time(2),
            source_updated_at: Some(time(2)),
        },
        version_kind: VersionKind::VersionOfRecord,
    }
}
fn head(p: &Paper, value: BibliographyValue, source: MetadataSource) -> FieldRevision {
    FieldRevision {
        id: RevisionId::new(),
        paper_id: p.id(),
        version_id: None,
        parents: vec![],
        value,
        provenance: Provenance {
            source,
            source_record_id: Some("record".into()),
            observed_at: time(0),
            source_updated_at: Some(time(0)),
        },
        created_at: time(0),
    }
}
#[test]
fn omitted_fields_do_not_change_and_manual_values_require_approval() {
    let p = paper();
    let c = candidate();
    let h = head(
        &p,
        BibliographyValue::Title(Some("manual".into())),
        MetadataSource::Manual,
    );
    let plan = plan_metadata_update(&p, &[h], &c, CandidateMode::Automatic).unwrap();
    assert!(plan.held.is_none());
    assert_eq!(plan.fields.len(), 2);
    assert_eq!(
        plan.fields[0].decision,
        UpdateDecision::Propose(ProposalReason::OverwriteManualValue)
    );
    assert_eq!(plan.fields[1].decision, UpdateDecision::Apply);
    assert!(
        !plan
            .fields
            .iter()
            .any(|f| f.field == BibliographyField::Authors)
    );
    assert_eq!(p.bibliography().title.as_deref(), Some("manual"));
}
#[test]
fn explicit_deletion_is_a_proposal_and_stale_source_is_ignored() {
    let mut p = paper();
    p.edit_bibliography(&BibliographyValue::Year(Some(2025)), time(1))
        .unwrap();
    let h = head(
        &p,
        BibliographyValue::Year(Some(2025)),
        MetadataSource::Crossref,
    );
    let mut c = candidate();
    c.present_fields = vec![BibliographyField::Year];
    c.bibliography.publication.year = None;
    let plan =
        plan_metadata_update(&p, std::slice::from_ref(&h), &c, CandidateMode::Automatic).unwrap();
    assert_eq!(
        plan.fields[0].decision,
        UpdateDecision::Propose(ProposalReason::ExternalDeletion)
    );
    c.provenance.source_updated_at = Some(time(-1));
    let plan = plan_metadata_update(&p, &[h], &c, CandidateMode::Automatic).unwrap();
    assert_eq!(plan.fields[0].decision, UpdateDecision::IgnoreStale);
}
#[test]
fn unknown_candidates_are_held_and_association_never_autofills() {
    let p = paper();
    let mut c = candidate();
    c.bibliography.identifiers = Identifiers::default();
    let plan = plan_metadata_update(&p, &[], &c, CandidateMode::Automatic).unwrap();
    assert_eq!(plan.held, Some(CandidateHoldReason::UnknownIdentity));
    assert!(plan.fields.is_empty());
    let plan = plan_metadata_update(&p, &[], &c, CandidateMode::UserAssociated).unwrap();
    assert!(plan.held.is_none());
    assert!(
        plan.fields
            .iter()
            .all(|f| matches!(f.decision, UpdateDecision::Propose(_)))
    );
}
#[test]
fn concurrent_heads_and_invalid_sources_are_not_silently_applied() {
    let p = paper();
    let mut c = candidate();
    let a = head(
        &p,
        BibliographyValue::Title(Some("manual".into())),
        MetadataSource::Manual,
    );
    let mut b = a.clone();
    b.id = RevisionId::new();
    let plan = plan_metadata_update(&p, &[a, b], &c, CandidateMode::Automatic).unwrap();
    assert_eq!(
        plan.fields[0].decision,
        UpdateDecision::Propose(ProposalReason::ConcurrentModification)
    );
    c.present_fields.push(BibliographyField::Title);
    assert!(c.validate().is_err());
    c.present_fields.pop();
    c.provenance.source = MetadataSource::Manual;
    assert!(c.validate().is_err());
}

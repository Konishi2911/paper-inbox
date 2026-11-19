use chrono::Utc;
use paper_core::{
    command::LibraryCommand,
    model::{
        Bibliography, BibliographyField as F, BibliographyValue as V, CandidateId, Doi,
        Identifiers, MetadataCandidate, MetadataSource, PaperId, ProposalId, ProposalKind,
        Provenance, Publication, VersionKind,
    },
    ports::{LibraryStore, PaperRepository},
    query::MetadataReview,
};
use paper_desktop::{
    DiffResolution as R,
    application::{Application, PaperForm},
    metadata_resolution::ResolutionInput,
};
use paper_storage::SqliteLibrary;
struct Fixture {
    path: std::path::PathBuf,
    app: Application,
    id: PaperId,
}
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("paper-resolution-{}.sqlite", PaperId::new()));
        let app = Application {
            store: SqliteLibrary::open_local(&path).unwrap(),
        };
        let id = app
            .save(
                None,
                &PaperForm {
                    citation_key: "Resolution2026".into(),
                    title: "manual title".into(),
                    year: "2025".into(),
                    doi: "10.1000/a".into(),
                    ..Default::default()
                },
            )
            .unwrap();
        Self { path, app, id }
    }
    fn review(&self) -> MetadataReview {
        self.app.metadata_review(self.id).unwrap()
    }
    fn submit(&self) {
        let c = MetadataCandidate {
            id: CandidateId::new(),
            present_fields: vec![F::Title, F::Year],
            bibliography: Bibliography {
                title: Some("external title".into()),
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
            version_kind: VersionKind::Unknown,
            provenance: Provenance {
                source: MetadataSource::Crossref,
                source_record_id: Some("record".into()),
                observed_at: Utc::now(),
                source_updated_at: None,
            },
        };
        self.app
            .store
            .commit(LibraryCommand::SubmitMetadataCandidate {
                paper_id: self.id,
                candidate: c,
            })
            .unwrap();
    }
    fn counts(&self) -> Vec<i64> {
        let db = rusqlite::Connection::open(&self.path).unwrap();
        ["revisions", "events", "bibliographies", "paper_snapshots"]
            .iter()
            .map(|t| {
                db.query_row(&format!("SELECT count(*) FROM {t}"), [], |r| r.get(0))
                    .unwrap()
            })
            .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
fn id(review: &MetadataReview, field: F) -> ProposalId {
    review
        .proposals
        .iter()
        .find_map(|d| match d.proposal.kind {
            ProposalKind::FieldUpdate { target, .. } if target.field == field => {
                Some(d.proposal.id)
            }
            _ => None,
        })
        .unwrap()
}
fn input(id: ProposalId, decision: R, text: &str) -> ResolutionInput {
    ResolutionInput {
        id,
        decision,
        text: text.into(),
        doi: String::new(),
        arxiv: String::new(),
    }
}
#[test]
fn batch_applies_only_decided_typed_values_and_customs_create_manual_revisions() {
    let f = Fixture::new();
    f.submit();
    let review = f.review();
    let before = f.counts();
    let decisions = vec![
        input(
            id(&review, F::Title),
            R::AcceptProposed,
            "ignored display text",
        ),
        input(id(&review, F::Year), R::Unresolved, ""),
    ];
    assert_eq!(f.counts(), before);
    f.app.resolve_metadata_batch(&review, &decisions).unwrap();
    let next = f.review();
    assert_eq!(
        next.state.paper.bibliography().title.as_deref(),
        Some("external title")
    );
    assert_eq!(next.state.paper.bibliography().publication.year, Some(2025));
    assert_eq!(next.proposals.len(), 1);
    f.app
        .resolve_metadata_batch(&next, &[input(id(&next, F::Year), R::Custom, "2027")])
        .unwrap();
    let paper = f.app.store.get(f.id).unwrap().unwrap();
    assert_eq!(paper.bibliography().publication.year, Some(2027));
    let revisions = f
        .app
        .store
        .revisions(&paper_core::model::FieldTarget {
            paper_id: f.id,
            version_id: None,
            field: F::Year,
        })
        .unwrap();
    assert!(
        revisions.iter().any(
            |r| r.value == V::Year(Some(2027)) && r.provenance.source == MetadataSource::Manual
        )
    );
}
#[test]
fn invalid_custom_duplicate_unknown_and_stale_decisions_leave_every_table_unchanged() {
    let f = Fixture::new();
    f.submit();
    let review = f.review();
    let title = id(&review, F::Title);
    let year = id(&review, F::Year);
    let before = f.counts();
    for decisions in [
        vec![
            input(title, R::AcceptProposed, ""),
            input(year, R::Custom, "invalid"),
        ],
        vec![
            input(title, R::AcceptProposed, ""),
            input(title, R::KeepCurrent, ""),
        ],
        vec![input(ProposalId::new(), R::AcceptProposed, "")],
        vec![input(year, R::Custom, "0")],
    ] {
        assert!(f.app.resolve_metadata_batch(&review, &decisions).is_err());
        assert_eq!(f.counts(), before);
    }
    let mut form = PaperForm::from_paper(&review.state.paper);
    form.title = "changed elsewhere".into();
    f.app.save(Some(&review.state), &form).unwrap();
    let after = f.counts();
    assert!(
        f.app
            .resolve_metadata_batch(
                &review,
                &[
                    input(title, R::AcceptProposed, ""),
                    input(year, R::KeepCurrent, "")
                ]
            )
            .is_err()
    );
    assert_eq!(f.counts(), after);
    let latest = f.review();
    assert!(
        latest
            .proposals
            .iter()
            .find(|d| d.proposal.id == title)
            .unwrap()
            .unavailable_reason
            .is_some()
    );
}
#[test]
fn late_failure_rolls_back_earlier_resolution_and_its_history() {
    let f = Fixture::new();
    f.submit();
    let review = f.review();
    let year = id(&review, F::Year);
    let db = rusqlite::Connection::open(&f.path).unwrap();
    db.execute_batch(&format!("CREATE TRIGGER fail_second BEFORE UPDATE OF status ON update_proposals WHEN OLD.id='{year}' BEGIN SELECT RAISE(ABORT,'injected'); END;")).unwrap();
    let before = f.counts();
    assert!(
        f.app
            .resolve_metadata_batch(
                &review,
                &[
                    input(id(&review, F::Title), R::AcceptProposed, ""),
                    input(year, R::AcceptProposed, "")
                ]
            )
            .is_err()
    );
    assert_eq!(f.counts(), before);
    let after = f.review();
    assert_eq!(after.proposals.len(), 2);
    assert_eq!(
        after.state.paper.bibliography().title.as_deref(),
        Some("manual title")
    );
}

#[test]
fn custom_identifiers_reject_invalid_input_and_preserve_unedited_identifier_fields() {
    let f = Fixture::new();
    let state = f.app.open(f.id).unwrap();
    let heads = state
        .heads
        .iter()
        .find(|(field, _)| *field == F::Identifiers)
        .unwrap()
        .1
        .clone();
    f.app
        .store
        .commit(LibraryCommand::EditBibliography {
            paper_id: f.id,
            version_id: None,
            expected_heads: heads,
            value: V::Identifiers(Identifiers {
                doi: Some(Doi::parse("10.1000/a").unwrap()),
                isbn: Some("preserved-isbn".into()),
                ..Default::default()
            }),
        })
        .unwrap();
    let c = MetadataCandidate {
        id: CandidateId::new(),
        present_fields: vec![F::Identifiers],
        bibliography: Bibliography {
            identifiers: Identifiers {
                doi: Some(Doi::parse("10.1000/b").unwrap()),
                ..Default::default()
            },
            ..Default::default()
        },
        version_kind: VersionKind::Unknown,
        provenance: Provenance {
            source: MetadataSource::Crossref,
            source_record_id: Some("different".into()),
            observed_at: Utc::now(),
            source_updated_at: None,
        },
    };
    let candidate_id = c.id;
    f.app
        .store
        .commit(LibraryCommand::SubmitMetadataCandidate {
            paper_id: f.id,
            candidate: c,
        })
        .unwrap();
    let state = f.app.open(f.id).unwrap();
    f.app
        .store
        .commit(LibraryCommand::AssociateMetadataCandidate {
            candidate_id,
            expected: state,
        })
        .unwrap();
    let review = f.review();
    let mut custom = input(id(&review, F::Identifiers), R::Custom, "");
    custom.doi = "invalid".into();
    let counts = f.counts();
    assert!(
        f.app
            .resolve_metadata_batch(&review, &[custom.clone()])
            .is_err()
    );
    assert_eq!(f.counts(), counts);
    custom.doi = "https://doi.org/10.1000/C".into();
    custom.arxiv = "2301.00001v2".into();
    f.app.resolve_metadata_batch(&review, &[custom]).unwrap();
    let paper = f.app.store.get(f.id).unwrap().unwrap();
    let ids = &paper.bibliography().identifiers;
    assert_eq!(ids.doi_text(), Some("10.1000/c"));
    assert_eq!(ids.arxiv_text(), Some("2301.00001v2"));
    assert_eq!(ids.isbn.as_deref(), Some("preserved-isbn"));
}

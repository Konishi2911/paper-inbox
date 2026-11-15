use chrono::{TimeZone, Utc};
use paper_core::{
    command::LibraryCommand,
    model::*,
    ports::{LibraryStore, PaperRepository},
    query::*,
};
use paper_storage::{SqliteLibrary, StorageError};
struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("paper-candidates-{}.sqlite", PaperId::new())))
    }
    fn open(&self) -> SqliteLibrary {
        SqliteLibrary::open_local(&self.0).unwrap()
    }
    fn raw(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(&self.0).unwrap()
    }
    fn counts(&self) -> Vec<i64> {
        let db = self.raw();
        [
            "papers",
            "bibliographies",
            "revisions",
            "events",
            "metadata_candidates",
            "update_proposals",
            "paper_versions",
        ]
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
        let _ = std::fs::remove_file(&self.0);
    }
}
fn time(n: i64) -> chrono::DateTime<Utc> {
    Utc.timestamp_opt(1000 + n, 0).unwrap()
}
fn paper() -> Paper {
    Paper::new(
        PaperId::new(),
        CitationKey::parse("CandidatePaper").unwrap(),
        Bibliography {
            title: Some("manual title".into()),
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
fn create(store: &SqliteLibrary) -> Paper {
    let p = paper();
    store
        .commit_at(LibraryCommand::CreatePaper { paper: p.clone() }, time(0))
        .unwrap();
    p
}
fn candidate() -> MetadataCandidate {
    MetadataCandidate {
        id: CandidateId::new(),
        present_fields: vec![BibliographyField::Title, BibliographyField::Year],
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
        provenance: Provenance {
            source: MetadataSource::Crossref,
            source_record_id: Some("record".into()),
            observed_at: time(2),
            source_updated_at: Some(time(2)),
        },
        version_kind: VersionKind::VersionOfRecord,
    }
}
fn submit(s: &SqliteLibrary, p: &Paper, c: MetadataCandidate) -> paper_core::command::CommitResult {
    s.commit_at(
        LibraryCommand::SubmitMetadataCandidate {
            paper_id: p.id(),
            candidate: c,
        },
        time(3),
    )
    .unwrap()
}
fn field_proposal(s: &SqliteLibrary, p: &Paper, field: BibliographyField) -> UpdateProposal {
    s.pending_proposals(p.id())
        .unwrap()
        .into_iter()
        .find(|p| matches!(p.kind,ProposalKind::FieldUpdate {target,..} if target.field==field))
        .unwrap()
}
#[test]
fn exact_candidate_autofills_only_missing_fields_and_retains_manual_values() {
    let f = Fixture::new();
    let s = f.open();
    let p = create(&s);
    let c = candidate();
    let result = submit(&s, &p, c.clone());
    let loaded = s.get(p.id()).unwrap().unwrap();
    assert_eq!(loaded.bibliography().title, p.bibliography().title);
    assert_eq!(loaded.bibliography().publication.year, Some(2026));
    assert!(loaded.canonical_version().is_none());
    let proposals = s.pending_proposals(p.id()).unwrap();
    assert_eq!(proposals.len(), 2);
    assert!(
        proposals
            .iter()
            .any(|p| matches!(p.kind, ProposalKind::CanonicalVersionChange { .. }))
    );
    let title = field_proposal(&s, &p, BibliographyField::Title);
    assert_eq!(title.reason, ProposalReason::OverwriteManualValue);
    assert_eq!(s.versions(p.id()).unwrap().len(), 1);
    let events = s.pending_events(1000).unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(
                e.operation,
                SyncOperation::MetadataCandidateSubmitted { .. }
            ))
            .count(),
        1
    );
    let counts = f.counts();
    let again = submit(&s, &p, c.clone());
    assert!(again.created_events.is_empty());
    assert_eq!(f.counts(), counts);
    let mut changed = c;
    changed.bibliography.title = Some("different payload".into());
    assert!(
        s.commit_at(
            LibraryCommand::SubmitMetadataCandidate {
                paper_id: p.id(),
                candidate: changed
            },
            time(4)
        )
        .is_err()
    );
    assert_eq!(f.counts(), counts);
    assert!(!result.created_events.is_empty());
    let mut q = PaperQuery::default();
    q.filter.has_pending_proposal = Some(true);
    assert_eq!(s.search(&q).unwrap().total_count, 1);
}
#[test]
fn proposal_accept_reject_custom_and_retries_preserve_history() {
    let f = Fixture::new();
    let s = f.open();
    let p = create(&s);
    let mut c = candidate();
    c.version_kind = VersionKind::Unknown;
    submit(&s, &p, c);
    let proposal = field_proposal(&s, &p, BibliographyField::Title);
    s.commit_at(
        LibraryCommand::ResolveProposal {
            proposal_id: proposal.id,
            resolution: ProposalResolution::Accept,
        },
        time(4),
    )
    .unwrap();
    assert_eq!(
        s.get(p.id())
            .unwrap()
            .unwrap()
            .bibliography()
            .title
            .as_deref(),
        Some("external title")
    );
    let counts = f.counts();
    assert!(
        s.commit_at(
            LibraryCommand::ResolveProposal {
                proposal_id: proposal.id,
                resolution: ProposalResolution::Accept
            },
            time(5)
        )
        .unwrap()
        .created_events
        .is_empty()
    );
    assert_eq!(f.counts(), counts);
    assert!(
        s.commit_at(
            LibraryCommand::ResolveProposal {
                proposal_id: proposal.id,
                resolution: ProposalResolution::Reject
            },
            time(5)
        )
        .is_err()
    );
    let mut next = candidate();
    next.version_kind = VersionKind::Unknown;
    next.provenance.source = MetadataSource::OpenAlex;
    next.bibliography.title = Some("alternative".into());
    s.commit_at(
        LibraryCommand::SubmitMetadataCandidate {
            paper_id: p.id(),
            candidate: next,
        },
        time(5),
    )
    .unwrap();
    let proposal = field_proposal(&s, &p, BibliographyField::Title);
    s.commit_at(
        LibraryCommand::ResolveProposal {
            proposal_id: proposal.id,
            resolution: ProposalResolution::CustomValue(BibliographyValue::Title(Some(
                "my resolution".into(),
            ))),
        },
        time(6),
    )
    .unwrap();
    assert_eq!(
        s.get(p.id())
            .unwrap()
            .unwrap()
            .bibliography()
            .title
            .as_deref(),
        Some("my resolution")
    );
    let target = FieldTarget {
        paper_id: p.id(),
        version_id: None,
        field: BibliographyField::Title,
    };
    assert_eq!(
        s.active_revisions(&target).unwrap()[0].provenance.source,
        MetadataSource::Manual
    );
    let events = s.pending_events(1000).unwrap();
    assert!(events.iter().any(|e|matches!(&e.operation,SyncOperation::ProposalCreated(p) if p.status==ProposalStatus::Pending && p.resolution.is_none())));
    assert!(events.iter().any(|e| matches!(
        &e.operation,
        SyncOperation::ProposalResolved {
            resolution: ProposalResolution::CustomValue(_),
            ..
        }
    )));
}
#[test]
fn stale_and_late_failures_roll_back_every_table() {
    let f = Fixture::new();
    let s = f.open();
    let p = create(&s);
    let c = candidate();
    f.raw().execute_batch("CREATE TRIGGER fail_proposal BEFORE INSERT ON update_proposals BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
    let before = f.counts();
    assert!(
        s.commit_at(
            LibraryCommand::SubmitMetadataCandidate {
                paper_id: p.id(),
                candidate: c.clone()
            },
            time(3)
        )
        .is_err()
    );
    assert_eq!(f.counts(), before);
    assert_eq!(
        s.get(p.id())
            .unwrap()
            .unwrap()
            .bibliography()
            .publication
            .year,
        None
    );
    f.raw()
        .execute_batch("DROP TRIGGER fail_proposal;")
        .unwrap();
    submit(&s, &p, c);
    let proposal = field_proposal(&s, &p, BibliographyField::Title);
    let target = FieldTarget {
        paper_id: p.id(),
        version_id: None,
        field: BibliographyField::Title,
    };
    let expected = s
        .active_revisions(&target)
        .unwrap()
        .iter()
        .map(|r| r.id)
        .collect();
    s.commit_at(
        LibraryCommand::EditBibliography {
            paper_id: p.id(),
            version_id: None,
            value: BibliographyValue::Title(Some("new manual".into())),
            expected_heads: expected,
        },
        time(4),
    )
    .unwrap();
    let before = f.counts();
    assert!(matches!(
        s.commit_at(
            LibraryCommand::ResolveProposal {
                proposal_id: proposal.id,
                resolution: ProposalResolution::Accept
            },
            time(5)
        ),
        Err(StorageError::Conflict(_))
    ));
    assert_eq!(f.counts(), before);
    s.commit_at(
        LibraryCommand::ResolveProposal {
            proposal_id: proposal.id,
            resolution: ProposalResolution::Reject,
        },
        time(5),
    )
    .unwrap();
    assert_eq!(
        s.proposal(proposal.id).unwrap().status,
        ProposalStatus::Rejected
    );
}
#[test]
fn unknown_candidate_can_be_associated_as_an_independent_version() {
    let f = Fixture::new();
    let s = f.open();
    let p = create(&s);
    let mut c = candidate();
    c.bibliography.identifiers = Identifiers::default();
    let id = c.id;
    submit(&s, &p, c);
    assert!(s.versions(p.id()).unwrap().is_empty());
    assert!(s.pending_proposals(p.id()).unwrap().is_empty());
    let state = s.load_edit_state(p.id()).unwrap();
    s.commit_at(
        LibraryCommand::AssociateMetadataCandidate {
            candidate_id: id,
            expected: state,
        },
        time(4),
    )
    .unwrap();
    assert_eq!(s.versions(p.id()).unwrap().len(), 1);
    assert_eq!(
        s.get(p.id())
            .unwrap()
            .unwrap()
            .bibliography()
            .publication
            .year,
        None
    );
    let proposals = s.pending_proposals(p.id()).unwrap();
    assert_eq!(proposals.len(), 3);
    let canonical = proposals
        .iter()
        .find(|p| matches!(p.kind, ProposalKind::CanonicalVersionChange { .. }))
        .unwrap();
    s.commit_at(
        LibraryCommand::ResolveProposal {
            proposal_id: canonical.id,
            resolution: ProposalResolution::Accept,
        },
        time(5),
    )
    .unwrap();
    let loaded = s.get(p.id()).unwrap().unwrap();
    assert!(loaded.canonical_version().is_some());
    assert_eq!(loaded.bibliography().title.as_deref(), Some("manual title"));
    assert_eq!(loaded.bibliography().publication.year, Some(2026));
    let remaining = s.pending_proposals(p.id()).unwrap();
    assert_eq!(remaining.iter().filter(|p|matches!(p.kind,ProposalKind::FieldUpdate {target,..} if target.field==BibliographyField::Title)).count(),1);
    assert!(!s.pending_events(1000).unwrap().is_empty());
    drop(s);
    let s = f.open();
    assert_eq!(
        s.get(p.id()).unwrap().unwrap().canonical_version(),
        loaded.canonical_version()
    );
}

#[test]
fn version_heads_ownership_and_events_are_independent() {
    let f = Fixture::new();
    let s = f.open();
    let p = create(&s);
    let version = PaperVersion {
        id: VersionId::new(),
        paper_id: p.id(),
        kind: VersionKind::VersionOfRecord,
        bibliography: candidate().bibliography,
        created_at: time(1),
        updated_at: time(1),
    };
    let id = version.id;
    s.commit_at(LibraryCommand::AddVersion { version }, time(1))
        .unwrap();
    assert!(
        s.get(p.id())
            .unwrap()
            .unwrap()
            .canonical_version()
            .is_none()
    );
    let canonical = s
        .pending_proposals(p.id())
        .unwrap()
        .into_iter()
        .find(|p| matches!(p.kind, ProposalKind::CanonicalVersionChange { .. }))
        .unwrap();
    let target = FieldTarget {
        paper_id: p.id(),
        version_id: Some(id),
        field: BibliographyField::Title,
    };
    let head = s.active_revisions(&target).unwrap()[0].id;
    s.commit_at(
        LibraryCommand::EditBibliography {
            paper_id: p.id(),
            version_id: Some(id),
            value: BibliographyValue::Title(Some("version edit".into())),
            expected_heads: vec![head],
        },
        time(2),
    )
    .unwrap();
    assert_eq!(
        s.versions(p.id()).unwrap()[0].bibliography.title.as_deref(),
        Some("version edit")
    );
    assert_eq!(
        s.get(p.id())
            .unwrap()
            .unwrap()
            .bibliography()
            .title
            .as_deref(),
        Some("manual title")
    );
    assert_eq!(s.revisions(&target).unwrap().len(), 2);
    assert!(matches!(
        s.commit_at(
            LibraryCommand::ResolveProposal {
                proposal_id: canonical.id,
                resolution: ProposalResolution::Accept
            },
            time(3)
        ),
        Err(StorageError::Conflict(_))
    ));
    let events = s.pending_events(1000).unwrap();
    let initial = events
        .iter()
        .find_map(|e| match &e.operation {
            SyncOperation::PaperVersionAdded(v) => Some(v),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        initial.bibliography.title.as_deref(),
        Some("external title")
    );
    let other = Paper::new(
        PaperId::new(),
        CitationKey::parse("OtherPaper").unwrap(),
        Bibliography::default(),
        time(0),
    )
    .unwrap();
    s.commit_at(
        LibraryCommand::CreatePaper {
            paper: other.clone(),
        },
        time(0),
    )
    .unwrap();
    let before = f.counts();
    assert!(
        s.commit_at(
            LibraryCommand::SetCanonicalVersion {
                paper_id: other.id(),
                version_id: Some(id)
            },
            time(4)
        )
        .is_err()
    );
    assert_eq!(f.counts(), before);
}
#[test]
fn resolution_failure_rolls_back_the_adopted_value_and_status() {
    let f = Fixture::new();
    let s = f.open();
    let p = create(&s);
    submit(&s, &p, candidate());
    let proposal = field_proposal(&s, &p, BibliographyField::Title);
    let before = f.counts();
    f.raw().execute_batch("CREATE TRIGGER fail_resolution BEFORE UPDATE OF status ON update_proposals BEGIN SELECT RAISE(ABORT,'injected resolution'); END;").unwrap();
    assert!(
        s.commit_at(
            LibraryCommand::ResolveProposal {
                proposal_id: proposal.id,
                resolution: ProposalResolution::Accept
            },
            time(4)
        )
        .is_err()
    );
    assert_eq!(f.counts(), before);
    assert_eq!(
        s.get(p.id())
            .unwrap()
            .unwrap()
            .bibliography()
            .title
            .as_deref(),
        Some("manual title")
    );
    assert_eq!(
        s.proposal(proposal.id).unwrap().status,
        ProposalStatus::Pending
    );
}
#[test]
fn migration_from_original_v2_preserves_rows_and_failed_migration_is_atomic() {
    for fail in [false, true] {
        let f = Fixture::new();
        let db = f.raw();
        db.execute_batch(include_str!("../src/schema.sql")).unwrap();
        let id = PaperId::new();
        db.execute("INSERT INTO bibliographies(id,title,kind) VALUES ('legacy-b','legacy title','Unknown')",[]).unwrap();
        db.execute("INSERT INTO paper_snapshots VALUES ('legacy-s',?1,'legacy-b','Legacy',?2,?2,NULL,'','Unread',0,0)",rusqlite::params![id.to_string(),time(0).to_rfc3339()]).unwrap();
        db.execute(
            "INSERT INTO papers VALUES (?1,'Legacy','legacy-s')",
            [id.to_string()],
        )
        .unwrap();
        db.pragma_update(None, "user_version", 2).unwrap();
        if fail {
            db.execute_batch("CREATE TABLE metadata_candidates(id INTEGER PRIMARY KEY);")
                .unwrap();
            assert!(SqliteLibrary::open_local(&f.0).is_err());
            let version: i64 = db
                .pragma_query_value(None, "user_version", |r| r.get(0))
                .unwrap();
            assert_eq!(version, 2);
            let count:i64=db.query_row("SELECT count(*) FROM pragma_table_info('paper_snapshots') WHERE name='canonical_version'",[],|r|r.get(0)).unwrap();
            assert_eq!(count, 0);
        } else {
            let s = f.open();
            assert_eq!(
                s.get(id).unwrap().unwrap().bibliography().title.as_deref(),
                Some("legacy title")
            );
            assert!(s.versions(id).unwrap().is_empty());
            let version: i64 = db
                .pragma_query_value(None, "user_version", |r| r.get(0))
                .unwrap();
            assert_eq!(version, 4);
        }
    }
}

#[test]
fn freshness_explicit_deletion_and_publisher_priority_survive_sql_round_trips() {
    let f = Fixture::new();
    let s = f.open();
    let p = create(&s);
    let mut c = candidate();
    c.present_fields = vec![BibliographyField::Year];
    let id = c.id;
    submit(&s, &p, c);
    let canonical = s
        .pending_proposals(p.id())
        .unwrap()
        .into_iter()
        .find(|p| matches!(p.kind, ProposalKind::CanonicalVersionChange { .. }))
        .unwrap();
    s.commit_at(
        LibraryCommand::ResolveProposal {
            proposal_id: canonical.id,
            resolution: ProposalResolution::Accept,
        },
        time(4),
    )
    .unwrap();
    let mut older = candidate();
    older.version_kind = VersionKind::Unknown;
    older.present_fields = vec![BibliographyField::Year];
    older.bibliography.publication.year = Some(1999);
    older.provenance.source_updated_at = Some(time(1));
    s.commit_at(
        LibraryCommand::SubmitMetadataCandidate {
            paper_id: p.id(),
            candidate: older,
        },
        time(5),
    )
    .unwrap();
    assert_eq!(
        s.get(p.id())
            .unwrap()
            .unwrap()
            .bibliography()
            .publication
            .year,
        Some(2026)
    );
    assert!(s.pending_proposals(p.id()).unwrap().is_empty());
    let mut deletion = candidate();
    deletion.version_kind = VersionKind::VersionOfRecord;
    deletion.present_fields = vec![BibliographyField::Year];
    deletion.bibliography.publication.year = None;
    deletion.provenance.source_updated_at = Some(time(6));
    s.commit_at(
        LibraryCommand::SubmitMetadataCandidate {
            paper_id: p.id(),
            candidate: deletion,
        },
        time(6),
    )
    .unwrap();
    let proposal = field_proposal(&s, &p, BibliographyField::Year);
    assert_eq!(proposal.reason, ProposalReason::ExternalDeletion);
    let detail = s
        .metadata_review(p.id())
        .unwrap()
        .proposals
        .into_iter()
        .find(|d| d.proposal.id == proposal.id)
        .unwrap();
    assert_eq!(
        detail.current_value,
        Some(BibliographyValue::Year(Some(2026)))
    );
    assert_eq!(detail.proposed_value, Some(BibliographyValue::Year(None)));
    s.commit_at(
        LibraryCommand::ResolveProposal {
            proposal_id: proposal.id,
            resolution: ProposalResolution::Accept,
        },
        time(7),
    )
    .unwrap();
    assert_eq!(
        s.get(p.id())
            .unwrap()
            .unwrap()
            .bibliography()
            .publication
            .year,
        None
    );
    let mut lower = candidate();
    lower.version_kind = VersionKind::Preprint;
    lower.present_fields = vec![BibliographyField::Publisher];
    lower.bibliography.publication.publisher = Some("preprint source".into());
    s.commit_at(
        LibraryCommand::SubmitMetadataCandidate {
            paper_id: p.id(),
            candidate: lower,
        },
        time(8),
    )
    .unwrap();
    assert_eq!(
        s.get(p.id())
            .unwrap()
            .unwrap()
            .bibliography()
            .publication
            .publisher,
        None
    );
    assert_eq!(
        field_proposal(&s, &p, BibliographyField::Publisher).reason,
        ProposalReason::ManualAssociation
    );
    assert!(
        s.metadata_review(p.id())
            .unwrap()
            .candidates
            .iter()
            .any(|c| c.candidate.id == id)
    );
    assert_eq!(
        f.raw()
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
                .get::<_, i64>(
                0
            ))
            .unwrap(),
        0
    );
}

#[test]
fn held_identity_conflicts_stale_association_and_batch_resolution_are_atomic() {
    let f = Fixture::new();
    let s = f.open();
    let p = create(&s);
    let mut conflict = candidate();
    conflict.bibliography.identifiers.doi = Some(Doi::parse("10.1000/conflict").unwrap());
    let id = conflict.id;
    submit(&s, &p, conflict);
    assert!(s.versions(p.id()).unwrap().is_empty());
    let old = s.load_edit_state(p.id()).unwrap();
    s.commit_at(
        LibraryCommand::UpdateNotes {
            paper_id: p.id(),
            notes: "concurrent edit".into(),
        },
        time(4),
    )
    .unwrap();
    let counts = f.counts();
    assert!(matches!(
        s.commit_at(
            LibraryCommand::AssociateMetadataCandidate {
                candidate_id: id,
                expected: old
            },
            time(5)
        ),
        Err(StorageError::Conflict(_))
    ));
    assert_eq!(f.counts(), counts);
    let state = s.load_edit_state(p.id()).unwrap();
    s.commit_at(
        LibraryCommand::AssociateMetadataCandidate {
            candidate_id: id,
            expected: state,
        },
        time(5),
    )
    .unwrap();
    let title = field_proposal(&s, &p, BibliographyField::Title);
    let year = field_proposal(&s, &p, BibliographyField::Year);
    s.commit_at(
        LibraryCommand::EditBibliography {
            paper_id: p.id(),
            version_id: None,
            value: BibliographyValue::Year(Some(2025)),
            expected_heads: vec![],
        },
        time(6),
    )
    .unwrap();
    let counts = f.counts();
    assert!(
        s.commit_batch_at(
            vec![
                LibraryCommand::ResolveProposal {
                    proposal_id: title.id,
                    resolution: ProposalResolution::Accept
                },
                LibraryCommand::ResolveProposal {
                    proposal_id: year.id,
                    resolution: ProposalResolution::Accept
                }
            ],
            &[],
            time(7)
        )
        .is_err()
    );
    assert_eq!(f.counts(), counts);
    assert_eq!(
        s.proposal(title.id).unwrap().status,
        ProposalStatus::Pending
    );
    assert_eq!(
        s.get(p.id())
            .unwrap()
            .unwrap()
            .bibliography()
            .title
            .as_deref(),
        Some("manual title")
    );
}

#[test]
fn review_keeps_current_and_candidate_revision_sources_separate() {
    let f = Fixture::new();
    let s = f.open();
    let p = create(&s);
    let mut c = candidate();
    c.provenance.observed_at = time(3);
    submit(&s, &p, c);
    let review = s.metadata_review(p.id()).unwrap();
    let detail=review.proposals.iter().find(|d|matches!(d.proposal.kind,ProposalKind::FieldUpdate {target,..} if target.field==BibliographyField::Title)).unwrap();
    assert_eq!(detail.current_revisions.len(), 1);
    let current = &detail.current_revisions[0];
    assert_eq!(current.provenance.source, MetadataSource::Manual);
    assert_eq!(current.value, detail.current_value.clone().unwrap());
    let ProposalKind::FieldUpdate { expected_heads, .. } = &detail.proposal.kind else {
        panic!("field proposal")
    };
    assert_eq!(expected_heads, &vec![current.id]);
    let source = detail.provenance.as_ref().unwrap();
    assert_eq!(source.source, MetadataSource::Crossref);
    assert_eq!(source.observed_at, time(3));
    assert_eq!(source.source_updated_at, Some(time(2)));
    assert_ne!(source.observed_at, current.provenance.observed_at);
}

#[test]
fn identity_change_marks_otherwise_unchanged_field_proposal_unavailable() {
    let f = Fixture::new();
    let s = f.open();
    let p = create(&s);
    submit(&s, &p, candidate());
    let proposal = field_proposal(&s, &p, BibliographyField::Title);
    let state = s.load_edit_state(p.id()).unwrap();
    let heads = state
        .heads
        .iter()
        .find(|(field, _)| *field == BibliographyField::Identifiers)
        .unwrap()
        .1
        .clone();
    s.commit_at(
        LibraryCommand::EditBibliography {
            paper_id: p.id(),
            version_id: None,
            value: BibliographyValue::Identifiers(Identifiers {
                doi: Some(Doi::parse("10.1000/other").unwrap()),
                ..Default::default()
            }),
            expected_heads: heads,
        },
        time(4),
    )
    .unwrap();
    let review = s.metadata_review(p.id()).unwrap();
    let detail = review
        .proposals
        .iter()
        .find(|d| d.proposal.id == proposal.id)
        .unwrap();
    assert!(
        detail
            .unavailable_reason
            .as_ref()
            .unwrap()
            .contains("同一性")
    );
    let before = f.counts();
    assert!(
        s.commit_at(
            LibraryCommand::ResolveProposal {
                proposal_id: proposal.id,
                resolution: ProposalResolution::Accept
            },
            time(5)
        )
        .is_err()
    );
    assert_eq!(f.counts(), before);
}

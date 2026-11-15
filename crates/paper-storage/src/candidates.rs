use super::*;
use paper_core::service::{
    CandidateMode, MetadataUpdatePlan, UpdateDecision, plan_metadata_update,
};

pub(super) fn migrate(db: &Connection) -> Result<()> {
    db.execute_batch(include_str!("candidate_schema.sql"))?;
    versions::add_column(
        db,
        "events",
        "candidate_id",
        "TEXT REFERENCES metadata_candidates(id)",
    )?;
    versions::add_column(
        db,
        "events",
        "proposal_id",
        "TEXT REFERENCES update_proposals(id)",
    )?;
    versions::add_column(db, "events", "resolution_kind", "TEXT")?;
    versions::add_column(
        db,
        "events",
        "resolution_bibliography_id",
        "TEXT REFERENCES bibliographies(id)",
    )?;
    Ok(())
}
fn stamp(now: DateTime<Utc>) -> String {
    now.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
}
fn snapshot(db: &Connection, id: PaperId) -> Result<String> {
    Ok(db.query_row(
        "SELECT snapshot_id FROM papers WHERE id=?1",
        [id.to_string()],
        |r| r.get(0),
    )?)
}
fn event(
    store: &SqliteLibrary,
    db: &Connection,
    kind: &str,
    paper: PaperId,
    references: (Option<CandidateId>, Option<ProposalId>),
    now: DateTime<Utc>,
    result: &mut CommitResult,
) -> Result<EventId> {
    let (candidate, proposal) = references;
    append_event(
        db,
        store.device_id,
        kind,
        &snapshot(db, paper)?,
        None,
        now,
        result,
    )?;
    let id = *result
        .created_events
        .last()
        .ok_or(StorageError::Corrupt("Missing metadata event".into()))?;
    db.execute(
        "UPDATE events SET candidate_id=?1,proposal_id=?2 WHERE id=?3",
        params![
            candidate.map(|i| i.to_string()),
            proposal.map(|i| i.to_string()),
            id.to_string()
        ],
    )?;
    Ok(id)
}
fn combine(a: &mut CommitResult, b: CommitResult) {
    a.affected_papers.extend(b.affected_papers);
    a.created_revisions.extend(b.created_revisions);
    a.created_proposals.extend(b.created_proposals);
    a.created_events.extend(b.created_events);
}
pub(super) fn read_candidate(db: &Connection, id: CandidateId) -> Result<StoredMetadataCandidate> {
    let d=db.query_row("SELECT paper_id,bibliography_id,source,source_detail,source_record_id,observed_at,source_updated_at,version_kind,received_at,status,associated_at,version_id FROM metadata_candidates WHERE id=?1",[id.to_string()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<String>>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,String>(5)?,r.get::<_,Option<String>>(6)?,r.get::<_,String>(7)?,r.get::<_,String>(8)?,r.get::<_,String>(9)?,r.get::<_,Option<String>>(10)?,r.get::<_,Option<String>>(11)?))).optional()?.ok_or(StorageError::CandidateNotFound(id))?;
    let mut stmt =
        db.prepare("SELECT field FROM candidate_fields WHERE candidate_id=?1 ORDER BY position")?;
    let fields = stmt
        .query_map([id.to_string()], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .map(sql::field)
        .collect::<Result<Vec<_>>>()?;
    Ok(StoredMetadataCandidate {
        paper_id: sql::decode(d.0)?,
        candidate: MetadataCandidate {
            id,
            present_fields: fields,
            bibliography: sql::read_bibliography(db, &d.1)?,
            provenance: Provenance {
                source: sql::source(d.2, d.3)?,
                source_record_id: d.4,
                observed_at: sql::time(d.5)?,
                source_updated_at: d.6.map(sql::time).transpose()?,
            },
            version_kind: sql::version_kind(d.7)?,
        },
        received_at: sql::time(d.8)?,
        status: match d.9.as_str() {
            "Held" => CandidateStatus::Held,
            "Processed" => CandidateStatus::Processed,
            "Associated" => CandidateStatus::Associated,
            _ => return Err(StorageError::Corrupt("Unknown candidate status".into())),
        },
        associated_at: d.10.map(sql::time).transpose()?,
        version_id: d.11.map(sql::decode).transpose()?,
    })
}
pub(super) fn list_candidates(
    db: &Connection,
    id: PaperId,
) -> Result<Vec<StoredMetadataCandidate>> {
    let mut stmt =
        db.prepare("SELECT id FROM metadata_candidates WHERE paper_id=?1 ORDER BY received_at,id")?;
    let ids = stmt
        .query_map([id.to_string()], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    ids.into_iter()
        .map(|id| read_candidate(db, sql::decode(id)?))
        .collect()
}
fn heads(db: &Connection, paper: PaperId) -> Result<Vec<FieldRevision>> {
    let mut result = vec![];
    for field in BibliographyField::ALL {
        for id in sql::heads(
            db,
            &FieldTarget {
                paper_id: paper,
                version_id: None,
                field,
            },
        )? {
            result.push(sql::read_revision(db, id)?);
        }
    }
    Ok(result)
}
pub(super) fn submit(
    store: &SqliteLibrary,
    db: &Connection,
    paper_id: PaperId,
    candidate: MetadataCandidate,
    now: DateTime<Utc>,
) -> Result<CommitResult> {
    candidate.validate()?;
    let existing: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM metadata_candidates WHERE id=?1)",
        [candidate.id.to_string()],
        |r| r.get(0),
    )?;
    if existing {
        let prior = read_candidate(db, candidate.id)?;
        if prior.paper_id == paper_id && prior.candidate == candidate {
            return Ok(CommitResult::default());
        }
        return Err(StorageError::InvalidOperation(
            "Candidate ID reused with different content or paper",
        ));
    }
    let paper = sql::get(db, paper_id)?.ok_or(StorageError::NotFound(paper_id))?;
    let mut check = paper.clone();
    check.touch(now)?;
    let plan = plan_metadata_update(
        &paper,
        &heads(db, paper_id)?,
        &candidate,
        CandidateMode::Automatic,
    )?;
    let bibliography = format!("candidate-{}", candidate.id);
    sql::write_bibliography(db, &bibliography, &candidate.bibliography)?;
    let (source, detail) = sql::source_name(&candidate.provenance.source);
    db.execute("INSERT INTO metadata_candidates(id,paper_id,bibliography_id,source,source_detail,source_record_id,observed_at,source_updated_at,version_kind,received_at,status) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'Held')",params![candidate.id.to_string(),paper_id.to_string(),bibliography,source,detail,candidate.provenance.source_record_id,stamp(candidate.provenance.observed_at),candidate.provenance.source_updated_at.map(stamp),sql::version_name(candidate.version_kind),stamp(now)])?;
    for (position, field) in candidate.present_fields.iter().enumerate() {
        db.execute(
            "INSERT INTO candidate_fields VALUES (?1,?2,?3)",
            params![
                candidate.id.to_string(),
                position as i64,
                sql::field_name(*field)
            ],
        )?;
    }
    let mut result = CommitResult {
        affected_papers: vec![paper_id],
        ..Default::default()
    };
    event(
        store,
        db,
        "MetadataCandidateSubmitted",
        paper_id,
        (Some(candidate.id), None),
        now,
        &mut result,
    )?;
    if plan.held.is_none() {
        process(store, db, paper_id, &candidate, false, now, &mut result)?;
    }
    Ok(result)
}
pub(super) fn associate(
    store: &SqliteLibrary,
    db: &Connection,
    id: CandidateId,
    expected: PaperEditState,
    now: DateTime<Utc>,
) -> Result<CommitResult> {
    let stored = read_candidate(db, id)?;
    if stored.status == CandidateStatus::Associated {
        return Ok(CommitResult::default());
    }
    if stored.status != CandidateStatus::Held {
        return Err(StorageError::InvalidOperation(
            "Candidate is already processed",
        ));
    }
    if expected.paper.id() != stored.paper_id || read_edit_state(db, stored.paper_id)? != expected {
        return Err(StorageError::Conflict(stored.paper_id));
    }
    let mut result = CommitResult::default();
    process(
        store,
        db,
        stored.paper_id,
        &stored.candidate,
        true,
        now,
        &mut result,
    )?;
    Ok(result)
}
fn process(
    store: &SqliteLibrary,
    db: &Connection,
    paper_id: PaperId,
    candidate: &MetadataCandidate,
    manual: bool,
    now: DateTime<Utc>,
    result: &mut CommitResult,
) -> Result<()> {
    let version = PaperVersion {
        id: VersionId::new(),
        paper_id,
        kind: candidate.version_kind,
        bibliography: candidate.bibliography.clone(),
        created_at: now,
        updated_at: now,
    };
    let version_id = version.id;
    combine(
        result,
        versions::add(
            store,
            db,
            version,
            &candidate.provenance,
            &candidate.present_fields,
            now,
        )?,
    );
    db.execute(
        "UPDATE metadata_candidates SET status=?1,associated_at=?2,version_id=?3 WHERE id=?4",
        params![
            if manual { "Associated" } else { "Processed" },
            if manual { Some(stamp(now)) } else { None },
            version_id.to_string(),
            candidate.id.to_string()
        ],
    )?;
    if manual {
        let id = event(
            store,
            db,
            "MetadataCandidateAssociated",
            paper_id,
            (Some(candidate.id), None),
            now,
            result,
        )?;
        let vs: String = db.query_row(
            "SELECT snapshot_id FROM paper_versions WHERE id=?1",
            [version_id.to_string()],
            |r| r.get(0),
        )?;
        db.execute(
            "UPDATE events SET version_snapshot_id=?1 WHERE id=?2",
            params![vs, id.to_string()],
        )?;
    }
    let paper = sql::get(db, paper_id)?.ok_or(StorageError::NotFound(paper_id))?;
    let lower_priority = paper
        .canonical_version()
        .map(|id| versions::read(db, id))
        .transpose()?
        .flatten()
        .is_some_and(|v| {
            v.kind == VersionKind::VersionOfRecord
                && candidate.version_kind != VersionKind::VersionOfRecord
        });
    let mode = if manual || lower_priority {
        CandidateMode::UserAssociated
    } else {
        CandidateMode::Automatic
    };
    let plan = plan_metadata_update(&paper, &heads(db, paper_id)?, candidate, mode)?;
    apply_plan(
        store,
        db,
        &paper,
        candidate,
        Some(candidate.id),
        version_id,
        &plan,
        now,
        result,
    )?;
    if candidate.version_kind == VersionKind::VersionOfRecord {
        propose_canonical(
            store,
            db,
            paper_id,
            version_id,
            Some(candidate.id),
            ProposalReason::PublisherVersionDiscovered,
            now,
            result,
        )?;
    }
    Ok(())
}
fn reason_name(reason: ProposalReason) -> &'static str {
    match reason {
        ProposalReason::OverwriteManualValue => "OverwriteManualValue",
        ProposalReason::ConflictingSources => "ConflictingSources",
        ProposalReason::UnknownFreshness => "UnknownFreshness",
        ProposalReason::ConcurrentModification => "ConcurrentModification",
        ProposalReason::PublisherVersionDiscovered => "PublisherVersionDiscovered",
        ProposalReason::DuplicatePaper => "DuplicatePaper",
        ProposalReason::ExternalDeletion => "ExternalDeletion",
        ProposalReason::ManualAssociation => "ManualAssociation",
        ProposalReason::CanonicalVersionSelection => "CanonicalVersionSelection",
    }
}
fn parse_reason(s: &str) -> Result<ProposalReason> {
    Ok(match s {
        "OverwriteManualValue" => ProposalReason::OverwriteManualValue,
        "ConflictingSources" => ProposalReason::ConflictingSources,
        "UnknownFreshness" => ProposalReason::UnknownFreshness,
        "ConcurrentModification" => ProposalReason::ConcurrentModification,
        "PublisherVersionDiscovered" => ProposalReason::PublisherVersionDiscovered,
        "DuplicatePaper" => ProposalReason::DuplicatePaper,
        "ExternalDeletion" => ProposalReason::ExternalDeletion,
        "ManualAssociation" => ProposalReason::ManualAssociation,
        "CanonicalVersionSelection" => ProposalReason::CanonicalVersionSelection,
        _ => return Err(StorageError::Corrupt("Unknown proposal reason".into())),
    })
}
fn supersede(
    store: &SqliteLibrary,
    db: &Connection,
    id: ProposalId,
    paper: PaperId,
    now: DateTime<Utc>,
    result: &mut CommitResult,
) -> Result<()> {
    db.execute(
        "UPDATE update_proposals SET status='Superseded',resolved_at=?1 WHERE id=?2",
        params![stamp(now), id.to_string()],
    )?;
    event(
        store,
        db,
        "ProposalSuperseded",
        paper,
        (None, Some(id)),
        now,
        result,
    )?;
    Ok(())
}
#[allow(clippy::too_many_arguments)]
fn apply_plan(
    store: &SqliteLibrary,
    db: &Connection,
    original: &Paper,
    candidate: &MetadataCandidate,
    candidate_id: Option<CandidateId>,
    version_id: VersionId,
    plan: &MetadataUpdatePlan,
    now: DateTime<Utc>,
    result: &mut CommitResult,
) -> Result<()> {
    if plan.held.is_some() {
        return Ok(());
    }
    let bibliography = if let Some(id) = candidate_id {
        db.query_row(
            "SELECT bibliography_id FROM metadata_candidates WHERE id=?1",
            [id.to_string()],
            |r| r.get::<_, String>(0),
        )?
    } else {
        db.query_row(
            "SELECT snapshot_id FROM paper_versions WHERE id=?1",
            [version_id.to_string()],
            |r| r.get::<_, String>(0),
        )?
    };
    let version_snapshot: String = db.query_row(
        "SELECT snapshot_id FROM paper_versions WHERE id=?1",
        [version_id.to_string()],
        |r| r.get(0),
    )?;
    let expected_bibliography = snapshot(db, original.id())?;
    let mut paper = original.clone();
    let mut applied = vec![];
    for change in &plan.fields {
        let old:Option<String>=db.query_row("SELECT id FROM update_proposals WHERE paper_id=?1 AND status='Pending' AND kind='FieldUpdate' AND field=?2 AND candidate_version_id=?3",params![paper.id().to_string(),sql::field_name(change.field),version_id.to_string()],|r|r.get(0)).optional()?;
        if let Some(old) = old {
            let old_id = sql::decode(old)?;
            let p = read_proposal(db, old_id)?;
            if let (UpdateDecision::Propose(_), ProposalKind::FieldUpdate { expected_heads, .. }) =
                (&change.decision, &p.kind)
                && expected_heads == &change.expected_heads
            {
                let extra:(String,String)=db.query_row("SELECT candidate_revision_id,expected_version_snapshot FROM update_proposals WHERE id=?1",[old_id.to_string()],|r|Ok((r.get(0)?,r.get(1)?)))?;
                if sql::read_revision(db, sql::decode(extra.0)?)?.value == change.candidate
                    && extra.1 == version_snapshot
                {
                    continue;
                }
            }
            supersede(store, db, old_id, paper.id(), now, result)?;
        }
        if matches!(
            change.decision,
            UpdateDecision::NoChange | UpdateDecision::IgnoreStale
        ) {
            continue;
        }
        let revision = FieldRevision {
            id: RevisionId::new(),
            paper_id: paper.id(),
            version_id: None,
            parents: change.expected_heads.clone(),
            value: change.candidate.clone(),
            provenance: candidate.provenance.clone(),
            created_at: now,
        };
        record_revision(db, &bibliography, &revision)?;
        result.created_revisions.push(revision.id);
        append_event(
            db,
            store.device_id,
            "FieldRevisionCreated",
            &snapshot(db, paper.id())?,
            Some(revision.id),
            now,
            result,
        )?;
        match change.decision {
            UpdateDecision::Apply => {
                paper.edit_bibliography(&change.candidate, now)?;
                set_head(db, &revision)?;
                applied.push(revision.id);
            }
            UpdateDecision::Propose(reason) => {
                let id = ProposalId::new();
                db.execute("INSERT INTO update_proposals(id,paper_id,candidate_id,kind,reason,status,created_at,field,candidate_revision_id,candidate_version_id,expected_canonical_version_id,expected_version_snapshot,expected_bibliography_id) VALUES (?1,?2,?3,'FieldUpdate',?4,'Pending',?5,?6,?7,?8,?9,?10,?11)",params![id.to_string(),paper.id().to_string(),candidate_id.map(|i|i.to_string()),reason_name(reason),stamp(now),sql::field_name(change.field),revision.id.to_string(),version_id.to_string(),if candidate_id.is_none() {Some(version_id.to_string())} else {None},version_snapshot,expected_bibliography])?;
                for head in &change.expected_heads {
                    db.execute(
                        "INSERT INTO proposal_heads VALUES (?1,?2)",
                        params![id.to_string(), head.to_string()],
                    )?;
                }
                result.created_proposals.push(id);
                event(
                    store,
                    db,
                    "ProposalCreated",
                    paper.id(),
                    (candidate_id, Some(id)),
                    now,
                    result,
                )?;
            }
            _ => {}
        }
    }
    if !applied.is_empty() {
        let snapshot = versions::persist_paper(db, &paper)?;
        for id in applied {
            append_event(
                db,
                store.device_id,
                "FieldHeadsChanged",
                &snapshot,
                Some(id),
                now,
                result,
            )?;
        }
    }
    result.affected_papers.push(paper.id());
    Ok(())
}

pub(super) fn read_proposal(db: &Connection, id: ProposalId) -> Result<UpdateProposal> {
    let d=db.query_row("SELECT paper_id,kind,reason,status,created_at,resolved_at,field,candidate_revision_id,candidate_version_id,expected_canonical_version_id,resolution_kind,resolution_bibliography_id FROM update_proposals WHERE id=?1",[id.to_string()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,Option<String>>(6)?,r.get::<_,Option<String>>(7)?,r.get::<_,Option<String>>(8)?,r.get::<_,Option<String>>(9)?,r.get::<_,Option<String>>(10)?,r.get::<_,Option<String>>(11)?))).optional()?.ok_or(StorageError::ProposalNotFound(id))?;
    let paper_id = sql::decode(d.0)?;
    let kind = if d.1 == "FieldUpdate" {
        let mut stmt = db.prepare(
            "SELECT revision_id FROM proposal_heads WHERE proposal_id=?1 ORDER BY revision_id",
        )?;
        let expected_heads = stmt
            .query_map([id.to_string()], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?
            .into_iter()
            .map(sql::decode)
            .collect::<Result<Vec<_>>>()?;
        ProposalKind::FieldUpdate {
            target: FieldTarget {
                paper_id,
                version_id: None,
                field: sql::field(
                    d.6.ok_or(StorageError::Corrupt("Missing proposal field".into()))?,
                )?,
            },
            expected_heads,
            candidate_revision: sql::decode(
                d.7.ok_or(StorageError::Corrupt("Missing candidate revision".into()))?,
            )?,
        }
    } else if d.1 == "CanonicalVersionChange" {
        ProposalKind::CanonicalVersionChange {
            paper_id,
            current_version: d.9.map(sql::decode).transpose()?,
            candidate_version: sql::decode(
                d.8.ok_or(StorageError::Corrupt("Missing candidate version".into()))?,
            )?,
        }
    } else {
        return Err(StorageError::Corrupt("Unknown proposal kind".into()));
    };
    let resolution = match d.10.as_deref() {
        None => None,
        Some("Accept") => Some(ProposalResolution::Accept),
        Some("Reject") => Some(ProposalResolution::Reject),
        Some("CustomValue") => {
            let ProposalKind::FieldUpdate { target, .. } = &kind else {
                return Err(StorageError::Corrupt("Custom canonical resolution".into()));
            };
            Some(ProposalResolution::CustomValue(
                sql::read_bibliography(
                    db,
                    &d.11
                        .ok_or(StorageError::Corrupt("Missing custom value".into()))?,
                )?
                .value(target.field),
            ))
        }
        _ => return Err(StorageError::Corrupt("Unknown resolution".into())),
    };
    Ok(UpdateProposal {
        id,
        kind,
        reason: parse_reason(&d.2)?,
        status: match d.3.as_str() {
            "Pending" => ProposalStatus::Pending,
            "Accepted" => ProposalStatus::Accepted,
            "Rejected" => ProposalStatus::Rejected,
            "Superseded" => ProposalStatus::Superseded,
            _ => return Err(StorageError::Corrupt("Unknown proposal status".into())),
        },
        created_at: sql::time(d.4)?,
        resolved_at: d.5.map(sql::time).transpose()?,
        resolution,
    })
}
pub(super) fn pending(db: &Connection, paper: PaperId) -> Result<Vec<UpdateProposal>> {
    let mut stmt=db.prepare("SELECT id FROM update_proposals WHERE paper_id=?1 AND status='Pending' ORDER BY created_at,id")?;
    let ids = stmt
        .query_map([paper.to_string()], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    ids.into_iter()
        .map(|s| read_proposal(db, sql::decode(s)?))
        .collect()
}
pub(super) fn details(db: &Connection, paper: PaperId) -> Result<Vec<MetadataProposalDetail>> {
    let current = sql::get(db, paper)?.ok_or(StorageError::NotFound(paper))?;
    pending(db, paper)?
        .into_iter()
        .map(|proposal| {
            let (expected_value, current_value, proposed_value, provenance) = match &proposal.kind {
                ProposalKind::FieldUpdate {
                    target,
                    candidate_revision,
                    ..
                } => {
                    let candidate = sql::read_revision(db, *candidate_revision)?;
                    let expected: String = db.query_row(
                        "SELECT expected_bibliography_id FROM update_proposals WHERE id=?1",
                        [proposal.id.to_string()],
                        |r| r.get(0),
                    )?;
                    (
                        Some(sql::read_bibliography(db, &expected)?.value(target.field)),
                        Some(current.bibliography().value(target.field)),
                        Some(candidate.value),
                        Some(candidate.provenance),
                    )
                }
                _ => (None, None, None, None),
            };
            let current_revisions = match &proposal.kind {
                ProposalKind::FieldUpdate { target, .. } => sql::heads(db, target)?
                    .into_iter()
                    .map(|id| sql::read_revision(db, id))
                    .collect::<Result<Vec<_>>>()?,
                _ => Vec::new(),
            };
            Ok(MetadataProposalDetail {
                current_revisions,
                proposal,
                expected_value,
                current_value,
                proposed_value,
                provenance,
            })
        })
        .collect()
}
#[allow(clippy::too_many_arguments)]
pub(super) fn propose_canonical(
    store: &SqliteLibrary,
    db: &Connection,
    paper_id: PaperId,
    version_id: VersionId,
    candidate: Option<CandidateId>,
    reason: ProposalReason,
    now: DateTime<Utc>,
    result: &mut CommitResult,
) -> Result<()> {
    let paper = sql::get(db, paper_id)?.ok_or(StorageError::NotFound(paper_id))?;
    let mut check = paper.clone();
    check.touch(now)?;
    let version =
        versions::read(db, version_id)?.ok_or(StorageError::VersionNotFound(version_id))?;
    if version.paper_id != paper_id {
        return Err(StorageError::InvalidOperation(
            "Version belongs to another paper",
        ));
    }
    if paper.canonical_version() == Some(version_id) {
        return Ok(());
    }
    let version_snapshot: String = db.query_row(
        "SELECT snapshot_id FROM paper_versions WHERE id=?1",
        [version_id.to_string()],
        |r| r.get(0),
    )?;
    let prior:Option<String>=db.query_row("SELECT id FROM update_proposals WHERE paper_id=?1 AND kind='CanonicalVersionChange' AND candidate_version_id=?2 AND expected_canonical_version_id IS ?3 AND expected_version_snapshot=?4 AND status='Pending'",params![paper_id.to_string(),version_id.to_string(),paper.canonical_version().map(|i|i.to_string()),version_snapshot],|r|r.get(0)).optional()?;
    if prior.is_some() {
        return Ok(());
    }
    let id = ProposalId::new();
    db.execute("INSERT INTO update_proposals(id,paper_id,candidate_id,kind,reason,status,created_at,candidate_version_id,expected_canonical_version_id,expected_version_snapshot) VALUES (?1,?2,?3,'CanonicalVersionChange',?4,'Pending',?5,?6,?7,?8)",params![id.to_string(),paper_id.to_string(),candidate.map(|id|id.to_string()),reason_name(reason),stamp(now),version_id.to_string(),paper.canonical_version().map(|id|id.to_string()),version_snapshot])?;
    result.created_proposals.push(id);
    result.affected_papers.push(paper_id);
    event(
        store,
        db,
        "ProposalCreated",
        paper_id,
        (candidate, Some(id)),
        now,
        result,
    )?;
    Ok(())
}
pub(super) fn set_canonical(
    store: &SqliteLibrary,
    db: &Connection,
    paper_id: PaperId,
    version_id: Option<VersionId>,
    now: DateTime<Utc>,
) -> Result<CommitResult> {
    let mut paper = sql::get(db, paper_id)?.ok_or(StorageError::NotFound(paper_id))?;
    paper.touch(now)?;
    if paper.canonical_version() == version_id {
        return Ok(CommitResult::default());
    }
    let version = version_id
        .map(|id| versions::read(db, id)?.ok_or(StorageError::VersionNotFound(id)))
        .transpose()?;
    if version.as_ref().is_some_and(|v| v.paper_id != paper_id) {
        return Err(StorageError::InvalidOperation(
            "Canonical version belongs to another paper",
        ));
    }
    paper.set_canonical_version(version_id, now)?;
    let snapshot = versions::persist_paper(db, &paper)?;
    let mut result = CommitResult {
        affected_papers: vec![paper_id],
        ..Default::default()
    };
    append_event(
        db,
        store.device_id,
        "CanonicalVersionChanged",
        &snapshot,
        None,
        now,
        &mut result,
    )?;
    if let Some(version) = version {
        for field in BibliographyField::ALL {
            let ids = sql::heads(
                db,
                &FieldTarget {
                    paper_id,
                    version_id: Some(version.id),
                    field,
                },
            )?;
            if ids.is_empty() {
                continue;
            }
            let source = sql::read_revision(db, ids[0])?.provenance;
            let manual = source.source == MetadataSource::Manual;
            let provenance = if manual {
                Provenance {
                    source: MetadataSource::Other("Selected manual version".into()),
                    source_record_id: Some(version.id.to_string()),
                    observed_at: now,
                    source_updated_at: None,
                }
            } else {
                source
            };
            let candidate = MetadataCandidate {
                id: CandidateId::from_uuid(*version.id.as_uuid()),
                present_fields: vec![field],
                bibliography: version.bibliography.clone(),
                provenance,
                version_kind: version.kind,
            };
            let current = sql::get(db, paper_id)?.ok_or(StorageError::NotFound(paper_id))?;
            let plan = plan_metadata_update(
                &current,
                &heads(db, paper_id)?,
                &candidate,
                if manual {
                    CandidateMode::UserAssociated
                } else {
                    CandidateMode::CanonicalSelection
                },
            )?;
            apply_plan(
                store,
                db,
                &current,
                &candidate,
                None,
                version.id,
                &plan,
                now,
                &mut result,
            )?;
        }
    }
    Ok(result)
}
fn same_resolution(left: &Option<ProposalResolution>, right: &ProposalResolution) -> bool {
    match (left, right) {
        (Some(ProposalResolution::Accept), ProposalResolution::Accept)
        | (Some(ProposalResolution::Reject), ProposalResolution::Reject) => true,
        (Some(ProposalResolution::CustomValue(a)), ProposalResolution::CustomValue(b)) => a == b,
        _ => false,
    }
}
pub(super) fn resolve(
    store: &SqliteLibrary,
    db: &Connection,
    id: ProposalId,
    resolution: ProposalResolution,
    now: DateTime<Utc>,
) -> Result<CommitResult> {
    let proposal = read_proposal(db, id)?;
    if proposal.status != ProposalStatus::Pending {
        if same_resolution(&proposal.resolution, &resolution) {
            return Ok(CommitResult::default());
        }
        return Err(StorageError::InvalidOperation("Proposal already resolved"));
    }
    if now < proposal.created_at {
        return Err(PaperError::InvalidTimestamp.into());
    }
    let paper_id = match proposal.kind {
        ProposalKind::FieldUpdate { target, .. } => target.paper_id,
        ProposalKind::CanonicalVersionChange { paper_id, .. } => paper_id,
        _ => return Err(StorageError::Unsupported("duplicate merge proposal")),
    };
    let mut result = CommitResult {
        affected_papers: vec![paper_id],
        ..Default::default()
    };
    let mut custom_bibliography = None;
    if !matches!(resolution, ProposalResolution::Reject) {
        let mut paper = sql::get(db, paper_id)?.ok_or(StorageError::NotFound(paper_id))?;
        paper.touch(now)?;
        let context=db.query_row("SELECT candidate_id,expected_version_snapshot,candidate_version_id,expected_canonical_version_id FROM update_proposals WHERE id=?1",[id.to_string()],|r|Ok((r.get::<_,Option<String>>(0)?,r.get::<_,Option<String>>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<String>>(3)?)))?;
        let current_version_snapshot: String = db.query_row(
            "SELECT snapshot_id FROM paper_versions WHERE id=?1",
            [&context.2],
            |r| r.get(0),
        )?;
        if context.1.as_ref() != Some(&current_version_snapshot) {
            return Err(StorageError::Conflict(paper_id));
        }
        match &proposal.kind {
            ProposalKind::FieldUpdate {
                target,
                expected_heads,
                candidate_revision,
            } => {
                if sql::heads(db, target)? != *expected_heads {
                    return Err(StorageError::Conflict(paper_id));
                }
                if let Some(candidate) = context.0 {
                    let source = read_candidate(db, sql::decode(candidate)?)?;
                    if source.status != CandidateStatus::Associated {
                        let assessment = paper_core::service::assess_identity(
                            paper.bibliography(),
                            &source.candidate.bibliography,
                        );
                        if assessment.outcome!=paper_core::service::IdentityMatch::Exact || assessment.evidence.iter().any(|e|matches!(e,paper_core::service::IdentityEvidence::UnvalidatedIdentifier {..})) {return Err(StorageError::Conflict(paper_id));}
                    }
                } else if paper.canonical_version().map(|v| v.to_string()) != context.3 {
                    return Err(StorageError::Conflict(paper_id));
                }
                let mut revision = sql::read_revision(db, *candidate_revision)?;
                if let ProposalResolution::CustomValue(value) = &resolution {
                    if value.field() != target.field {
                        return Err(StorageError::InvalidOperation(
                            "Custom value field mismatch",
                        ));
                    }
                    revision = FieldRevision {
                        id: RevisionId::new(),
                        paper_id,
                        version_id: None,
                        parents: expected_heads.clone(),
                        value: value.clone(),
                        provenance: Provenance {
                            source: MetadataSource::Manual,
                            source_record_id: None,
                            observed_at: now,
                            source_updated_at: None,
                        },
                        created_at: now,
                    };
                    let bibliography = EventId::new().to_string();
                    let mut b = Bibliography::default();
                    b.apply(value);
                    b.identifiers.validate_input().map_err(PaperError::from)?;
                    sql::write_bibliography(db, &bibliography, &b)?;
                    record_revision(db, &bibliography, &revision)?;
                    custom_bibliography = Some(bibliography);
                    result.created_revisions.push(revision.id);
                    append_event(
                        db,
                        store.device_id,
                        "FieldRevisionCreated",
                        &snapshot(db, paper_id)?,
                        Some(revision.id),
                        now,
                        &mut result,
                    )?;
                }
                paper.edit_bibliography(&revision.value, now)?;
                set_head(db, &revision)?;
                let snapshot = versions::persist_paper(db, &paper)?;
                append_event(
                    db,
                    store.device_id,
                    "FieldHeadsChanged",
                    &snapshot,
                    Some(revision.id),
                    now,
                    &mut result,
                )?;
            }
            ProposalKind::CanonicalVersionChange {
                current_version,
                candidate_version,
                ..
            } => {
                if !matches!(resolution, ProposalResolution::Accept) {
                    return Err(StorageError::InvalidOperation(
                        "Custom canonical version resolution",
                    ));
                }
                if paper.canonical_version() != *current_version {
                    return Err(StorageError::Conflict(paper_id));
                }
                combine(
                    &mut result,
                    set_canonical(store, db, paper_id, Some(*candidate_version), now)?,
                );
            }
            _ => return Err(StorageError::Unsupported("duplicate merge proposal")),
        }
    }
    let resolution_name = match resolution {
        ProposalResolution::Accept => "Accept",
        ProposalResolution::Reject => "Reject",
        ProposalResolution::CustomValue(_) => "CustomValue",
    };
    db.execute("UPDATE update_proposals SET status=?1,resolved_at=?2,resolution_kind=?3,resolution_bibliography_id=?4 WHERE id=?5",params![if matches!(resolution,ProposalResolution::Reject) {"Rejected"} else {"Accepted"},stamp(now),resolution_name,custom_bibliography,id.to_string()])?;
    let event = event(
        store,
        db,
        "ProposalResolved",
        paper_id,
        (None, Some(id)),
        now,
        &mut result,
    )?;
    db.execute(
        "UPDATE events SET resolution_kind=?1,resolution_bibliography_id=?2 WHERE id=?3",
        params![resolution_name, custom_bibliography, event.to_string()],
    )?;
    Ok(result)
}
pub(super) fn read_event(db: &Connection, id: &str, kind: &str) -> Result<SyncOperation> {
    let d=db.query_row("SELECT candidate_id,proposal_id,resolution_kind,resolution_bibliography_id,version_snapshot_id FROM events WHERE id=?1",[id],|r|Ok((r.get::<_,Option<String>>(0)?,r.get::<_,Option<String>>(1)?,r.get::<_,Option<String>>(2)?,r.get::<_,Option<String>>(3)?,r.get::<_,Option<String>>(4)?)))?;
    match kind {
        "MetadataCandidateSubmitted" => {
            let stored = read_candidate(
                db,
                sql::decode(d.0.ok_or(StorageError::Corrupt("Missing candidate event".into()))?)?,
            )?;
            Ok(SyncOperation::MetadataCandidateSubmitted {
                paper_id: stored.paper_id,
                candidate: stored.candidate,
            })
        }
        "MetadataCandidateAssociated" => {
            let version = versions::read_snapshot(
                db,
                &d.4.ok_or(StorageError::Corrupt("Missing association version".into()))?,
            )?;
            Ok(SyncOperation::MetadataCandidateAssociated {
                candidate_id: sql::decode(
                    d.0.ok_or(StorageError::Corrupt("Missing candidate event".into()))?,
                )?,
                version_id: version.id,
            })
        }
        "ProposalCreated" => {
            let mut p = read_proposal(
                db,
                sql::decode(d.1.ok_or(StorageError::Corrupt("Missing proposal event".into()))?)?,
            )?;
            p.status = ProposalStatus::Pending;
            p.resolved_at = None;
            p.resolution = None;
            Ok(SyncOperation::ProposalCreated(p))
        }
        "ProposalSuperseded" => Ok(SyncOperation::ProposalSuperseded {
            proposal_id: sql::decode(
                d.1.ok_or(StorageError::Corrupt("Missing proposal event".into()))?,
            )?,
        }),
        "ProposalResolved" => {
            let id =
                sql::decode(d.1.ok_or(StorageError::Corrupt("Missing proposal event".into()))?)?;
            let resolution = match d.2.as_deref() {
                Some("Accept") => ProposalResolution::Accept,
                Some("Reject") => ProposalResolution::Reject,
                Some("CustomValue") => {
                    let p = read_proposal(db, id)?;
                    let ProposalKind::FieldUpdate { target, .. } = p.kind else {
                        return Err(StorageError::Corrupt("Custom canonical event".into()));
                    };
                    ProposalResolution::CustomValue(
                        sql::read_bibliography(
                            db,
                            &d.3.ok_or(StorageError::Corrupt("Missing resolution value".into()))?,
                        )?
                        .value(target.field),
                    )
                }
                _ => return Err(StorageError::Corrupt("Unknown resolution event".into())),
            };
            Ok(SyncOperation::ProposalResolved {
                proposal_id: id,
                resolution,
            })
        }
        _ => Err(StorageError::Corrupt(format!("Unknown event kind {kind}"))),
    }
}

/// New-paper preview confirmation adopts supplied fields with source provenance.
/// All work is inside the caller's transaction. Canonical approval stays pending.
pub(super) fn register(
    store: &SqliteLibrary,
    db: &Connection,
    paper_id: PaperId,
    key: CitationKey,
    candidate: MetadataCandidate,
    now: DateTime<Utc>,
) -> Result<CommitResult> {
    candidate.validate()?;
    if let Some(existing) = sql::get(db, paper_id)? {
        let prior = read_candidate(db, candidate.id)?;
        if prior.paper_id == paper_id
            && prior.candidate == candidate
            && existing.citation_key() == &key
        {
            return Ok(CommitResult::default());
        }
        return Err(StorageError::InvalidOperation(
            "Registration ID reused with different content",
        ));
    }
    let paper = Paper::new(paper_id, key, Bibliography::default(), now)?;
    let mut result = store.apply_command(db, LibraryCommand::CreatePaper { paper }, now)?;
    // No identity is inferred for a new blank document. Preview confirmation is
    // the user's explicit association and adoption of the supplied metadata.
    combine(
        &mut result,
        submit(store, db, paper_id, candidate.clone(), now)?,
    );
    if read_candidate(db, candidate.id)?.status == CandidateStatus::Held {
        combine(
            &mut result,
            associate(store, db, candidate.id, read_edit_state(db, paper_id)?, now)?,
        );
    }
    for proposal in pending(db, paper_id)? {
        if matches!(proposal.kind, ProposalKind::FieldUpdate { .. }) {
            combine(
                &mut result,
                resolve(store, db, proposal.id, ProposalResolution::Accept, now)?,
            );
        }
    }
    Ok(result)
}

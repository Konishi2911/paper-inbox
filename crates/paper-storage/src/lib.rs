use chrono::{DateTime, Utc};
use paper_core::{
    command::{CommitResult, LibraryCommand, PaperEditState},
    model::*,
    ports::{ApplyEventResult, LibraryStore, PaperRepository},
    query::*,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::{
    path::Path,
    sync::{Mutex, MutexGuard},
};
mod sql;

pub type Result<T> = std::result::Result<T, StorageError>;
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Domain(#[from] PaperError),
    #[error(transparent)]
    Edit(#[from] paper_core::command::EditError),
    #[error("Paper not found: {0}")]
    NotFound(PaperId),
    #[error("Citation key already exists: {0}")]
    DuplicateCitationKey(CitationKey),
    #[error("Paper already exists: {0}")]
    DuplicatePaper(PaperId),
    #[error("Unsupported operation: {0}")]
    Unsupported(&'static str),
    #[error("Invalid stored data: {0}")]
    Corrupt(String),
    #[error("Paper changed since it was opened: {0}")]
    Conflict(PaperId),
    #[error("Database lock was poisoned")]
    Poisoned,
}

/// Local normalized SQLite store. Remote event application is not implemented yet.
pub struct SqliteLibrary {
    connection: Mutex<Connection>,
    device_id: DeviceId,
}
impl SqliteLibrary {
    pub fn open(path: impl AsRef<Path>, device_id: DeviceId) -> Result<Self> {
        Self::initialize(Connection::open(path)?, Some(device_id))
    }
    pub fn in_memory(device_id: DeviceId) -> Result<Self> {
        Self::initialize(Connection::open_in_memory()?, Some(device_id))
    }
    pub fn open_local(path: impl AsRef<Path>) -> Result<Self> {
        Self::initialize(Connection::open(path)?, None)
    }
    pub fn device_id(&self) -> DeviceId {
        self.device_id
    }
    fn initialize(mut db: Connection, requested_device: Option<DeviceId>) -> Result<Self> {
        db.busy_timeout(std::time::Duration::from_secs(5))?;
        db.pragma_update(None, "foreign_keys", true)?;
        let version: u32 = db.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > 2 {
            return Err(StorageError::Corrupt(format!(
                "Unsupported schema version {version}"
            )));
        }
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(include_str!("schema.sql"))?;
        tx.execute_batch("CREATE TABLE IF NOT EXISTS local_settings (singleton INTEGER PRIMARY KEY CHECK(singleton=1), device_id TEXT NOT NULL REFERENCES devices(id));")?;
        let local_device: Option<String> = tx
            .query_row(
                "SELECT device_id FROM local_settings WHERE singleton=1",
                [],
                |r| r.get(0),
            )
            .optional()?;
        let device_id = match requested_device {
            Some(id) => id,
            None => local_device
                .map(sql::decode)
                .transpose()?
                .unwrap_or_else(DeviceId::new),
        };

        tx.execute(
            "INSERT OR IGNORE INTO devices VALUES (?1,0)",
            [device_id.to_string()],
        )?;
        if requested_device.is_none() {
            tx.execute(
                "INSERT OR IGNORE INTO local_settings VALUES (1,?1)",
                [device_id.to_string()],
            )?;
        }
        tx.pragma_update(None, "user_version", 2)?;
        tx.commit()?;
        Ok(Self {
            connection: Mutex::new(db),
            device_id,
        })
    }
    fn lock(&self) -> Result<MutexGuard<'_, Connection>> {
        self.connection.lock().map_err(|_| StorageError::Poisoned)
    }
    /// Explicit clock for repeatable tests. A command is atomic including history and outbox.
    pub fn commit_at(&self, command: LibraryCommand, now: DateTime<Utc>) -> Result<CommitResult> {
        self.commit_batch_at(vec![command], &[], now)
    }
    pub fn load_edit_state(&self, id: PaperId) -> Result<PaperEditState> {
        let guard = self.lock()?;
        let tx = guard.unchecked_transaction()?;
        read_edit_state(&tx, id)
    }
    pub fn commit_batch_at(
        &self,
        commands: Vec<LibraryCommand>,
        expected: &[PaperEditState],
        now: DateTime<Utc>,
    ) -> Result<CommitResult> {
        let mut db = self.lock()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for state in expected {
            if read_edit_state(&tx, state.paper.id())? != *state {
                return Err(StorageError::Conflict(state.paper.id()));
            }
        }
        let mut combined = CommitResult::default();
        for command in commands {
            let result = self.apply_command(&tx, command, now)?;
            combined.affected_papers.extend(result.affected_papers);
            combined.created_revisions.extend(result.created_revisions);
            combined.created_proposals.extend(result.created_proposals);
            combined.created_events.extend(result.created_events);
        }
        combined.affected_papers.sort();
        combined.affected_papers.dedup();
        tx.commit()?;
        Ok(combined)
    }
    fn apply_command(
        &self,
        tx: &Connection,
        command: LibraryCommand,
        now: DateTime<Utc>,
    ) -> Result<CommitResult> {
        let mut result = CommitResult::default();
        let (mut paper, kind, revision) = match command {
            LibraryCommand::CreatePaper { paper } => {
                paper
                    .bibliography()
                    .identifiers
                    .validate_input()
                    .map_err(PaperError::from)?;
                if sql::get(tx, paper.id())?.is_some() {
                    return Err(StorageError::DuplicatePaper(paper.id()));
                }
                (paper, "PaperCreated", None)
            }
            LibraryCommand::EditBibliography {
                paper_id,
                version_id,
                value,
                expected_heads,
            } => {
                if version_id.is_some() {
                    return Err(StorageError::Unsupported("version editing"));
                }
                let paper = sql::get(tx, paper_id)?.ok_or(StorageError::NotFound(paper_id))?;
                let target = FieldTarget {
                    paper_id,
                    version_id,
                    field: value.field(),
                };
                let current_heads = sql::heads(tx, &target)?;
                let (next, revision) = paper_core::command::prepare_bibliography_edit(
                    &paper,
                    value,
                    &expected_heads,
                    &current_heads,
                    now,
                )?;
                (next, "FieldRevisionCreated", Some(revision))
            }
            command => {
                let id = match &command {
                    LibraryCommand::ChangeCitationKey { paper_id, .. }
                    | LibraryCommand::DeletePaper { paper_id }
                    | LibraryCommand::RestorePaper { paper_id }
                    | LibraryCommand::UpdateNotes { paper_id, .. }
                    | LibraryCommand::SetReadingStatus { paper_id, .. }
                    | LibraryCommand::SetFavorite { paper_id, .. }
                    | LibraryCommand::SetArchived { paper_id, .. } => *paper_id,
                    _ => {
                        return Err(StorageError::Unsupported(
                            "command is outside the initial local persistence milestone",
                        ));
                    }
                };
                let mut paper = sql::get(tx, id)?.ok_or(StorageError::NotFound(id))?;
                let kind = match command {
                    LibraryCommand::ChangeCitationKey { citation_key, .. } => {
                        paper.change_citation_key(citation_key, now)?;
                        "CitationKeyChanged"
                    }
                    LibraryCommand::DeletePaper { .. } => {
                        paper.delete(now)?;
                        "PaperDeleted"
                    }
                    LibraryCommand::RestorePaper { .. } => {
                        paper.restore(now)?;
                        "PaperRestored"
                    }
                    other => {
                        let mut metadata = paper.metadata().clone();
                        let kind = match other {
                            LibraryCommand::UpdateNotes { notes, .. } => {
                                metadata.notes = notes;
                                "NotesChanged"
                            }
                            LibraryCommand::SetReadingStatus { status, .. } => {
                                metadata.reading_status = status;
                                "ReadingStatusChanged"
                            }
                            LibraryCommand::SetFavorite { favorite, .. } => {
                                metadata.favorite = favorite;
                                "FavoriteChanged"
                            }
                            LibraryCommand::SetArchived { archived, .. } => {
                                metadata.archived = archived;
                                "ArchivedChanged"
                            }
                            _ => unreachable!(),
                        };
                        paper.update_metadata(metadata, now)?;
                        kind
                    }
                };
                (paper, kind, None)
            }
        };
        // Validate construction/deserialization paths too, before writing anything.
        paper = Paper::from_parts(
            paper.id(),
            paper.citation_key().clone(),
            paper.bibliography().clone(),
            paper.canonical_version(),
            paper.metadata().clone(),
            paper.created_at(),
            paper.updated_at(),
            paper.deleted_at(),
        )?;
        let owner: Option<String> = tx
            .query_row(
                "SELECT id FROM papers WHERE citation_key=?1",
                [paper.citation_key().as_str()],
                |r| r.get(0),
            )
            .optional()?;
        if owner
            .as_ref()
            .is_some_and(|id| id != &paper.id().to_string())
        {
            return Err(StorageError::DuplicateCitationKey(
                paper.citation_key().clone(),
            ));
        }
        let snapshot = EventId::new().to_string();
        sql::write_snapshot(tx, &snapshot, &paper)?;
        tx.execute("INSERT INTO papers VALUES (?1,?2,?3) ON CONFLICT(id) DO UPDATE SET citation_key=excluded.citation_key,snapshot_id=excluded.snapshot_id",
            params![paper.id().to_string(),paper.citation_key().as_str(),snapshot])?;
        result.affected_papers.push(paper.id());
        if kind == "PaperCreated" {
            append_event(
                tx,
                self.device_id,
                "PaperCreated",
                &snapshot,
                None,
                now,
                &mut result,
            )?;
            // Protect supplied initial values; leave unset fields available for external completion.
            for field in ALL_FIELDS {
                let value = sql::value(paper.bibliography(), *field);
                if !value.has_initial_value() {
                    continue;
                }
                let revision = FieldRevision {
                    id: RevisionId::new(),
                    paper_id: paper.id(),
                    version_id: None,
                    parents: vec![],
                    value,
                    provenance: Provenance {
                        source: MetadataSource::Manual,
                        source_record_id: None,
                        observed_at: now,
                        source_updated_at: None,
                    },
                    created_at: now,
                };
                save_revision(tx, &snapshot, &revision)?;
                result.created_revisions.push(revision.id);
                append_event(
                    tx,
                    self.device_id,
                    "FieldRevisionCreated",
                    &snapshot,
                    Some(revision.id),
                    now,
                    &mut result,
                )?;
                append_event(
                    tx,
                    self.device_id,
                    "FieldHeadsChanged",
                    &snapshot,
                    Some(revision.id),
                    now,
                    &mut result,
                )?;
            }
        } else if let Some(revision) = revision {
            save_revision(tx, &snapshot, &revision)?;
            result.created_revisions.push(revision.id);
            append_event(
                tx,
                self.device_id,
                "FieldRevisionCreated",
                &snapshot,
                Some(revision.id),
                now,
                &mut result,
            )?;
            append_event(
                tx,
                self.device_id,
                "FieldHeadsChanged",
                &snapshot,
                Some(revision.id),
                now,
                &mut result,
            )?;
        } else {
            append_event(tx, self.device_id, kind, &snapshot, None, now, &mut result)?;
        }
        Ok(result)
    }
}
const ALL_FIELDS: &[BibliographyField] = &[
    BibliographyField::Title,
    BibliographyField::Authors,
    BibliographyField::Editors,
    BibliographyField::PublicationType,
    BibliographyField::ContainerTitle,
    BibliographyField::Publisher,
    BibliographyField::Year,
    BibliographyField::Month,
    BibliographyField::Volume,
    BibliographyField::Issue,
    BibliographyField::Pages,
    BibliographyField::ArticleNumber,
    BibliographyField::Identifiers,
    BibliographyField::Abstract,
    BibliographyField::Keywords,
];
fn save_revision(db: &Connection, snapshot: &str, revision: &FieldRevision) -> Result<()> {
    db.execute(
        "INSERT INTO revisions VALUES (?1,?2,?3,?4,'Manual',NULL,?5,NULL,?6)",
        params![
            revision.id.to_string(),
            revision.paper_id.to_string(),
            sql::field_name(revision.value.field()),
            snapshot,
            revision
                .provenance
                .observed_at
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
            revision
                .created_at
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
        ],
    )?;
    for parent in &revision.parents {
        db.execute(
            "INSERT INTO revision_parents VALUES (?1,?2)",
            params![revision.id.to_string(), parent.to_string()],
        )?;
    }
    db.execute(
        "DELETE FROM field_heads WHERE paper_id=?1 AND field=?2",
        params![
            revision.paper_id.to_string(),
            sql::field_name(revision.value.field())
        ],
    )?;
    db.execute(
        "INSERT INTO field_heads VALUES (?1,?2,?3)",
        params![
            revision.paper_id.to_string(),
            sql::field_name(revision.value.field()),
            revision.id.to_string()
        ],
    )?;
    Ok(())
}
fn append_event(
    db: &Connection,
    device: DeviceId,
    kind: &str,
    snapshot: &str,
    revision: Option<RevisionId>,
    now: DateTime<Utc>,
    result: &mut CommitResult,
) -> Result<()> {
    let parent: Option<String> = db
        .query_row(
            "SELECT id FROM events WHERE device_id=?1 ORDER BY sequence DESC LIMIT 1",
            [device.to_string()],
            |r| r.get(0),
        )
        .optional()?;
    db.execute(
        "UPDATE devices SET sequence=sequence+1 WHERE id=?1",
        [device.to_string()],
    )?;
    let sequence: i64 = db.query_row(
        "SELECT sequence FROM devices WHERE id=?1",
        [device.to_string()],
        |r| r.get(0),
    )?;
    let id = EventId::new();
    db.execute("INSERT INTO events(id,device_id,sequence,timestamp,schema_version,kind,snapshot_id,revision_id) VALUES (?1,?2,?3,?4,1,?5,?6,?7)",
        params![id.to_string(),device.to_string(),sequence,now.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),kind,snapshot,revision.map(|r|r.to_string())])?;
    if let Some(parent) = parent {
        db.execute(
            "INSERT INTO event_parents VALUES (?1,?2)",
            params![id.to_string(), parent],
        )?;
    }
    result.created_events.push(id);
    Ok(())
}

impl PaperRepository for SqliteLibrary {
    type Error = StorageError;
    fn get(&self, id: PaperId) -> Result<Option<Paper>> {
        sql::get(&*self.lock()?, id)
    }
    fn search(&self, query: &PaperQuery) -> Result<SearchResult> {
        let guard = self.lock()?;
        let db = guard.unchecked_transaction()?;
        let f = &query.filter;
        if !f.tags.is_empty() || f.collection.is_some() || query.sort.field == SortField::Relevance
        {
            return Err(StorageError::Unsupported(
                "tag/collection filters or relevance ranking",
            ));
        }
        let mut conditions = match f.deletion {
            DeletionFilter::Active => vec!["s.deleted_at IS NULL".to_owned()],
            DeletionFilter::Deleted => vec!["s.deleted_at IS NOT NULL".to_owned()],
            DeletionFilter::All => vec!["1".to_owned()],
        };
        let mut values: Vec<rusqlite::types::Value> = vec![];
        if !f.include_archived {
            conditions.push("s.archived=0".into());
        }
        let mut condition = |clause: &str, value: rusqlite::types::Value| {
            conditions.push(clause.into());
            values.push(value);
        };
        if let Some(text) = query.text.as_ref().filter(|s| !s.is_empty()) {
            // instr treats SQL metacharacters as literal text, rather than LIKE patterns.
            condition(
                "(instr(lower(coalesce(b.title,'')),lower(?))>0 OR instr(lower(s.citation_key),lower(?))>0 OR EXISTS (SELECT 1 FROM contributors c WHERE c.bibliography_id=b.id AND c.role='author' AND instr(lower(coalesce(c.given_name,'') || ' ' || c.name),lower(?))>0))",
                text.clone().into(),
            );
            values.push(text.clone().into());
            values.push(text.clone().into());
        }
        if let Some(v) = f.publication_type {
            conditions.push("b.kind=?".into());
            values.push(sql::publication_name(v).to_owned().into());
        }
        if let Some(v) = f.reading_status {
            conditions.push("s.reading_status=?".into());
            values.push(sql::status_name(v).to_owned().into());
        }
        if let Some(v) = f.year_from {
            conditions.push("b.year>=?".into());
            values.push(i64::from(v).into());
        }
        if let Some(v) = f.year_to {
            conditions.push("b.year<=?".into());
            values.push(i64::from(v).into());
        }
        if let Some(v) = f.favorite {
            conditions.push("s.favorite=?".into());
            values.push(i64::from(v).into());
        }
        // These entities cannot yet be added through this store.
        if f.has_attachment == Some(true) || f.has_pending_proposal == Some(true) {
            conditions.push("0".into());
        }
        let from = format!(
            " FROM papers p JOIN paper_snapshots s ON p.snapshot_id=s.id JOIN bibliographies b ON s.bibliography_id=b.id WHERE {}",
            conditions.join(" AND ")
        );
        let total_count: i64 = db.query_row(
            &format!("SELECT count(*){from}"),
            rusqlite::params_from_iter(values.iter()),
            |r| r.get(0),
        )?;
        let sort = match query.sort.field {
            SortField::Title => "b.title COLLATE NOCASE",
            SortField::PublicationYear => "b.year",
            SortField::DateAdded => "s.created_at",
            SortField::DateModified => "s.updated_at",
            SortField::Relevance => unreachable!(),
        };
        let order = if query.sort.order == SortOrder::Ascending {
            "ASC"
        } else {
            "DESC"
        };
        let limit = i64::try_from(query.pagination.limit)
            .map_err(|_| StorageError::Corrupt("Pagination limit overflow".into()))?;
        let offset = i64::try_from(query.pagination.offset)
            .map_err(|_| StorageError::Corrupt("Pagination offset overflow".into()))?;
        values.push(limit.into());
        values.push(offset.into());
        let mut stmt = db.prepare(&format!(
            "SELECT s.id{from} ORDER BY {sort} {order},p.id ASC LIMIT ? OFFSET ?"
        ))?;
        let snapshots = stmt
            .query_map(rusqlite::params_from_iter(values.iter()), |r| {
                r.get::<_, String>(0)
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let papers = snapshots
            .into_iter()
            .map(|s| {
                let p = sql::read_snapshot(&db, &s)?;
                let b = p.bibliography();
                let m = p.metadata();
                Ok(PaperSummary {
                    id: p.id(),
                    citation_key: p.citation_key().clone(),
                    title: b.title.clone(),
                    authors: b.authors.clone(),
                    publication_type: b.publication.kind,
                    container_title: b.publication.container_title.clone(),
                    year: b.publication.year,
                    reading_status: m.reading_status,
                    favorite: m.favorite,
                    archived: m.archived,
                    tags: m.tags.clone(),
                    has_attachment: false,
                    has_pending_proposal: false,
                    created_at: p.created_at(),
                    updated_at: p.updated_at(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(SearchResult {
            papers,
            total_count: total_count as usize,
        })
    }
    fn versions(&self, _paper_id: PaperId) -> Result<Vec<PaperVersion>> {
        Ok(vec![])
    }
    fn attachments(&self, _paper_id: PaperId) -> Result<Vec<Attachment>> {
        Ok(vec![])
    }
    fn revisions(&self, target: &FieldTarget) -> Result<Vec<FieldRevision>> {
        if target.version_id.is_some() {
            return Err(StorageError::Unsupported("version revisions"));
        }
        let db = self.lock()?;
        let mut stmt = db.prepare(
            "SELECT id FROM revisions WHERE paper_id=?1 AND field=?2 ORDER BY created_at,id",
        )?;
        let ids = stmt
            .query_map(
                params![target.paper_id.to_string(), sql::field_name(target.field)],
                |r| r.get::<_, String>(0),
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|id| sql::read_revision(&db, sql::decode(id)?))
            .collect()
    }
    fn active_revisions(&self, target: &FieldTarget) -> Result<Vec<FieldRevision>> {
        let db = self.lock()?;
        sql::heads(&db, target)?
            .into_iter()
            .map(|id| sql::read_revision(&db, id))
            .collect()
    }
    fn pending_proposals(&self, _paper_id: PaperId) -> Result<Vec<UpdateProposal>> {
        Ok(vec![])
    }
}
impl LibraryStore for SqliteLibrary {
    fn commit(&self, command: LibraryCommand) -> Result<CommitResult> {
        self.commit_at(command, Utc::now())
    }
    fn apply_remote_event(&self, _event: &SyncEvent) -> Result<ApplyEventResult> {
        Err(StorageError::Unsupported("remote event application"))
    }
    fn pending_events(&self, limit: usize) -> Result<Vec<SyncEvent>> {
        let db = self.lock()?;
        let limit = i64::try_from(limit)
            .map_err(|_| StorageError::Corrupt("Event limit overflow".into()))?;
        // Local commits form a chain; rowid preserves insertion order across devices too.
        let mut stmt = db.prepare("SELECT id,device_id,sequence,timestamp,schema_version,kind,snapshot_id,revision_id FROM events WHERE uploaded=0 ORDER BY rowid LIMIT ?1")?;
        let rows = stmt.query_map([limit], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, u64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, u32>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, Option<String>>(7)?,
            ))
        })?;
        let mut events = vec![];
        for row in rows {
            let (id, device, sequence, timestamp, schema_version, kind, snapshot, revision) = row?;
            let p = sql::read_snapshot(&db, &snapshot)?;
            let operation = match kind.as_str() {
                "PaperCreated" => SyncOperation::PaperCreated(p),
                "FieldRevisionCreated" | "FieldHeadsChanged" => {
                    let revision = sql::read_revision(
                        &db,
                        sql::decode(
                            revision
                                .ok_or_else(|| StorageError::Corrupt("Missing revision".into()))?,
                        )?,
                    )?;
                    if kind == "FieldRevisionCreated" {
                        SyncOperation::FieldRevisionCreated(revision)
                    } else {
                        SyncOperation::FieldHeadsChanged {
                            target: revision.target(),
                            heads: vec![revision.id],
                        }
                    }
                }
                "CitationKeyChanged" => SyncOperation::CitationKeyChanged {
                    paper_id: p.id(),
                    citation_key: p.citation_key().clone(),
                },
                "NotesChanged" => SyncOperation::NotesChanged {
                    paper_id: p.id(),
                    notes: p.metadata().notes.clone(),
                },
                "ReadingStatusChanged" => SyncOperation::ReadingStatusChanged {
                    paper_id: p.id(),
                    status: p.metadata().reading_status,
                },
                "FavoriteChanged" => SyncOperation::FavoriteChanged {
                    paper_id: p.id(),
                    favorite: p.metadata().favorite,
                },
                "ArchivedChanged" => SyncOperation::ArchivedChanged {
                    paper_id: p.id(),
                    archived: p.metadata().archived,
                },
                "PaperDeleted" => SyncOperation::PaperDeleted(p.id()),
                "PaperRestored" => SyncOperation::PaperRestored(p.id()),
                _ => return Err(StorageError::Corrupt(format!("Unknown event kind {kind}"))),
            };
            let mut parent_stmt = db.prepare(
                "SELECT parent_id FROM event_parents WHERE event_id=?1 ORDER BY parent_id",
            )?;
            let parents = parent_stmt
                .query_map([&id], |r| r.get::<_, String>(0))?
                .collect::<std::result::Result<Vec<_>, _>>()?
                .into_iter()
                .map(sql::decode)
                .collect::<Result<_>>()?;
            events.push(SyncEvent {
                id: sql::decode(id)?,
                device_id: sql::decode(device)?,
                sequence,
                parents,
                timestamp: sql::time(timestamp)?,
                schema_version,
                operation,
            });
        }
        Ok(events)
    }
    fn mark_events_uploaded(&self, event_ids: &[EventId]) -> Result<()> {
        let mut db = self.lock()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for id in event_ids {
            tx.execute("UPDATE events SET uploaded=1 WHERE id=?1", [id.to_string()])?;
        }
        tx.commit()?;
        Ok(())
    }
}

fn read_edit_state(db: &Connection, id: PaperId) -> Result<PaperEditState> {
    let paper = sql::get(db, id)?.ok_or(StorageError::NotFound(id))?;
    let heads = ALL_FIELDS
        .iter()
        .map(|field| {
            Ok((
                *field,
                sql::heads(
                    db,
                    &FieldTarget {
                        paper_id: id,
                        version_id: None,
                        field: *field,
                    },
                )?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(PaperEditState { paper, heads })
}

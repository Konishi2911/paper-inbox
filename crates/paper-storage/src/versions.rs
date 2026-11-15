use super::*;

pub(super) fn migrate(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS version_snapshots (id TEXT PRIMARY KEY,version_id TEXT NOT NULL,paper_id TEXT NOT NULL REFERENCES papers(id),bibliography_id TEXT NOT NULL REFERENCES bibliographies(id),kind TEXT NOT NULL,created_at TEXT NOT NULL,updated_at TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS paper_versions (id TEXT PRIMARY KEY,paper_id TEXT NOT NULL REFERENCES papers(id),snapshot_id TEXT NOT NULL REFERENCES version_snapshots(id));")?;
    add_column(
        db,
        "paper_snapshots",
        "canonical_version",
        "TEXT REFERENCES paper_versions(id)",
    )?;
    add_column(
        db,
        "revisions",
        "version_id",
        "TEXT REFERENCES paper_versions(id)",
    )?;
    add_column(db, "revisions", "source_detail", "TEXT")?;
    add_column(
        db,
        "field_heads",
        "version_id",
        "TEXT REFERENCES paper_versions(id)",
    )?;
    add_column(
        db,
        "events",
        "version_snapshot_id",
        "TEXT REFERENCES version_snapshots(id)",
    )?;
    Ok(())
}
pub(super) fn add_column(
    db: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<()> {
    let mut stmt = db.prepare(&format!("PRAGMA table_info({table})"))?;
    let names = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if !names.iter().any(|name| name == column) {
        db.execute_batch(&format!(
            "ALTER TABLE {table} ADD COLUMN {column} {definition}"
        ))?;
    }
    Ok(())
}
pub(super) fn read(db: &Connection, id: VersionId) -> Result<Option<PaperVersion>> {
    let snapshot: Option<String> = db
        .query_row(
            "SELECT snapshot_id FROM paper_versions WHERE id=?1",
            [id.to_string()],
            |r| r.get(0),
        )
        .optional()?;
    snapshot.map(|s| read_snapshot(db, &s)).transpose()
}
pub(super) fn read_snapshot(db: &Connection, id: &str) -> Result<PaperVersion> {
    let data=db.query_row("SELECT version_id,paper_id,bibliography_id,kind,created_at,updated_at FROM version_snapshots WHERE id=?1",[id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?)))?;
    let version = PaperVersion {
        id: sql::decode(data.0)?,
        paper_id: sql::decode(data.1)?,
        bibliography: sql::read_bibliography(db, &data.2)?,
        kind: sql::version_kind(data.3)?,
        created_at: sql::time(data.4)?,
        updated_at: sql::time(data.5)?,
    };
    version.validate()?;
    Ok(version)
}
pub(super) fn write(db: &Connection, version: &PaperVersion) -> Result<String> {
    version.validate()?;
    let snapshot = EventId::new().to_string();
    sql::write_bibliography(db, &snapshot, &version.bibliography)?;
    db.execute(
        "INSERT INTO version_snapshots VALUES (?1,?2,?3,?1,?4,?5,?6)",
        params![
            snapshot,
            version.id.to_string(),
            version.paper_id.to_string(),
            sql::version_name(version.kind),
            version
                .created_at
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
            version
                .updated_at
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
        ],
    )?;
    db.execute("INSERT INTO paper_versions VALUES (?1,?2,?3) ON CONFLICT(id) DO UPDATE SET snapshot_id=excluded.snapshot_id",params![version.id.to_string(),version.paper_id.to_string(),snapshot])?;
    Ok(snapshot)
}
pub(super) fn persist_paper(db: &Connection, paper: &Paper) -> Result<String> {
    let snapshot = EventId::new().to_string();
    sql::write_snapshot(db, &snapshot, paper)?;
    db.execute(
        "UPDATE papers SET snapshot_id=?1 WHERE id=?2",
        params![snapshot, paper.id().to_string()],
    )?;
    Ok(snapshot)
}
pub(super) fn add(
    store: &SqliteLibrary,
    db: &Connection,
    version: PaperVersion,
    source: &Provenance,
    present: &[BibliographyField],
    now: DateTime<Utc>,
) -> Result<CommitResult> {
    if read(db, version.id)?.is_some() {
        return Err(StorageError::InvalidOperation("Version already exists"));
    }
    version.validate()?;
    version
        .bibliography
        .identifiers
        .validate_input()
        .map_err(PaperError::from)?;
    let mut paper =
        sql::get(db, version.paper_id)?.ok_or(StorageError::NotFound(version.paper_id))?;
    paper.touch(now)?;
    let version_snapshot = write(db, &version)?;
    let snapshot = persist_paper(db, &paper)?;
    let mut result = CommitResult {
        affected_papers: vec![paper.id()],
        ..Default::default()
    };
    append_event(
        db,
        store.device_id,
        "PaperVersionAdded",
        &snapshot,
        None,
        now,
        &mut result,
    )?;
    let event = result
        .created_events
        .last()
        .ok_or(StorageError::Corrupt("Missing version event".into()))?;
    db.execute(
        "UPDATE events SET version_snapshot_id=?1 WHERE id=?2",
        params![version_snapshot, event.to_string()],
    )?;
    for field in present {
        let revision = FieldRevision {
            id: RevisionId::new(),
            paper_id: paper.id(),
            version_id: Some(version.id),
            parents: vec![],
            value: version.bibliography.value(*field),
            provenance: source.clone(),
            created_at: now,
        };
        save_revision(db, &version_snapshot, &revision)?;
        result.created_revisions.push(revision.id);
        append_event(
            db,
            store.device_id,
            "FieldRevisionCreated",
            &snapshot,
            Some(revision.id),
            now,
            &mut result,
        )?;
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
    Ok(result)
}
pub(super) fn edit(
    store: &SqliteLibrary,
    db: &Connection,
    paper_id: PaperId,
    id: VersionId,
    value: BibliographyValue,
    expected: Vec<RevisionId>,
    now: DateTime<Utc>,
) -> Result<CommitResult> {
    let mut version = read(db, id)?.ok_or(StorageError::VersionNotFound(id))?;
    if version.paper_id != paper_id {
        return Err(StorageError::InvalidOperation(
            "Version belongs to another paper",
        ));
    }
    let mut paper = sql::get(db, paper_id)?.ok_or(StorageError::NotFound(paper_id))?;
    paper.touch(now)?;
    let target = FieldTarget {
        paper_id,
        version_id: Some(id),
        field: value.field(),
    };
    let actual = sql::heads(db, &target)?;
    let mut expected = expected;
    expected.sort();
    if expected.windows(2).any(|p| p[0] == p[1]) {
        return Err(paper_core::command::EditError::DuplicateHeads.into());
    }
    if expected != actual {
        return Err(paper_core::command::EditError::Conflict { expected, actual }.into());
    }
    version.edit_bibliography(&value, now)?;
    let b = write(db, &version)?;
    let snapshot = persist_paper(db, &paper)?;
    let revision = FieldRevision {
        id: RevisionId::new(),
        paper_id,
        version_id: Some(id),
        parents: actual,
        value,
        provenance: Provenance {
            source: MetadataSource::Manual,
            source_record_id: None,
            observed_at: now,
            source_updated_at: None,
        },
        created_at: now,
    };
    save_revision(db, &b, &revision)?;
    let mut result = CommitResult {
        affected_papers: vec![paper_id],
        created_revisions: vec![revision.id],
        ..Default::default()
    };
    append_event(
        db,
        store.device_id,
        "FieldRevisionCreated",
        &snapshot,
        Some(revision.id),
        now,
        &mut result,
    )?;
    append_event(
        db,
        store.device_id,
        "FieldHeadsChanged",
        &snapshot,
        Some(revision.id),
        now,
        &mut result,
    )?;
    Ok(result)
}

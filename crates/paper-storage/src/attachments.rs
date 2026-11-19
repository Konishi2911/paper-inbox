//! Attachment rows. The files themselves live in the content-addressed `FileBlobStore`;
//! this table records which paper refers to which hash. Removing an attachment only marks
//! the row, so the history keeps the metadata and the file is reclaimed explicitly later.
use super::{Result, SqliteLibrary, StorageError, append_event, blob, sql};
use chrono::{DateTime, SecondsFormat, Utc};
use paper_core::{
    command::CommitResult,
    model::{Attachment, AttachmentId, ContentHash, PaperId, SyncOperation, VersionId},
};
use rusqlite::{Connection, OptionalExtension, params};

pub(super) fn migrate(db: &Connection) -> Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS attachments (
 id TEXT PRIMARY KEY, paper_id TEXT NOT NULL REFERENCES papers(id),
 version_id TEXT REFERENCES paper_versions(id),
 content_hash TEXT NOT NULL CHECK(length(content_hash)=64),
 filename TEXT NOT NULL, media_type TEXT NOT NULL,
 size_bytes INTEGER NOT NULL CHECK(size_bytes >= 0), kind TEXT NOT NULL,
 created_at TEXT NOT NULL, removed_at TEXT
);
CREATE INDEX IF NOT EXISTS attachments_by_paper ON attachments(paper_id, removed_at);
CREATE INDEX IF NOT EXISTS attachments_by_hash ON attachments(content_hash);",
    )?;
    super::versions::add_column(
        db,
        "events",
        "attachment_id",
        "TEXT REFERENCES attachments(id)",
    )
}

fn validate(attachment: &Attachment) -> Result<()> {
    let name = &attachment.filename;
    if name.trim().is_empty()
        || name.chars().count() > 255
        || name
            .chars()
            .any(|c| c.is_control() || c == '/' || c == '\\')
    {
        return Err(StorageError::InvalidOperation(
            "the file name is empty, too long or contains a path separator",
        ));
    }
    if attachment.media_type.trim().is_empty()
        || attachment.media_type.chars().count() > 127
        || attachment.media_type.chars().any(char::is_control)
    {
        return Err(StorageError::InvalidOperation("the media type is invalid"));
    }
    if i64::try_from(attachment.size_bytes).is_err() {
        return Err(StorageError::InvalidOperation("the file size is too large"));
    }
    Ok(())
}

fn event(
    store: &SqliteLibrary,
    db: &Connection,
    kind: &str,
    paper_id: PaperId,
    attachment_id: AttachmentId,
    now: DateTime<Utc>,
    result: &mut CommitResult,
) -> Result<()> {
    let snapshot: String = db.query_row(
        "SELECT snapshot_id FROM papers WHERE id=?1",
        [paper_id.to_string()],
        |r| r.get(0),
    )?;
    append_event(db, store.device_id, kind, &snapshot, None, now, result)?;
    let event = *result
        .created_events
        .last()
        .ok_or_else(|| StorageError::Corrupt("Missing attachment event".into()))?;
    db.execute(
        "UPDATE events SET attachment_id=?1 WHERE id=?2",
        params![attachment_id.to_string(), event.to_string()],
    )?;
    result.affected_papers.push(paper_id);
    Ok(())
}

pub(super) fn add(
    store: &SqliteLibrary,
    db: &Connection,
    attachment: Attachment,
    now: DateTime<Utc>,
) -> Result<CommitResult> {
    validate(&attachment)?;
    let paper =
        sql::get(db, attachment.paper_id)?.ok_or(StorageError::NotFound(attachment.paper_id))?;
    if paper.deleted_at().is_some() {
        return Err(StorageError::InvalidOperation(
            "files cannot be attached to a deleted paper",
        ));
    }
    if let Some(version_id) = attachment.version_id {
        let known: bool = db.query_row(
            "SELECT EXISTS(SELECT 1 FROM paper_versions WHERE id=?1 AND paper_id=?2)",
            params![version_id.to_string(), attachment.paper_id.to_string()],
            |r| r.get(0),
        )?;
        if !known {
            return Err(StorageError::VersionNotFound(version_id));
        }
    }
    let hash = blob::content_hash_hex(&attachment.content_hash);
    let duplicate: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM attachments WHERE paper_id=?1 AND content_hash=?2 AND removed_at IS NULL)",
        params![attachment.paper_id.to_string(), hash],
        |r| r.get(0),
    )?;
    if duplicate {
        return Err(StorageError::InvalidOperation(
            "the same file is already attached to this paper",
        ));
    }
    let known_id: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM attachments WHERE id=?1)",
        [attachment.id.to_string()],
        |r| r.get(0),
    )?;
    if known_id {
        return Err(StorageError::InvalidOperation(
            "an attachment with this id already exists",
        ));
    }
    db.execute(
        "INSERT INTO attachments(id,paper_id,version_id,content_hash,filename,media_type,size_bytes,kind,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![
            attachment.id.to_string(),
            attachment.paper_id.to_string(),
            attachment.version_id.map(|v| v.to_string()),
            hash,
            attachment.filename,
            attachment.media_type,
            attachment.size_bytes as i64,
            sql::attachment_kind_name(attachment.kind),
            attachment
                .created_at
                .to_rfc3339_opts(SecondsFormat::Nanos, true),
        ],
    )?;
    let mut result = CommitResult::default();
    event(
        store,
        db,
        "AttachmentAdded",
        attachment.paper_id,
        attachment.id,
        now,
        &mut result,
    )?;
    Ok(result)
}

pub(super) fn remove(
    store: &SqliteLibrary,
    db: &Connection,
    attachment_id: AttachmentId,
    now: DateTime<Utc>,
) -> Result<CommitResult> {
    let paper: Option<String> = db
        .query_row(
            "SELECT paper_id FROM attachments WHERE id=?1 AND removed_at IS NULL",
            [attachment_id.to_string()],
            |r| r.get(0),
        )
        .optional()?;
    let paper_id: PaperId = sql::decode(paper.ok_or(StorageError::InvalidOperation(
        "the attachment does not exist or was already removed",
    ))?)?;
    db.execute(
        "UPDATE attachments SET removed_at=?1 WHERE id=?2",
        params![
            now.to_rfc3339_opts(SecondsFormat::Nanos, true),
            attachment_id.to_string()
        ],
    )?;
    let mut result = CommitResult::default();
    event(
        store,
        db,
        "AttachmentRemoved",
        paper_id,
        attachment_id,
        now,
        &mut result,
    )?;
    Ok(result)
}

fn read(db: &Connection, id: &str) -> Result<Attachment> {
    let row = db.query_row(
        "SELECT id,paper_id,version_id,content_hash,filename,media_type,size_bytes,kind,created_at FROM attachments WHERE id=?1",
        [id],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, String>(8)?,
            ))
        },
    )?;
    Ok(Attachment {
        id: sql::decode(row.0)?,
        paper_id: sql::decode(row.1)?,
        version_id: row.2.map(sql::decode::<VersionId>).transpose()?,
        content_hash: blob::parse_content_hash(&row.3)
            .ok_or_else(|| StorageError::Corrupt(format!("Invalid content hash: {}", row.3)))?,
        filename: row.4,
        media_type: row.5,
        size_bytes: u64::try_from(row.6)
            .map_err(|_| StorageError::Corrupt("Negative attachment size".into()))?,
        kind: sql::attachment_kind(row.7)?,
        created_at: sql::time(row.8)?,
    })
}

/// Attachments still in use by the paper, oldest first.
pub(super) fn list(db: &Connection, paper_id: PaperId) -> Result<Vec<Attachment>> {
    let mut stmt = db.prepare(
        "SELECT id FROM attachments WHERE paper_id=?1 AND removed_at IS NULL ORDER BY created_at,id",
    )?;
    let ids = stmt
        .query_map([paper_id.to_string()], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    ids.iter().map(|id| read(db, id)).collect()
}

/// Hashes still referenced by an attachment that has not been removed.
pub(super) fn referenced_hashes(db: &Connection) -> Result<Vec<ContentHash>> {
    let mut stmt = db.prepare(
        "SELECT DISTINCT content_hash FROM attachments WHERE removed_at IS NULL ORDER BY content_hash",
    )?;
    let texts = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    texts
        .into_iter()
        .map(|t| {
            blob::parse_content_hash(&t)
                .ok_or_else(|| StorageError::Corrupt(format!("Invalid content hash: {t}")))
        })
        .collect()
}

pub(super) fn read_event(db: &Connection, event_id: &str, kind: &str) -> Result<SyncOperation> {
    let attachment: Option<String> = db.query_row(
        "SELECT attachment_id FROM events WHERE id=?1",
        [event_id],
        |r| r.get(0),
    )?;
    let attachment = attachment
        .ok_or_else(|| StorageError::Corrupt("Missing attachment event reference".into()))?;
    if kind == "AttachmentAdded" {
        Ok(SyncOperation::AttachmentAdded(read(db, &attachment)?))
    } else {
        Ok(SyncOperation::AttachmentRemoved {
            attachment_id: sql::decode(attachment)?,
        })
    }
}

//! Flat tags. A tag is a name shared by papers; `name_key` makes names unique after
//! folding case, width and spacing (see `paper_core::model::normalize_tag_name`). The
//! assignments live outside the immutable paper snapshots, so `Paper::metadata().tags`
//! is not filled; ask `paper_tags` or the search summary. Assignments are kept when a
//! paper is deleted, so restoring it brings its tags back.
use super::{Result, SqliteLibrary, StorageError, append_event_for as append_event, sql};
use chrono::{DateTime, SecondsFormat, Utc};
use paper_core::{
    command::CommitResult,
    model::{PaperId, SyncOperation, Tag, TagId, TagNameError, normalize_tag_name},
    query::TagCount,
};
use rusqlite::{Connection, OptionalExtension, params};

pub(super) fn migrate(db: &Connection) -> Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS tags (
 id TEXT PRIMARY KEY, name TEXT NOT NULL, name_key TEXT NOT NULL UNIQUE,
 created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS paper_tags (
 paper_id TEXT NOT NULL REFERENCES papers(id), tag_id TEXT NOT NULL REFERENCES tags(id),
 added_at TEXT NOT NULL, PRIMARY KEY(paper_id, tag_id)
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS paper_tags_by_tag ON paper_tags(tag_id, paper_id);",
    )?;
    super::versions::add_column(db, "events", "tag_id", "TEXT REFERENCES tags(id)")
}

fn name_error(error: TagNameError) -> StorageError {
    StorageError::InvalidOperation(match error {
        TagNameError::Empty => "the tag name is empty",
        TagNameError::TooLong => "the tag name is too long",
        TagNameError::Control => "the tag name contains a control character",
    })
}

fn time(now: DateTime<Utc>) -> String {
    now.to_rfc3339_opts(SecondsFormat::Nanos, true)
}

fn tag_event(
    store: &SqliteLibrary,
    db: &Connection,
    kind: &str,
    snapshot: Option<&str>,
    tag_id: TagId,
    now: DateTime<Utc>,
    result: &mut CommitResult,
) -> Result<()> {
    append_event(db, store.device_id, kind, snapshot, None, now, result)?;
    let event = *result
        .created_events
        .last()
        .ok_or_else(|| StorageError::Corrupt("Missing tag event".into()))?;
    db.execute(
        "UPDATE events SET tag_id=?1 WHERE id=?2",
        params![tag_id.to_string(), event.to_string()],
    )?;
    Ok(())
}

pub(super) fn create(
    store: &SqliteLibrary,
    db: &Connection,
    tag: Tag,
    now: DateTime<Utc>,
) -> Result<CommitResult> {
    let (name, key) = normalize_tag_name(&tag.name).map_err(name_error)?;
    let id_known: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM tags WHERE id=?1)",
        [tag.id.to_string()],
        |r| r.get(0),
    )?;
    if id_known {
        return Err(StorageError::InvalidOperation(
            "a tag with this id already exists",
        ));
    }
    let name_known: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM tags WHERE name_key=?1)",
        [&key],
        |r| r.get(0),
    )?;
    if name_known {
        return Err(StorageError::InvalidOperation(
            "a tag with this name already exists",
        ));
    }
    db.execute(
        "INSERT INTO tags(id,name,name_key,created_at) VALUES (?1,?2,?3,?4)",
        params![tag.id.to_string(), name, key, time(now)],
    )?;
    let mut result = CommitResult::default();
    tag_event(store, db, "TagCreated", None, tag.id, now, &mut result)?;
    Ok(result)
}

fn known_tag(db: &Connection, tag_id: TagId) -> Result<()> {
    let known: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM tags WHERE id=?1)",
        [tag_id.to_string()],
        |r| r.get(0),
    )?;
    if known {
        Ok(())
    } else {
        Err(StorageError::InvalidOperation("the tag does not exist"))
    }
}

fn snapshot_of(db: &Connection, paper_id: PaperId) -> Result<String> {
    db.query_row(
        "SELECT snapshot_id FROM papers WHERE id=?1",
        [paper_id.to_string()],
        |r| r.get(0),
    )
    .optional()?
    .ok_or(StorageError::NotFound(paper_id))
}

pub(super) fn add(
    store: &SqliteLibrary,
    db: &Connection,
    paper_id: PaperId,
    tag_id: TagId,
    now: DateTime<Utc>,
) -> Result<CommitResult> {
    let paper = sql::get(db, paper_id)?.ok_or(StorageError::NotFound(paper_id))?;
    if paper.deleted_at().is_some() {
        return Err(StorageError::InvalidOperation(
            "tags cannot be added to a deleted paper",
        ));
    }
    known_tag(db, tag_id)?;
    let assigned: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM paper_tags WHERE paper_id=?1 AND tag_id=?2)",
        params![paper_id.to_string(), tag_id.to_string()],
        |r| r.get(0),
    )?;
    if assigned {
        return Err(StorageError::InvalidOperation(
            "the paper already has this tag",
        ));
    }
    db.execute(
        "INSERT INTO paper_tags(paper_id,tag_id,added_at) VALUES (?1,?2,?3)",
        params![paper_id.to_string(), tag_id.to_string(), time(now)],
    )?;
    let snapshot = snapshot_of(db, paper_id)?;
    let mut result = CommitResult::default();
    tag_event(
        store,
        db,
        "TagAdded",
        Some(&snapshot),
        tag_id,
        now,
        &mut result,
    )?;
    result.affected_papers.push(paper_id);
    Ok(result)
}

pub(super) fn remove(
    store: &SqliteLibrary,
    db: &Connection,
    paper_id: PaperId,
    tag_id: TagId,
    now: DateTime<Utc>,
) -> Result<CommitResult> {
    let snapshot = snapshot_of(db, paper_id)?;
    let removed = db.execute(
        "DELETE FROM paper_tags WHERE paper_id=?1 AND tag_id=?2",
        params![paper_id.to_string(), tag_id.to_string()],
    )?;
    if removed == 0 {
        return Err(StorageError::InvalidOperation(
            "the paper does not have this tag",
        ));
    }
    let mut result = CommitResult::default();
    tag_event(
        store,
        db,
        "TagRemoved",
        Some(&snapshot),
        tag_id,
        now,
        &mut result,
    )?;
    result.affected_papers.push(paper_id);
    Ok(result)
}

fn read(db: &Connection, id: &str) -> Result<Tag> {
    let name: String = db.query_row("SELECT name FROM tags WHERE id=?1", [id], |r| r.get(0))?;
    Ok(Tag {
        id: sql::decode(id.to_owned())?,
        name,
    })
}

/// Every tag by name with its paper count; deleted papers never count.
pub(super) fn counts(db: &Connection, include_archived: bool) -> Result<Vec<TagCount>> {
    let mut stmt = db.prepare(
        "SELECT t.id, t.name,
  (SELECT COUNT(*) FROM paper_tags pt
    JOIN papers p ON p.id=pt.paper_id
    JOIN paper_snapshots s ON s.id=p.snapshot_id
    WHERE pt.tag_id=t.id AND s.deleted_at IS NULL AND (?1 OR s.archived=0))
 FROM tags t ORDER BY t.name_key, t.id",
    )?;
    let rows = stmt
        .query_map([include_archived], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    rows.into_iter()
        .map(|(id, name, count)| {
            Ok(TagCount {
                tag: Tag {
                    id: sql::decode(id)?,
                    name,
                },
                paper_count: usize::try_from(count)
                    .map_err(|_| StorageError::Corrupt("Negative tag count".into()))?,
            })
        })
        .collect()
}

pub(super) fn of_paper(db: &Connection, paper_id: PaperId) -> Result<Vec<Tag>> {
    let mut stmt = db.prepare(
        "SELECT t.id FROM paper_tags pt JOIN tags t ON t.id=pt.tag_id
 WHERE pt.paper_id=?1 ORDER BY t.name_key, t.id",
    )?;
    let ids = stmt
        .query_map([paper_id.to_string()], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    ids.iter().map(|id| read(db, id)).collect()
}

pub(super) fn read_event(
    db: &Connection,
    event_id: &str,
    kind: &str,
    paper: Option<PaperId>,
) -> Result<SyncOperation> {
    let tag: Option<String> =
        db.query_row("SELECT tag_id FROM events WHERE id=?1", [event_id], |r| {
            r.get(0)
        })?;
    let tag = tag.ok_or_else(|| StorageError::Corrupt("Missing tag event reference".into()))?;
    let paper_id = || {
        paper.ok_or_else(|| StorageError::Corrupt("Tag assignment event without a paper".into()))
    };
    match kind {
        "TagCreated" => Ok(SyncOperation::TagCreated(read(db, &tag)?)),
        "TagAdded" => Ok(SyncOperation::TagAdded {
            paper_id: paper_id()?,
            tag_id: sql::decode(tag)?,
        }),
        _ => Ok(SyncOperation::TagRemoved {
            paper_id: paper_id()?,
            tag_id: sql::decode(tag)?,
        }),
    }
}

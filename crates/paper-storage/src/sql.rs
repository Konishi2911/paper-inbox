use crate::{Result, StorageError};
use chrono::{DateTime, Utc};
use paper_core::model::*;
use rusqlite::{Connection, OptionalExtension, params};
use serde::de::{DeserializeOwned, value::StringDeserializer};

pub fn decode<T: DeserializeOwned>(value: String) -> Result<T> {
    T::deserialize(StringDeserializer::<serde::de::value::Error>::new(value))
        .map_err(|e| StorageError::Corrupt(e.to_string()))
}
pub fn time(value: String) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&value)
        .map(|v| v.with_timezone(&Utc))
        .map_err(|e| StorageError::Corrupt(e.to_string()))
}
macro_rules! codec {
    ($encode:ident, $decode:ident, $ty:ident, [$($variant:ident),*]) => {
        pub fn $encode(value: $ty) -> &'static str {
            match value { $($ty::$variant => stringify!($variant)),* }
        }
        pub fn $decode(value: String) -> Result<$ty> {
            match value.as_str() { $(stringify!($variant) => Ok($ty::$variant),)*
                _ => Err(StorageError::Corrupt(format!("Unknown {}: {value}", stringify!($ty)))) }
        }
    }
}
codec!(
    publication_name,
    publication,
    PublicationType,
    [
        JournalArticle,
        ConferencePaper,
        Book,
        BookChapter,
        MastersThesis,
        DoctoralThesis,
        Thesis,
        TechnicalReport,
        Preprint,
        Patent,
        Other,
        Unknown
    ]
);
codec!(status_name, status, ReadingStatus, [Unread, Reading, Read]);
codec!(
    field_name,
    field,
    BibliographyField,
    [
        Title,
        Authors,
        Editors,
        PublicationType,
        ContainerTitle,
        Publisher,
        Year,
        Month,
        Volume,
        Issue,
        Pages,
        ArticleNumber,
        Identifiers,
        Abstract,
        Keywords
    ]
);

pub fn write_bibliography(db: &Connection, id: &str, b: &Bibliography) -> Result<()> {
    b.validate()?;
    db.execute("INSERT INTO bibliographies VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
        params![id,b.title,publication_name(b.publication.kind),b.publication.container_title,b.publication.publisher,
        b.publication.year,b.publication.month,b.publication.volume,b.publication.issue,b.publication.pages,
        b.publication.article_number, b.identifiers.doi_text(), b.identifiers.arxiv_text(),
        b.identifiers.isbn,b.identifiers.issn,b.identifiers.url,b.abstract_text])?;
    for (role, people) in [("author", &b.authors), ("editor", &b.editors)] {
        for (position, c) in people.iter().enumerate() {
            let (kind, given, name) = match &c.name {
                ContributorName::Person { given, family } => ("Person", given.as_deref(), family),
                ContributorName::Organization(name) => ("Organization", None, name),
                ContributorName::Literal(name) => ("Literal", None, name),
            };
            db.execute(
                "INSERT INTO contributors VALUES (?1,?2,?3,?4,?5,?6,?7)",
                params![id, role, position as i64, kind, given, name, c.orcid],
            )?;
        }
    }
    for (i, word) in b.keywords.iter().enumerate() {
        db.execute(
            "INSERT INTO keywords VALUES (?1,?2,?3)",
            params![id, i as i64, word],
        )?;
    }
    Ok(())
}

pub fn read_bibliography(db: &Connection, id: &str) -> Result<Bibliography> {
    let mut b = db.query_row("SELECT title,kind,container_title,publisher,year,month,volume,issue,pages,article_number,doi,arxiv_id,isbn,issn,url,abstract_text FROM bibliographies WHERE id=?1", [id], |r| {
        Ok((Bibliography {
            title: r.get(0)?, publication: Publication {
                container_title: r.get(2)?, publisher: r.get(3)?,year: r.get(4)?,month:r.get(5)?,
                volume:r.get(6)?,issue:r.get(7)?,pages:r.get(8)?,article_number:r.get(9)?,..Default::default()
            }, identifiers: Identifiers { isbn:r.get(12)?,issn:r.get(13)?,url:r.get(14)?,..Default::default() },
            abstract_text:r.get(15)?,..Default::default()
        }, r.get::<_,String>(1)?,r.get::<_,Option<String>>(10)?,r.get::<_,Option<String>>(11)?))
    })?;
    b.0.publication.kind = publication(b.1)?;
    b.0.identifiers.restore_doi(b.2);
    b.0.identifiers.restore_arxiv(b.3);
    let mut stmt = db.prepare("SELECT role,kind,given_name,name,orcid FROM contributors WHERE bibliography_id=?1 ORDER BY role,position")?;
    let rows = stmt.query_map([id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, Option<String>>(4)?,
        ))
    })?;
    for row in rows {
        let (role, kind, given, name, orcid) = row?;
        let name = match kind.as_str() {
            "Person" => ContributorName::Person {
                given,
                family: name,
            },
            "Organization" => ContributorName::Organization(name),
            "Literal" => ContributorName::Literal(name),
            _ => return Err(StorageError::Corrupt("Unknown contributor kind".into())),
        };
        let list = if role == "author" {
            &mut b.0.authors
        } else {
            &mut b.0.editors
        };
        list.push(Contributor { name, orcid });
    }
    let mut stmt =
        db.prepare("SELECT value FROM keywords WHERE bibliography_id=?1 ORDER BY position")?;
    b.0.keywords = stmt
        .query_map([id], |r| r.get(0))?
        .collect::<std::result::Result<_, _>>()?;
    b.0.validate()?;
    Ok(b.0)
}

pub fn write_snapshot(db: &Connection, id: &str, paper: &Paper) -> Result<()> {
    write_bibliography(db, id, paper.bibliography())?;
    let m = paper.metadata();
    if !m.tags.is_empty() || !m.collections.is_empty() {
        return Err(StorageError::Unsupported(
            "tags and collections are not implemented yet",
        ));
    }
    if let Some(id) = paper.canonical_version() {
        let v = super::versions::read(db, id)?.ok_or(StorageError::VersionNotFound(id))?;
        if v.paper_id != paper.id() {
            return Err(StorageError::InvalidOperation(
                "Canonical version belongs to another paper",
            ));
        }
    }
    db.execute(
        "INSERT INTO paper_snapshots(id,paper_id,bibliography_id,citation_key,created_at,updated_at,deleted_at,notes,reading_status,favorite,archived,canonical_version) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        params![
            id,
            paper.id().to_string(),
            id,
            paper.citation_key().as_str(),
            paper
                .created_at()
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
            paper
                .updated_at()
                .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true),
            paper
                .deleted_at()
                .map(|t| t.to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)),
            m.notes,
            status_name(m.reading_status),
            m.favorite,
            m.archived,
            paper.canonical_version().map(|id|id.to_string())
        ],
    )?;
    Ok(())
}
pub fn read_snapshot(db: &Connection, id: &str) -> Result<Paper> {
    let data = db.query_row("SELECT paper_id,bibliography_id,citation_key,created_at,updated_at,deleted_at,notes,reading_status,favorite,archived FROM paper_snapshots WHERE id=?1", [id], |r| {
        Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,String>(6)?,r.get::<_,String>(7)?,r.get::<_,bool>(8)?,r.get::<_,bool>(9)?))
    })?;
    let canonical: Option<String> = db.query_row(
        "SELECT canonical_version FROM paper_snapshots WHERE id=?1",
        [id],
        |r| r.get(0),
    )?;
    Ok(Paper::from_parts(
        decode(data.0)?,
        decode(data.2)?,
        read_bibliography(db, &data.1)?,
        canonical.map(decode).transpose()?,
        UserMetadata {
            notes: data.6,
            reading_status: status(data.7)?,
            favorite: data.8,
            archived: data.9,
            ..Default::default()
        },
        time(data.3)?,
        time(data.4)?,
        data.5.map(time).transpose()?,
    )?)
}
pub fn get(db: &Connection, id: PaperId) -> Result<Option<Paper>> {
    let snapshot: Option<String> = db
        .query_row(
            "SELECT snapshot_id FROM papers WHERE id=?1",
            [id.to_string()],
            |r| r.get(0),
        )
        .optional()?;
    snapshot.map(|s| read_snapshot(db, &s)).transpose()
}

pub fn value(b: &Bibliography, field: BibliographyField) -> BibliographyValue {
    b.value(field)
}
pub fn heads(db: &Connection, target: &FieldTarget) -> Result<Vec<RevisionId>> {
    let mut stmt = db.prepare(
        "SELECT revision_id FROM field_heads WHERE paper_id=?1 AND field=?2 AND version_id IS ?3 ORDER BY revision_id",
    )?;
    let ids = stmt
        .query_map(
            params![
                target.paper_id.to_string(),
                field_name(target.field),
                target.version_id.map(|id| id.to_string())
            ],
            |r| r.get::<_, String>(0),
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    ids.into_iter().map(decode).collect()
}
pub fn read_revision(db: &Connection, id: RevisionId) -> Result<FieldRevision> {
    let d = db.query_row("SELECT paper_id,field,bibliography_id,source,source_record_id,observed_at,source_updated_at,created_at FROM revisions WHERE id=?1", [id.to_string()], |r| {
        Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,String>(5)?,r.get::<_,Option<String>>(6)?,r.get::<_,String>(7)?))
    })?;
    let extra = db.query_row(
        "SELECT version_id,source_detail FROM revisions WHERE id=?1",
        [id.to_string()],
        |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, Option<String>>(1)?,
            ))
        },
    )?;
    let mut stmt = db.prepare(
        "SELECT parent_id FROM revision_parents WHERE revision_id=?1 ORDER BY parent_id",
    )?;
    let parents = stmt
        .query_map([id.to_string()], |r| r.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .map(decode)
        .collect::<Result<Vec<_>>>()?;
    Ok(FieldRevision {
        id,
        paper_id: decode(d.0)?,
        version_id: extra.0.map(decode).transpose()?,
        parents,
        value: value(&read_bibliography(db, &d.2)?, field(d.1)?),
        provenance: Provenance {
            source: source(d.3, extra.1)?,
            source_record_id: d.4,
            observed_at: time(d.5)?,
            source_updated_at: d.6.map(time).transpose()?,
        },
        created_at: time(d.7)?,
    })
}

codec!(
    version_name,
    version_kind,
    VersionKind,
    [
        VersionOfRecord,
        AcceptedManuscript,
        SubmittedManuscript,
        Preprint,
        Unknown
    ]
);
pub fn source_name(source: &MetadataSource) -> (&'static str, Option<&str>) {
    match source {
        MetadataSource::Manual => ("Manual", None),
        MetadataSource::Publisher => ("Publisher", None),
        MetadataSource::Crossref => ("Crossref", None),
        MetadataSource::OpenAlex => ("OpenAlex", None),
        MetadataSource::Arxiv => ("Arxiv", None),
        MetadataSource::PdfEmbedded => ("PdfEmbedded", None),
        MetadataSource::Other(label) => ("Other", Some(label)),
    }
}
pub fn source(kind: String, detail: Option<String>) -> Result<MetadataSource> {
    Ok(match kind.as_str() {
        "Manual" => MetadataSource::Manual,
        "Publisher" => MetadataSource::Publisher,
        "Crossref" => MetadataSource::Crossref,
        "OpenAlex" => MetadataSource::OpenAlex,
        "Arxiv" => MetadataSource::Arxiv,
        "PdfEmbedded" => MetadataSource::PdfEmbedded,
        "Other" => MetadataSource::Other(
            detail.ok_or(StorageError::Corrupt("Missing source label".into()))?,
        ),
        _ => return Err(StorageError::Corrupt("Unknown metadata source".into())),
    })
}

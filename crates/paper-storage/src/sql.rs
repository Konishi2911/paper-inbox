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
        b.publication.article_number, b.identifiers.doi.as_ref().map(Doi::as_str), b.identifiers.arxiv_id.as_ref().map(ArxivId::as_str),
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
    b.0.identifiers.doi = b.2.map(decode).transpose()?;
    b.0.identifiers.arxiv_id = b.3.map(decode).transpose()?;
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
    if paper.canonical_version().is_some() || !m.tags.is_empty() || !m.collections.is_empty() {
        return Err(StorageError::Unsupported(
            "versions, tags and collections are not implemented yet",
        ));
    }
    db.execute(
        "INSERT INTO paper_snapshots VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
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
            m.archived
        ],
    )?;
    Ok(())
}
pub fn read_snapshot(db: &Connection, id: &str) -> Result<Paper> {
    let data = db.query_row("SELECT paper_id,bibliography_id,citation_key,created_at,updated_at,deleted_at,notes,reading_status,favorite,archived FROM paper_snapshots WHERE id=?1", [id], |r| {
        Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,String>(6)?,r.get::<_,String>(7)?,r.get::<_,bool>(8)?,r.get::<_,bool>(9)?))
    })?;
    Ok(Paper::from_parts(
        decode(data.0)?,
        decode(data.2)?,
        read_bibliography(db, &data.1)?,
        None,
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
    use BibliographyField as F;
    match field {
        F::Title => BibliographyValue::Title(b.title.clone()),
        F::Authors => BibliographyValue::Authors(b.authors.clone()),
        F::Editors => BibliographyValue::Editors(b.editors.clone()),
        F::PublicationType => BibliographyValue::PublicationType(b.publication.kind),
        F::ContainerTitle => {
            BibliographyValue::ContainerTitle(b.publication.container_title.clone())
        }
        F::Publisher => BibliographyValue::Publisher(b.publication.publisher.clone()),
        F::Year => BibliographyValue::Year(b.publication.year),
        F::Month => BibliographyValue::Month(b.publication.month),
        F::Volume => BibliographyValue::Volume(b.publication.volume.clone()),
        F::Issue => BibliographyValue::Issue(b.publication.issue.clone()),
        F::Pages => BibliographyValue::Pages(b.publication.pages.clone()),
        F::ArticleNumber => BibliographyValue::ArticleNumber(b.publication.article_number.clone()),
        F::Identifiers => BibliographyValue::Identifiers(b.identifiers.clone()),
        F::Abstract => BibliographyValue::Abstract(b.abstract_text.clone()),
        F::Keywords => BibliographyValue::Keywords(b.keywords.clone()),
    }
}
pub fn heads(db: &Connection, target: &FieldTarget) -> Result<Vec<RevisionId>> {
    if target.version_id.is_some() {
        return Err(StorageError::Unsupported("version revisions"));
    }
    let mut stmt = db.prepare(
        "SELECT revision_id FROM field_heads WHERE paper_id=?1 AND field=?2 ORDER BY revision_id",
    )?;
    let ids = stmt
        .query_map(
            params![target.paper_id.to_string(), field_name(target.field)],
            |r| r.get::<_, String>(0),
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    ids.into_iter().map(decode).collect()
}
pub fn read_revision(db: &Connection, id: RevisionId) -> Result<FieldRevision> {
    let d = db.query_row("SELECT paper_id,field,bibliography_id,source,source_record_id,observed_at,source_updated_at,created_at FROM revisions WHERE id=?1", [id.to_string()], |r| {
        Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,Option<String>>(4)?,r.get::<_,String>(5)?,r.get::<_,Option<String>>(6)?,r.get::<_,String>(7)?))
    })?;
    if d.3 != "Manual" {
        return Err(StorageError::Corrupt("Unsupported revision source".into()));
    }
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
        version_id: None,
        parents,
        value: value(&read_bibliography(db, &d.2)?, field(d.1)?),
        provenance: Provenance {
            source: MetadataSource::Manual,
            source_record_id: d.4,
            observed_at: time(d.5)?,
            source_updated_at: d.6.map(time).transpose()?,
        },
        created_at: time(d.7)?,
    })
}

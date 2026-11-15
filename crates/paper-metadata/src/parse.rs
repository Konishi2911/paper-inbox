use crate::LookupError;
use chrono::{DateTime, Datelike, Utc};
use paper_core::model::*;
use serde_json::Value;

fn normalized(s: &str) -> Option<String> {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    (!s.is_empty()).then_some(s)
}
fn string(v: &Value) -> Option<String> {
    v.as_str().and_then(normalized)
}
fn first(v: &Value) -> Option<String> {
    v.as_array()?.first().and_then(string)
}
fn timestamp(s: &str) -> Result<DateTime<Utc>, LookupError> {
    DateTime::parse_from_rfc3339(s)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|_| LookupError::Format)
}
fn candidate(
    bibliography: Bibliography,
    fields: Vec<BibliographyField>,
    source: MetadataSource,
    record: String,
    updated: Option<DateTime<Utc>>,
    observed: DateTime<Utc>,
    kind: VersionKind,
) -> Result<MetadataCandidate, LookupError> {
    let c = MetadataCandidate {
        id: CandidateId::new(),
        bibliography,
        present_fields: fields,
        provenance: Provenance {
            source,
            source_record_id: Some(record),
            observed_at: observed,
            source_updated_at: updated,
        },
        version_kind: kind,
    };
    c.validate().map_err(|_| LookupError::Format)?;
    Ok(c)
}
fn people(v: &Value) -> Result<Vec<Contributor>, LookupError> {
    v.as_array()
        .ok_or(LookupError::Format)?
        .iter()
        .map(|a| {
            let name = if let Some(family) = string(&a["family"]) {
                ContributorName::Person {
                    family,
                    given: string(&a["given"]),
                }
            } else {
                ContributorName::Literal(string(&a["name"]).ok_or(LookupError::Format)?)
            };
            Ok(Contributor {
                name,
                orcid: string(&a["ORCID"]),
            })
        })
        .collect()
}
pub fn parse_crossref(
    body: &str,
    requested: &Doi,
    observed: DateTime<Utc>,
) -> Result<MetadataCandidate, LookupError> {
    let root: Value = serde_json::from_str(body).map_err(|_| LookupError::Format)?;
    if root["status"] != "ok" {
        return Err(LookupError::Format);
    }
    let m = &root["message"];
    let doi = string(&m["DOI"])
        .and_then(|s| Doi::parse(&s).ok())
        .ok_or(LookupError::Format)?;
    if doi != *requested {
        return Err(LookupError::Mismatch);
    }
    let mut b = Bibliography::default();
    let mut fields = vec![BibliographyField::Identifiers];
    b.identifiers.doi = Some(doi);
    b.identifiers.url = string(&m["URL"]);
    b.identifiers.isbn = first(&m["ISBN"]);
    b.identifiers.issn = first(&m["ISSN"]);
    macro_rules! text {
        ($key:literal,$target:expr,$field:ident,$parser:ident) => {
            if let Some(value) = $parser(&m[$key]) {
                $target = Some(value);
                fields.push(BibliographyField::$field);
            }
        };
    }
    text!("title", b.title, Title, first);
    text!(
        "container-title",
        b.publication.container_title,
        ContainerTitle,
        first
    );
    text!("publisher", b.publication.publisher, Publisher, string);
    text!("volume", b.publication.volume, Volume, string);
    text!("issue", b.publication.issue, Issue, string);
    text!("page", b.publication.pages, Pages, string);
    text!(
        "article-number",
        b.publication.article_number,
        ArticleNumber,
        string
    );
    for (key, field) in [
        ("author", BibliographyField::Authors),
        ("editor", BibliographyField::Editors),
    ] {
        if !m[key].is_null() {
            let values = people(&m[key])?;
            if !values.is_empty() {
                if field == BibliographyField::Authors {
                    b.authors = values;
                } else {
                    b.editors = values;
                }
                fields.push(field);
            }
        }
    }
    let kind = match m["type"].as_str() {
        Some("journal-article") => PublicationType::JournalArticle,
        Some("proceedings-article") => PublicationType::ConferencePaper,
        Some("book") => PublicationType::Book,
        Some("book-chapter") => PublicationType::BookChapter,
        Some("posted-content") => PublicationType::Preprint,
        _ => PublicationType::Unknown,
    };
    if kind != PublicationType::Unknown {
        b.publication.kind = kind;
        fields.push(BibliographyField::PublicationType);
    }
    for date in ["published", "published-print", "published-online", "issued"] {
        if let Some(parts) = m[date]["date-parts"]
            .as_array()
            .and_then(|v| v.first())
            .and_then(Value::as_array)
            && let Some(year) = parts
                .first()
                .and_then(Value::as_u64)
                .and_then(|n| u16::try_from(n).ok())
                .filter(|n| *n > 0)
        {
            b.publication.year = Some(year);
            fields.push(BibliographyField::Year);
            if let Some(month) = parts
                .get(1)
                .and_then(Value::as_u64)
                .filter(|n| (1..=12).contains(n))
            {
                b.publication.month = Some(month as u8);
                fields.push(BibliographyField::Month);
            }
            break;
        }
    }
    // Crossref abstracts may contain JATS XML; preserve its text, never execute it.
    if let Some(raw) = m["abstract"].as_str() {
        let wrapper = format!("<root xmlns:jats='http://www.ncbi.nlm.nih.gov/JATS1'>{raw}</root>");
        let value = roxmltree::Document::parse(&wrapper).ok().and_then(|d| {
            normalized(
                &d.descendants()
                    .filter(|n| n.is_text())
                    .filter_map(|n| n.text())
                    .collect::<String>(),
            )
        });
        if let Some(value) = value {
            b.abstract_text = Some(value);
            fields.push(BibliographyField::Abstract);
        }
    }
    if let Some(subjects) = m["subject"].as_array() {
        b.keywords = subjects.iter().filter_map(string).collect();
        if !b.keywords.is_empty() {
            fields.push(BibliographyField::Keywords);
        }
    }
    let updated = m["deposited"]["date-time"]
        .as_str()
        .map(timestamp)
        .transpose()?;
    let version = if kind == PublicationType::Preprint {
        VersionKind::Preprint
    } else if matches!(
        kind,
        PublicationType::JournalArticle
            | PublicationType::ConferencePaper
            | PublicationType::Book
            | PublicationType::BookChapter
    ) {
        VersionKind::VersionOfRecord
    } else {
        VersionKind::Unknown
    };
    candidate(
        b,
        fields,
        MetadataSource::Crossref,
        requested.as_str().into(),
        updated,
        observed,
        version,
    )
}
const ATOM: &str = "http://www.w3.org/2005/Atom";
fn child<'a>(
    node: roxmltree::Node<'a, 'a>,
    namespace: &str,
    name: &str,
) -> Option<roxmltree::Node<'a, 'a>> {
    node.children().find(|n| n.has_tag_name((namespace, name)))
}
fn atom_text(node: roxmltree::Node<'_, '_>, name: &str) -> Option<String> {
    child(node, ATOM, name)?.text().and_then(normalized)
}
pub fn parse_arxiv(
    body: &str,
    requested: &ArxivId,
    observed: DateTime<Utc>,
) -> Result<MetadataCandidate, LookupError> {
    let doc = roxmltree::Document::parse(body).map_err(|_| LookupError::Format)?;
    let feed = doc.root_element();
    if !feed.has_tag_name((ATOM, "feed")) {
        return Err(LookupError::Format);
    }
    let entries: Vec<_> = feed
        .children()
        .filter(|n| n.has_tag_name((ATOM, "entry")))
        .collect();
    if entries.is_empty() {
        return Err(LookupError::NotFound);
    }
    if entries.len() != 1 {
        return Err(LookupError::Format);
    }
    let entry = entries[0];
    let id = atom_text(entry, "id").ok_or(LookupError::Format)?;
    let returned = ArxivId::parse(&id).map_err(|_| LookupError::Format)?;
    if returned.base_id() != requested.base_id()
        || requested
            .version()
            .is_some_and(|v| returned.version() != Some(v))
    {
        return Err(LookupError::Mismatch);
    }
    let mut b = Bibliography {
        title: Some(atom_text(entry, "title").ok_or(LookupError::Format)?),
        ..Default::default()
    };
    b.identifiers.arxiv_id = Some(returned.clone());
    b.identifiers.url = Some(format!("https://arxiv.org/abs/{}", returned.as_str()));
    // A DOI on an arXiv entry normally refers to another publication version.
    // Do not attach it to the preprint or use it as equality evidence.
    b.publication.kind = PublicationType::Preprint;
    let published = timestamp(&atom_text(entry, "published").ok_or(LookupError::Format)?)?;
    b.publication.year = Some(u16::try_from(published.year()).map_err(|_| LookupError::Format)?);
    b.publication.month = Some(published.month() as u8);
    let mut fields = vec![
        BibliographyField::Title,
        BibliographyField::Identifiers,
        BibliographyField::PublicationType,
        BibliographyField::Year,
        BibliographyField::Month,
    ];
    for author in entry
        .children()
        .filter(|n| n.has_tag_name((ATOM, "author")))
    {
        b.authors.push(Contributor {
            name: ContributorName::Literal(atom_text(author, "name").ok_or(LookupError::Format)?),
            orcid: None,
        });
    }
    if !b.authors.is_empty() {
        fields.push(BibliographyField::Authors);
    }
    if let Some(summary) = atom_text(entry, "summary") {
        b.abstract_text = Some(summary);
        fields.push(BibliographyField::Abstract);
    }
    let updated = timestamp(&atom_text(entry, "updated").ok_or(LookupError::Format)?)?;
    candidate(
        b,
        fields,
        MetadataSource::Arxiv,
        returned.as_str().into(),
        Some(updated),
        observed,
        VersionKind::Preprint,
    )
}

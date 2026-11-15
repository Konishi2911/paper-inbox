use chrono::{TimeZone, Utc};
use paper_core::model::*;
use paper_metadata::{LookupError, parse_arxiv, parse_crossref, parse_identifier};
const CROSSREF: &str = r#"{"status":"ok","message":{"DOI":"10.1000/ABC","title":["Graph methods"],"author":[{"given":"Ada","family":"Smith","ORCID":"0000-0000-0000-0001"},{"name":"ACME"}],"editor":[{"family":"Jones"}],"type":"journal-article","published":{"date-parts":[[2024,3,2]]},"container-title":["Journal"],"publisher":"Press","volume":"12","issue":"3","page":"4-10","article-number":"e2","abstract":"<jats:p>A &amp; B</jats:p>","subject":["Graphs"],"deposited":{"date-time":"2024-04-01T00:00:00Z"}}}"#;
const ARXIV: &str = r#"<feed xmlns="http://www.w3.org/2005/Atom" xmlns:arxiv="http://arxiv.org/schemas/atom"><entry><id>http://arxiv.org/abs/2401.12345v2</id><title>Graph
 methods</title><author><name>Ada Smith</name></author><published>2024-01-01T00:00:00Z</published><updated>2024-02-01T00:00:00Z</updated><summary>Some findings.</summary><arxiv:doi>10.1000/published</arxiv:doi><arxiv:journal_ref>Journal 10 (2025)</arxiv:journal_ref></entry></feed>"#;
fn now() -> chrono::DateTime<Utc> {
    Utc.timestamp_opt(2_000_000_000, 0).unwrap()
}
#[test]
fn crossref_maps_provenance_contributors_and_only_supplied_fields() {
    let doi = Doi::parse("10.1000/abc").unwrap();
    let c = parse_crossref(CROSSREF, &doi, now()).unwrap();
    assert_eq!(c.bibliography.title.as_deref(), Some("Graph methods"));
    assert_eq!(c.bibliography.authors.len(), 2);
    assert_eq!(c.bibliography.publication.year, Some(2024));
    assert_eq!(c.bibliography.publication.month, Some(3));
    assert_eq!(c.bibliography.abstract_text.as_deref(), Some("A & B"));
    assert_eq!(c.version_kind, VersionKind::VersionOfRecord);
    assert_eq!(c.provenance.source, MetadataSource::Crossref);
    assert!(c.provenance.source_updated_at.is_some());
    let minimal = r#"{"status":"ok","message":{"DOI":"10.1000/abc","title":[],"author":[]}}"#;
    let c = parse_crossref(minimal, &doi, now()).unwrap();
    assert_eq!(c.present_fields, vec![BibliographyField::Identifiers]);
    assert_eq!(c.provenance.source_updated_at, None);
    assert_eq!(c.version_kind, VersionKind::Unknown);
    assert!(matches!(
        parse_crossref(CROSSREF, &Doi::parse("10.1000/wrong").unwrap(), now()),
        Err(LookupError::Mismatch)
    ));
    assert!(parse_crossref("{}", &doi, now()).is_err());
    assert!(
        parse_crossref(
            &CROSSREF.replace("\"family\":\"Smith\"", "\"family\":null"),
            &doi,
            now()
        )
        .is_err()
    );
}
#[test]
fn arxiv_keeps_exact_version_and_does_not_claim_the_publisher_doi() {
    let id = ArxivId::parse("2401.12345v2").unwrap();
    let c = parse_arxiv(ARXIV, &id, now()).unwrap();
    assert_eq!(c.bibliography.identifiers.arxiv_id, Some(id));
    assert_eq!(c.bibliography.identifiers.doi, None);
    assert_eq!(c.bibliography.publication.kind, PublicationType::Preprint);
    assert_eq!(c.version_kind, VersionKind::Preprint);
    assert_eq!(c.bibliography.title.as_deref(), Some("Graph methods"));
    assert_eq!(
        c.provenance.source_record_id.as_deref(),
        Some("2401.12345v2")
    );
    assert!(matches!(
        c.bibliography.authors[0].name,
        ContributorName::Literal(_)
    ));
    assert!(parse_arxiv(ARXIV, &ArxivId::parse("2401.12345").unwrap(), now()).is_ok());
    assert!(matches!(
        parse_arxiv(ARXIV, &ArxivId::parse("2401.12345v1").unwrap(), now()),
        Err(LookupError::Mismatch)
    ));
    assert!(matches!(
        parse_arxiv(
            "<feed xmlns='http://www.w3.org/2005/Atom'/>",
            &ArxivId::parse("2401.12345").unwrap(),
            now()
        ),
        Err(LookupError::NotFound)
    ));
    assert!(
        parse_arxiv(
            "<!DOCTYPE feed [<!ENTITY x 'oops'>]><feed/>",
            &ArxivId::parse("2401.12345").unwrap(),
            now()
        )
        .is_err()
    );
    assert!(parse_identifier("https://example.test/?q=10.1000/a").is_err());
}

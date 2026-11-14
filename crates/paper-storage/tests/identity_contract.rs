use paper_core::{
    command::LibraryCommand,
    model::*,
    ports::{LibraryStore, PaperRepository},
    service::*,
};
use paper_storage::SqliteLibrary;
use rusqlite::Connection;
struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("paper-identity-sql-{}.sqlite", PaperId::new())))
    }
    fn open(&self) -> SqliteLibrary {
        SqliteLibrary::open_local(&self.0).unwrap()
    }
    fn raw(&self) -> Connection {
        Connection::open(&self.0).unwrap()
    }
    fn counts(&self) -> Vec<i64> {
        let db = self.raw();
        [
            "papers",
            "paper_snapshots",
            "bibliographies",
            "revisions",
            "field_heads",
            "events",
        ]
        .iter()
        .map(|table| {
            db.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
                .unwrap()
        })
        .collect()
    }
    fn set_identifiers(&self, id: PaperId, doi: Option<&str>, arxiv: Option<&str>) {
        self.raw().execute("UPDATE bibliographies SET doi=?1,arxiv_id=?2 WHERE id=(SELECT bibliography_id FROM paper_snapshots WHERE id=(SELECT snapshot_id FROM papers WHERE id=?3))",rusqlite::params![doi,arxiv,id.to_string()]).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn bibliography(doi: Option<&str>, arxiv: Option<&str>) -> Bibliography {
    Bibliography {
        identifiers: Identifiers {
            doi: doi.map(|s| Doi::parse(s).unwrap()),
            arxiv_id: arxiv.map(|s| ArxivId::parse(s).unwrap()),
            ..Default::default()
        },
        ..Default::default()
    }
}
fn save(store: &SqliteLibrary, key: &str, bibliography: Bibliography) -> PaperId {
    let paper = Paper::new(
        PaperId::new(),
        CitationKey::parse(key).unwrap(),
        bibliography,
        chrono::Utc::now(),
    )
    .unwrap();
    let id = paper.id();
    store.commit(LibraryCommand::CreatePaper { paper }).unwrap();
    id
}
#[test]
fn assessments_are_unchanged_after_sql_roundtrip_and_reopen() {
    let fixture = Fixture::new();
    let store = fixture.open();
    let mut expected = vec![];
    for (i, (ld, la, rd, ra)) in [
        (Some("10.1000/a"), None, Some("10.1000/a"), None),
        (None, Some("2401.12345v1"), None, Some("2401.12345v1")),
        (
            None,
            Some("solv-int/9901001v1"),
            None,
            Some("solv-int/9901001v2"),
        ),
        (None, Some("2401.12345"), None, Some("2401.12345v2")),
        (
            Some("10.1000/a"),
            Some("2401.12345"),
            Some("10.1000/a"),
            Some("2401.54321"),
        ),
        (Some("10.1000/a"), None, Some("10.1000/b"), None),
        (None, None, None, None),
    ]
    .into_iter()
    .enumerate()
    {
        let left = bibliography(ld, la);
        let right = bibliography(rd, ra);
        let assessment = assess_identity(&left, &right);
        expected.push((
            save(&store, &format!("Left{i}"), left),
            save(&store, &format!("Right{i}"), right),
            assessment,
        ));
    }
    let counts = fixture.counts();
    drop(store);
    let store = fixture.open();
    for (left, right, expected) in expected {
        let papers = store.get_many(&[left, right]).unwrap();
        let result = assess_identity(papers[0].bibliography(), papers[1].bibliography());
        assert_eq!(result, expected);
        assert_eq!(
            compare_identity(papers[1].bibliography(), papers[0].bibliography()),
            expected.outcome
        );
    }
    assert_eq!(fixture.counts(), counts);
}
#[test]
fn invalid_sql_values_require_repair_and_are_not_identity_evidence() {
    let fixture = Fixture::new();
    let store = fixture.open();
    let left = save(&store, "LegacyLeft", Bibliography::default());
    let right = save(&store, "LegacyRight", Bibliography::default());
    fixture.set_identifiers(left, Some("same invalid DOI"), None);
    fixture.set_identifiers(right, Some("same invalid DOI"), None);
    let counts = fixture.counts();
    let papers = store.get_many(&[left, right]).unwrap();
    let assessment = assess_identity(papers[0].bibliography(), papers[1].bibliography());
    assert_eq!(assessment.outcome, IdentityMatch::Unknown);
    assert_eq!(
        assessment
            .evidence
            .iter()
            .filter(|e| matches!(
                e,
                IdentityEvidence::UnvalidatedIdentifier {
                    identifier: IdentityIdentifier::Doi,
                    ..
                }
            ))
            .count(),
        2
    );
    fixture.set_identifiers(left, Some("invalid left DOI"), Some("2401.12345v1"));
    fixture.set_identifiers(right, Some("invalid right DOI"), Some("2401.12345v1"));
    let papers = store.get_many(&[left, right]).unwrap();
    let assessment = assess_identity(papers[0].bibliography(), papers[1].bibliography());
    assert_eq!(assessment.outcome, IdentityMatch::Exact);
    assert!(assessment.evidence.contains(&IdentityEvidence::ArxivMatch {
        id: ArxivId::parse("2401.12345v1").unwrap()
    }));
    assert!(!assessment.evidence.iter().any(|e| matches!(
        e,
        IdentityEvidence::DoiMatch { .. } | IdentityEvidence::DoiMismatch { .. }
    )));
    let stored = store.get(left).unwrap().unwrap();
    assert_eq!(
        stored.bibliography().identifiers.raw_doi.as_deref(),
        Some("invalid left DOI")
    );
    assert_eq!(fixture.counts(), counts);
}
#[test]
fn stored_presentation_variants_normalize_for_assessment_without_rewriting_sql() {
    let fixture = Fixture::new();
    let store = fixture.open();
    let left = save(&store, "PresentationLeft", Bibliography::default());
    let right = save(&store, "PresentationRight", Bibliography::default());
    fixture.set_identifiers(
        left,
        Some(" DOI:10.1000/AbC "),
        Some("https://arxiv.org/abs/2401.12345v2"),
    );
    fixture.set_identifiers(
        right,
        Some("https://doi.org/10.1000/abc"),
        Some("arXiv:2401.12345v2"),
    );
    let counts = fixture.counts();
    let papers = store.get_many(&[left, right]).unwrap();
    let expected = assess_identity(
        &bibliography(Some("10.1000/abc"), Some("2401.12345v2")),
        &bibliography(Some("10.1000/abc"), Some("2401.12345v2")),
    );
    assert_eq!(
        assess_identity(papers[0].bibliography(), papers[1].bibliography()),
        expected
    );
    let raw:String=fixture.raw().query_row("SELECT doi FROM bibliographies WHERE id=(SELECT bibliography_id FROM paper_snapshots WHERE id=(SELECT snapshot_id FROM papers WHERE id=?1))",[left.to_string()],|r|r.get(0)).unwrap();
    assert_eq!(raw, " DOI:10.1000/AbC ");
    assert_eq!(fixture.counts(), counts);
}

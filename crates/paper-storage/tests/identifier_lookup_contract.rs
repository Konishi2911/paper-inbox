use chrono::{DateTime, TimeZone, Utc};
use paper_core::{command::LibraryCommand, model::*, ports::PaperRepository};
use paper_storage::SqliteLibrary;

fn now(n: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(1_000 + n, 0).unwrap()
}
fn paper(key: &str, doi: Option<&str>, arxiv: Option<&str>) -> Paper {
    let mut bibliography = Bibliography {
        title: Some(format!("Title of {key}")),
        ..Default::default()
    };
    bibliography.identifiers.doi = doi.map(|d| Doi::parse(d).unwrap());
    bibliography.identifiers.arxiv_id = arxiv.map(|a| ArxivId::parse(a).unwrap());
    Paper::new(
        PaperId::new(),
        CitationKey::parse(key).unwrap(),
        bibliography,
        now(0),
    )
    .unwrap()
}
fn create(store: &SqliteLibrary, p: &Paper) {
    store
        .commit_at(LibraryCommand::CreatePaper { paper: p.clone() }, now(0))
        .unwrap();
}
fn keys(found: Vec<paper_core::query::IdentifierMatch>) -> Vec<String> {
    found.into_iter().map(|m| m.citation_key).collect()
}

#[test]
fn doi_and_arxiv_ids_find_saved_papers_whatever_their_spelling_or_version() {
    let store = SqliteLibrary::in_memory(DeviceId::new()).unwrap();
    create(&store, &paper("A2024", Some("10.1000/ABC"), None));
    create(&store, &paper("B2024", None, Some("2401.01234v2")));
    create(
        &store,
        &paper("C2024", Some("10.1000/other"), Some("hep-th/9901001")),
    );

    let doi = Doi::parse("https://doi.org/10.1000/abc").unwrap();
    assert_eq!(
        keys(store.find_by_identifiers(Some(&doi), None).unwrap()),
        ["A2024"],
        "DOIs compare in their normalized form"
    );
    // An ID without a version finds the saved version, and the other way round.
    let bare = ArxivId::parse("2401.01234").unwrap();
    assert_eq!(
        keys(store.find_by_identifiers(None, Some(&bare)).unwrap()),
        ["B2024"]
    );
    let other_version = ArxivId::parse("2401.01234v1").unwrap();
    assert_eq!(
        keys(
            store
                .find_by_identifiers(None, Some(&other_version))
                .unwrap()
        ),
        ["B2024"]
    );
    let old = ArxivId::parse("hep-th/9901001v3").unwrap();
    assert_eq!(
        keys(store.find_by_identifiers(None, Some(&old)).unwrap()),
        ["C2024"]
    );
    // Either identifier is enough, and the same paper is listed once.
    let both = store
        .find_by_identifiers(
            Some(&Doi::parse("10.1000/other").unwrap()),
            Some(&ArxivId::parse("hep-th/9901001").unwrap()),
        )
        .unwrap();
    assert_eq!(keys(both), ["C2024"]);
    // A different number in the same month is a different paper.
    let longer = ArxivId::parse("2401.01235").unwrap();
    assert!(
        store
            .find_by_identifiers(None, Some(&longer))
            .unwrap()
            .is_empty()
    );
    assert!(store.find_by_identifiers(None, None).unwrap().is_empty());
}

#[test]
fn versions_count_and_deleted_papers_are_listed_after_live_ones() {
    let store = SqliteLibrary::in_memory(DeviceId::new()).unwrap();
    let published = paper("Pub2024", Some("10.1000/published"), None);
    create(&store, &published);
    // The preprint's arXiv ID is held by a version of the same paper.
    let mut preprint = Bibliography::default();
    preprint.identifiers.arxiv_id = Some(ArxivId::parse("2402.00001").unwrap());
    store
        .commit_at(
            LibraryCommand::AddVersion {
                version: PaperVersion {
                    id: VersionId::new(),
                    paper_id: published.id(),
                    kind: VersionKind::Preprint,
                    bibliography: preprint,
                    created_at: now(1),
                    updated_at: now(1),
                },
            },
            now(1),
        )
        .unwrap();
    let id = ArxivId::parse("2402.00001v1").unwrap();
    let found = store.find_by_identifiers(None, Some(&id)).unwrap();
    assert_eq!(keys(found.clone()), ["Pub2024"]);
    assert_eq!(found[0].title.as_deref(), Some("Title of Pub2024"));
    assert!(!found[0].deleted);

    // The same DOI saved twice cannot be told apart by key; the live paper comes first.
    let doi = Doi::parse("10.1000/shared").unwrap();
    let old = paper("Zed2020", Some("10.1000/shared"), None);
    let live = paper("Alpha2021", Some("10.1000/shared"), None);
    create(&store, &old);
    create(&store, &live);
    store
        .commit_at(LibraryCommand::DeletePaper { paper_id: old.id() }, now(2))
        .unwrap();
    let found = store.find_by_identifiers(Some(&doi), None).unwrap();
    assert_eq!(keys(found.clone()), ["Alpha2021", "Zed2020"]);
    assert_eq!(
        found.iter().map(|m| m.deleted).collect::<Vec<_>>(),
        [false, true]
    );
}

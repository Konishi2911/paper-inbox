use paper_core::{model::*, service::suggest_citation_key};
fn b() -> Bibliography {
    Bibliography {
        title: Some("The Graph methods".into()),
        authors: vec![Contributor {
            name: ContributorName::Person {
                given: Some("Ada".into()),
                family: "Smith".into(),
            },
            orcid: None,
        }],
        publication: Publication {
            year: Some(2024),
            ..Default::default()
        },
        ..Default::default()
    }
}
#[test]
fn author_year_then_title_then_suffix_is_deterministic_and_export_safe() {
    let b = b();
    assert_eq!(suggest_citation_key(&b, &[]).as_str(), "Smith2024");
    assert_eq!(
        suggest_citation_key(&b, &["smith2024".into()]).as_str(),
        "Smith2024Graph"
    );
    assert_eq!(
        suggest_citation_key(
            &b,
            &[
                "Smith2024".into(),
                "SMITH2024GRAPH".into(),
                "Smith2024Graph2".into()
            ]
        )
        .as_str(),
        "Smith2024Graph3"
    );
    let mut missing = Bibliography::default();
    assert_eq!(
        suggest_citation_key(&missing, &[]).as_str(),
        "AuthorUndated"
    );
    missing.authors = vec![Contributor {
        name: ContributorName::Literal("日本語".into()),
        orcid: None,
    }];
    assert_eq!(
        suggest_citation_key(&missing, &["AuthorUndated".into()]).as_str(),
        "AuthorUndatedWork"
    );
    missing.authors[0].name = ContributorName::Organization("ACME & Co.".into());
    assert_eq!(
        suggest_citation_key(&missing, &[]).as_str(),
        "ACMECoUndated"
    );
}

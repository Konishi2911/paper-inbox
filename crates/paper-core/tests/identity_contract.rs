use paper_core::{model::*, service::*};
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
#[test]
fn strong_identifier_and_conflict_table() {
    let cases = [
        (
            Some("DOI:10.1000/A"),
            None,
            Some("https://doi.org/10.1000/a"),
            None,
            IdentityMatch::Exact,
        ),
        (
            None,
            Some("2401.12345v1"),
            None,
            Some("arXiv:2401.12345v1"),
            IdentityMatch::Exact,
        ),
        (
            None,
            Some("2401.12345"),
            None,
            Some("2401.12345"),
            IdentityMatch::Exact,
        ),
        (
            None,
            Some("2401.12345v1"),
            None,
            Some("2401.12345v2"),
            IdentityMatch::RelatedVersion,
        ),
        (
            None,
            Some("2401.12345"),
            None,
            Some("2401.12345v2"),
            IdentityMatch::RelatedVersion,
        ),
        (
            Some("10.1000/a"),
            Some("2401.12345v1"),
            Some("10.1000/a"),
            Some("2401.12345v2"),
            IdentityMatch::Exact,
        ),
        (
            Some("10.1000/a"),
            Some("2401.12345"),
            Some("10.1000/a"),
            Some("2401.54321"),
            IdentityMatch::NeedsReview,
        ),
        (
            Some("10.1000/a"),
            Some("2401.12345v1"),
            Some("10.1000/b"),
            Some("2401.12345v1"),
            IdentityMatch::NeedsReview,
        ),
        (
            Some("10.1000/a"),
            Some("2401.12345v1"),
            Some("10.1000/b"),
            Some("2401.12345v2"),
            IdentityMatch::NeedsReview,
        ),
        (
            Some("10.1000/a"),
            None,
            Some("10.1000/b"),
            None,
            IdentityMatch::Unknown,
        ),
        (
            None,
            Some("2401.12345"),
            None,
            Some("2401.54321"),
            IdentityMatch::Unknown,
        ),
        (
            Some("10.1000/a"),
            Some("2401.12345"),
            Some("10.1000/b"),
            Some("2401.54321"),
            IdentityMatch::Unknown,
        ),
        (
            Some("10.1000/a"),
            None,
            None,
            Some("2401.12345"),
            IdentityMatch::Unknown,
        ),
        (None, None, None, None, IdentityMatch::Unknown),
    ];
    for (ld, la, rd, ra, expected) in cases {
        let left = bibliography(ld, la);
        let right = bibliography(rd, ra);
        let report = assess_identity(&left, &right);
        assert_eq!(report.outcome, expected, "{ld:?} {la:?} vs {rd:?} {ra:?}");
        assert_eq!(compare_identity(&left, &right), expected);
        assert_eq!(compare_identity(&right, &left), expected);
        assert!(!report.evidence.is_empty());
    }
}
#[test]
fn evidence_keeps_matches_conflicts_and_missing_sides() {
    let left = bibliography(Some("10.1000/a"), Some("2401.12345"));
    let right = bibliography(Some("10.1000/a"), Some("2401.54321"));
    assert_eq!(
        assess_identity(&left, &right).evidence,
        vec![
            IdentityEvidence::DoiMatch {
                doi: Doi::parse("10.1000/a").unwrap()
            },
            IdentityEvidence::ArxivMismatch {
                left: ArxivId::parse("2401.12345").unwrap(),
                right: ArxivId::parse("2401.54321").unwrap()
            },
        ]
    );
    let missing = assess_identity(
        &bibliography(Some("10.1000/a"), None),
        &Bibliography::default(),
    );
    assert!(
        missing
            .evidence
            .contains(&IdentityEvidence::MissingIdentifier {
                identifier: IdentityIdentifier::Doi,
                side: IdentitySide::Right
            })
    );
    let related = assess_identity(
        &bibliography(None, Some("2401.12345v1")),
        &bibliography(None, Some("2401.12345v2")),
    );
    assert!(
        related
            .evidence
            .contains(&IdentityEvidence::ArxivVersionRelation {
                left: ArxivId::parse("2401.12345v1").unwrap(),
                right: ArxivId::parse("2401.12345v2").unwrap()
            })
    );
}
#[test]
fn invalid_legacy_fields_are_reported_and_never_used_as_evidence() {
    let mut left = Bibliography::default();
    left.identifiers.raw_doi = Some("10.1000/a".into());
    let right = bibliography(Some("10.1000/a"), None);
    let report = assess_identity(&left, &right);
    assert_eq!(report.outcome, IdentityMatch::Unknown);
    assert!(
        report
            .evidence
            .contains(&IdentityEvidence::UnvalidatedIdentifier {
                identifier: IdentityIdentifier::Doi,
                side: IdentitySide::Left,
                raw: "10.1000/a".into()
            })
    );
    // Even an ambiguous public struct cannot sneak its typed value into comparison.
    left.identifiers.doi = right.identifiers.doi.clone();
    assert_eq!(compare_identity(&left, &right), IdentityMatch::Unknown);
    left.identifiers.arxiv_id = Some(ArxivId::parse("2401.12345").unwrap());
    let right = bibliography(Some("10.1000/b"), Some("2401.12345"));
    let report = assess_identity(&left, &right);
    assert_eq!(report.outcome, IdentityMatch::Exact);
    assert!(
        report
            .evidence
            .iter()
            .any(|e| matches!(e, IdentityEvidence::UnvalidatedIdentifier { .. }))
    );
    assert!(!report.evidence.iter().any(|e| matches!(
        e,
        IdentityEvidence::DoiMatch { .. } | IdentityEvidence::DoiMismatch { .. }
    )));
    let mut both = Bibliography::default();
    both.identifiers.raw_arxiv_id = Some("same invalid value".into());
    assert_eq!(compare_identity(&both, &both), IdentityMatch::Unknown);
}
#[test]
fn arxiv_parts_preserve_old_archives_and_arbitrary_length_versions() {
    for (raw, base, version) in [
        ("2401.12345v1", "2401.12345", Some("1")),
        ("2401.12345", "2401.12345", None),
        ("solv-int/9901001", "solv-int/9901001", None),
        ("solv-int/9901001v2", "solv-int/9901001", Some("2")),
        (
            "math.GT/0301001v123456789012345678901234567890",
            "math.GT/0301001",
            Some("123456789012345678901234567890"),
        ),
    ] {
        let id = ArxivId::parse(raw).unwrap();
        assert_eq!(id.base_id(), base);
        assert_eq!(id.version(), version);
        assert_eq!(id.as_str(), raw);
    }
    assert_eq!(
        compare_identity(
            &bibliography(None, Some("solv-int/9901001v1")),
            &bibliography(None, Some("solv-int/9901001v2"))
        ),
        IdentityMatch::RelatedVersion
    );
}
#[test]
fn matching_titles_authors_urls_and_isbns_do_not_imply_identity() {
    let mut left = Bibliography {
        title: Some("Exactly the same title".into()),
        authors: vec![Contributor {
            name: ContributorName::Literal("Same author".into()),
            orcid: None,
        }],
        ..Default::default()
    };
    left.identifiers.url = Some("https://example.test/same".into());
    left.identifiers.isbn = Some("same isbn".into());
    assert_eq!(compare_identity(&left, &left), IdentityMatch::Unknown);
    left.identifiers.doi = Some(Doi::parse("10.1000/a").unwrap());
    let mut right = left.clone();
    right.title = Some("Entirely different title".into());
    right.authors.clear();
    assert_eq!(compare_identity(&left, &right), IdentityMatch::Exact);
}
fn reverse(e: IdentityEvidence) -> IdentityEvidence {
    match e {
        IdentityEvidence::DoiMismatch { left, right } => IdentityEvidence::DoiMismatch {
            left: right,
            right: left,
        },
        IdentityEvidence::ArxivVersionRelation { left, right } => {
            IdentityEvidence::ArxivVersionRelation {
                left: right,
                right: left,
            }
        }
        IdentityEvidence::ArxivMismatch { left, right } => IdentityEvidence::ArxivMismatch {
            left: right,
            right: left,
        },
        IdentityEvidence::MissingIdentifier { identifier, side } => {
            IdentityEvidence::MissingIdentifier {
                identifier,
                side: flip(side),
            }
        }
        IdentityEvidence::UnvalidatedIdentifier {
            identifier,
            side,
            raw,
        } => IdentityEvidence::UnvalidatedIdentifier {
            identifier,
            side: flip(side),
            raw,
        },
        other => other,
    }
}
fn flip(side: IdentitySide) -> IdentitySide {
    match side {
        IdentitySide::Left => IdentitySide::Right,
        IdentitySide::Right => IdentitySide::Left,
    }
}
fn sorted(e: Vec<IdentityEvidence>) -> Vec<String> {
    let mut e: Vec<_> = e.iter().map(|e| format!("{e:?}")).collect();
    e.sort();
    e
}
#[test]
fn assessments_are_symmetric_deterministic_and_do_not_mutate_inputs() {
    let mut inputs = vec![];
    for doi in [None, Some("10.1000/a"), Some("10.1000/b")] {
        for arxiv in [
            None,
            Some("2401.12345"),
            Some("2401.12345v1"),
            Some("2401.12345v2"),
            Some("2401.54321"),
            Some("solv-int/9901001v1"),
            Some("solv-int/9901001v2"),
        ] {
            inputs.push(bibliography(doi, arxiv));
        }
    }
    let mut invalid = Bibliography::default();
    invalid.identifiers.raw_doi = Some("bad DOI".into());
    inputs.push(invalid.clone());
    invalid.identifiers.raw_arxiv_id = Some("bad arxiv".into());
    inputs.push(invalid.clone());
    invalid.identifiers.doi = Some(Doi::parse("10.1000/a").unwrap());
    inputs.push(invalid);
    for left in &inputs {
        for right in &inputs {
            let before = (left.clone(), right.clone());
            let report = assess_identity(left, right);
            let reversed = assess_identity(right, left);
            assert_eq!(report.outcome, reversed.outcome);
            assert_eq!(report, assess_identity(left, right));
            assert_eq!(
                sorted(report.evidence.into_iter().map(reverse).collect()),
                sorted(reversed.evidence)
            );
            assert_eq!(before, (left.clone(), right.clone()));
            assert!(!matches!(
                report.outcome,
                IdentityMatch::Different | IdentityMatch::ProbableDuplicate { .. }
            ));
        }
    }
}

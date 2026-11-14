use paper_core::{model::*, service::*};
fn paper(key: &str, kind: PublicationType) -> Paper {
    Paper::new(
        PaperId::new(),
        CitationKey::parse(key).unwrap(),
        Bibliography {
            title: Some("SQL & Rust {a} \\ title_100% # $ ~ ^ 日本語".into()),
            authors: vec![
                Contributor {
                    name: ContributorName::Person {
                        given: Some("Ada".into()),
                        family: "van And, Family".into(),
                    },
                    orcid: None,
                },
                Contributor {
                    name: ContributorName::Organization("Research and Development".into()),
                    orcid: None,
                },
            ],
            publication: Publication {
                kind,
                container_title: Some("Journal".into()),
                publisher: Some("Publisher".into()),
                year: Some(2026),
                ..Default::default()
            },
            identifiers: Identifiers {
                doi: Some(Doi::parse("10.1000/ABC").unwrap()),
                arxiv_id: Some(ArxivId::parse("2401.12345v2").unwrap()),
                url: Some("https://example.test/a_b".into()),
                ..Default::default()
            },
            abstract_text: Some("optional abstract".into()),
            keywords: vec!["optional keyword".into()],
            ..Default::default()
        },
        chrono::Utc::now(),
    )
    .unwrap()
}
#[test]
fn entry_types_fields_and_fallback_warnings_are_deterministic() {
    for (kind, expected) in [
        (PublicationType::JournalArticle, "article"),
        (PublicationType::ConferencePaper, "inproceedings"),
        (PublicationType::Book, "book"),
        (PublicationType::BookChapter, "incollection"),
        (PublicationType::MastersThesis, "mastersthesis"),
        (PublicationType::DoctoralThesis, "phdthesis"),
        (PublicationType::TechnicalReport, "techreport"),
        (PublicationType::Preprint, "unpublished"),
        (PublicationType::Thesis, "misc"),
        (PublicationType::Patent, "misc"),
        (PublicationType::Other, "misc"),
        (PublicationType::Unknown, "misc"),
    ] {
        let p = paper("type", kind);
        let a = render_entry(&p, &BibtexOptions::default());
        let b = render_entry(&p, &BibtexOptions::default());
        assert!(a.content.starts_with(&format!("@{expected}{{type,")));
        assert_eq!(a.content, b.content);
        assert_eq!(a.warnings, b.warnings);
        assert_eq!(
            a.warnings
                .iter()
                .any(|w| matches!(w, BibtexWarning::FallbackEntryType { .. })),
            expected == "misc"
        );
    }
}
#[test]
fn fields_are_escaped_and_optional_fields_respect_options() {
    let p = paper("escape", PublicationType::JournalArticle);
    let a = render_entry(&p, &BibtexOptions::default());
    assert!(a.content.contains("\\&"));
    assert!(a.content.contains("\\_100\\%"));
    assert!(a.content.contains("\\textbraceleft"));
    assert!(a.content.contains("\\textbackslash"));
    assert!(
        a.content
            .contains("{van And, Family}, {Ada} and {Research and Development}")
    );
    assert!(a.content.contains("日本語"));
    assert!(a.content.contains("eprint = {2401.12345v2}"));
    assert!(a.content.contains("doi = {10.1000/abc}"));
    assert!(!a.content.contains("optional abstract"));
    assert!(!a.content.contains("optional keyword"));
    assert!(a.warnings.is_empty());
    let a = render_entry(
        &p,
        &BibtexOptions {
            include_abstract: true,
            include_keywords: true,
            include_doi: false,
            include_url: false,
        },
    );
    assert!(a.content.contains("optional abstract"));
    assert!(a.content.contains("optional keyword"));
    assert!(!a.content.contains("  doi ="));
    assert!(!a.content.contains("  url ="));
}
#[test]
fn duplicate_keys_are_rejected_without_partial_output_and_empty_export_is_empty() {
    let a = paper("Case", PublicationType::Book);
    let b = paper("case", PublicationType::Book);
    assert!(matches!(
        export_bibtex(&[a.clone(), a], &BibtexOptions::default()),
        Err(BibtexError::DuplicateCitationKey(_))
    ));
    assert!(
        export_bibtex(
            &[b.clone(), paper("CASE", PublicationType::Book)],
            &BibtexOptions::default()
        )
        .is_err()
    );
    assert_eq!(
        export_bibtex(&[], &BibtexOptions::default())
            .unwrap()
            .content,
        ""
    );
}
#[test]
fn required_fields_and_legacy_values_produce_warnings() {
    let now = chrono::Utc::now();
    let p = Paper::from_parts(
        PaperId::new(),
        CitationKey::parse("legacy").unwrap(),
        Bibliography {
            publication: Publication {
                kind: PublicationType::JournalArticle,
                ..Default::default()
            },
            identifiers: Identifiers {
                raw_doi: Some("bad DOI".into()),
                raw_arxiv_id: Some("bad arxiv".into()),
                ..Default::default()
            },
            ..Default::default()
        },
        None,
        UserMetadata::default(),
        now,
        now,
        None,
    )
    .unwrap();
    let a = render_entry(&p, &BibtexOptions::default());
    for required in ["author", "title", "journal", "year"] {
        assert!(a.warnings.contains(&BibtexWarning::MissingRequiredField {
            paper_id: p.id(),
            field: required
        }));
    }
    assert_eq!(
        a.warnings
            .iter()
            .filter(|w| matches!(w, BibtexWarning::InvalidStoredIdentifier { .. }))
            .count(),
        2
    );
    assert!(!a.content.contains("bad DOI"));
    assert!(!a.content.contains("bad arxiv"));
    let thesis = render_entry(
        &paper("thesis", PublicationType::DoctoralThesis),
        &BibtexOptions::default(),
    );
    assert!(thesis.warnings.iter().any(|w| matches!(
        w,
        BibtexWarning::MissingRequiredField {
            field: "school",
            ..
        }
    )));
    assert!(!thesis.content.contains("school = {Publisher}"));
}
#[test]
fn actual_bibtex_engine_accepts_escaped_utf8_entries() {
    use std::{fs, process::Command};
    if Command::new("bibtex").arg("--version").output().is_err() {
        eprintln!("BibTeX not installed; engine test skipped");
        return;
    }
    let path = std::env::temp_dir().join(format!("paper-bibtex-engine-{}", PaperId::new()));
    fs::create_dir(&path).unwrap();
    let p = paper("Engine2026", PublicationType::JournalArticle);
    let output = export_bibtex(&[p], &BibtexOptions::default()).unwrap();
    fs::write(path.join("export.bib"), &output.content).unwrap();
    fs::write(
        path.join("test.aux"),
        "\\relax\n\\citation{*}\n\\bibstyle{plain}\n\\bibdata{export}\n",
    )
    .unwrap();
    let result = Command::new("bibtex")
        .arg("test")
        .current_dir(&path)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr),
        output.content
    );
    let bbl = fs::read_to_string(path.join("test.bbl")).unwrap();
    assert!(bbl.contains("Engine2026"));
    assert!(bbl.contains("日本語"));
    assert!(bbl.contains("Research and Development"));
    fs::remove_dir_all(&path).unwrap();
}

use paper_core::model::*;
use paper_desktop::metadata::{CandidateForm, candidate_from_form};
#[test]
fn selected_empty_fields_are_deletions_and_identifiers_are_only_witnesses() {
    let mut form = CandidateForm {
        id: CandidateId::new().to_string(),
        observed_at: chrono::Utc::now().to_rfc3339(),
        doi: "https://doi.org/10.1000/ABC".into(),
        use_title: true,
        title: " ".into(),
        year: "invalid unselected".into(),
        ..Default::default()
    };
    let first = candidate_from_form(&form).unwrap();
    assert_eq!(first.present_fields, vec![BibliographyField::Title]);
    assert_eq!(first.bibliography.title, None);
    assert_eq!(
        first.bibliography.identifiers.doi_text(),
        Some("10.1000/abc")
    );
    assert_eq!(candidate_from_form(&form).unwrap(), first);
    form.use_year = true;
    assert!(candidate_from_form(&form).is_err());
    form.year = "0".into();
    assert!(candidate_from_form(&form).is_err());
    form.year = "".into();
    assert!(
        candidate_from_form(&form)
            .unwrap()
            .present_fields
            .contains(&BibliographyField::Year)
    );
    form.source = 5;
    assert!(candidate_from_form(&form).is_err());
    form.source_name = "Library import".into();
    assert!(candidate_from_form(&form).is_ok());
    form.updated_at = "yesterday".into();
    assert!(candidate_from_form(&form).is_err());
}

use paper_core::{
    model::{BibliographyValue as V, Contributor, ContributorName, Doi, Identifiers},
    service::{FieldChangeKind as K, compare_field_values as diff},
};
#[test]
fn absence_deletion_empty_and_exact_changes_are_distinct() {
    let value = V::Title(Some("A 数式 x²".into()));
    assert_eq!(diff(&value, None).unwrap(), None);
    assert_eq!(diff(&value, Some(&value)).unwrap(), Some(K::Unchanged));
    assert_eq!(diff(&V::Title(None), Some(&value)).unwrap(), Some(K::Added));
    assert_eq!(
        diff(&value, Some(&V::Title(None))).unwrap(),
        Some(K::Removed)
    );
    assert_eq!(
        diff(&value, Some(&V::Title(Some(String::new())))).unwrap(),
        Some(K::Modified)
    );
    assert_eq!(
        diff(&V::Title(None), Some(&V::Title(Some(String::new())))).unwrap(),
        Some(K::Added)
    );
    assert_eq!(
        diff(&value, Some(&V::Title(Some("a 数式 x²".into())))).unwrap(),
        Some(K::Modified)
    );
    assert!(diff(&value, Some(&V::Year(None))).is_err());
}
#[test]
fn authors_preserve_order_and_identifiers_use_normalized_typed_values() {
    let a = Contributor {
        name: ContributorName::Literal("山田".into()),
        orcid: None,
    };
    let b = Contributor {
        name: ContributorName::Organization("Lab".into()),
        orcid: None,
    };
    let current = V::Authors(vec![a.clone(), b.clone()]);
    for proposed in [
        V::Authors(vec![b.clone(), a.clone()]),
        V::Authors(vec![a.clone()]),
        V::Authors(vec![a.clone(), b.clone(), a.clone()]),
    ] {
        assert_eq!(diff(&current, Some(&proposed)).unwrap(), Some(K::Modified));
    }
    assert_eq!(
        diff(&current, Some(&V::Authors(vec![]))).unwrap(),
        Some(K::Removed)
    );
    let ids = |text: &str| {
        V::Identifiers(Identifiers {
            doi: Some(Doi::try_from(text.to_owned()).unwrap()),
            ..Default::default()
        })
    };
    assert_eq!(
        diff(
            &ids("10.1000/ABC"),
            Some(&ids("https://doi.org/10.1000/abc"))
        )
        .unwrap(),
        Some(K::Unchanged)
    );
    assert!(Doi::try_from("invalid".to_owned()).is_err());
}

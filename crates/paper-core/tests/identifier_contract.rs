use paper_core::model::*;
use serde::de::value::{Error, StringDeserializer};
fn de<T: serde::de::DeserializeOwned>(s: &str) -> Result<T, Error> {
    T::deserialize(StringDeserializer::<Error>::new(s.into()))
}
#[test]
fn doi_normalization_is_shared_by_every_constructor() {
    for raw in [
        "10.1000/AbC",
        " DOI:10.1000/AbC ",
        "https://doi.org/10.1000/AbC",
        "http://dx.doi.org/10.1000/AbC",
        "HTTPS://DOI.ORG/10.1000/AbC",
    ] {
        let value = Doi::parse(raw).unwrap();
        assert_eq!(value.as_str(), "10.1000/abc");
        assert_eq!(value, Doi::try_from(raw.to_string()).unwrap());
        assert_eq!(value, raw.parse().unwrap());
        assert_eq!(value, de::<Doi>(raw).unwrap());
    }
    assert_eq!(
        Doi::parse("https://doi.org/10.1000/a%2Fb%23c")
            .unwrap()
            .as_str(),
        "10.1000/a/b#c"
    );
    assert_eq!(
        Doi::parse("https://doi.org/10.1000/a/../b")
            .unwrap()
            .as_str(),
        "10.1000/a/../b"
    );
    assert!(Doi::parse("https://doi.org/10.1000/a%ZZ").is_err());
    assert_eq!(
        Doi::parse("10.1000.12/ΣuffiX").unwrap().as_str(),
        "10.1000.12/Σuffix"
    );
    for raw in [
        "",
        "10.1000/",
        "10./x",
        "11.1000/x",
        "10.abc/x",
        "10..1/x",
        "10.1000/has space",
        "https://example.org/10.1000/x",
        "https://doi.org/10.1000/x?q=1",
        "https://doi.org/10.1000/x#part",
        "10.1000/\nvalue",
    ] {
        assert!(Doi::parse(raw).is_err(), "{raw}");
        assert!(de::<Doi>(raw).is_err(), "{raw}");
    }
}
#[test]
fn arxiv_supports_old_new_and_versioned_forms_without_losing_versions() {
    for (raw, canonical) in [
        (" arXiv:2401.12345v2 ", "2401.12345v2"),
        ("https://arxiv.org/abs/0704.0001v1", "0704.0001v1"),
        (
            "https://arxiv.org/pdf/hep-th/9901001v12.pdf",
            "hep-th/9901001v12",
        ),
        ("math.GT/0301001", "math.GT/0301001"),
        ("solv-int/9901001", "solv-int/9901001"),
        ("1501.00001", "1501.00001"),
        ("1412.0001", "1412.0001"),
    ] {
        let value = ArxivId::parse(raw).unwrap();
        assert_eq!(value.as_str(), canonical);
        assert_eq!(value, de::<ArxivId>(raw).unwrap());
        assert_eq!(value, raw.parse().unwrap());
    }
    for raw in [
        "",
        "2400.12345",
        "2413.12345",
        "2401.1234",
        "1412.12345",
        "0703.0001",
        "1501.00000",
        "2401.12345v0",
        "2401.12345v01",
        "2401.12345v",
        "2401.12345vabc",
        "hep-th/0704001",
        "hep-th/9106001",
        "hep-th/9901000",
        "https://example.org/abs/2401.12345",
        "https://arxiv.org/abs/2401.12345?x=1",
        "math.gt/0301001",
    ] {
        assert!(ArxivId::parse(raw).is_err(), "{raw}");
        assert!(de::<ArxivId>(raw).is_err(), "{raw}");
    }
}
#[test]
fn legacy_identifiers_are_distinct_from_validated_input() {
    let mut ids = Identifiers::default();
    ids.restore_doi(Some("bad DOI".into()));
    ids.restore_arxiv(Some("bad arxiv".into()));
    assert_eq!(ids.raw_doi.as_deref(), Some("bad DOI"));
    assert!(ids.doi.is_none());
    assert!(ids.validate().is_ok());
    assert!(ids.validate_input().is_err());
    let b = Bibliography {
        identifiers: ids.clone(),
        ..Default::default()
    };
    assert!(
        Paper::new(
            PaperId::new(),
            CitationKey::parse("new").unwrap(),
            b,
            chrono::Utc::now()
        )
        .is_err()
    );
    ids.doi = Some(Doi::parse("10.1000/example").unwrap());
    assert!(ids.validate().is_err());
}

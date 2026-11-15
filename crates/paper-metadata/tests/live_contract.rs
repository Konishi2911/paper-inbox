use paper_metadata::{HttpMetadataResolver, parse_identifier};
#[test]
#[ignore = "explicit public-provider smoke test; ordinary tests are offline"]
fn live_doi_and_arxiv_lookup() {
    let resolver = HttpMetadataResolver::new().unwrap();
    for input in ["10.1038/nphys1170", "1706.03762v1"] {
        let candidates = resolver
            .resolve_blocking(&parse_identifier(input).unwrap())
            .unwrap();
        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].bibliography.title.is_some());
        assert!(!candidates[0].bibliography.authors.is_empty());
    }
}

use paper_desktop::metadata_diff::{DiffKind, inline_text, word_diff};
#[test]
fn word_diff_reconstructs_both_unicode_inputs_without_case_folding() {
    for (a, b) in [
        ("", ""),
        ("same", "same"),
        ("A Graph α² 日本語", "A New graph α³ 日本語"),
        ("a\nb", "b\na"),
        ("", "added"),
        ("removed", ""),
    ] {
        let diff = word_diff(a, b).unwrap();
        let old: String = diff
            .iter()
            .filter(|s| s.kind != DiffKind::Insert)
            .map(|s| s.text.as_str())
            .collect();
        let new: String = diff
            .iter()
            .filter(|s| s.kind != DiffKind::Delete)
            .map(|s| s.text.as_str())
            .collect();
        assert_eq!(old, a);
        assert_eq!(new, b);
    }
    assert!(inline_text("A", "a").contains("【−A】"));
}
#[test]
fn huge_or_many_token_inputs_fall_back_to_original_comparison() {
    assert!(word_diff(&"日".repeat(100000), "x").is_none());
    assert!(word_diff(&"a ".repeat(1000), "x").is_none());
    assert!(inline_text(&"a ".repeat(1000), "x").contains("全文表示"));
}

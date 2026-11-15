use paper_core::{
    model::{Contributor, ContributorName},
    service::{MergeResult, compare_contributors, compare_sets, three_way_merge},
};
fn author(name: &str, orcid: Option<&str>) -> Contributor {
    Contributor {
        name: ContributorName::Literal(name.into()),
        orcid: orcid.map(str::to_owned),
    }
}
#[test]
fn ordered_authors_distinguish_add_remove_move_and_verified_name_changes() {
    let a = author("A", Some("0000-0001"));
    let b = author("B", None);
    let c = author("C", None);
    let diff = compare_contributors(
        &[a.clone(), b.clone()],
        &[b.clone(), author("Corrected A", Some("0000-0001")), c],
    );
    assert!(diff[0].moved);
    assert!(diff[1].moved && diff[1].changed);
    assert_eq!(diff[2].current_index, None);
    let removed = compare_contributors(&[a, b], &[]);
    assert_eq!(removed.len(), 2);
    assert!(removed.iter().all(|d| d.proposed_index.is_none()));
}
#[test]
fn same_name_or_duplicate_orcid_never_proves_identity() {
    let diff = compare_contributors(&[author("A", None)], &[author("A", Some("orcid"))]);
    assert_eq!(diff.len(), 2);
    assert!(diff.iter().all(|d| d.identity_uncertain));
    let diff = compare_contributors(
        &[
            author("A", Some("duplicate")),
            author("B", Some("duplicate")),
        ],
        &[author("renamed", Some("duplicate"))],
    );
    assert_eq!(diff[0].current_index, None);
}
#[test]
fn tags_are_sets_and_three_way_comparison_does_not_merge_ordered_values() {
    let diff = compare_sets(&["a", "a", "b"], &["b", "c", "c"]);
    assert_eq!(diff.added, vec!["c"]);
    assert_eq!(diff.removed, vec!["a"]);
    assert!(matches!(
        three_way_merge(&vec!["a", "b"], &vec!["b", "a"], &vec!["a", "b", "c"]),
        MergeResult::Conflict { .. }
    ));
    assert_eq!(
        three_way_merge(&"base", &"base", &"remote"),
        MergeResult::TakeRemote("remote")
    );
    assert_eq!(
        three_way_merge(&"base", &"local", &"base"),
        MergeResult::TakeLocal("local")
    );
    assert_eq!(
        three_way_merge(&"base", &"same", &"same"),
        MergeResult::Converged("same")
    );
    assert_eq!(
        three_way_merge(&"base", &"base", &"base"),
        MergeResult::Unchanged("base")
    );
}

#[test]
fn attachment_content_and_filename_changes_are_independent() {
    use paper_core::{
        model::{Attachment, AttachmentId, AttachmentKind, ContentHash, PaperId},
        service::compare_attachment,
    };
    let a = Attachment {
        id: AttachmentId::new(),
        paper_id: PaperId::new(),
        version_id: None,
        content_hash: ContentHash::from_bytes([1; 32]),
        filename: "paper.pdf".into(),
        media_type: "application/pdf".into(),
        size_bytes: 10,
        kind: AttachmentKind::FullText,
        created_at: chrono::Utc::now(),
    };
    let mut b = a.clone();
    b.filename = "renamed.pdf".into();
    let d = compare_attachment(&a, &b);
    assert!(d.metadata_changed && !d.content_changed);
    b = a.clone();
    b.content_hash = ContentHash::from_bytes([2; 32]);
    let d = compare_attachment(&a, &b);
    assert!(d.content_changed && !d.metadata_changed);
}

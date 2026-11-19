//! Comparisons for presentation/inspection, never automatic merge policies.
use crate::model::{Attachment, Contributor};
use std::{collections::HashSet, hash::Hash};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContributorDiff {
    pub current_index: Option<usize>,
    pub proposed_index: Option<usize>,
    pub moved: bool,
    pub changed: bool,
    pub identity_uncertain: bool,
}
pub fn compare_contributors(
    current: &[Contributor],
    proposed: &[Contributor],
) -> Vec<ContributorDiff> {
    let mut used = HashSet::new();
    let mut result = Vec::new();
    for (next_index, next) in proposed.iter().enumerate() {
        let unique_orcid = next.orcid.as_ref().filter(|id| {
            !id.trim().is_empty()
                && current
                    .iter()
                    .filter(|c| c.orcid.as_ref() == Some(id))
                    .count()
                    == 1
                && proposed
                    .iter()
                    .filter(|c| c.orcid.as_ref() == Some(id))
                    .count()
                    == 1
        });
        let matched = unique_orcid
            .and_then(|id| current.iter().position(|c| c.orcid.as_ref() == Some(id)))
            .or_else(|| {
                if current.iter().filter(|c| *c == next).count() == 1
                    && proposed.iter().filter(|c| *c == next).count() == 1
                {
                    current.iter().position(|c| c == next)
                } else {
                    None
                }
            })
            .filter(|i| !used.contains(i));
        if let Some(index) = matched {
            used.insert(index);
            result.push(ContributorDiff {
                current_index: Some(index),
                proposed_index: Some(next_index),
                moved: index != next_index,
                changed: current[index] != *next,
                identity_uncertain: false,
            });
        } else {
            result.push(ContributorDiff {
                current_index: None,
                proposed_index: Some(next_index),
                moved: false,
                changed: false,
                identity_uncertain: current.iter().any(|c| c.name == next.name),
            });
        }
    }
    for (index, entry) in current.iter().enumerate() {
        if !used.contains(&index) {
            result.push(ContributorDiff {
                current_index: Some(index),
                proposed_index: None,
                moved: false,
                changed: false,
                identity_uncertain: proposed.iter().any(|c| c.name == entry.name),
            });
        }
    }
    result
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetDiff<T> {
    pub added: Vec<T>,
    pub removed: Vec<T>,
}
pub fn compare_sets<T: Clone + Eq + Hash>(current: &[T], proposed: &[T]) -> SetDiff<T> {
    let old: HashSet<_> = current.iter().collect();
    let new: HashSet<_> = proposed.iter().collect();
    let mut emitted = HashSet::new();
    let added = proposed
        .iter()
        .filter(|v| !old.contains(v) && emitted.insert(*v))
        .cloned()
        .collect();
    emitted.clear();
    let removed = current
        .iter()
        .filter(|v| !new.contains(v) && emitted.insert(*v))
        .cloned()
        .collect();
    SetDiff { added, removed }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttachmentDiff {
    pub content_changed: bool,
    pub metadata_changed: bool,
}
pub fn compare_attachment(current: &Attachment, proposed: &Attachment) -> AttachmentDiff {
    AttachmentDiff {
        content_changed: current.content_hash != proposed.content_hash,
        metadata_changed: current.filename != proposed.filename
            || current.media_type != proposed.media_type
            || current.kind != proposed.kind
            || current.size_bytes != proposed.size_bytes
            || current.version_id != proposed.version_id,
    }
}

//! Three-way inspection groundwork. This does not execute synchronization or apply merges.
use super::{MergeResult, three_way_merge};
use crate::model::{BibliographyField, BibliographyValue, FieldRevision, RevisionId};
use std::collections::{HashMap, HashSet};
use thiserror::Error;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreeWayFieldComparison {
    pub base: FieldRevision,
    pub local: FieldRevision,
    pub remote: FieldRevision,
    pub values: MergeResult<BibliographyValue>,
    /// Collections/identity/ordered people need their own policy even if values compare cleanly.
    pub requires_field_policy: bool,
}
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RevisionComparisonError {
    #[error("Revision targets differ")]
    TargetMismatch,
    #[error("Revision ID has different payloads: {0}")]
    RevisionCollision(RevisionId),
    #[error("Common ancestry cannot be established from supplied history")]
    UnverifiedAncestry,
}
pub fn compare_three_way_revisions(
    base: &FieldRevision,
    local: &FieldRevision,
    remote: &FieldRevision,
    history: &[FieldRevision],
) -> Result<ThreeWayFieldComparison, RevisionComparisonError> {
    if base.target() != local.target() || base.target() != remote.target() {
        return Err(RevisionComparisonError::TargetMismatch);
    }
    let mut revisions = HashMap::new();
    for revision in history.iter().chain([base, local, remote]) {
        if let Some(previous) = revisions.insert(revision.id, revision)
            && previous != revision
        {
            return Err(RevisionComparisonError::RevisionCollision(revision.id));
        }
    }
    let descends = |head: &FieldRevision| {
        let mut todo = vec![head.id];
        let mut visited = HashSet::new();
        while let Some(id) = todo.pop() {
            if id == base.id {
                return true;
            }
            if !visited.insert(id) {
                continue;
            }
            if let Some(revision) = revisions.get(&id)
                && revision.target() == base.target()
            {
                todo.extend_from_slice(&revision.parents);
            }
        }
        false
    };
    if !descends(local) || !descends(remote) {
        return Err(RevisionComparisonError::UnverifiedAncestry);
    }
    Ok(ThreeWayFieldComparison {
        base: base.clone(),
        local: local.clone(),
        remote: remote.clone(),
        values: three_way_merge(&base.value, &local.value, &remote.value),
        requires_field_policy: matches!(
            base.value.field(),
            BibliographyField::Authors
                | BibliographyField::Editors
                | BibliographyField::Identifiers
                | BibliographyField::Keywords
        ),
    })
}

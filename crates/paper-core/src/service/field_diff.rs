//! Semantic comparison only. Display formatting must never decide adoption.
use crate::model::{BibliographyField, BibliographyValue, Identifiers, PublicationType};
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldChangeKind {
    Unchanged,
    Added,
    Removed,
    Modified,
}

#[derive(Debug, Error)]
#[error("Cannot compare different bibliography fields: {current:?} and {proposed:?}")]
pub struct FieldDiffError {
    pub current: BibliographyField,
    pub proposed: BibliographyField,
}

/// None means the source did not supply this field, rather than a deletion.
/// Empty strings remain distinct from unset values. Contributor order is significant.
pub fn compare_field_values(
    current: &BibliographyValue,
    proposed: Option<&BibliographyValue>,
) -> Result<Option<FieldChangeKind>, FieldDiffError> {
    let Some(proposed) = proposed else {
        return Ok(None);
    };
    if current.field() != proposed.field() {
        return Err(FieldDiffError {
            current: current.field(),
            proposed: proposed.field(),
        });
    }
    Ok(Some(if current == proposed {
        FieldChangeKind::Unchanged
    } else if unset(current) {
        FieldChangeKind::Added
    } else if unset(proposed) {
        FieldChangeKind::Removed
    } else {
        FieldChangeKind::Modified
    }))
}
fn unset(value: &BibliographyValue) -> bool {
    match value {
        BibliographyValue::Title(v)
        | BibliographyValue::ContainerTitle(v)
        | BibliographyValue::Publisher(v)
        | BibliographyValue::Volume(v)
        | BibliographyValue::Issue(v)
        | BibliographyValue::Pages(v)
        | BibliographyValue::ArticleNumber(v)
        | BibliographyValue::Abstract(v) => v.is_none(),
        BibliographyValue::Authors(v) | BibliographyValue::Editors(v) => v.is_empty(),
        BibliographyValue::Year(v) => v.is_none(),
        BibliographyValue::Month(v) => v.is_none(),
        BibliographyValue::PublicationType(v) => *v == PublicationType::Unknown,
        BibliographyValue::Identifiers(v) => *v == Identifiers::default(),
        BibliographyValue::Keywords(v) => v.is_empty(),
    }
}

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{
    Contributor, Identifiers, PaperId, Provenance, PublicationType, RevisionId, VersionId,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BibliographyField {
    Title,
    Authors,
    Editors,
    PublicationType,
    ContainerTitle,
    Publisher,
    Year,
    Month,
    Volume,
    Issue,
    Pages,
    ArticleNumber,
    Identifiers,
    Abstract,
    Keywords,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BibliographyValue {
    Title(Option<String>),
    Authors(Vec<Contributor>),
    Editors(Vec<Contributor>),

    PublicationType(PublicationType),

    ContainerTitle(Option<String>),
    Publisher(Option<String>),

    Year(Option<u16>),
    Month(Option<u8>),

    Volume(Option<String>),
    Issue(Option<String>),
    Pages(Option<String>),
    ArticleNumber(Option<String>),

    Identifiers(Identifiers),

    Abstract(Option<String>),
    Keywords(Vec<String>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldRevision {
    pub id: RevisionId,

    pub paper_id: PaperId,
    pub version_id: Option<VersionId>,

    pub parents: Vec<RevisionId>,

    pub value: BibliographyValue,
    pub provenance: Provenance,

    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FieldTarget {
    pub paper_id: PaperId,
    pub version_id: Option<VersionId>,
    pub field: BibliographyField,
}

impl BibliographyValue {
    pub fn field(&self) -> BibliographyField {
        use BibliographyField as F;

        match self {
            Self::Title(_) => F::Title,
            Self::Authors(_) => F::Authors,
            Self::Editors(_) => F::Editors,
            Self::PublicationType(_) => F::PublicationType,
            Self::ContainerTitle(_) => F::ContainerTitle,
            Self::Publisher(_) => F::Publisher,
            Self::Year(_) => F::Year,
            Self::Month(_) => F::Month,
            Self::Volume(_) => F::Volume,
            Self::Issue(_) => F::Issue,
            Self::Pages(_) => F::Pages,
            Self::ArticleNumber(_) => F::ArticleNumber,
            Self::Identifiers(_) => F::Identifiers,
            Self::Abstract(_) => F::Abstract,
            Self::Keywords(_) => F::Keywords,
        }
    }
}

impl FieldRevision {
    pub fn target(&self) -> FieldTarget {
        FieldTarget {
            paper_id: self.paper_id,
            version_id: self.version_id,
            field: self.value.field(),
        }
    }
}

impl BibliographyValue {
    /// Whether initial manual registration supplied a value for this field.
    /// Only use when seeding creation history. Explicit edits, including clears,
    /// always create revisions even when this returns false.
    pub fn has_initial_value(&self) -> bool {
        fn supplied(value: &Option<String>) -> bool {
            value.as_ref().is_some_and(|s| !s.trim().is_empty())
        }
        match self {
            Self::Title(v)
            | Self::ContainerTitle(v)
            | Self::Publisher(v)
            | Self::Volume(v)
            | Self::Issue(v)
            | Self::Pages(v)
            | Self::ArticleNumber(v)
            | Self::Abstract(v) => supplied(v),
            Self::Authors(v) | Self::Editors(v) => !v.is_empty(),
            Self::PublicationType(v) => *v != super::PublicationType::Unknown,
            Self::Year(v) => v.is_some(),
            Self::Month(v) => v.is_some(),
            Self::Keywords(v) => v.iter().any(|s| !s.trim().is_empty()),
            Self::Identifiers(v) => [
                v.doi.as_ref().map(super::Doi::as_str),
                v.arxiv_id.as_ref().map(super::ArxivId::as_str),
                v.raw_doi.as_deref(),
                v.raw_arxiv_id.as_deref(),
                v.isbn.as_deref(),
                v.issn.as_deref(),
                v.url.as_deref(),
            ]
            .into_iter()
            .flatten()
            .any(|s| !s.trim().is_empty()),
        }
    }
}

impl BibliographyField {
    pub const ALL: [Self; 15] = [
        Self::Title,
        Self::Authors,
        Self::Editors,
        Self::PublicationType,
        Self::ContainerTitle,
        Self::Publisher,
        Self::Year,
        Self::Month,
        Self::Volume,
        Self::Issue,
        Self::Pages,
        Self::ArticleNumber,
        Self::Identifiers,
        Self::Abstract,
        Self::Keywords,
    ];
}
impl super::Bibliography {
    pub fn value(&self, field: BibliographyField) -> BibliographyValue {
        use BibliographyField as F;
        match field {
            F::Title => BibliographyValue::Title(self.title.clone()),
            F::Authors => BibliographyValue::Authors(self.authors.clone()),
            F::Editors => BibliographyValue::Editors(self.editors.clone()),
            F::PublicationType => BibliographyValue::PublicationType(self.publication.kind),
            F::ContainerTitle => {
                BibliographyValue::ContainerTitle(self.publication.container_title.clone())
            }
            F::Publisher => BibliographyValue::Publisher(self.publication.publisher.clone()),
            F::Year => BibliographyValue::Year(self.publication.year),
            F::Month => BibliographyValue::Month(self.publication.month),
            F::Volume => BibliographyValue::Volume(self.publication.volume.clone()),
            F::Issue => BibliographyValue::Issue(self.publication.issue.clone()),
            F::Pages => BibliographyValue::Pages(self.publication.pages.clone()),
            F::ArticleNumber => {
                BibliographyValue::ArticleNumber(self.publication.article_number.clone())
            }
            F::Identifiers => BibliographyValue::Identifiers(self.identifiers.clone()),
            F::Abstract => BibliographyValue::Abstract(self.abstract_text.clone()),
            F::Keywords => BibliographyValue::Keywords(self.keywords.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn initial_presence_distinguishes_unset_and_supplied_values() {
        for value in [
            BibliographyValue::Title(None),
            BibliographyValue::Title(Some(" \t".into())),
            BibliographyValue::Authors(vec![]),
            BibliographyValue::Editors(vec![]),
            BibliographyValue::PublicationType(super::super::PublicationType::Unknown),
            BibliographyValue::Year(None),
            BibliographyValue::Month(None),
            BibliographyValue::Identifiers(Identifiers::default()),
            BibliographyValue::Keywords(vec![" ".into()]),
        ] {
            assert!(!value.has_initial_value(), "{value:?}");
        }
        for value in [
            BibliographyValue::Title(Some("title".into())),
            BibliographyValue::PublicationType(super::super::PublicationType::Other),
            BibliographyValue::Year(Some(2026)),
            BibliographyValue::Month(Some(1)),
            BibliographyValue::Keywords(vec!["keyword".into()]),
            BibliographyValue::Identifiers(Identifiers {
                url: Some("https://example.test".into()),
                ..Default::default()
            }),
        ] {
            assert!(value.has_initial_value(), "{value:?}");
        }
    }
}

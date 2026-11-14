use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{
    PaperId,
    VersionId,
    RevisionId,
    Contributor,
    PublicationType,
    Identifiers,
    Provenance,
};

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash,
    Serialize, Deserialize,
)]
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

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash,
    Serialize, Deserialize,
)]
pub struct FieldTarget {
    pub paper_id: PaperId,
    pub version_id: Option<VersionId>,
    pub field: BibliographyField,
}


impl BibliographyValue {
    pub fn field(&self) -> BibliographyField {
        use BibliographyField as F;

        match self {
            Self::Title(_)           => F::Title,
            Self::Authors(_)         => F::Authors,
            Self::Editors(_)         => F::Editors,
            Self::PublicationType(_) => F::PublicationType,
            Self::ContainerTitle(_)  => F::ContainerTitle,
            Self::Publisher(_)       => F::Publisher,
            Self::Year(_)            => F::Year,
            Self::Month(_)           => F::Month,
            Self::Volume(_)          => F::Volume,
            Self::Issue(_)           => F::Issue,
            Self::Pages(_)           => F::Pages,
            Self::ArticleNumber(_)   => F::ArticleNumber,
            Self::Identifiers(_)     => F::Identifiers,
            Self::Abstract(_)        => F::Abstract,
            Self::Keywords(_)        => F::Keywords,
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
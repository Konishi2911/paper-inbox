use serde::{Deserialize, Serialize};

use super::{
    Doi,
    ArxivId,
    Contributor,
    Publication,
    Provenance,
    VersionKind,
};

#[derive(
    Clone, Debug, Default, PartialEq, Eq,
    Serialize, Deserialize,
)]
pub struct Identifiers {
    pub doi: Option<Doi>,
    pub arxiv_id: Option<ArxivId>,
    pub isbn: Option<String>,
    pub issn: Option<String>,
    pub url: Option<String>,
}

#[derive(
    Clone, Debug, Default, PartialEq, Eq,
    Serialize, Deserialize,
)]
pub struct Bibliography {
    pub title: Option<String>,

    pub authors: Vec<Contributor>,
    pub editors: Vec<Contributor>,

    pub publication: Publication,
    pub identifiers: Identifiers,

    pub abstract_text: Option<String>,
    pub keywords: Vec<String>,
}

/// Bibliographic information obtained from an external source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataCandidate {
    pub bibliography: Bibliography,
    pub provenance: Provenance,
    pub version_kind: VersionKind,
}
use serde::{Deserialize, Serialize};

use super::{ArxivId, Contributor, Doi, Provenance, Publication, VersionKind};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identifiers {
    pub doi: Option<Doi>,
    pub arxiv_id: Option<ArxivId>,
    /// Invalid legacy SQL values are retained for display and explicit repair.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_doi: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_arxiv_id: Option<String>,
    pub isbn: Option<String>,
    pub issn: Option<String>,
    pub url: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
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

impl Identifiers {
    pub fn doi_text(&self) -> Option<&str> {
        self.doi
            .as_ref()
            .map(Doi::as_str)
            .or(self.raw_doi.as_deref())
    }
    pub fn arxiv_text(&self) -> Option<&str> {
        self.arxiv_id
            .as_ref()
            .map(ArxivId::as_str)
            .or(self.raw_arxiv_id.as_deref())
    }
    pub fn validate(&self) -> Result<(), super::IdentifierError> {
        if (self.doi.is_some() && self.raw_doi.is_some())
            || (self.arxiv_id.is_some() && self.raw_arxiv_id.is_some())
        {
            return Err(super::IdentifierError::AmbiguousIdentifier);
        }
        Ok(())
    }
    pub fn validate_input(&self) -> Result<(), super::IdentifierError> {
        self.validate()?;
        if self.raw_doi.is_some() || self.raw_arxiv_id.is_some() {
            return Err(super::IdentifierError::UnvalidatedIdentifier);
        }
        Ok(())
    }
    pub fn restore_doi(&mut self, raw: Option<String>) {
        self.doi = None;
        self.raw_doi = None;
        if let Some(raw) = raw {
            match Doi::parse(&raw) {
                Ok(value) => self.doi = Some(value),
                Err(_) => self.raw_doi = Some(raw),
            }
        }
    }
    pub fn restore_arxiv(&mut self, raw: Option<String>) {
        self.arxiv_id = None;
        self.raw_arxiv_id = None;
        if let Some(raw) = raw {
            match ArxivId::parse(&raw) {
                Ok(value) => self.arxiv_id = Some(value),
                Err(_) => self.raw_arxiv_id = Some(raw),
            }
        }
    }
}

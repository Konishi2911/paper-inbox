use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{
    Bibliography, BibliographyValue, CitationKey, PaperId, UserMetadata, VersionId, VersionKind,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Paper {
    id: PaperId,

    bibliography: Bibliography,
    canonical_version: Option<VersionId>,

    citation_key: CitationKey,
    metadata: UserMetadata,

    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    deleted_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaperVersion {
    pub id: VersionId,
    pub paper_id: PaperId,

    pub kind: VersionKind,
    pub bibliography: Bibliography,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum PaperError {
    #[error("Publication month must be between 1 and 12")]
    InvalidMonth,
    #[error(transparent)]
    Identifier(#[from] super::IdentifierError),
    #[error("Publication year must be greater than zero")]
    InvalidYear,
    #[error("Cannot edit a deleted paper")]
    Deleted,
    #[error("Modification time predates the last update")]
    InvalidTimestamp,
}

impl Paper {
    pub fn new(
        id: PaperId,
        citation_key: CitationKey,
        bibliography: Bibliography,
        now: DateTime<Utc>,
    ) -> Result<Self, PaperError> {
        bibliography.identifiers.validate_input()?;
        Self::from_parts(
            id,
            citation_key,
            bibliography,
            None,
            UserMetadata::default(),
            now,
            now,
            None,
        )
    }

    /// Reconstruct a stored paper while validating domain invariants.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: PaperId,
        citation_key: CitationKey,
        bibliography: Bibliography,
        canonical_version: Option<VersionId>,
        metadata: UserMetadata,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
        deleted_at: Option<DateTime<Utc>>,
    ) -> Result<Self, PaperError> {
        bibliography.validate()?;
        if updated_at < created_at || deleted_at.is_some_and(|t| t < created_at || t > updated_at) {
            return Err(PaperError::InvalidTimestamp);
        }
        Ok(Self {
            id,
            citation_key,
            bibliography,
            canonical_version,
            metadata,
            created_at,
            updated_at,
            deleted_at,
        })
    }
    pub fn id(&self) -> PaperId {
        self.id
    }
    pub fn citation_key(&self) -> &CitationKey {
        &self.citation_key
    }
    pub fn bibliography(&self) -> &Bibliography {
        &self.bibliography
    }
    pub fn canonical_version(&self) -> Option<VersionId> {
        self.canonical_version
    }
    pub fn metadata(&self) -> &UserMetadata {
        &self.metadata
    }
    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }
    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }
    pub fn deleted_at(&self) -> Option<DateTime<Utc>> {
        self.deleted_at
    }
    fn check_edit(&self, now: DateTime<Utc>) -> Result<(), PaperError> {
        if self.deleted_at.is_some() {
            return Err(PaperError::Deleted);
        }
        if now < self.updated_at {
            return Err(PaperError::InvalidTimestamp);
        }
        Ok(())
    }
    pub fn edit_bibliography(
        &mut self,
        value: &BibliographyValue,
        now: DateTime<Utc>,
    ) -> Result<(), PaperError> {
        self.check_edit(now)?;
        if let BibliographyValue::Identifiers(ids) = value {
            ids.validate_input()?;
        }
        let mut next = self.bibliography.clone();
        next.apply(value);
        next.validate()?;
        self.bibliography = next;
        self.updated_at = now;
        Ok(())
    }
    pub fn change_citation_key(
        &mut self,
        key: CitationKey,
        now: DateTime<Utc>,
    ) -> Result<(), PaperError> {
        self.check_edit(now)?;
        self.citation_key = key;
        self.updated_at = now;
        Ok(())
    }
    pub fn update_metadata(
        &mut self,
        metadata: UserMetadata,
        now: DateTime<Utc>,
    ) -> Result<(), PaperError> {
        self.check_edit(now)?;
        self.metadata = metadata;
        self.updated_at = now;
        Ok(())
    }
    pub fn delete(&mut self, now: DateTime<Utc>) -> Result<(), PaperError> {
        if now < self.updated_at {
            return Err(PaperError::InvalidTimestamp);
        }
        if self.deleted_at.is_none() {
            self.deleted_at = Some(now);
            self.updated_at = now;
        }
        Ok(())
    }
    pub fn restore(&mut self, now: DateTime<Utc>) -> Result<(), PaperError> {
        if now < self.updated_at {
            return Err(PaperError::InvalidTimestamp);
        }
        if self.deleted_at.is_some() {
            self.deleted_at = None;
            self.updated_at = now;
        }
        Ok(())
    }
}

impl Bibliography {
    pub fn validate(&self) -> Result<(), PaperError> {
        self.identifiers.validate()?;
        if self
            .publication
            .month
            .is_some_and(|m| !(1..=12).contains(&m))
        {
            return Err(PaperError::InvalidMonth);
        }
        if self.publication.year == Some(0) {
            return Err(PaperError::InvalidYear);
        }
        Ok(())
    }
    pub fn apply(&mut self, value: &BibliographyValue) {
        use BibliographyValue as V;
        match value {
            V::Title(v) => self.title = v.clone(),
            V::Authors(v) => self.authors = v.clone(),
            V::Editors(v) => self.editors = v.clone(),
            V::PublicationType(v) => self.publication.kind = *v,
            V::ContainerTitle(v) => self.publication.container_title = v.clone(),
            V::Publisher(v) => self.publication.publisher = v.clone(),
            V::Year(v) => self.publication.year = *v,
            V::Month(v) => self.publication.month = *v,
            V::Volume(v) => self.publication.volume = v.clone(),
            V::Issue(v) => self.publication.issue = v.clone(),
            V::Pages(v) => self.publication.pages = v.clone(),
            V::ArticleNumber(v) => self.publication.article_number = v.clone(),
            V::Identifiers(v) => self.identifiers = v.clone(),
            V::Abstract(v) => self.abstract_text = v.clone(),
            V::Keywords(v) => self.keywords = v.clone(),
        }
    }
}

#[cfg(test)]
#[path = "paper_tests.rs"]
mod tests;

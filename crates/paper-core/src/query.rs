use chrono::{DateTime, Utc};

use crate::model::{
    CitationKey, CollectionId, Contributor, PaperId, PublicationType, ReadingStatus, TagId,
};

/// Search conditions for the paper library.
#[derive(Clone, Debug, Default)]
pub struct PaperQuery {
    pub text: Option<String>,
    pub filter: PaperFilter,
    pub sort: PaperSort,
    pub pagination: Pagination,
}

/// Filtering conditions.
#[derive(Clone, Debug, Default)]
pub struct PaperFilter {
    pub tags: Vec<TagId>,
    pub tag_match: TagMatch,
    pub collection: Option<CollectionId>,
    pub publication_type: Option<PublicationType>,
    pub reading_status: Option<ReadingStatus>,
    pub year_from: Option<u16>,
    pub year_to: Option<u16>,
    pub favorite: Option<bool>,
    pub has_attachment: Option<bool>,
    pub has_pending_proposal: Option<bool>,
    pub include_archived: bool,
    pub deletion: DeletionFilter,
}

/// Matching strategy for multiple tags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TagMatch {
    #[default]
    All,
    Any,
}

/// Sort specification.
#[derive(Clone, Copy, Debug)]
pub struct PaperSort {
    pub field: SortField,
    pub order: SortOrder,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortField {
    Relevance,
    Title,
    PublicationYear,
    DateAdded,
    DateModified,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl Default for PaperSort {
    fn default() -> Self {
        Self {
            field: SortField::DateAdded,
            order: SortOrder::Descending,
        }
    }
}

/// Offset-based pagination.
#[derive(Clone, Copy, Debug)]
pub struct Pagination {
    pub limit: usize,
    pub offset: usize,
}

impl Default for Pagination {
    fn default() -> Self {
        Self {
            limit: 100,
            offset: 0,
        }
    }
}

/// Lightweight projection for library listings.
#[derive(Clone, Debug)]
pub struct PaperSummary {
    pub id: PaperId,
    pub citation_key: CitationKey,
    pub title: Option<String>,
    pub authors: Vec<Contributor>,
    pub publication_type: PublicationType,
    pub container_title: Option<String>,
    pub year: Option<u16>,
    pub reading_status: ReadingStatus,
    pub favorite: bool,
    pub archived: bool,
    pub tags: Vec<TagId>,
    pub has_attachment: bool,
    pub has_pending_proposal: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Paginated search result.
#[derive(Clone, Debug)]
pub struct SearchResult {
    pub papers: Vec<PaperSummary>,
    pub total_count: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DeletionFilter {
    #[default]
    Active,
    Deleted,
    All,
}

#[derive(Clone, Debug)]
pub struct MetadataProposalDetail {
    pub proposal: crate::model::UpdateProposal,
    pub expected_value: Option<crate::model::BibliographyValue>,
    pub current_value: Option<crate::model::BibliographyValue>,
    pub proposed_value: Option<crate::model::BibliographyValue>,
    pub provenance: Option<crate::model::Provenance>,
}
#[derive(Clone, Debug)]
pub struct MetadataReview {
    pub state: crate::command::PaperEditState,
    pub candidates: Vec<crate::model::StoredMetadataCandidate>,
    pub proposals: Vec<MetadataProposalDetail>,
    pub versions: Vec<crate::model::PaperVersion>,
}

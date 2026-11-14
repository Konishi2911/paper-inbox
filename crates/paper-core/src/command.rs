use crate::model::{
    Attachment, AttachmentId, BibliographyValue, CitationKey, Collection, CollectionId, EventId,
    MetadataCandidate, Paper, PaperId, PaperVersion, ProposalId, ProposalResolution, ReadingStatus,
    RevisionId, Tag, TagId, VersionId,
};

#[derive(Clone, Debug)]
pub enum LibraryCommand {
    CreatePaper {
        paper: Paper,
    },

    AddVersion {
        version: PaperVersion,
    },

    SetCanonicalVersion {
        paper_id: PaperId,
        version_id: Option<VersionId>,
    },

    EditBibliography {
        paper_id: PaperId,
        version_id: Option<VersionId>,
        value: BibliographyValue,
        expected_heads: Vec<RevisionId>,
    },

    ChangeCitationKey {
        paper_id: PaperId,
        citation_key: CitationKey,
    },

    SubmitMetadataCandidate {
        paper_id: PaperId,
        candidate: MetadataCandidate,
    },

    ResolveProposal {
        proposal_id: ProposalId,
        resolution: ProposalResolution,
    },

    UpdateNotes {
        paper_id: PaperId,
        notes: String,
    },

    SetReadingStatus {
        paper_id: PaperId,
        status: ReadingStatus,
    },

    SetFavorite {
        paper_id: PaperId,
        favorite: bool,
    },

    SetArchived {
        paper_id: PaperId,
        archived: bool,
    },

    CreateTag {
        tag: Tag,
    },

    AddTag {
        paper_id: PaperId,
        tag_id: TagId,
    },

    RemoveTag {
        paper_id: PaperId,
        tag_id: TagId,
    },

    CreateCollection {
        collection: Collection,
    },

    AddToCollection {
        paper_id: PaperId,
        collection_id: CollectionId,
    },

    RemoveFromCollection {
        paper_id: PaperId,
        collection_id: CollectionId,
    },

    AddAttachment {
        attachment: Attachment,
    },

    RemoveAttachment {
        attachment_id: AttachmentId,
    },

    DeletePaper {
        paper_id: PaperId,
    },

    RestorePaper {
        paper_id: PaperId,
    },
}

#[derive(Clone, Debug, Default)]
pub struct CommitResult {
    pub affected_papers: Vec<PaperId>,
    pub created_revisions: Vec<RevisionId>,
    pub created_proposals: Vec<ProposalId>,
    pub created_events: Vec<EventId>,
}

#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("Field has changed since it was read")]
    Conflict {
        expected: Vec<RevisionId>,
        actual: Vec<RevisionId>,
    },
    #[error("Duplicate revision IDs in the expected heads")]
    DuplicateHeads,
    #[error(transparent)]
    Paper(#[from] crate::model::PaperError),
}

/// Validate optimistic concurrency before preparing a manual revision.
/// The caller must atomically persist the returned paper, revision and new head.
pub fn prepare_bibliography_edit(
    paper: &Paper,
    value: BibliographyValue,
    expected_heads: &[RevisionId],
    actual_heads: &[RevisionId],
    now: chrono::DateTime<chrono::Utc>,
) -> Result<(Paper, crate::model::FieldRevision), EditError> {
    let mut expected = expected_heads.to_vec();
    expected.sort();
    if expected.windows(2).any(|p| p[0] == p[1]) {
        return Err(EditError::DuplicateHeads);
    }
    let mut actual = actual_heads.to_vec();
    actual.sort();
    actual.dedup();
    if expected != actual {
        return Err(EditError::Conflict { expected, actual });
    }
    let mut next = paper.clone();
    next.edit_bibliography(&value, now)?;
    let revision = crate::model::FieldRevision {
        id: RevisionId::new(),
        paper_id: paper.id(),
        version_id: None,
        parents: actual,
        value,
        provenance: crate::model::Provenance {
            source: crate::model::MetadataSource::Manual,
            source_record_id: None,
            observed_at: now,
            source_updated_at: None,
        },
        created_at: now,
    };
    Ok((next, revision))
}

/// Consistent read of the document and its heads, used for an atomic form save.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaperEditState {
    pub paper: Paper,
    pub heads: Vec<(crate::model::BibliographyField, Vec<RevisionId>)>,
}

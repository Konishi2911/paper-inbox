use crate::model::{
    Paper,
    PaperId,
    VersionId,
    RevisionId,
    ProposalId,
    AttachmentId,
    TagId,
    CollectionId,
    EventId,
    CitationKey,
    PaperVersion,
    BibliographyValue,
    MetadataCandidate,
    ProposalResolution,
    ReadingStatus,
    Attachment,
    Tag,
    Collection,
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
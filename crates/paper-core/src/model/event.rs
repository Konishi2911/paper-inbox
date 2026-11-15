use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{
    Attachment, AttachmentId, CitationKey, Collection, CollectionId, DeviceId, EventId,
    FieldRevision, FieldTarget, Paper, PaperId, PaperVersion, ProposalId, ProposalResolution,
    ReadingStatus, RevisionId, Tag, TagId, UpdateProposal, VersionId,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncEvent {
    pub id: EventId,
    pub device_id: DeviceId,

    pub sequence: u64,
    pub parents: Vec<EventId>,

    pub timestamp: DateTime<Utc>,
    pub schema_version: u32,

    pub operation: SyncOperation,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SyncOperation {
    ProposalSuperseded {
        proposal_id: ProposalId,
    },
    MetadataCandidateSubmitted {
        paper_id: PaperId,
        candidate: super::MetadataCandidate,
    },
    MetadataCandidateAssociated {
        candidate_id: super::CandidateId,
        version_id: VersionId,
    },
    PaperCreated(Paper),

    PaperVersionAdded(PaperVersion),

    CanonicalVersionChanged {
        paper_id: PaperId,
        version_id: Option<VersionId>,
    },

    FieldRevisionCreated(FieldRevision),

    FieldHeadsChanged {
        target: FieldTarget,
        heads: Vec<RevisionId>,
    },

    CitationKeyChanged {
        paper_id: PaperId,
        citation_key: CitationKey,
    },

    NotesChanged {
        paper_id: PaperId,
        notes: String,
    },

    ReadingStatusChanged {
        paper_id: PaperId,
        status: ReadingStatus,
    },

    FavoriteChanged {
        paper_id: PaperId,
        favorite: bool,
    },

    ArchivedChanged {
        paper_id: PaperId,
        archived: bool,
    },

    TagCreated(Tag),

    TagAdded {
        paper_id: PaperId,
        tag_id: TagId,
    },

    TagRemoved {
        paper_id: PaperId,
        tag_id: TagId,
    },

    CollectionCreated(Collection),

    CollectionAdded {
        paper_id: PaperId,
        collection_id: CollectionId,
    },

    CollectionRemoved {
        paper_id: PaperId,
        collection_id: CollectionId,
    },

    AttachmentAdded(Attachment),

    AttachmentRemoved {
        attachment_id: AttachmentId,
    },

    ProposalCreated(UpdateProposal),

    ProposalResolved {
        proposal_id: ProposalId,
        resolution: ProposalResolution,
    },

    PaperDeleted(PaperId),
    PaperRestored(PaperId),
}

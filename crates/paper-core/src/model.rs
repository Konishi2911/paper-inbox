mod attachment;
mod bibliography;
mod contributor;
mod event;
mod identifier;
mod ids;
mod import;
mod paper;
mod proposal;
mod provenance;
mod publication;
mod revision;
mod user_metadata;

pub use ids::{
    AttachmentId, CandidateId, CollectionId, DeviceId, EventId, ImportId, PaperId, ProposalId,
    RevisionId, TagId, VersionId,
};

pub use identifier::{ArxivId, CitationKey, ContentHash, Doi, IdentifierError, PaperIdentifier};

pub use contributor::{Contributor, ContributorName};

pub use publication::{Publication, PublicationType, VersionKind};

pub use bibliography::{
    Bibliography, CandidateStatus, Identifiers, MetadataCandidate, StoredMetadataCandidate,
};

pub use user_metadata::{
    Collection, MAX_TAG_NAME_CHARS, ReadingStatus, Tag, TagNameError, UserMetadata,
    normalize_tag_name,
};

pub use paper::{Paper, PaperError, PaperVersion};

pub use attachment::{Attachment, AttachmentKind};

pub use provenance::{MetadataSource, Provenance};

pub use revision::{BibliographyField, BibliographyValue, FieldRevision, FieldTarget};

pub use proposal::{
    ProposalKind, ProposalReason, ProposalResolution, ProposalStatus, UpdateProposal,
};

pub use import::{ImportJob, ImportSource, ImportStatus};

pub use event::{SyncEvent, SyncOperation};

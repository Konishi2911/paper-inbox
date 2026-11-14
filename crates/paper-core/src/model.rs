mod ids;
mod identifier;
mod contributor;
mod publication;
mod bibliography;
mod user_metadata;
mod paper;
mod attachment;
mod provenance;
mod revision;
mod proposal;
mod import;
mod event;

pub use ids::{
    PaperId,
    VersionId,
    AttachmentId,
    RevisionId,
    ProposalId,
    ImportId,
    EventId,
    DeviceId,
    TagId,
    CollectionId,
};

pub use identifier::{
    Doi,
    ArxivId,
    CitationKey,
    ContentHash,
    PaperIdentifier,
    IdentifierError,
};

pub use contributor::{
    ContributorName,
    Contributor,
};

pub use publication::{
    PublicationType,
    VersionKind,
    Publication,
};

pub use bibliography::{
    Identifiers,
    Bibliography,
    MetadataCandidate,
};

pub use user_metadata::{
    UserMetadata,
    ReadingStatus,
    Tag,
    Collection,
};

pub use paper::{
    Paper,
    PaperVersion,
};

pub use attachment::{
    Attachment,
    AttachmentKind,
};

pub use provenance::{
    Provenance,
    MetadataSource,
};

pub use revision::{
    BibliographyField,
    BibliographyValue,
    FieldRevision,
    FieldTarget,
};

pub use proposal::{
    UpdateProposal,
    ProposalKind,
    ProposalReason,
    ProposalStatus,
    ProposalResolution,
};

pub use import::{
    ImportSource,
    ImportStatus,
    ImportJob,
};

pub use event::{
    SyncEvent,
    SyncOperation,
};
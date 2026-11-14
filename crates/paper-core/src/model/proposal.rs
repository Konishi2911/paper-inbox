use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{BibliographyValue, FieldTarget, PaperId, ProposalId, RevisionId, VersionId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalReason {
    OverwriteManualValue,
    ConflictingSources,
    UnknownFreshness,
    ConcurrentModification,
    PublisherVersionDiscovered,
    DuplicatePaper,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalStatus {
    #[default]
    Pending,
    Accepted,
    Rejected,
    Superseded,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ProposalKind {
    FieldUpdate {
        target: FieldTarget,
        expected_heads: Vec<RevisionId>,
        candidate_revision: RevisionId,
    },

    CanonicalVersionChange {
        paper_id: PaperId,
        current_version: Option<VersionId>,
        candidate_version: VersionId,
    },

    DuplicateMerge {
        primary: PaperId,
        duplicate: PaperId,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ProposalResolution {
    Accept,
    Reject,

    CustomValue(BibliographyValue),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateProposal {
    pub id: ProposalId,

    pub kind: ProposalKind,
    pub reason: ProposalReason,
    pub status: ProposalStatus,

    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,

    pub resolution: Option<ProposalResolution>,
}

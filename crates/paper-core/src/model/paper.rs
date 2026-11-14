use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{
    PaperId,
    VersionId,
    CitationKey,
    Bibliography,
    UserMetadata,
    VersionKind,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
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
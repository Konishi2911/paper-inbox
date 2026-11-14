use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{ContentHash, ImportId, PaperId, PaperIdentifier};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportSource {
    Identifier(PaperIdentifier),

    File {
        content_hash: ContentHash,
        filename: String,
        media_type: String,
        size_bytes: u64,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportStatus {
    #[default]
    Pending,

    ResolvingMetadata,
    AwaitingApproval,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug)]
pub struct ImportJob {
    pub id: ImportId,

    pub source: ImportSource,
    pub status: ImportStatus,

    pub paper_id: Option<PaperId>,

    pub retry_count: u32,
    pub last_error: Option<String>,

    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

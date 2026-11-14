use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{
    AttachmentId,
    PaperId,
    VersionId,
    ContentHash,
};

#[derive(
    Clone, Copy, Debug, PartialEq, Eq,
    Serialize, Deserialize,
)]
pub enum AttachmentKind {
    FullText,
    Supplementary,
    Figure,
    Other,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Attachment {
    pub id: AttachmentId,

    pub paper_id: PaperId,
    pub version_id: Option<VersionId>,

    pub content_hash: ContentHash,

    pub filename: String,
    pub media_type: String,
    pub size_bytes: u64,

    pub kind: AttachmentKind,

    pub created_at: DateTime<Utc>,
}
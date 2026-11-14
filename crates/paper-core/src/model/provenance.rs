use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Debug, PartialEq, Eq, Hash,
    Serialize, Deserialize,
)]
pub enum MetadataSource {
    Manual,
    Publisher,
    Crossref,
    OpenAlex,
    Arxiv,
    PdfEmbedded,
    Other(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub source: MetadataSource,

    pub source_record_id: Option<String>,

    pub observed_at: DateTime<Utc>,
    pub source_updated_at: Option<DateTime<Utc>>,
}
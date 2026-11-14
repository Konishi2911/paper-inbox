use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash,
    Serialize, Deserialize,
)]
pub enum PublicationType {
    JournalArticle,
    ConferencePaper,
    Book,
    BookChapter,
    MastersThesis,
    DoctoralThesis,
    Thesis,
    TechnicalReport,
    Preprint,
    Patent,
    Other,

    #[default]
    Unknown,
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash,
    Serialize, Deserialize,
)]
pub enum VersionKind {
    VersionOfRecord,
    AcceptedManuscript,
    SubmittedManuscript,
    Preprint,

    #[default]
    Unknown,
}

#[derive(
    Clone, Debug, Default, PartialEq, Eq,
    Serialize, Deserialize,
)]
pub struct Publication {
    pub kind: PublicationType,

    pub container_title: Option<String>,
    pub publisher: Option<String>,

    pub year: Option<u16>,
    pub month: Option<u8>,

    pub volume: Option<String>,
    pub issue: Option<String>,
    pub pages: Option<String>,
    pub article_number: Option<String>,
}
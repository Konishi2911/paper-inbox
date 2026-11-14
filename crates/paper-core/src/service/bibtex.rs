use thiserror::Error;

use crate::model::{
    Paper,
    PaperId,
    CitationKey,
};

#[derive(Clone, Debug)]
pub struct BibtexOptions {
    pub include_abstract: bool,
    pub include_doi: bool,
    pub include_url: bool,
    pub include_keywords: bool,
}

impl Default for BibtexOptions {
    fn default() -> Self {
        Self {
            include_abstract: false,
            include_doi: true,
            include_url: true,
            include_keywords: false,
        }
    }
}

#[derive(Debug, Error)]
pub enum BibtexError {
    #[error("Duplicate citation key: {0:?}")]
    DuplicateCitationKey(CitationKey),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BibtexWarning {
    MissingRequiredField {
        paper_id: PaperId,
        field: &'static str,
    },

    FallbackEntryType {
        paper_id: PaperId,
    },
}

#[derive(Clone, Debug)]
pub struct BibtexOutput {
    pub content: String,
    pub warnings: Vec<BibtexWarning>,
}

pub fn render_entry(
    paper: &Paper,
    options: &BibtexOptions,
) -> BibtexOutput {
    todo!()
}

pub fn export_bibtex(
    papers: &[Paper],
    options: &BibtexOptions,
) -> Result<BibtexOutput, BibtexError> {
    todo!()
}
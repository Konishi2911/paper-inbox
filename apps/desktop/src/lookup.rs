use crate::application::{AppError, Application, PaperForm};
use paper_core::{command::LibraryCommand, model::*, service::suggest_citation_key};

/// Personal values entered on the unified add screen. They are not
/// bibliographic data, so registering them keeps the fetched provenance intact.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LookupUserInput {
    pub notes: String,
    pub reading_status: ReadingStatus,
    pub favorite: bool,
}

/// Structured, read-only presentation of a fetched candidate.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LookupPreview {
    pub source: String,
    pub fields: Vec<(String, String)>,
}

impl Application {
    pub fn suggest_key(
        &self,
        form: &PaperForm,
        exclude: Option<PaperId>,
    ) -> Result<String, AppError> {
        self.suggest_key_for(&form.bibliography()?, exclude)
    }
    pub fn suggest_key_for(
        &self,
        b: &Bibliography,
        exclude: Option<PaperId>,
    ) -> Result<String, AppError> {
        Ok(suggest_citation_key(b, &self.store.citation_keys(exclude)?).to_string())
    }
    /// Registers the fetched candidate and the personal values in one transaction.
    pub fn register_lookup(
        &self,
        id: PaperId,
        candidate: &MetadataCandidate,
        key: &str,
        input: &LookupUserInput,
    ) -> Result<PaperId, AppError> {
        let key = CitationKey::parse(key.trim()).map_err(|_| {
            AppError::Input("引用キーは英数字と _ - : . で入力してください。".into())
        })?;
        let mut commands = vec![LibraryCommand::RegisterMetadataCandidate {
            paper_id: id,
            citation_key: key,
            candidate: candidate.clone(),
        }];
        if !input.notes.trim().is_empty() {
            commands.push(LibraryCommand::UpdateNotes {
                paper_id: id,
                notes: input.notes.clone(),
            });
        }
        if input.reading_status != ReadingStatus::Unread {
            commands.push(LibraryCommand::SetReadingStatus {
                paper_id: id,
                status: input.reading_status,
            });
        }
        if input.favorite {
            commands.push(LibraryCommand::SetFavorite {
                paper_id: id,
                favorite: true,
            });
        }
        self.store
            .commit_batch_at(commands, &[], chrono::Utc::now())?;
        Ok(id)
    }
}

pub fn preview(c: &MetadataCandidate) -> LookupPreview {
    let mut source = format!(
        "{}・{}",
        crate::metadata::source_label(&c.provenance.source),
        crate::metadata::version_label(c.version_kind),
    );
    if let Some(record) = c.provenance.source_record_id.as_deref() {
        source.push_str(&format!("・記録 {record}"));
    }
    if let Some(updated) = c.provenance.source_updated_at {
        source.push_str(&format!("・情報源の更新 {}", updated.format("%Y-%m-%d")));
    }
    let fields = c
        .present_fields
        .iter()
        .map(|field| {
            (
                crate::metadata::field_label(*field).to_string(),
                crate::metadata::value_label(&c.bibliography.value(*field)),
            )
        })
        .collect();
    LookupPreview { source, fields }
}

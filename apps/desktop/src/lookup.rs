use crate::application::{AppError, Application, PaperForm, tag_name_message};
use paper_core::{
    command::LibraryCommand, model::*, ports::PaperRepository, query::IdentifierMatch,
    service::suggest_citation_key,
};

/// Personal values entered on the unified add screen. They are not
/// bibliographic data, so registering them keeps the fetched provenance intact.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LookupUserInput {
    pub notes: String,
    pub reading_status: ReadingStatus,
    pub favorite: bool,
    /// Tag names to give the new paper. Existing tags are reused by their folded name.
    pub tags: Vec<String>,
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
    /// Saved papers (deleted ones last) that have the same DOI or arXiv ID as this bibliography.
    pub fn duplicates_of(&self, b: &Bibliography) -> Result<Vec<IdentifierMatch>, AppError> {
        Ok(self
            .store
            .find_by_identifiers(b.identifiers.doi.as_ref(), b.identifiers.arxiv_id.as_ref())?)
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
        // Tags are resolved before anything is written, so one unusable name rejects the
        // whole registration instead of leaving a paper without its tags.
        let existing = self.store.tags(true)?;
        let mut seen = std::collections::HashSet::new();
        for name in &input.tags {
            let (display, tag_key) = normalize_tag_name(name)
                .map_err(|e| AppError::Input(tag_name_message(e).to_owned()))?;
            if !seen.insert(tag_key.clone()) {
                continue;
            }
            let found = existing
                .iter()
                .map(|c| &c.tag)
                .find(|t| t.key().ok().as_deref() == Some(tag_key.as_str()));
            let tag_id = match found {
                Some(tag) => tag.id,
                None => {
                    let tag = Tag {
                        id: TagId::new(),
                        name: display,
                    };
                    let tag_id = tag.id;
                    commands.push(LibraryCommand::CreateTag { tag });
                    tag_id
                }
            };
            commands.push(LibraryCommand::AddTag {
                paper_id: id,
                tag_id,
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

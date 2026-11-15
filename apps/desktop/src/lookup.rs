use crate::application::{AppError, Application, PaperForm};
use paper_core::{command::LibraryCommand, model::*, service::suggest_citation_key};

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
    pub fn register_lookup(
        &self,
        id: PaperId,
        candidate: &MetadataCandidate,
        key: &str,
    ) -> Result<PaperId, AppError> {
        let key = CitationKey::parse(key.trim()).map_err(|_| {
            AppError::Input("引用キーは英数字と _ - : . で入力してください。".into())
        })?;
        self.store.commit_at(
            LibraryCommand::RegisterMetadataCandidate {
                paper_id: id,
                citation_key: key,
                candidate: candidate.clone(),
            },
            chrono::Utc::now(),
        )?;
        Ok(id)
    }
}

pub fn preview(c: &MetadataCandidate) -> String {
    let mut result = format!(
        "情報源: {}\n記録: {}\n版: {}\n情報源の更新日時: {}\n取得日時: {}\n\n",
        crate::metadata::source_label(&c.provenance.source),
        c.provenance.source_record_id.as_deref().unwrap_or("不明"),
        crate::metadata::version_label(c.version_kind),
        c.provenance
            .source_updated_at
            .map(|t| t.to_rfc3339())
            .unwrap_or_else(|| "不明".into()),
        c.provenance.observed_at.to_rfc3339()
    );
    for field in &c.present_fields {
        result.push_str(&format!(
            "{}: {}\n",
            crate::metadata::field_label(*field),
            crate::metadata::value_label(&c.bibliography.value(*field))
        ));
    }
    result
}

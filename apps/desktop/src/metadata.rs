use crate::application::{AppError, Application};
use chrono::{DateTime, Utc};
use paper_core::{
    command::{LibraryCommand, PaperEditState},
    model::*,
    query::MetadataReview,
};

#[derive(Clone, Debug, Default)]
pub struct CandidateForm {
    pub id: String,
    pub observed_at: String,
    pub source: usize,
    pub source_name: String,
    pub record_id: String,
    pub updated_at: String,
    pub version_kind: usize,
    pub doi: String,
    pub arxiv: String,
    pub title: String,
    pub use_title: bool,
    pub year: String,
    pub use_year: bool,
    pub container: String,
    pub use_container: bool,
    pub publisher: String,
    pub use_publisher: bool,
    pub abstract_text: String,
    pub use_abstract: bool,
}
fn optional(s: &str) -> Option<String> {
    let s = s.trim();
    (!s.is_empty()).then(|| s.into())
}
pub fn candidate_from_form(form: &CandidateForm) -> Result<MetadataCandidate, AppError> {
    let id = uuid::Uuid::parse_str(&form.id)
        .map(CandidateId::from_uuid)
        .map_err(|_| AppError::Input("候補の入力を開き直してください。".into()))?;
    let observed_at = DateTime::parse_from_rfc3339(&form.observed_at)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|_| AppError::Input("候補の入力を開き直してください。".into()))?;
    let source = match form.source {
        0 => MetadataSource::Crossref,
        1 => MetadataSource::Publisher,
        2 => MetadataSource::OpenAlex,
        3 => MetadataSource::Arxiv,
        4 => MetadataSource::PdfEmbedded,
        5 => MetadataSource::Other(form.source_name.trim().into()),
        _ => return Err(AppError::Input("情報源を選択してください。".into())),
    };
    let version_kind = match form.version_kind {
        0 => VersionKind::Unknown,
        1 => VersionKind::VersionOfRecord,
        2 => VersionKind::AcceptedManuscript,
        3 => VersionKind::SubmittedManuscript,
        4 => VersionKind::Preprint,
        _ => return Err(AppError::Input("版の種類を選択してください。".into())),
    };
    let source_updated_at = optional(&form.updated_at)
        .map(|s| {
            DateTime::parse_from_rfc3339(&s)
                .map(|t| t.with_timezone(&Utc))
                .map_err(|_| {
                    AppError::Input(
                        "情報源の更新日時は、例のように時差付きで入力してください。".into(),
                    )
                })
        })
        .transpose()?;
    let mut bibliography = Bibliography::default();
    let mut present_fields = vec![];
    bibliography.identifiers.doi = optional(&form.doi)
        .map(|s| {
            Doi::parse(&s)
                .map_err(|_| AppError::Input("候補の DOI の形式を確認してください。".into()))
        })
        .transpose()?;
    bibliography.identifiers.arxiv_id = optional(&form.arxiv)
        .map(|s| {
            ArxivId::parse(&s)
                .map_err(|_| AppError::Input("候補の arXiv ID の形式を確認してください。".into()))
        })
        .transpose()?;
    if form.use_title {
        present_fields.push(BibliographyField::Title);
        bibliography.title = optional(&form.title);
    }
    if form.use_year {
        present_fields.push(BibliographyField::Year);
        bibliography.publication.year = optional(&form.year)
            .map(|s| {
                s.parse::<u16>().ok().filter(|n| *n > 0).ok_or_else(|| {
                    AppError::Input(
                        "出版年は正の整数、または明示的な削除のための空欄にしてください。".into(),
                    )
                })
            })
            .transpose()?;
    }
    if form.use_container {
        present_fields.push(BibliographyField::ContainerTitle);
        bibliography.publication.container_title = optional(&form.container);
    }
    if form.use_publisher {
        present_fields.push(BibliographyField::Publisher);
        bibliography.publication.publisher = optional(&form.publisher);
    }
    if form.use_abstract {
        present_fields.push(BibliographyField::Abstract);
        bibliography.abstract_text = optional(&form.abstract_text);
    }
    let candidate = MetadataCandidate {
        id,
        present_fields,
        bibliography,
        provenance: Provenance {
            source,
            source_record_id: optional(&form.record_id),
            observed_at,
            source_updated_at,
        },
        version_kind,
    };
    candidate
        .validate()
        .map_err(|e| AppError::Input(e.to_string()))?;
    Ok(candidate)
}
impl Application {
    pub fn metadata_review(&self, id: PaperId) -> Result<MetadataReview, AppError> {
        Ok(self.store.metadata_review(id)?)
    }
    pub fn submit_candidate(
        &self,
        state: &PaperEditState,
        form: &CandidateForm,
    ) -> Result<(), AppError> {
        let candidate = candidate_from_form(form)?;
        self.store.commit_batch_at(
            vec![LibraryCommand::SubmitMetadataCandidate {
                paper_id: state.paper.id(),
                candidate,
            }],
            std::slice::from_ref(state),
            Utc::now(),
        )?;
        Ok(())
    }
    pub fn associate_candidate(
        &self,
        state: &PaperEditState,
        id: CandidateId,
    ) -> Result<(), AppError> {
        self.store.commit_batch_at(
            vec![LibraryCommand::AssociateMetadataCandidate {
                candidate_id: id,
                expected: state.clone(),
            }],
            std::slice::from_ref(state),
            Utc::now(),
        )?;
        Ok(())
    }
    pub fn resolve_metadata(
        &self,
        state: &PaperEditState,
        id: ProposalId,
        accept: bool,
    ) -> Result<(), AppError> {
        self.store.commit_batch_at(
            vec![LibraryCommand::ResolveProposal {
                proposal_id: id,
                resolution: if accept {
                    ProposalResolution::Accept
                } else {
                    ProposalResolution::Reject
                },
            }],
            std::slice::from_ref(state),
            Utc::now(),
        )?;
        Ok(())
    }
    pub fn propose_canonical(&self, state: &PaperEditState, id: VersionId) -> Result<(), AppError> {
        self.store.commit_batch_at(
            vec![LibraryCommand::ProposeCanonicalVersion {
                paper_id: state.paper.id(),
                version_id: id,
            }],
            std::slice::from_ref(state),
            Utc::now(),
        )?;
        Ok(())
    }
}
pub fn version_label(kind: VersionKind) -> &'static str {
    match kind {
        VersionKind::VersionOfRecord => "出版社版",
        VersionKind::AcceptedManuscript => "受理原稿",
        VersionKind::SubmittedManuscript => "投稿原稿",
        VersionKind::Preprint => "プレプリント",
        VersionKind::Unknown => "版種別未指定",
    }
}
pub fn source_label(source: &MetadataSource) -> String {
    match source {
        MetadataSource::Manual => "手動".into(),
        MetadataSource::Publisher => "出版社".into(),
        MetadataSource::Crossref => "Crossref".into(),
        MetadataSource::OpenAlex => "OpenAlex".into(),
        MetadataSource::Arxiv => "arXiv".into(),
        MetadataSource::PdfEmbedded => "PDF 埋込情報".into(),
        MetadataSource::Other(s) => s.clone(),
    }
}
pub fn field_label(field: BibliographyField) -> &'static str {
    match field {
        BibliographyField::Title => "タイトル",
        BibliographyField::Authors => "著者",
        BibliographyField::Editors => "編集者",
        BibliographyField::PublicationType => "出版種別",
        BibliographyField::ContainerTitle => "掲載先",
        BibliographyField::Publisher => "出版社",
        BibliographyField::Year => "出版年",
        BibliographyField::Month => "出版月",
        BibliographyField::Volume => "巻",
        BibliographyField::Issue => "号",
        BibliographyField::Pages => "ページ",
        BibliographyField::ArticleNumber => "記事番号",
        BibliographyField::Identifiers => "識別子",
        BibliographyField::Abstract => "概要",
        BibliographyField::Keywords => "キーワード",
    }
}
pub fn value_label(value: &BibliographyValue) -> String {
    let empty = "（空欄）";
    match value {
        BibliographyValue::Title(v)
        | BibliographyValue::ContainerTitle(v)
        | BibliographyValue::Publisher(v)
        | BibliographyValue::Volume(v)
        | BibliographyValue::Issue(v)
        | BibliographyValue::Pages(v)
        | BibliographyValue::ArticleNumber(v)
        | BibliographyValue::Abstract(v) => v.clone().unwrap_or_else(|| empty.into()),
        BibliographyValue::Year(v) => v.map(|v| v.to_string()).unwrap_or_else(|| empty.into()),
        BibliographyValue::Month(v) => v.map(|v| v.to_string()).unwrap_or_else(|| empty.into()),
        BibliographyValue::Authors(v) | BibliographyValue::Editors(v) => {
            if v.is_empty() {
                empty.into()
            } else {
                v.iter()
                    .map(|c| match &c.name {
                        ContributorName::Person { given, family } => {
                            format!("{family} {}", given.as_deref().unwrap_or(""))
                        }
                        ContributorName::Organization(s) | ContributorName::Literal(s) => s.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join(" / ")
            }
        }
        BibliographyValue::Identifiers(v) => format!(
            "DOI: {}\narXiv: {}\nISBN: {}\nISSN: {}\nURL: {}",
            v.doi_text().unwrap_or(empty),
            v.arxiv_text().unwrap_or(empty),
            v.isbn.as_deref().unwrap_or(empty),
            v.issn.as_deref().unwrap_or(empty),
            v.url.as_deref().unwrap_or(empty)
        ),
        BibliographyValue::Keywords(v) => {
            if v.is_empty() {
                empty.into()
            } else {
                v.join(", ")
            }
        }
        BibliographyValue::PublicationType(v) => {
            crate::application::publication_type_label(*v).to_owned()
        }
    }
}
pub fn reason_label(reason: ProposalReason) -> &'static str {
    match reason {
        ProposalReason::OverwriteManualValue => "手動値への上書き",
        ProposalReason::ConflictingSources => "情報源の値が競合",
        ProposalReason::UnknownFreshness => "新旧関係が不明",
        ProposalReason::ConcurrentModification => "同時編集の競合",
        ProposalReason::PublisherVersionDiscovered => "出版社版を発見",
        ProposalReason::DuplicatePaper => "重複候補",
        ProposalReason::ExternalDeletion => "外部情報からの明示的な削除",
        ProposalReason::ManualAssociation => "手動で関連付けた候補",
        ProposalReason::CanonicalVersionSelection => "正規版の手動選択",
    }
}

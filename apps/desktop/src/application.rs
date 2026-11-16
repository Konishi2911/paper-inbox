use chrono::Utc;
use paper_core::{
    command::{LibraryCommand, PaperEditState},
    model::*,
    ports::PaperRepository,
    query::*,
};
use paper_storage::{SqliteLibrary, StorageError};

/// Labels shown for `PUBLICATION_TYPES`, in the same order as the Slint model.
pub const PUBLICATION_TYPE_LABELS: &[&str] = &[
    "未指定",
    "学術誌論文",
    "会議論文",
    "書籍",
    "書籍の章",
    "修士論文",
    "博士論文",
    "学位論文",
    "技術報告",
    "プレプリント",
    "特許",
    "その他",
];
pub const PUBLICATION_TYPES: &[PublicationType] = &[
    PublicationType::Unknown,
    PublicationType::JournalArticle,
    PublicationType::ConferencePaper,
    PublicationType::Book,
    PublicationType::BookChapter,
    PublicationType::MastersThesis,
    PublicationType::DoctoralThesis,
    PublicationType::Thesis,
    PublicationType::TechnicalReport,
    PublicationType::Preprint,
    PublicationType::Patent,
    PublicationType::Other,
];

/// Japanese label for a publication type, matching the editor's choices.
pub fn publication_type_label(kind: PublicationType) -> &'static str {
    PUBLICATION_TYPES
        .iter()
        .position(|t| *t == kind)
        .and_then(|i| PUBLICATION_TYPE_LABELS.get(i).copied())
        .unwrap_or("その他")
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AuthorInput {
    pub kind: i32,
    pub given: String,
    pub name: String,
    pub orcid: String,
}
impl AuthorInput {
    fn from_contributor(c: &Contributor) -> Self {
        let (kind, given, name) = match &c.name {
            ContributorName::Person { given, family } => {
                (0, given.clone().unwrap_or_default(), family.clone())
            }
            ContributorName::Organization(name) => (1, String::new(), name.clone()),
            ContributorName::Literal(name) => (2, String::new(), name.clone()),
        };
        Self {
            kind,
            given,
            name,
            orcid: c.orcid.clone().unwrap_or_default(),
        }
    }
    fn contributor(&self) -> Result<Contributor, AppError> {
        if self.name.trim().is_empty() {
            return Err(AppError::Input(
                "著者の姓または名称を入力してください。不要な行は削除できます。".into(),
            ));
        }
        let name = match self.kind {
            0 => ContributorName::Person {
                given: optional(&self.given),
                family: self.name.trim().into(),
            },
            1 => ContributorName::Organization(self.name.trim().into()),
            2 => ContributorName::Literal(self.name.trim().into()),
            _ => return Err(AppError::Input("著者の種類を選択してください。".into())),
        };
        Ok(Contributor {
            name,
            orcid: optional(&self.orcid),
        })
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PaperForm {
    pub citation_key: String,
    pub title: String,
    pub authors: Vec<AuthorInput>,
    pub publication_type: usize,
    pub container_title: String,
    pub year: String,
    pub publisher: String,
    pub volume: String,
    pub issue: String,
    pub pages: String,
    pub doi: String,
    pub arxiv_id: String,
    pub url: String,
    pub abstract_text: String,
    /// Keywords separated by `;` so that a keyword may contain commas.
    pub keywords: String,
    pub notes: String,
    pub reading_status: usize,
    pub favorite: bool,
    pub archived: bool,
}
fn optional(s: &str) -> Option<String> {
    let s = s.trim();
    (!s.is_empty()).then(|| s.into())
}
pub const KEYWORD_SEPARATOR: &str = "; ";
fn keyword_list(s: &str) -> Vec<String> {
    s.split(';')
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(Into::into)
        .collect()
}
impl PaperForm {
    pub fn from_paper(p: &Paper) -> Self {
        let b = p.bibliography();
        let m = p.metadata();
        Self {
            citation_key: p.citation_key().as_str().into(),
            title: b.title.clone().unwrap_or_default(),
            authors: b
                .authors
                .iter()
                .map(AuthorInput::from_contributor)
                .collect(),
            publication_type: PUBLICATION_TYPES
                .iter()
                .position(|t| *t == b.publication.kind)
                .unwrap_or(0),
            container_title: b.publication.container_title.clone().unwrap_or_default(),
            year: b
                .publication
                .year
                .map(|n| n.to_string())
                .unwrap_or_default(),
            publisher: b.publication.publisher.clone().unwrap_or_default(),
            volume: b.publication.volume.clone().unwrap_or_default(),
            issue: b.publication.issue.clone().unwrap_or_default(),
            pages: b.publication.pages.clone().unwrap_or_default(),
            doi: b.identifiers.doi_text().unwrap_or_default().into(),
            arxiv_id: b.identifiers.arxiv_text().unwrap_or_default().into(),
            url: b.identifiers.url.clone().unwrap_or_default(),
            abstract_text: b.abstract_text.clone().unwrap_or_default(),
            keywords: b.keywords.join(KEYWORD_SEPARATOR),
            notes: m.notes.clone(),
            reading_status: match m.reading_status {
                ReadingStatus::Unread => 0,
                ReadingStatus::Reading => 1,
                ReadingStatus::Read => 2,
            },
            favorite: m.favorite,
            archived: m.archived,
        }
    }
    pub(crate) fn bibliography(&self) -> Result<Bibliography, AppError> {
        let year = if self.year.trim().is_empty() {
            None
        } else {
            Some(
                self.year
                    .trim()
                    .parse::<u16>()
                    .ok()
                    .filter(|v| *v > 0)
                    .ok_or_else(|| {
                        AppError::Input(
                            "出版年は1〜65535の整数、または空欄で指定してください。".into(),
                        )
                    })?,
            )
        };
        let kind = *PUBLICATION_TYPES
            .get(self.publication_type)
            .ok_or_else(|| AppError::Input("出版種別を選択してください。".into()))?;
        Ok(Bibliography {
            title: optional(&self.title),
            authors: self
                .authors
                .iter()
                .map(AuthorInput::contributor)
                .collect::<Result<_, _>>()?,
            publication: Publication {
                kind,
                container_title: optional(&self.container_title),
                publisher: optional(&self.publisher),
                year,
                volume: optional(&self.volume),
                issue: optional(&self.issue),
                pages: optional(&self.pages),
                ..Default::default()
            },
            abstract_text: optional(&self.abstract_text),
            keywords: keyword_list(&self.keywords),
            ..Default::default()
        })
    }
    fn metadata(&self, old: Option<&UserMetadata>) -> Result<UserMetadata, AppError> {
        let mut m = old.cloned().unwrap_or_default();
        m.notes = self.notes.clone();
        m.favorite = self.favorite;
        m.archived = self.archived;
        m.reading_status = match self.reading_status {
            0 => ReadingStatus::Unread,
            1 => ReadingStatus::Reading,
            2 => ReadingStatus::Read,
            _ => return Err(AppError::Input("既読状態を選択してください。".into())),
        };
        Ok(m)
    }
}
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AppError {
    #[error("{0}")]
    Input(String),
    #[error(
        "この文献は別の操作で変更されています。入力は保持しています。内容を控え、再読込してから変更を確認してください。"
    )]
    Conflict,
    #[error(
        "この引用キーは使用済みです。削除済み文献もキーを保持します。別のキーを指定してください。"
    )]
    DuplicateKey,
    #[error("文献が見つかりません。一覧を更新してください。")]
    Missing,
    #[error("保存先の処理に失敗しました: {0}")]
    Storage(String),
}
impl From<StorageError> for AppError {
    fn from(e: StorageError) -> Self {
        match e {
            StorageError::Conflict(_)
            | StorageError::Edit(paper_core::command::EditError::Conflict { .. }) => Self::Conflict,
            StorageError::DuplicateCitationKey(_) => Self::DuplicateKey,
            StorageError::NotFound(_)
            | StorageError::CandidateNotFound(_)
            | StorageError::ProposalNotFound(_) => Self::Missing,
            other => Self::Storage(other.to_string()),
        }
    }
}
pub struct Application {
    pub store: SqliteLibrary,
}
impl Application {
    pub fn list(&self, query: &PaperQuery) -> Result<SearchResult, AppError> {
        Ok(self.store.search(query)?)
    }
    pub fn open(&self, id: PaperId) -> Result<PaperEditState, AppError> {
        Ok(self.store.load_edit_state(id)?)
    }
    pub fn save(
        &self,
        state: Option<&PaperEditState>,
        form: &PaperForm,
    ) -> Result<PaperId, AppError> {
        let key = CitationKey::parse(form.citation_key.trim()).map_err(|_| {
            AppError::Input(
                "引用キーは英数字と _ - : . で入力してください。空欄にはできません。".into(),
            )
        })?;
        let mut b = form.bibliography()?;
        let original = state.map(|s| PaperForm::from_paper(&s.paper));
        let identifiers_changed = original.as_ref().is_none_or(|old| {
            old.doi != form.doi
                || old.arxiv_id != form.arxiv_id
                || old.url.trim() != form.url.trim()
        });
        if identifiers_changed {
            b.identifiers.doi = if form.doi.trim().is_empty() {
                None
            } else {
                Some(
                    Doi::parse(&form.doi)
                        .map_err(|_| AppError::Input("DOI の形式を確認してください。".into()))?,
                )
            };
            b.identifiers.arxiv_id =
                if form.arxiv_id.trim().is_empty() {
                    None
                } else {
                    Some(ArxivId::parse(&form.arxiv_id).map_err(|_| {
                        AppError::Input("arXiv ID の形式を確認してください。".into())
                    })?)
                };
            b.identifiers.url = optional(&form.url);
            if let Some(state) = state {
                let old = &state.paper.bibliography().identifiers;
                b.identifiers.isbn = old.isbn.clone();
                b.identifiers.issn = old.issn.clone();
            }
        }
        let now = Utc::now();
        match state {
            None => {
                let mut paper = Paper::new(PaperId::new(), key, b, now)
                    .map_err(|e| AppError::Input(e.to_string()))?;
                paper
                    .update_metadata(form.metadata(None)?, now)
                    .map_err(|e| AppError::Input(e.to_string()))?;
                let id = paper.id();
                self.store
                    .commit_at(LibraryCommand::CreatePaper { paper }, now)?;
                Ok(id)
            }
            Some(state) => {
                let p = &state.paper;
                let original = PaperForm::from_paper(p);
                let id = p.id();
                let mut commands = vec![];
                if key != *p.citation_key() {
                    commands.push(LibraryCommand::ChangeCitationKey {
                        paper_id: id,
                        citation_key: key,
                    });
                }
                let edits = [
                    (
                        identifiers_changed,
                        BibliographyValue::Identifiers(b.identifiers),
                    ),
                    (
                        form.title != original.title,
                        BibliographyValue::Title(b.title),
                    ),
                    (
                        form.authors != original.authors,
                        BibliographyValue::Authors(b.authors),
                    ),
                    (
                        form.publication_type != original.publication_type,
                        BibliographyValue::PublicationType(b.publication.kind),
                    ),
                    (
                        form.container_title != original.container_title,
                        BibliographyValue::ContainerTitle(b.publication.container_title),
                    ),
                    (
                        form.year != original.year,
                        BibliographyValue::Year(b.publication.year),
                    ),
                    (
                        form.publisher.trim() != original.publisher.trim(),
                        BibliographyValue::Publisher(b.publication.publisher),
                    ),
                    (
                        form.volume.trim() != original.volume.trim(),
                        BibliographyValue::Volume(b.publication.volume),
                    ),
                    (
                        form.issue.trim() != original.issue.trim(),
                        BibliographyValue::Issue(b.publication.issue),
                    ),
                    (
                        form.pages.trim() != original.pages.trim(),
                        BibliographyValue::Pages(b.publication.pages),
                    ),
                    (
                        form.abstract_text.trim() != original.abstract_text.trim(),
                        BibliographyValue::Abstract(b.abstract_text),
                    ),
                    (
                        keyword_list(&form.keywords) != keyword_list(&original.keywords),
                        BibliographyValue::Keywords(b.keywords),
                    ),
                ];
                for (changed, value) in edits {
                    if changed {
                        let expected_heads = state
                            .heads
                            .iter()
                            .find(|(f, _)| *f == value.field())
                            .map(|(_, h)| h.clone())
                            .unwrap_or_default();
                        commands.push(LibraryCommand::EditBibliography {
                            paper_id: id,
                            version_id: None,
                            value,
                            expected_heads,
                        });
                    }
                }
                let m = form.metadata(Some(p.metadata()))?;
                if m.notes != p.metadata().notes {
                    commands.push(LibraryCommand::UpdateNotes {
                        paper_id: id,
                        notes: m.notes,
                    });
                }
                if m.reading_status != p.metadata().reading_status {
                    commands.push(LibraryCommand::SetReadingStatus {
                        paper_id: id,
                        status: m.reading_status,
                    });
                }
                if m.favorite != p.metadata().favorite {
                    commands.push(LibraryCommand::SetFavorite {
                        paper_id: id,
                        favorite: m.favorite,
                    });
                }
                if m.archived != p.metadata().archived {
                    commands.push(LibraryCommand::SetArchived {
                        paper_id: id,
                        archived: m.archived,
                    });
                }
                self.store
                    .commit_batch_at(commands, std::slice::from_ref(state), now)?;
                Ok(id)
            }
        }
    }
    pub fn set_deleted(&self, state: &PaperEditState, deleted: bool) -> Result<(), AppError> {
        let paper_id = state.paper.id();
        let command = if deleted {
            LibraryCommand::DeletePaper { paper_id }
        } else {
            LibraryCommand::RestorePaper { paper_id }
        };
        self.store
            .commit_batch_at(vec![command], std::slice::from_ref(state), Utc::now())?;
        Ok(())
    }
}

/// A single personal-metadata change made directly from the read-only detail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuickChange {
    ReadingStatus(ReadingStatus),
    Favorite(bool),
    Archived(bool),
    Notes(String),
}

impl Application {
    /// Applies one personal-metadata change against the displayed state, so a
    /// concurrent edit is rejected instead of being overwritten.
    pub fn quick_update(
        &self,
        state: &PaperEditState,
        change: QuickChange,
    ) -> Result<(), AppError> {
        if state.paper.deleted_at().is_some() {
            return Err(AppError::Input(
                "削除済みの文献は変更できません。復元してから操作してください。".into(),
            ));
        }
        let paper_id = state.paper.id();
        let command = match change {
            QuickChange::ReadingStatus(status) => {
                LibraryCommand::SetReadingStatus { paper_id, status }
            }
            QuickChange::Favorite(favorite) => LibraryCommand::SetFavorite { paper_id, favorite },
            QuickChange::Archived(archived) => LibraryCommand::SetArchived { paper_id, archived },
            QuickChange::Notes(notes) => LibraryCommand::UpdateNotes { paper_id, notes },
        };
        self.store
            .commit_batch_at(vec![command], std::slice::from_ref(state), Utc::now())?;
        Ok(())
    }
}

/// Splits pasted author text into rows for review in the form.
///
/// Entries are separated by `;` or line breaks. `Family, Given` is a person.
/// Without a comma, an ASCII name is read as `Given Family` and any other name
/// (for example Japanese) as `Family Given`. A single word becomes a family name.
pub fn parse_author_list(text: &str) -> Vec<AuthorInput> {
    text.split([';', '\n', '\r'])
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let (name, given) = if let Some((family, given)) = entry.split_once(',') {
                (family.trim().to_string(), given.trim().to_string())
            } else {
                let words: Vec<_> = entry.split_whitespace().collect();
                match words.as_slice() {
                    [] | [_] => (entry.to_string(), String::new()),
                    _ if entry.is_ascii() => (
                        words[words.len() - 1].to_string(),
                        words[..words.len() - 1].join(" "),
                    ),
                    _ => (words[0].to_string(), words[1..].join(" ")),
                }
            };
            AuthorInput {
                kind: 0,
                given,
                name,
                orcid: String::new(),
            }
        })
        .filter(|author| !author.name.is_empty())
        .collect()
}

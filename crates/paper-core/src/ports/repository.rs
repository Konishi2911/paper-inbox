use crate::model::{
    ArxivId, Attachment, Doi, EventId, FieldRevision, FieldTarget, Paper, PaperId, PaperVersion,
    SyncEvent, Tag, UpdateProposal,
};

use crate::query::{IdentifierMatch, PaperQuery, SearchResult, TagCount};

use crate::command::{CommitResult, LibraryCommand};

pub trait PaperRepository: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    fn get(&self, id: PaperId) -> Result<Option<Paper>, Self::Error>;

    fn search(&self, query: &PaperQuery) -> Result<SearchResult, Self::Error>;

    fn versions(&self, paper_id: PaperId) -> Result<Vec<PaperVersion>, Self::Error>;

    fn attachments(&self, paper_id: PaperId) -> Result<Vec<Attachment>, Self::Error>;

    /// Every tag by name, with how many papers carry it. Deleted papers are not counted and
    /// archived ones only when asked for. Tags nobody carries are included with a count of 0.
    fn tags(&self, include_archived: bool) -> Result<Vec<TagCount>, Self::Error>;

    /// The tags assigned to the paper, by name. Assignments are kept for deleted papers.
    fn paper_tags(&self, paper_id: PaperId) -> Result<Vec<Tag>, Self::Error>;

    /// Papers whose bibliography, or any of whose versions, has this DOI or arXiv ID. An arXiv ID
    /// matches with or without a version suffix (`2401.01234` finds `2401.01234v2`). Papers in the
    /// trash are included, marked as deleted; live ones come first.
    fn find_by_identifiers(
        &self,
        doi: Option<&Doi>,
        arxiv: Option<&ArxivId>,
    ) -> Result<Vec<IdentifierMatch>, Self::Error>;

    fn revisions(&self, target: &FieldTarget) -> Result<Vec<FieldRevision>, Self::Error>;

    fn active_revisions(&self, target: &FieldTarget) -> Result<Vec<FieldRevision>, Self::Error>;

    fn pending_proposals(&self, paper_id: PaperId) -> Result<Vec<UpdateProposal>, Self::Error>;
}

#[derive(Clone, Debug)]
pub enum ApplyEventResult {
    Applied,
    AlreadyApplied,

    MissingDependencies { events: Vec<EventId> },
}

pub trait LibraryStore: PaperRepository {
    fn commit(&self, command: LibraryCommand) -> Result<CommitResult, Self::Error>;

    fn apply_remote_event(&self, event: &SyncEvent) -> Result<ApplyEventResult, Self::Error>;

    fn pending_events(&self, limit: usize) -> Result<Vec<SyncEvent>, Self::Error>;

    fn mark_events_uploaded(&self, event_ids: &[EventId]) -> Result<(), Self::Error>;
}

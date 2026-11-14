use crate::model::{
    Paper,
    PaperId,
    PaperVersion,
    Attachment,
    FieldTarget,
    FieldRevision,
    UpdateProposal,
    SyncEvent,
    EventId,
};

use crate::query::{
    PaperQuery,
    SearchResult,
};

use crate::command::{
    LibraryCommand,
    CommitResult,
};

pub trait PaperRepository: Send + Sync {
    type Error: std::error::Error
        + Send
        + Sync
        + 'static;

    fn get(
        &self,
        id: PaperId,
    ) -> Result<Option<Paper>, Self::Error>;

    fn search(
        &self,
        query: &PaperQuery,
    ) -> Result<SearchResult, Self::Error>;

    fn versions(
        &self,
        paper_id: PaperId,
    ) -> Result<Vec<PaperVersion>, Self::Error>;

    fn attachments(
        &self,
        paper_id: PaperId,
    ) -> Result<Vec<Attachment>, Self::Error>;

    fn revisions(
        &self,
        target: &FieldTarget,
    ) -> Result<Vec<FieldRevision>, Self::Error>;

    fn active_revisions(
        &self,
        target: &FieldTarget,
    ) -> Result<Vec<FieldRevision>, Self::Error>;

    fn pending_proposals(
        &self,
        paper_id: PaperId,
    ) -> Result<Vec<UpdateProposal>, Self::Error>;
}

#[derive(Clone, Debug)]
pub enum ApplyEventResult {
    Applied,
    AlreadyApplied,

    MissingDependencies {
        events: Vec<EventId>,
    },
}

pub trait LibraryStore: PaperRepository {
    fn commit(
        &self,
        command: LibraryCommand,
    ) -> Result<CommitResult, Self::Error>;

    fn apply_remote_event(
        &self,
        event: &SyncEvent,
    ) -> Result<ApplyEventResult, Self::Error>;

    fn pending_events(
        &self,
        limit: usize,
    ) -> Result<Vec<SyncEvent>, Self::Error>;

    fn mark_events_uploaded(
        &self,
        event_ids: &[EventId],
    ) -> Result<(), Self::Error>;
}
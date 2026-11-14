use serde::{Deserialize, Serialize};

use super::{CollectionId, TagId};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ReadingStatus {
    #[default]
    Unread,
    Reading,
    Read,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserMetadata {
    pub notes: String,

    pub tags: Vec<TagId>,
    pub collections: Vec<CollectionId>,

    pub reading_status: ReadingStatus,
    pub favorite: bool,
    pub archived: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    pub id: TagId,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collection {
    pub id: CollectionId,
    pub name: String,
    pub parent: Option<CollectionId>,
}

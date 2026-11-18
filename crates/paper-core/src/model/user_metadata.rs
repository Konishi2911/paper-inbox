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

/// Longest tag name, in characters.
pub const MAX_TAG_NAME_CHARS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TagNameError {
    #[error("the tag name is empty")]
    Empty,
    #[error("the tag name is longer than {MAX_TAG_NAME_CHARS} characters")]
    TooLong,
    #[error("the tag name contains a control character")]
    Control,
}

impl Tag {
    /// A tag with a cleaned display name; see [`normalize_tag_name`].
    pub fn new(id: TagId, name: &str) -> Result<Self, TagNameError> {
        Ok(Self {
            id,
            name: normalize_tag_name(name)?.0,
        })
    }

    /// The comparison key of this tag's name; two tags with equal keys are the same tag.
    pub fn key(&self) -> Result<String, TagNameError> {
        Ok(normalize_tag_name(&self.name)?.1)
    }
}

/// Returns the display name (surrounding whitespace removed, inner runs collapsed to one
/// space) and the comparison key. The key also folds full-width ASCII and the ideographic
/// space to their half-width forms and ignores case, so `ML`, `ml`, `ＭＬ` and ` ML ` are
/// one tag. This is not full Unicode NFKC: half-width katakana, for example, stay distinct.
pub fn normalize_tag_name(name: &str) -> Result<(String, String), TagNameError> {
    if name.chars().any(char::is_control) {
        return Err(TagNameError::Control);
    }
    let display = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if display.is_empty() {
        return Err(TagNameError::Empty);
    }
    if display.chars().count() > MAX_TAG_NAME_CHARS {
        return Err(TagNameError::TooLong);
    }
    let folded: String = display
        .chars()
        .map(|c| match c {
            '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
            c => c,
        })
        .collect();
    let key = folded
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    Ok((display, key))
}

#[cfg(test)]
mod tag_tests {
    use super::*;

    #[test]
    fn names_that_look_the_same_share_a_key() {
        let key = |s| normalize_tag_name(s).unwrap().1;
        assert_eq!(key("ML"), key("ml"));
        assert_eq!(key("ML"), key("ＭＬ"));
        assert_eq!(key("  deep   learning "), key("Deep Learning"));
        assert_ne!(key("ML"), key("ML2"));
        assert_eq!(
            normalize_tag_name("  deep   learning ").unwrap().0,
            "deep learning"
        );
        assert_eq!(normalize_tag_name("グラフ").unwrap().1, "グラフ");
    }

    #[test]
    fn empty_long_and_control_names_are_refused() {
        assert_eq!(normalize_tag_name("  \u{3000} "), Err(TagNameError::Empty));
        assert_eq!(normalize_tag_name("a\u{0}b"), Err(TagNameError::Control));
        assert_eq!(
            normalize_tag_name(&"a".repeat(65)),
            Err(TagNameError::TooLong)
        );
        assert!(normalize_tag_name(&"あ".repeat(64)).is_ok());
    }
}

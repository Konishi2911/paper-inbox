use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            pub const fn from_uuid(value: Uuid) -> Self {
                Self(value)
            }

            pub const fn as_uuid(&self) -> &Uuid {
                &self.0
            }
        }

        /// Generates a fresh UUID, just like `new()`; never a nil sentinel.
        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Display::fmt(&self.0, f)
            }
        }
    };
}

define_id!(CandidateId);
define_id!(PaperId);
define_id!(VersionId);
define_id!(AttachmentId);
define_id!(RevisionId);
define_id!(ProposalId);
define_id!(ImportId);
define_id!(EventId);
define_id!(DeviceId);
define_id!(TagId);
define_id!(CollectionId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_ids_are_fresh_non_nil_v4_identifiers() {
        macro_rules! check {
            ($($ty:ty),+ $(,)?) => {$({
                let first = <$ty>::default();
                let second = <$ty>::default();
                assert!(!first.as_uuid().is_nil());
                assert_eq!(first.as_uuid().get_version_num(), 4);
                assert_ne!(first, second);
            })+};
        }
        check!(
            CandidateId,
            PaperId,
            VersionId,
            AttachmentId,
            RevisionId,
            ProposalId,
            ImportId,
            EventId,
            DeviceId,
            TagId,
            CollectionId
        );
    }
}

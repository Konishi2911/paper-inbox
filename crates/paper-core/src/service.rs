mod bibtex;
mod identity;
mod merge;
mod update;

pub use identity::{
    IdentityAssessment, IdentityEvidence, IdentityIdentifier, IdentityMatch, IdentitySide,
    assess_identity, compare_identity,
};

pub use update::{Freshness, UpdateDecision, UpdateError, compare_freshness, evaluate_update};

pub use merge::{MergeResult, three_way_merge};

pub use bibtex::{
    BibtexError, BibtexOptions, BibtexOutput, BibtexWarning, export_bibtex, render_entry,
};

mod candidates;
pub use candidates::{
    CandidateError, CandidateHoldReason, CandidateMode, MetadataUpdatePlan, PlannedMetadataField,
    plan_metadata_update,
};

mod citation_keys;
pub use citation_keys::suggest_citation_key;

mod field_diff;
pub use field_diff::{FieldChangeKind, FieldDiffError, compare_field_values};

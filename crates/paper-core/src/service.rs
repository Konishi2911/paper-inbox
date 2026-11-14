mod bibtex;
mod identity;
mod merge;
mod update;

pub use identity::{IdentityMatch, compare_identity};

pub use update::{Freshness, UpdateDecision, UpdateError, compare_freshness, evaluate_update};

pub use merge::{MergeResult, three_way_merge};

pub use bibtex::{
    BibtexError, BibtexOptions, BibtexOutput, BibtexWarning, export_bibtex, render_entry,
};

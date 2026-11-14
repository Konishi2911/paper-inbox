mod identity;
mod update;
mod merge;
mod bibtex;

pub use identity::{
    IdentityMatch,
    compare_identity,
};

pub use update::{
    Freshness,
    UpdateDecision,
    UpdateError,
    compare_freshness,
    evaluate_update,
};

pub use merge::{
    MergeResult,
    three_way_merge,
};

pub use bibtex::{
    BibtexOptions,
    BibtexError,
    BibtexWarning,
    BibtexOutput,
    render_entry,
    export_bibtex,
};
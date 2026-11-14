use crate::model::Bibliography;

#[derive(Clone, Debug, PartialEq)]
pub enum IdentityMatch {
    Exact,
    RelatedVersion,

    ProbableDuplicate { confidence: f64 },

    Different,
    Unknown,
}

/// Placeholder for document identity matching.
///
/// # Panics
/// This function is not implemented yet and always panics.
pub fn compare_identity(_lhs: &Bibliography, _rhs: &Bibliography) -> IdentityMatch {
    todo!()
}

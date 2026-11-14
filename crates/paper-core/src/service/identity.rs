use crate::model::Bibliography;

#[derive(Clone, Debug, PartialEq)]
pub enum IdentityMatch {
    Exact,
    RelatedVersion,

    ProbableDuplicate {
        confidence: f64,
    },

    Different,
    Unknown,
}

pub fn compare_identity(
    lhs: &Bibliography,
    rhs: &Bibliography,
) -> IdentityMatch {
    todo!()
}
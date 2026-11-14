use crate::model::{ArxivId, Bibliography, Doi};

#[derive(Clone, Debug, PartialEq)]
pub enum IdentityMatch {
    /// Matching validated identifiers; does not guarantee identical PDF contents.
    Exact,
    RelatedVersion,
    /// Matching evidence and contradictory identifiers coexist.
    NeedsReview,
    /// Reserved for a later, separately designed similarity matcher.
    ProbableDuplicate {
        confidence: f64,
    },
    /// Reserved: unequal identifiers alone do not establish different works.
    Different,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityIdentifier {
    Doi,
    Arxiv,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentitySide {
    Left,
    Right,
}

/// Directional evidence, in stable DOI-then-arXiv order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdentityEvidence {
    DoiMatch {
        doi: Doi,
    },
    DoiMismatch {
        left: Doi,
        right: Doi,
    },
    ArxivMatch {
        id: ArxivId,
    },
    ArxivVersionRelation {
        left: ArxivId,
        right: ArxivId,
    },
    ArxivMismatch {
        left: ArxivId,
        right: ArxivId,
    },
    MissingIdentifier {
        identifier: IdentityIdentifier,
        side: IdentitySide,
    },
    /// A raw legacy value is never reinterpreted as validated evidence here.
    /// This field is excluded even if a caller also supplies a typed value.
    UnvalidatedIdentifier {
        identifier: IdentityIdentifier,
        side: IdentitySide,
        raw: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct IdentityAssessment {
    pub outcome: IdentityMatch,
    pub evidence: Vec<IdentityEvidence>,
}

/// Determine identity using validated DOI/arXiv identifiers only.
///
/// A common arXiv base with different or partly omitted versions is related.
/// A DOI match takes precedence over that weaker version relation. Any identifier
/// mismatch combined with a match or version relation requires review. Mismatches
/// without a positive link remain Unknown. No probabilities, title similarity,
/// network lookups, automatic merge or bibliography updates are performed.
///
/// The outcome is symmetric. Evidence retains left/right values so callers can
/// explain the assessment. Unvalidated fields require repair independently of the
/// outcome: an Exact result is not authorization to merge or update a document.
pub fn assess_identity(lhs: &Bibliography, rhs: &Bibliography) -> IdentityAssessment {
    let mut evidence = Vec::new();
    let left = &lhs.identifiers;
    let right = &rhs.identifiers;
    let ld = usable(
        left.doi.as_ref(),
        left.raw_doi.as_ref(),
        IdentityIdentifier::Doi,
        IdentitySide::Left,
        &mut evidence,
    );
    let rd = usable(
        right.doi.as_ref(),
        right.raw_doi.as_ref(),
        IdentityIdentifier::Doi,
        IdentitySide::Right,
        &mut evidence,
    );
    let mut matched = false;
    let mut related = false;
    let mut mismatched = false;
    if let (Some(left), Some(right)) = (ld, rd) {
        if left == right {
            matched = true;
            evidence.push(IdentityEvidence::DoiMatch { doi: left.clone() });
        } else {
            mismatched = true;
            evidence.push(IdentityEvidence::DoiMismatch {
                left: left.clone(),
                right: right.clone(),
            });
        }
    }
    let la = usable(
        left.arxiv_id.as_ref(),
        left.raw_arxiv_id.as_ref(),
        IdentityIdentifier::Arxiv,
        IdentitySide::Left,
        &mut evidence,
    );
    let ra = usable(
        right.arxiv_id.as_ref(),
        right.raw_arxiv_id.as_ref(),
        IdentityIdentifier::Arxiv,
        IdentitySide::Right,
        &mut evidence,
    );
    if let (Some(left), Some(right)) = (la, ra) {
        if left == right {
            matched = true;
            evidence.push(IdentityEvidence::ArxivMatch { id: left.clone() });
        } else if left.base_id() == right.base_id() {
            related = true;
            evidence.push(IdentityEvidence::ArxivVersionRelation {
                left: left.clone(),
                right: right.clone(),
            });
        } else {
            mismatched = true;
            evidence.push(IdentityEvidence::ArxivMismatch {
                left: left.clone(),
                right: right.clone(),
            });
        }
    }
    let outcome = if mismatched && (matched || related) {
        IdentityMatch::NeedsReview
    } else if matched {
        IdentityMatch::Exact
    } else if related {
        IdentityMatch::RelatedVersion
    } else {
        IdentityMatch::Unknown
    };
    IdentityAssessment { outcome, evidence }
}

/// Compatibility entry point for callers that need only the verdict.
pub fn compare_identity(lhs: &Bibliography, rhs: &Bibliography) -> IdentityMatch {
    assess_identity(lhs, rhs).outcome
}

fn usable<'a, T>(
    value: Option<&'a T>,
    raw: Option<&String>,
    identifier: IdentityIdentifier,
    side: IdentitySide,
    evidence: &mut Vec<IdentityEvidence>,
) -> Option<&'a T> {
    if let Some(raw) = raw {
        evidence.push(IdentityEvidence::UnvalidatedIdentifier {
            identifier,
            side,
            raw: raw.clone(),
        });
        None
    } else {
        if value.is_none() {
            evidence.push(IdentityEvidence::MissingIdentifier { identifier, side });
        }
        value
    }
}

use thiserror::Error;

use crate::model::{FieldRevision, FieldTarget, MetadataSource, ProposalReason, Provenance};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Freshness {
    Newer,
    Older,
    Equal,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateDecision {
    NoChange,
    Apply,
    IgnoreStale,
    Propose(ProposalReason),
}

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("Mismatched field targets")]
    TargetMismatch {
        current: FieldTarget,
        candidate: FieldTarget,
    },

    #[error("Metadata candidate must originate from an external source")]
    InvalidCandidateSource,
}

pub fn compare_freshness(current: &Provenance, candidate: &Provenance) -> Freshness {
    use std::cmp::Ordering;

    let same_record = current.source == candidate.source
        && current.source_record_id.is_some()
        && current.source_record_id == candidate.source_record_id;

    if !same_record {
        return Freshness::Unknown;
    }

    match (
        current.source_updated_at.as_ref(),
        candidate.source_updated_at.as_ref(),
    ) {
        (Some(current_at), Some(candidate_at)) => match candidate_at.cmp(current_at) {
            Ordering::Greater => Freshness::Newer,
            Ordering::Less => Freshness::Older,
            Ordering::Equal => Freshness::Equal,
        },
        _ => Freshness::Unknown,
    }
}

pub fn evaluate_update(
    current: Option<&FieldRevision>,
    candidate: &FieldRevision,
) -> Result<UpdateDecision, UpdateError> {
    if candidate.provenance.source == MetadataSource::Manual {
        return Err(UpdateError::InvalidCandidateSource);
    }
    let Some(current) = current else {
        return Ok(UpdateDecision::Apply);
    };
    if current.target() != candidate.target() {
        return Err(UpdateError::TargetMismatch {
            current: current.target(),
            candidate: candidate.target(),
        });
    }
    if current.value == candidate.value {
        return Ok(UpdateDecision::NoChange);
    }
    let freshness = compare_freshness(&current.provenance, &candidate.provenance);
    if freshness == Freshness::Older {
        return Ok(UpdateDecision::IgnoreStale);
    }
    if current.provenance.source == MetadataSource::Manual {
        return Ok(UpdateDecision::Propose(
            ProposalReason::OverwriteManualValue,
        ));
    }
    Ok(match freshness {
        Freshness::Newer => UpdateDecision::Apply,
        Freshness::Older => UpdateDecision::IgnoreStale,
        Freshness::Equal => UpdateDecision::Propose(ProposalReason::ConflictingSources),
        Freshness::Unknown => UpdateDecision::Propose(ProposalReason::UnknownFreshness),
    })
}

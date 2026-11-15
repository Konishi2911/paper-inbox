use super::{
    IdentityAssessment, IdentityEvidence, IdentityMatch, UpdateDecision, UpdateError,
    assess_identity, evaluate_update,
};
use crate::model::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateMode {
    Automatic,
    UserAssociated,
    CanonicalSelection,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateHoldReason {
    RelatedVersion,
    ConflictingIdentifiers,
    UnknownIdentity,
    UnvalidatedIdentifiers,
}
#[derive(Clone, Debug, PartialEq)]
pub struct MetadataUpdatePlan {
    pub identity: IdentityAssessment,
    pub held: Option<CandidateHoldReason>,
    pub fields: Vec<PlannedMetadataField>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedMetadataField {
    pub field: BibliographyField,
    pub current: BibliographyValue,
    pub candidate: BibliographyValue,
    pub expected_heads: Vec<RevisionId>,
    pub decision: UpdateDecision,
}
#[derive(Debug, thiserror::Error)]
pub enum CandidateError {
    #[error("Candidate source must be external")]
    ManualSource,
    #[error("Candidate field list contains duplicates")]
    DuplicateFields,
    #[error("Empty source name or record ID")]
    InvalidSource,
    #[error("Revision does not belong to the requested paper-level field")]
    InvalidRevision,
    #[error(transparent)]
    Paper(#[from] PaperError),
    #[error(transparent)]
    Identifier(#[from] IdentifierError),
    #[error(transparent)]
    Update(#[from] UpdateError),
}
impl MetadataCandidate {
    pub fn validate(&self) -> Result<(), CandidateError> {
        if self.provenance.source == MetadataSource::Manual {
            return Err(CandidateError::ManualSource);
        }
        if self
            .provenance
            .source_record_id
            .as_ref()
            .is_some_and(|s| s.trim().is_empty())
            || matches!(&self.provenance.source,MetadataSource::Other(s) if s.trim().is_empty())
        {
            return Err(CandidateError::InvalidSource);
        }
        let mut fields = std::collections::HashSet::new();
        for field in &self.present_fields {
            if !fields.insert(*field) {
                return Err(CandidateError::DuplicateFields);
            }
        }
        self.bibliography.validate()?;
        self.bibliography.identifiers.validate_input()?;
        Ok(())
    }
}
/// Plan only explicitly supplied fields. No writes or identities are fabricated.
pub fn plan_metadata_update(
    paper: &Paper,
    heads: &[FieldRevision],
    candidate: &MetadataCandidate,
    mode: CandidateMode,
) -> Result<MetadataUpdatePlan, CandidateError> {
    candidate.validate()?;
    if heads
        .iter()
        .any(|r| r.paper_id != paper.id() || r.version_id.is_some())
    {
        return Err(CandidateError::InvalidRevision);
    }
    let identity = assess_identity(paper.bibliography(), &candidate.bibliography);
    let held = if mode != CandidateMode::Automatic {
        None
    } else if identity
        .evidence
        .iter()
        .any(|e| matches!(e, IdentityEvidence::UnvalidatedIdentifier { .. }))
    {
        Some(CandidateHoldReason::UnvalidatedIdentifiers)
    } else {
        match identity.outcome {
            IdentityMatch::Exact => None,
            IdentityMatch::RelatedVersion => Some(CandidateHoldReason::RelatedVersion),
            IdentityMatch::NeedsReview => Some(CandidateHoldReason::ConflictingIdentifiers),
            _ => Some(CandidateHoldReason::UnknownIdentity),
        }
    };
    let mut plan = MetadataUpdatePlan {
        identity,
        held,
        fields: vec![],
    };
    if held.is_some() {
        return Ok(plan);
    }
    for field in BibliographyField::ALL {
        if candidate.present_fields.contains(&field) {
            let current = paper.bibliography().value(field);
            let value = candidate.bibliography.value(field);
            let mut revisions: Vec<_> = heads.iter().filter(|r| r.value.field() == field).collect();
            revisions.sort_by_key(|r| r.id);
            let expected_heads = revisions.iter().map(|r| r.id).collect();
            let revision = FieldRevision {
                id: RevisionId::from_uuid(*candidate.id.as_uuid()),
                paper_id: paper.id(),
                version_id: None,
                parents: vec![],
                value: value.clone(),
                provenance: candidate.provenance.clone(),
                created_at: candidate.provenance.observed_at,
            };
            let decision = if revisions.len() > 1 {
                UpdateDecision::Propose(ProposalReason::ConcurrentModification)
            } else if current == value {
                UpdateDecision::NoChange
            } else {
                let decision = evaluate_update(revisions.first().copied(), &revision)?;
                if decision == UpdateDecision::IgnoreStale {
                    decision
                } else if !value.has_initial_value() {
                    UpdateDecision::Propose(ProposalReason::ExternalDeletion)
                } else if mode == CandidateMode::UserAssociated {
                    UpdateDecision::Propose(ProposalReason::ManualAssociation)
                } else if revisions.is_empty() && current.has_initial_value() {
                    UpdateDecision::Propose(ProposalReason::UnknownFreshness)
                } else {
                    decision
                }
            };
            plan.fields.push(PlannedMetadataField {
                field,
                current,
                candidate: value,
                expected_heads,
                decision,
            });
        }
    }
    Ok(plan)
}

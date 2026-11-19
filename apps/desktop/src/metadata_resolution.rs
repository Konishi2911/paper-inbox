//! Typed resolution input. Display labels/current/proposed strings are never parsed back.
use crate::{
    DiffResolution,
    application::{AppError, Application},
};
use chrono::Utc;
use paper_core::{
    command::LibraryCommand,
    model::{ArxivId, BibliographyValue as V, Doi, ProposalId, ProposalKind, ProposalResolution},
    query::MetadataReview,
};

#[derive(Clone, Debug)]
pub struct ResolutionInput {
    pub id: ProposalId,
    pub decision: DiffResolution,
    pub text: String,
    pub doi: String,
    pub arxiv: String,
}
pub fn custom_kind(value: &V) -> i32 {
    match value {
        V::Title(_)
        | V::ContainerTitle(_)
        | V::Publisher(_)
        | V::Year(_)
        | V::Month(_)
        | V::Volume(_)
        | V::Issue(_)
        | V::Pages(_)
        | V::ArticleNumber(_)
        | V::Abstract(_) => 1,
        V::Identifiers(_) => 2,
        _ => 0,
    }
}
pub fn custom_text(value: &V) -> String {
    match value {
        V::Title(v)
        | V::ContainerTitle(v)
        | V::Publisher(v)
        | V::Volume(v)
        | V::Issue(v)
        | V::Pages(v)
        | V::ArticleNumber(v)
        | V::Abstract(v) => v.clone().unwrap_or_default(),
        V::Year(v) => v.map(|n| n.to_string()).unwrap_or_default(),
        V::Month(v) => v.map(|n| n.to_string()).unwrap_or_default(),
        _ => String::new(),
    }
}
fn custom_value(original: &V, input: &ResolutionInput) -> Result<V, AppError> {
    let text = if input.text.is_empty() {
        None
    } else {
        Some(input.text.clone())
    };
    let invalid =
        || AppError::Input("数値の形式または範囲が不正です。入力を確認してください。".into());
    Ok(match original {
        V::Title(_) => V::Title(text),
        V::ContainerTitle(_) => V::ContainerTitle(text),
        V::Publisher(_) => V::Publisher(text),
        V::Volume(_) => V::Volume(text),
        V::Issue(_) => V::Issue(text),
        V::Pages(_) => V::Pages(text),
        V::ArticleNumber(_) => V::ArticleNumber(text),
        V::Abstract(_) => V::Abstract(text),
        V::Year(_) => V::Year(
            text.map(|s| {
                s.trim()
                    .parse::<u16>()
                    .ok()
                    .filter(|n| *n > 0)
                    .ok_or_else(invalid)
            })
            .transpose()?,
        ),
        V::Month(_) => V::Month(
            text.map(|s| {
                s.trim()
                    .parse::<u8>()
                    .ok()
                    .filter(|n| (1..=12).contains(n))
                    .ok_or_else(invalid)
            })
            .transpose()?,
        ),
        V::Identifiers(original) => {
            let mut ids = original.clone();
            ids.doi = if input.doi.trim().is_empty() {
                None
            } else {
                Some(Doi::parse(&input.doi).map_err(|e| AppError::Input(e.to_string()))?)
            };
            ids.arxiv_id = if input.arxiv.trim().is_empty() {
                None
            } else {
                Some(ArxivId::parse(&input.arxiv).map_err(|e| AppError::Input(e.to_string()))?)
            };
            ids.raw_doi = None;
            ids.raw_arxiv_id = None;
            V::Identifiers(ids)
        }
        _ => {
            return Err(AppError::Input(
                "この項目のカスタム編集は文献編集画面で行ってください。".into(),
            ));
        }
    })
}
impl Application {
    pub fn resolve_metadata_batch(
        &self,
        review: &MetadataReview,
        inputs: &[ResolutionInput],
    ) -> Result<(), AppError> {
        let mut commands = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut fields = std::collections::HashSet::new();
        let mut canonical = false;
        for input in inputs {
            if input.decision == DiffResolution::Unresolved {
                continue;
            }
            if !seen.insert(input.id) {
                return Err(AppError::Input("同じ提案の判断が重複しています。".into()));
            }
            let detail = review
                .proposals
                .iter()
                .find(|d| d.proposal.id == input.id)
                .ok_or_else(|| {
                    AppError::Input("表示対象にない提案です。表示を更新してください。".into())
                })?;
            let keep = input.decision == DiffResolution::KeepCurrent;
            if !keep && detail.unavailable_reason.is_some() {
                return Err(AppError::Conflict);
            }
            if !keep {
                match &detail.proposal.kind {
                    ProposalKind::FieldUpdate { target, .. } => {
                        if !fields.insert(*target) {
                            return Err(AppError::Input(
                                "同じ項目の提案は一つだけ採用してください。".into(),
                            ));
                        }
                    }
                    ProposalKind::CanonicalVersionChange { .. } => {
                        if canonical {
                            return Err(AppError::Input(
                                "正規版切替は一つだけ採用してください。".into(),
                            ));
                        }
                        canonical = true;
                    }
                    _ => return Err(AppError::Input("この提案は解決できません。".into())),
                }
            }
            let resolution = match input.decision {
                DiffResolution::KeepCurrent => ProposalResolution::Reject,
                DiffResolution::AcceptProposed => ProposalResolution::Accept,
                DiffResolution::Custom => {
                    let original = detail.current_value.as_ref().ok_or_else(|| {
                        AppError::Input("この提案はカスタム編集できません。".into())
                    })?;
                    let value = custom_value(original, input)?;
                    let mut paper = review.state.paper.clone();
                    paper
                        .edit_bibliography(&value, Utc::now())
                        .map_err(|e| AppError::Input(e.to_string()))?;
                    ProposalResolution::CustomValue(value)
                }
                DiffResolution::Unresolved => continue,
            };
            let order = if keep {
                0
            } else if matches!(
                detail.proposal.kind,
                ProposalKind::CanonicalVersionChange { .. }
            ) {
                2
            } else {
                1
            };
            commands.push((
                order,
                LibraryCommand::ResolveProposal {
                    proposal_id: input.id,
                    resolution,
                },
            ));
        }
        if commands.is_empty() {
            return Err(AppError::Input("適用する判断がありません。".into()));
        }
        commands.sort_by_key(|(order, _)| *order);
        self.store.commit_batch_at(
            commands.into_iter().map(|(_, c)| c).collect(),
            std::slice::from_ref(&review.state),
            Utc::now(),
        )?;
        Ok(())
    }
}

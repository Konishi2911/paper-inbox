use crate::{
    CandidateRow, DetailView, MainWindow, ProposalRow, VersionRow,
    metadata::{self, CandidateForm},
};
use paper_core::{
    model::*,
    query::MetadataReview,
    service::{IdentityMatch, assess_identity},
};
use slint::{ModelRc, VecModel};
pub fn read_form(ui: &MainWindow) -> CandidateForm {
    CandidateForm {
        id: ui.get_candidate_id().into(),
        observed_at: ui.get_candidate_observed().into(),
        source: ui.get_candidate_source().max(0) as usize,
        source_name: ui.get_candidate_source_name().into(),
        record_id: ui.get_candidate_record().into(),
        updated_at: ui.get_candidate_updated().into(),
        version_kind: ui.get_candidate_kind().max(0) as usize,
        doi: ui.get_candidate_doi().into(),
        arxiv: ui.get_candidate_arxiv().into(),
        title: ui.get_candidate_title().into(),
        use_title: ui.get_candidate_use_title(),
        year: ui.get_candidate_year().into(),
        use_year: ui.get_candidate_use_year(),
        container: ui.get_candidate_container().into(),
        use_container: ui.get_candidate_use_container(),
        publisher: ui.get_candidate_publisher().into(),
        use_publisher: ui.get_candidate_use_publisher(),
        abstract_text: ui.get_candidate_abstract().into(),
        use_abstract: ui.get_candidate_use_abstract(),
    }
}
pub fn begin_form(ui: &MainWindow) {
    ui.set_candidate_id(CandidateId::new().to_string().into());
    ui.set_candidate_observed(chrono::Utc::now().to_rfc3339().into());
    ui.set_candidate_source(0);
    ui.set_candidate_source_name("".into());
    ui.set_candidate_record("".into());
    ui.set_candidate_updated("".into());
    ui.set_candidate_kind(0);
    ui.set_candidate_doi("".into());
    ui.set_candidate_arxiv("".into());
    ui.set_candidate_title("".into());
    ui.set_candidate_year("".into());
    ui.set_candidate_container("".into());
    ui.set_candidate_publisher("".into());
    ui.set_candidate_abstract("".into());
    ui.set_candidate_use_title(false);
    ui.set_candidate_use_year(false);
    ui.set_candidate_use_container(false);
    ui.set_candidate_use_publisher(false);
    ui.set_candidate_use_abstract(false);
    ui.set_candidate_dirty(false);
    ui.set_detail_view(DetailView::MetadataCandidateEditor);
}
pub fn show_review(ui: &MainWindow, review: &MetadataReview) {
    ui.set_review_label(
        format!(
            "{}\n{} — 承認待ち {} 件（正規版切替を含む）",
            review
                .state
                .paper
                .bibliography()
                .title
                .as_deref()
                .unwrap_or("タイトルなし"),
            review.state.paper.citation_key(),
            review.proposals.len()
        )
        .into(),
    );
    let candidates=review.candidates.iter().map(|stored| {
        let identity=assess_identity(review.state.paper.bibliography(),&stored.candidate.bibliography);
        let label=match identity.outcome {IdentityMatch::Exact=>"識別子が一致",IdentityMatch::RelatedVersion=>"関連する版",IdentityMatch::NeedsReview=>"識別子に矛盾",_=>"同一性を判定できません"};
        let status=match stored.status {CandidateStatus::Held=>"保留",CandidateStatus::Processed=>"処理済み",CandidateStatus::Associated=>"手動関連付け済み"};
        let fields=stored.candidate.present_fields.iter().map(|f|metadata::field_label(*f)).collect::<Vec<_>>().join("、");
        CandidateRow {id:stored.candidate.id.to_string().into(),label:format!("{status} / {label} / {} / {}",metadata::source_label(&stored.candidate.provenance.source),metadata::version_label(stored.candidate.version_kind)).into(),
            details:format!("タイトル: {}\nDOI: {} / arXiv: {}\n提示項目: {}\n情報源の記録: {} / 更新日時: {}",stored.candidate.bibliography.title.as_deref().unwrap_or("（未提示）"),stored.candidate.bibliography.identifiers.doi_text().unwrap_or("（未提示）"),stored.candidate.bibliography.identifiers.arxiv_text().unwrap_or("（未提示）"),fields,stored.candidate.provenance.source_record_id.as_deref().unwrap_or("（不明）"),stored.candidate.provenance.source_updated_at.map(|t|t.to_rfc3339()).unwrap_or_else(||"（不明）".into())).into(),held:stored.status==CandidateStatus::Held}
    }).collect::<Vec<_>>();
    ui.set_candidate_rows(ModelRc::new(VecModel::from(candidates)));
    let proposals = review
        .proposals
        .iter()
        .map(|detail| {
            let (label, current, proposed, stale) = match &detail.proposal.kind {
                ProposalKind::FieldUpdate {
                    target,
                    expected_heads,
                    ..
                } => {
                    let current = detail
                        .current_value
                        .as_ref()
                        .map(metadata::value_label)
                        .unwrap_or_default();
                    let proposed = detail
                        .proposed_value
                        .as_ref()
                        .map(metadata::value_label)
                        .unwrap_or_default();
                    let heads = review
                        .state
                        .heads
                        .iter()
                        .find(|(f, _)| *f == target.field)
                        .map(|(_, h)| h.as_slice())
                        .unwrap_or(&[]);
                    (
                        metadata::field_label(target.field).to_owned(),
                        current,
                        proposed,
                        heads != expected_heads.as_slice(),
                    )
                }
                ProposalKind::CanonicalVersionChange {
                    current_version,
                    candidate_version,
                    ..
                } => {
                    let target = review.versions.iter().find(|v| v.id == *candidate_version);
                    (
                        "正規版の切替".into(),
                        review
                            .state
                            .paper
                            .canonical_version()
                            .and_then(|id| review.versions.iter().find(|v| v.id == id))
                            .map(|v| {
                                format!(
                                    "{} / {}",
                                    metadata::version_label(v.kind),
                                    v.bibliography.title.as_deref().unwrap_or("タイトルなし")
                                )
                            })
                            .unwrap_or_else(|| "正規版未指定".into()),
                        target
                            .map(|v| {
                                format!(
                                    "{} / {}",
                                    metadata::version_label(v.kind),
                                    v.bibliography.title.as_deref().unwrap_or("タイトルなし")
                                )
                            })
                            .unwrap_or_else(|| "版が見つかりません".into()),
                        *current_version != review.state.paper.canonical_version(),
                    )
                }
                _ => ("重複の統合".into(), String::new(), String::new(), true),
            };
            let source = detail
                .provenance
                .as_ref()
                .map(|p| {
                    format!(
                        "{} / 記録: {} / 更新日時: {}",
                        metadata::source_label(&p.source),
                        p.source_record_id.as_deref().unwrap_or("（不明）"),
                        p.source_updated_at
                            .map(|t| t.to_rfc3339())
                            .unwrap_or_else(|| "（不明）".into())
                    )
                })
                .unwrap_or_else(|| "版の切替。書誌情報は別途確認します。".into());
            let change_label = match (&detail.current_value, &detail.proposed_value) {
                (Some(current), proposed) => {
                    match paper_core::service::compare_field_values(current, proposed.as_ref()) {
                        Ok(Some(paper_core::service::FieldChangeKind::Unchanged)) => "変更なし",
                        Ok(Some(paper_core::service::FieldChangeKind::Added)) => "追加",
                        Ok(Some(paper_core::service::FieldChangeKind::Removed)) => "削除",
                        Ok(Some(paper_core::service::FieldChangeKind::Modified)) => "変更",
                        Ok(None) => "未提示",
                        Err(_) => "比較できません",
                    }
                }
                _ => "正規版切替（項目更新とは別操作）",
            };
            let current_source = if detail.current_revisions.is_empty() {
                "出典記録なし".to_owned()
            } else {
                detail
                    .current_revisions
                    .iter()
                    .map(|r| metadata::source_label(&r.provenance.source))
                    .collect::<Vec<_>>()
                    .join(" / ")
            };
            let current_details = detail
                .current_revisions
                .iter()
                .map(|r| format!("改訂: {}\n{}", r.id, provenance_details(&r.provenance)))
                .collect::<Vec<_>>()
                .join("\n\n");
            let proposed_details = match &detail.proposal.kind {
                ProposalKind::FieldUpdate {
                    candidate_revision, ..
                } => format!(
                    "提案: {}\n候補改訂: {}\n{}",
                    detail.proposal.id,
                    candidate_revision,
                    detail
                        .provenance
                        .as_ref()
                        .map(provenance_details)
                        .unwrap_or_else(|| "出典記録なし".into())
                ),
                _ => format!(
                    "提案: {}\n作成日時: {}\n正規版切替は書誌項目を直ちに置き換えません。",
                    detail.proposal.id,
                    detail.proposal.created_at.to_rfc3339()
                ),
            };
            ProposalRow {
                current_preview: preview_text(&current).into(),
                proposed_preview: preview_text(&proposed).into(),
                change_label: change_label.into(),
                conflict: matches!(
                    detail.proposal.reason,
                    ProposalReason::OverwriteManualValue
                        | ProposalReason::ConflictingSources
                        | ProposalReason::UnknownFreshness
                        | ProposalReason::ConcurrentModification
                        | ProposalReason::ManualAssociation
                ),
                current_source: current_source.into(),
                current_details: current_details.into(),
                proposed_details: proposed_details.into(),
                long_text: current.chars().count() > 600 || proposed.chars().count() > 600,
                id: detail.proposal.id.to_string().into(),
                label: label.into(),
                current: current.into(),
                proposed: proposed.into(),
                reason: metadata::reason_label(detail.proposal.reason).into(),
                source: source.into(),
                stale,
            }
        })
        .collect::<Vec<_>>();
    ui.set_proposal_rows(ModelRc::new(VecModel::from(proposals)));
    let versions = review
        .versions
        .iter()
        .map(|v| VersionRow {
            id: v.id.to_string().into(),
            label: format!(
                "{} / {}",
                metadata::version_label(v.kind),
                v.bibliography.title.as_deref().unwrap_or("タイトルなし")
            )
            .into(),
            details: format!(
                "DOI: {} / arXiv: {} / 出版年: {}",
                v.bibliography
                    .identifiers
                    .doi_text()
                    .unwrap_or("（未提示）"),
                v.bibliography
                    .identifiers
                    .arxiv_text()
                    .unwrap_or("（未提示）"),
                v.bibliography
                    .publication
                    .year
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "（未提示）".into())
            )
            .into(),
            canonical: review.state.paper.canonical_version() == Some(v.id),
        })
        .collect::<Vec<_>>();
    ui.set_version_rows(ModelRc::new(VecModel::from(versions)));
    ui.set_load_required(false);
    ui.set_associate_confirm(false);
}

fn provenance_details(p: &Provenance) -> String {
    format!(
        "情報源: {}\nレコード: {}\n取得日時: {}\n情報源更新日時: {}",
        metadata::source_label(&p.source),
        p.source_record_id.as_deref().unwrap_or("（不明）"),
        p.observed_at.to_rfc3339(),
        p.source_updated_at
            .map(|t| t.to_rfc3339())
            .unwrap_or_else(|| "（不明）".into())
    )
}

fn preview_text(value: &str) -> String {
    let mut chars = value.chars();
    let mut preview: String = chars.by_ref().take(600).collect();
    if chars.next().is_some() {
        preview.push_str("…（全文を表示できます）");
    }
    preview
}

#[cfg(test)]
mod diff_display_tests {
    use super::preview_text;
    #[test]
    fn unicode_preview_is_bounded_without_changing_short_text() {
        assert_eq!(preview_text("α² 日本語"), "α² 日本語");
        let long = "日本語α²".repeat(20000);
        let preview = preview_text(&long);
        assert!(preview.starts_with(&long.chars().take(600).collect::<String>()));
        assert!(preview.chars().count() < 650);
        assert!(preview.ends_with("（全文を表示できます）"));
    }
}

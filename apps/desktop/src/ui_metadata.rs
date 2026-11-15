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
                        .map(review_value_label)
                        .unwrap_or_default();
                    let proposed = detail
                        .proposed_value
                        .as_ref()
                        .map(review_value_label)
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
            let stale = stale || detail.unavailable_reason.is_some();
            let requires_review=matches!(detail.proposal.reason,ProposalReason::OverwriteManualValue | ProposalReason::ConflictingSources | ProposalReason::UnknownFreshness | ProposalReason::ConcurrentModification | ProposalReason::ManualAssociation | ProposalReason::ExternalDeletion)
                || detail.current_revisions.iter().any(|r|r.provenance.source==MetadataSource::Manual)
                || detail.current_revisions.len()>1
                || matches!(detail.proposal.kind,ProposalKind::FieldUpdate {target,..} if target.field==BibliographyField::Identifiers);

            let custom_kind=detail.current_value.as_ref().map(crate::metadata_resolution::custom_kind).unwrap_or(0);
            let custom_text=detail.current_value.as_ref().map(crate::metadata_resolution::custom_text).unwrap_or_default();
            let (custom_doi,custom_arxiv)=match &detail.current_value {
                Some(BibliographyValue::Identifiers(ids))=>(ids.doi_text().unwrap_or_default(),ids.arxiv_text().unwrap_or_default()),
                _=>("",""),
            };
            ProposalRow {
                custom_kind,custom_text:custom_text.into(),custom_doi:custom_doi.into(),custom_arxiv:custom_arxiv.into(),
                bulk_safe: !stale && !requires_review && matches!(detail.proposal.kind,ProposalKind::FieldUpdate {..}),
                decision:crate::DiffResolution::Unresolved,
                text_diff: match (&detail.current_value,&detail.proposed_value) {
                    (Some(BibliographyValue::Title(_)|BibliographyValue::Abstract(_)),Some(_))=>crate::metadata_diff::inline_text(&current,&proposed).into(),
                    _=>"".into(),
                },
                structured_diff:match (&detail.current_value,&detail.proposed_value) {
                    (Some(a),Some(b))=>crate::metadata_diff::structured_text(a,b).into(),_=>"".into(),
                },
                current_preview: preview_text(&current).into(),
                proposed_preview: preview_text(&proposed).into(),
                change_label: change_label.into(),
                conflict: requires_review,
                current_source: current_source.into(),
                current_details: current_details.into(),
                proposed_details: proposed_details.into(),
                long_text: current.chars().count() > 600 || proposed.chars().count() > 600,
                id: detail.proposal.id.to_string().into(),
                label: label.into(),
                current: current.into(),
                proposed: proposed.into(),
                reason: detail
                    .unavailable_reason
                    .as_deref()
                    .unwrap_or_else(|| metadata::reason_label(detail.proposal.reason))
                    .into(),
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
    ui.set_review_dirty(false);
    update_summary(ui);
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

pub fn read_decisions(
    ui: &MainWindow,
) -> Result<Vec<crate::metadata_resolution::ResolutionInput>, crate::application::AppError> {
    use slint::Model;
    ui.get_proposal_rows()
        .iter()
        .filter(|r| r.decision != crate::DiffResolution::Unresolved)
        .map(|r| {
            Ok(crate::metadata_resolution::ResolutionInput {
                id: ProposalId::from_uuid(uuid::Uuid::parse_str(&r.id).map_err(|_| {
                    crate::application::AppError::Input("提案IDが不正です。".into())
                })?),
                decision: r.decision,
                text: r.custom_text.into(),
                doi: r.custom_doi.into(),
                arxiv: r.custom_arxiv.into(),
            })
        })
        .collect()
}
fn update_summary(ui: &MainWindow) {
    use slint::Model;
    let rows = ui.get_proposal_rows();
    let rows = rows.iter().collect::<Vec<_>>();
    let count = |label: &str| rows.iter().filter(|r| r.change_label == label).count();
    let decided = rows
        .iter()
        .filter(|r| r.decision != crate::DiffResolution::Unresolved)
        .count();
    ui.set_review_dirty(decided > 0);
    refresh_visible(ui);
    ui.set_diff_summary(
        format!(
            "追加 {} / 変更 {} / 削除 {} / 要確認 {}（差分と重複）\n未判断 {} / 判断済み {}",
            count("追加"),
            count("変更"),
            count("削除"),
            rows.iter().filter(|r| r.conflict || r.stale).count(),
            rows.len() - decided,
            decided
        )
        .into(),
    );
}
fn mutate_rows(ui: &MainWindow, mut change: impl FnMut(&mut crate::ProposalRow)) {
    use slint::Model;
    let rows = ui.get_proposal_rows();
    for index in 0..rows.row_count() {
        if let Some(mut row) = rows.row_data(index) {
            change(&mut row);
            rows.set_row_data(index, row);
        }
    }
    update_summary(ui);
}
pub fn attach_decisions(ui: &MainWindow) {
    let weak = slint::ComponentHandle::as_weak(ui);
    ui.on_refilter_metadata(move || {
        if let Some(ui) = weak.upgrade() {
            refresh_visible(&ui);
        }
    });
    let weak = slint::ComponentHandle::as_weak(ui);
    ui.on_choose_metadata(move |id, decision| {
        if let Some(ui) = weak.upgrade() {
            mutate_rows(&ui, |r| {
                if r.id == id
                    && (!r.stale
                        || matches!(
                            decision,
                            crate::DiffResolution::Unresolved | crate::DiffResolution::KeepCurrent
                        ))
                    && (decision != crate::DiffResolution::Custom || r.custom_kind > 0)
                {
                    r.decision = decision;
                }
            });
        }
    });
    let weak = slint::ComponentHandle::as_weak(ui);
    ui.on_edit_metadata(move |id, field, value| {
        if let Some(ui) = weak.upgrade() {
            mutate_rows(&ui, |r| {
                if r.id == id && r.decision == crate::DiffResolution::Custom {
                    match field {
                        crate::DiffInputField::Value => r.custom_text = value.clone(),
                        crate::DiffInputField::Doi => r.custom_doi = value.clone(),
                        crate::DiffInputField::Arxiv => r.custom_arxiv = value.clone(),
                    }
                }
            });
        }
    });
    let weak = slint::ComponentHandle::as_weak(ui);
    ui.on_bulk_metadata(move |action| {
        if let Some(ui) = weak.upgrade() {
            use slint::Model;
            let safe=ui.get_proposal_rows().iter().filter(|r|r.bulk_safe && !r.stale && r.decision==crate::DiffResolution::Unresolved).count();
            mutate_rows(&ui, |r| match action {
                crate::DiffBulkAction::Reset => r.decision = crate::DiffResolution::Unresolved,
                crate::DiffBulkAction::KeepAll => r.decision = crate::DiffResolution::KeepCurrent,
                crate::DiffBulkAction::AcceptSafe
                    if r.bulk_safe
                        && !r.stale
                        && r.decision == crate::DiffResolution::Unresolved =>
                {
                    r.decision = crate::DiffResolution::AcceptProposed
                }
                _ => {}
            });
            if action==crate::DiffBulkAction::AcceptSafe {ui.set_status(format!("一括採用の判断を {} 件選択しました。要確認の項目は個別に判断してください。",safe).into());}
        }
    });
}

fn review_value_label(value: &BibliographyValue) -> String {
    match value {
        BibliographyValue::Authors(authors) | BibliographyValue::Editors(authors)
            if !authors.is_empty() =>
        {
            authors
                .iter()
                .enumerate()
                .map(|(index, author)| {
                    let name = match &author.name {
                        ContributorName::Person { given, family } => {
                            format!("{} {}", given.as_deref().unwrap_or(""), family)
                        }
                        ContributorName::Organization(n) | ContributorName::Literal(n) => n.clone(),
                    };
                    format!(
                        "{}. {} / ORCID: {}",
                        index + 1,
                        name.trim(),
                        author.orcid.as_deref().unwrap_or("（未設定）")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        _ => metadata::value_label(value),
    }
}

fn refresh_visible(ui: &MainWindow) {
    use slint::Model;
    let filter = ui.get_diff_filter();
    let selected = ui
        .get_proposal_rows()
        .iter()
        .filter(|r| match filter {
            crate::DiffFilter::All => true,
            crate::DiffFilter::Changed => r.change_label != "変更なし",
            crate::DiffFilter::Conflict => r.conflict || r.stale,
            crate::DiffFilter::Unresolved => r.decision == crate::DiffResolution::Unresolved,
            crate::DiffFilter::Decided => r.decision != crate::DiffResolution::Unresolved,
        })
        .collect::<Vec<_>>();
    let visible = ui.get_filtered_proposal_rows();
    if visible.row_count() == selected.len()
        && selected
            .iter()
            .enumerate()
            .all(|(i, r)| visible.row_data(i).is_some_and(|old| old.id == r.id))
    {
        for (i, row) in selected.into_iter().enumerate() {
            if visible.row_data(i).as_ref() != Some(&row) {
                visible.set_row_data(i, row);
            }
        }
    } else {
        ui.set_filtered_proposal_rows(ModelRc::new(VecModel::from(selected)));
    }
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

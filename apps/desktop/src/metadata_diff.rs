//! Bounded presentation diff. Never consulted for adoption, freshness or identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffKind {
    Equal,
    Insert,
    Delete,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffSegment {
    pub kind: DiffKind,
    pub text: String,
}
/// LCS over words/whitespace, capped at 128 tokens and 8192 UTF-8 bytes per side.
/// Inputs beyond the bound retain side-by-side comparison instead of quadratic work.
pub fn word_diff(current: &str, proposed: &str) -> Option<Vec<DiffSegment>> {
    if current.len() > 8192 || proposed.len() > 8192 {
        return None;
    }
    let tokenize = |s: &str| {
        let mut result: Vec<String> = Vec::new();
        let mut last = None;
        for c in s.chars() {
            let kind = if c.is_whitespace() {
                0
            } else if c.is_alphanumeric() {
                1
            } else {
                2
            };
            if last == Some(kind) && kind != 2 {
                if let Some(token) = result.last_mut() {
                    token.push(c);
                }
            } else {
                result.push(c.to_string());
                last = Some(kind);
            }
            if result.len() > 128 {
                return None;
            }
        }
        Some(result)
    };
    let a = tokenize(current)?;
    let b = tokenize(proposed)?;
    let mut lengths = vec![vec![0u16; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            lengths[i][j] = if a[i] == b[j] {
                1 + lengths[i + 1][j + 1]
            } else {
                lengths[i + 1][j].max(lengths[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut result: Vec<DiffSegment> = Vec::new();
    while i < a.len() || j < b.len() {
        let (kind, text) = if i < a.len() && j < b.len() && a[i] == b[j] {
            let t = a[i].clone();
            i += 1;
            j += 1;
            (DiffKind::Equal, t)
        } else if i < a.len() && (j == b.len() || lengths[i + 1][j] >= lengths[i][j + 1]) {
            let t = a[i].clone();
            i += 1;
            (DiffKind::Delete, t)
        } else {
            let t = b[j].clone();
            j += 1;
            (DiffKind::Insert, t)
        };
        if let Some(segment) = result.last_mut().filter(|s| s.kind == kind) {
            segment.text.push_str(&text);
        } else {
            result.push(DiffSegment { kind, text });
        }
    }
    Some(result)
}
pub fn inline_text(current: &str, proposed: &str) -> String {
    match word_diff(current, proposed) {
        Some(segments) => segments
            .into_iter()
            .map(|s| match s.kind {
                DiffKind::Equal => s.text,
                DiffKind::Insert => format!("【＋{}】", s.text),
                DiffKind::Delete => format!("【−{}】", s.text),
            })
            .collect(),
        None => {
            "入力が長いため単語差分を省略しました。現在値と提案値を全文表示して比較してください。"
                .into()
        }
    }
}
pub fn structured_text(
    current: &paper_core::model::BibliographyValue,
    proposed: &paper_core::model::BibliographyValue,
) -> String {
    use paper_core::{model::BibliographyValue as V, service::compare_contributors};
    match (current,proposed) {
        (V::Authors(a),V::Authors(b)) | (V::Editors(a),V::Editors(b))=>{
            if a.len()>128 || b.len()>128 {return "著者が多数のため変更の要約を省略します。順序付きの現在値・提案値を比較してください。".into();}
            compare_contributors(a,b).iter().map(|d| {
                let status=match (d.current_index,d.proposed_index) {
                    (None,Some(n))=>format!("＋ 提案 {}: 追加",n+1),
                    (Some(n),None)=>format!("− 現在 {}: 削除",n+1),
                    (Some(old),Some(new))=>format!("現在 {} → 提案 {}: {}{}",old+1,new+1,if d.moved {"順序変更 "}else{""},if d.changed {"氏名・表記変更（同じORCID）"}else if !d.moved {"変更なし"}else{""}),
                    _=>String::new(),
                };
                if d.identity_uncertain {format!("{status} / 同一名ですが同一人物と確定できません。ORCIDの追加・変更を確認してください。")}else{status}
            }).collect::<Vec<_>>().join("\n")
        }
        (V::Identifiers(_),V::Identifiers(_))=>"識別子の変更は文献の同一性・版の関連付けに影響します。通常の項目とは別に個別判断してください。".into(),
        (V::Keywords(a),V::Keywords(b))=>{let d=paper_core::service::compare_sets(a,b);format!("追加: {}\n削除: {}",d.added.join(" / "),d.removed.join(" / "))},
        _=>String::new(),
    }
}

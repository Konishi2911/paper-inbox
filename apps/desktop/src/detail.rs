//! Presentation helpers for the read-only paper detail and its external links.
//!
//! Links are always built from validated identifiers or an explicit http(s)
//! URL and handed to the OS default browser without a shell.

use crate::application::{AuthorInput, PUBLICATION_TYPE_LABELS, PaperForm};
use paper_core::model::{ArxivId, Doi};

const SHORT_AUTHOR_COUNT: usize = 3;
const ABSTRACT_PREVIEW_CHARS: usize = 300;

fn display_name(author: &AuthorInput) -> String {
    if author.kind == 0 && !author.given.trim().is_empty() {
        format!("{} {}", author.given.trim(), author.name.trim())
    } else {
        author.name.trim().to_string()
    }
}

/// All authors in stored order.
pub fn authors_full(form: &PaperForm) -> String {
    form.authors
        .iter()
        .map(display_name)
        .collect::<Vec<_>>()
        .join(", ")
}

/// The first authors followed by the number of omitted authors.
pub fn authors_short(form: &PaperForm) -> String {
    let names: Vec<_> = form.authors.iter().map(display_name).collect();
    if names.len() <= SHORT_AUTHOR_COUNT {
        return names.join(", ");
    }
    format!(
        "{} 他{}名",
        names[..SHORT_AUTHOR_COUNT].join(", "),
        names.len() - SHORT_AUTHOR_COUNT
    )
}

/// One line such as `Journal 30(2), pp. 1–10, 2024・学術誌論文・Publisher`.
pub fn publication_summary(form: &PaperForm) -> String {
    let mut citation = Vec::new();
    let container = form.container_title.trim();
    let volume = form.volume.trim();
    let issue = form.issue.trim();
    let volume_issue = match (volume.is_empty(), issue.is_empty()) {
        (false, false) => format!("{volume}({issue})"),
        (false, true) => volume.to_string(),
        (true, false) => format!("({issue})"),
        (true, true) => String::new(),
    };
    match (container.is_empty(), volume_issue.is_empty()) {
        (false, false) => citation.push(format!("{container} {volume_issue}")),
        (false, true) => citation.push(container.to_string()),
        (true, false) => citation.push(volume_issue),
        (true, true) => {}
    }
    if !form.pages.trim().is_empty() {
        citation.push(format!("pp. {}", form.pages.trim()));
    }
    if !form.year.trim().is_empty() {
        citation.push(form.year.trim().to_string());
    }
    let mut parts = Vec::new();
    if !citation.is_empty() {
        parts.push(citation.join(", "));
    }
    if form.publication_type != 0
        && let Some(label) = PUBLICATION_TYPE_LABELS.get(form.publication_type)
    {
        parts.push((*label).to_string());
    }
    if !form.publisher.trim().is_empty() {
        parts.push(form.publisher.trim().to_string());
    }
    parts.join("・")
}

/// A preview cut on a character boundary, and whether text was omitted.
pub fn abstract_preview(text: &str) -> (String, bool) {
    let text = text.trim();
    match text.char_indices().nth(ABSTRACT_PREVIEW_CHARS) {
        Some((cut, _)) => (format!("{}…", &text[..cut]), true),
        None => (text.to_string(), false),
    }
}

/// Keywords for display, separated by a middle dot.
pub fn keywords_line(form: &PaperForm) -> String {
    form.keywords
        .split(';')
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

fn encode_path(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~/:;()".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// `https://doi.org/...` for a valid DOI; invalid stored values are not opened.
pub fn doi_url(value: &str) -> Option<String> {
    let doi = Doi::parse(value.trim()).ok()?;
    Some(format!("https://doi.org/{}", encode_path(doi.as_str())))
}

/// The official abstract page for a valid arXiv ID.
pub fn arxiv_url(value: &str) -> Option<String> {
    let id = ArxivId::parse(value.trim()).ok()?;
    Some(format!(
        "https://arxiv.org/abs/{}",
        encode_path(id.as_str())
    ))
}

/// Only absolute http(s) URLs without whitespace or control characters.
pub fn web_url(value: &str) -> Option<String> {
    let value = value.trim();
    let lower = value.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))?;
    let host_ok = rest
        .split(['/', '?', '#'])
        .next()
        .is_some_and(|host| !host.is_empty());
    (host_ok && !value.chars().any(|c| c.is_whitespace() || c.is_control()))
        .then(|| value.to_string())
}

/// Opens a link built by the functions above in the default browser.
pub fn open_in_browser(url: &str) -> Result<(), String> {
    webbrowser::open(url).map_err(|e| format!("ブラウザーで開けませんでした: {e}"))
}

/// File size for people: `340 KB`, `1.2 MB` (decimal units, one decimal above 1 MB).
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1000;
    const MB: u64 = 1000 * KB;
    const GB: u64 = 1000 * MB;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{} KB", bytes.div_ceil(KB))
    } else {
        format!("{bytes} B")
    }
}

/// The muted line under an attachment's name.
pub fn attachment_details(attachment: &paper_core::model::Attachment, missing: bool) -> String {
    if missing {
        return "ファイルが見つかりません。ライブラリのフォルダーを確認してください。".into();
    }
    format!(
        "{} · {} に追加",
        format_size(attachment.size_bytes),
        attachment.created_at.format("%Y-%m-%d")
    )
}

#[cfg(test)]
mod size_tests {
    use super::format_size;
    #[test]
    fn sizes_are_short_and_round_up_small_files() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(999), "999 B");
        assert_eq!(format_size(1001), "2 KB");
        assert_eq!(format_size(340_000), "340 KB");
        assert_eq!(format_size(1_250_000), "1.2 MB");
        assert_eq!(format_size(3_400_000_000), "3.4 GB");
    }
}

use crate::model::{Bibliography, CitationKey, ContributorName};
use std::collections::HashSet;

/// Deterministic proposal only. Saving still checks the database constraint.
pub fn suggest_citation_key(b: &Bibliography, occupied: &[String]) -> CitationKey {
    let used: HashSet<_> = occupied.iter().map(|s| s.to_ascii_lowercase()).collect();
    let author = b
        .authors
        .first()
        .map(|a| match &a.name {
            ContributorName::Person { family, .. } => family.as_str(),
            ContributorName::Organization(s) | ContributorName::Literal(s) => s.as_str(),
        })
        .unwrap_or("");
    let ascii = |s: &str, limit| {
        s.chars()
            .filter(char::is_ascii_alphanumeric)
            .take(limit)
            .collect::<String>()
    };
    let mut author = ascii(author, 64);
    if author.is_empty() {
        author = "Author".into();
    }
    let year = b
        .publication
        .year
        .map(|n| n.to_string())
        .unwrap_or_else(|| "Undated".into());
    let base = format!("{author}{year}");
    let available = |s: &str| !used.contains(&s.to_ascii_lowercase());
    if available(&base) {
        return CitationKey::parse(&base).expect("ASCII key");
    }
    let stop = [
        "a", "an", "the", "on", "of", "for", "to", "in", "and", "with", "by", "from", "at", "as",
        "is", "using",
    ];
    let word = b
        .title
        .as_deref()
        .unwrap_or("")
        .split(|c: char| !c.is_alphanumeric())
        .map(|w| ascii(w, 24))
        .find(|w| !w.is_empty() && !stop.contains(&w.to_ascii_lowercase().as_str()))
        .unwrap_or_else(|| "Work".into());
    let word = format!("{}{}", word[..1].to_ascii_uppercase(), &word[1..]);
    let expanded = format!("{base}{word}");
    if available(&expanded) {
        return CitationKey::parse(&expanded).expect("ASCII key");
    }
    for n in 2u64.. {
        let key = format!("{expanded}{n}");
        if available(&key) {
            return CitationKey::parse(&key).expect("ASCII key");
        }
    }
    unreachable!("finite library exhausts before u64")
}

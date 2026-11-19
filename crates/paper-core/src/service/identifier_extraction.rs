use crate::model::{ArxivId, Doi, PaperIdentifier};

/// Where a candidate identifier was read from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoundIn {
    /// The PDF's own metadata (information dictionary or XMP).
    DocumentInfo,
    /// The text of one of the first pages; `page` starts at 1.
    PageText { page: usize },
}

/// A DOI or arXiv ID found in a document, with what makes it likely to be the document's own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundIdentifier {
    pub identifier: PaperIdentifier,
    pub found_in: FoundIn,
    /// Higher is more likely to identify the document itself. Only the order is meaningful.
    pub score: i32,
}

/// Looks for the document's own DOI or arXiv ID.
///
/// `info` are the metadata strings of the file, `pages` the text of its first pages in order.
/// A page's text is cut at a "References" heading so that cited works are not mistaken for the
/// document itself. The result is sorted best first and has each identifier once. Nothing is
/// guessed: a string is returned only when it parses as a DOI or arXiv ID.
pub fn find_identifiers(info: &[String], pages: &[String]) -> Vec<FoundIdentifier> {
    let mut found: Vec<FoundIdentifier> = Vec::new();
    for text in info {
        collect(text, FoundIn::DocumentInfo, 20, &mut found);
    }
    for (index, text) in pages.iter().enumerate() {
        let page = index + 1;
        let own = before_references(text);
        // Earlier pages are more likely to carry the document's own identifier.
        let penalty = -5 * i32::try_from(index).unwrap_or(i32::MAX / 10);
        collect(own, FoundIn::PageText { page }, penalty, &mut found);
    }
    found.sort_by_key(|f| std::cmp::Reverse(f.score));
    found
}

fn collect(text: &str, found_in: FoundIn, bonus: i32, out: &mut Vec<FoundIdentifier>) {
    let text = normalize(text);
    let mut candidates = find_dois(&text);
    candidates.extend(find_arxiv_ids(&text));
    for (identifier, labeled) in candidates {
        let mut score = 10 + bonus + if labeled { 30 } else { 0 };
        if matches!(identifier, PaperIdentifier::Doi(_)) {
            // The version of record is preferred over a preprint of the same work.
            score += 5;
        }
        match out.iter_mut().find(|f| f.identifier == identifier) {
            // The same identifier again (a header and a footer, say) makes it more likely.
            Some(existing) => existing.score += 3 + if labeled { 5 } else { 0 },
            None => out.push(FoundIdentifier {
                identifier,
                found_in,
                score,
            }),
        }
    }
}

/// Dashes that extraction produces for a hyphen become ASCII, and the text is split into lines
/// that are rejoined only where a DOI is cut.
fn normalize(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\u{2010}'..='\u{2015}' | '\u{2212}' => '-',
            '\u{00a0}' | '\u{200b}' | '\u{feff}' => ' ',
            c => c,
        })
        .collect()
}

/// The part of a page before a line that is only "References" (or similar).
fn before_references(text: &str) -> &str {
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let heading = line
            .trim()
            .trim_matches(|c: char| c.is_ascii_digit() || c == '.' || c.is_whitespace())
            .to_ascii_lowercase();
        if matches!(
            heading.as_str(),
            "references" | "bibliography" | "literature cited" | "参考文献" | "文献"
        ) {
            return &text[..offset];
        }
        offset += line.len();
    }
    text
}

fn is_doi_stop(byte: u8) -> bool {
    byte.is_ascii_whitespace()
        || byte >= 0x80
        || matches!(
            byte,
            b'"' | b'<' | b'>' | b'|' | b'\\' | b'^' | b'`' | b'{' | b'}'
        )
}

fn find_dois(text: &str) -> Vec<(PaperIdentifier, bool)> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(at) = text[from..].find("10.") {
        let start = from + at;
        from = start + 3;
        if start > 0 && bytes[start - 1].is_ascii_alphanumeric() {
            continue;
        }
        let mut end = start;
        while end < bytes.len() && !is_doi_stop(bytes[end]) {
            end += 1;
        }
        let mut token = text[start..end].to_owned();
        // A DOI cut at the end of a line continues on the next one.
        let mut next = end;
        while (token.ends_with('-') || token.ends_with('/'))
            && bytes.get(next).is_some_and(|b| *b == b'\n' || *b == b'\r')
        {
            let rest = text[next..].trim_start_matches(['\r', '\n', ' ']);
            let len = rest.bytes().take_while(|b| !is_doi_stop(*b)).count();
            if len == 0 {
                break;
            }
            token.push_str(&rest[..len]);
            next = text.len() - rest.len() + len;
        }
        let trimmed = trim_doi_tail(&token);
        if has_registrant_code(trimmed)
            && let Ok(doi) = Doi::parse(trimmed)
        {
            let before = text[..start].to_ascii_lowercase();
            let near = before
                .chars()
                .rev()
                .take(14)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>();
            out.push((PaperIdentifier::Doi(doi), near.contains("doi")));
        }
    }
    out
}

/// Registrant codes are four to nine digits (optionally with dotted sub-codes); shorter ones are
/// almost always something else, such as a number in a table.
fn has_registrant_code(token: &str) -> bool {
    token
        .strip_prefix("10.")
        .and_then(|rest| rest.split_once('/'))
        .is_some_and(|(code, _)| {
            let main = code.split('.').next().unwrap_or("");
            (4..=9).contains(&main.len())
        })
}

/// Removes punctuation that belongs to the sentence around a DOI, keeping balanced brackets.
fn trim_doi_tail(token: &str) -> &str {
    let mut end = token.len();
    loop {
        let current = &token[..end];
        let Some(last) = current.chars().last() else {
            break;
        };
        let strip = match last {
            '.' | ',' | ';' | ':' | '\'' | '!' | '?' | '*' => true,
            ')' => current.matches('(').count() < current.matches(')').count(),
            ']' => current.matches('[').count() < current.matches(']').count(),
            _ => false,
        };
        if !strip {
            break;
        }
        end -= last.len_utf8();
    }
    &token[..end]
}

fn find_arxiv_ids(text: &str) -> Vec<(PaperIdentifier, bool)> {
    let lower = text.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(at) = lower[from..].find("arxiv") {
        let after = from + at + 5;
        from = after;
        let rest = &text[after..];
        // "arXiv:2401.01234", "arXiv 2401.01234", "arxiv.org/abs/2401.01234".
        let rest = rest
            .strip_prefix(".org")
            .map(|r| {
                r.strip_prefix("/abs/")
                    .or_else(|| r.strip_prefix("/pdf/"))
                    .unwrap_or(r)
            })
            .unwrap_or(rest)
            .trim_start_matches([':', ' ']);
        let token: String = rest
            .bytes()
            .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'/' | b'-'))
            .map(char::from)
            .collect();
        let token = token.trim_end_matches(['.', '-', '/']);
        // The stamp is followed by a category such as "[cs.LG]"; the pattern check below keeps
        // only a well-formed identifier, so a year or page number after the word is not taken.
        if looks_like_arxiv(token)
            && let Ok(id) = ArxivId::parse(token)
        {
            out.push((PaperIdentifier::Arxiv(id), true));
        }
    }
    out
}

fn looks_like_arxiv(token: &str) -> bool {
    let (body, version) = match token.rsplit_once('v') {
        Some((body, version))
            if !version.is_empty() && version.bytes().all(|b| b.is_ascii_digit()) =>
        {
            (body, Some(version))
        }
        _ => (token, None),
    };
    let _ = version;
    // Old style first: archive[.XX]/YYMMNNN. Its subject class contains a period (math.GT), so
    // the slash, not the period, tells the two styles apart.
    if let Some((archive, number)) = body.split_once('/') {
        return !archive.is_empty()
            && archive
                .bytes()
                .all(|b| b.is_ascii_alphabetic() || matches!(b, b'-' | b'.'))
            && number.len() == 7
            && number.bytes().all(|b| b.is_ascii_digit());
    }
    // New style: YYMM.NNNN or YYMM.NNNNN.
    body.split_once('.').is_some_and(|(year_month, number)| {
        year_month.len() == 4
            && year_month.bytes().all(|b| b.is_ascii_digit())
            && (4..=5).contains(&number.len())
            && number.bytes().all(|b| b.is_ascii_digit())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(found: &[FoundIdentifier]) -> Vec<String> {
        found
            .iter()
            .map(|f| match &f.identifier {
                PaperIdentifier::Doi(d) => format!("doi:{}", d.as_str()),
                PaperIdentifier::Arxiv(a) => format!("arxiv:{}", a.as_str()),
                PaperIdentifier::Url(u) => u.clone(),
            })
            .collect()
    }
    fn pages(texts: &[&str]) -> Vec<String> {
        texts.iter().map(|t| (*t).to_owned()).collect()
    }

    #[test]
    fn finds_a_labeled_doi_and_drops_sentence_punctuation() {
        for text in [
            "Published online. DOI: 10.1038/s41586-020-2649-2.",
            "See https://doi.org/10.1038/s41586-020-2649-2, for details",
            "(doi:10.1038/s41586-020-2649-2)",
            "doi 10.1038/s41586-020-2649-2;",
        ] {
            let found = find_identifiers(&[], &pages(&[text]));
            assert_eq!(ids(&found), ["doi:10.1038/s41586-020-2649-2"], "{text}");
        }
    }

    #[test]
    fn keeps_brackets_that_belong_to_the_doi() {
        let found = find_identifiers(
            &[],
            &pages(&["DOI 10.1002/(SICI)1097-4571(199806)49:8<693::AID-ASI4>3.0.CO;2-O"]),
        );
        // '<' ends the token in extracted text, so only the safe part before it is kept.
        assert_eq!(ids(&found), ["doi:10.1002/(sici)1097-4571(199806)49:8"]);
    }

    #[test]
    fn joins_a_doi_cut_at_the_end_of_a_line() {
        let found = find_identifiers(
            &[],
            &pages(&["https://doi.org/10.1038/s41586-\n020-2649-2\nNext line"]),
        );
        assert_eq!(ids(&found), ["doi:10.1038/s41586-020-2649-2"]);
        let found = find_identifiers(&[], &pages(&["DOI: 10.1038/\ns41586-020-2649-2"]));
        assert_eq!(ids(&found), ["doi:10.1038/s41586-020-2649-2"]);
    }

    #[test]
    fn unicode_hyphens_become_ascii() {
        let found = find_identifiers(
            &[],
            &pages(&["doi: 10.1038/s41586\u{2010}020\u{2013}2649\u{2212}2"]),
        );
        assert_eq!(ids(&found), ["doi:10.1038/s41586-020-2649-2"]);
    }

    #[test]
    fn ignores_numbers_that_only_look_like_a_doi_prefix() {
        for text in [
            "Version 110.5 of the model",
            "a 2010.1234 measurement and 1.10.1000 values",
            "x10.1038/not-after-a-letter",
            "10.123/too-short-registrant",
        ] {
            assert!(find_identifiers(&[], &pages(&[text])).is_empty(), "{text}");
        }
    }

    #[test]
    fn finds_new_and_old_arxiv_ids_only_after_the_word_arxiv() {
        let found = find_identifiers(&[], &pages(&["arXiv:2401.01234v2 [cs.LG] 2 Jan 2024"]));
        assert_eq!(ids(&found), ["arxiv:2401.01234v2"]);
        let found = find_identifiers(&[], &pages(&["see arxiv.org/abs/hep-th/9901001v1."]));
        assert_eq!(ids(&found), ["arxiv:hep-th/9901001v1"]);
        // An old-style archive may carry a subject class with a period: math.GT/0309136.
        let found = find_identifiers(&[], &pages(&["arXiv:math.GT/0309136 [math.GT] 8 Sep 2003"]));
        assert_eq!(ids(&found), ["arxiv:math.GT/0309136"]);
        let found = find_identifiers(&[], &pages(&["Page 2401.01234 of 5 and arXiv 2024"]));
        assert!(found.is_empty(), "{found:?}");
    }

    #[test]
    fn references_are_not_the_document_itself() {
        let text =
            "Title\nDOI: 10.1000/own\nAbstract\n\nReferences\n[1] A. Author. doi:10.1000/cited\n";
        let found = find_identifiers(&[], &pages(&[text]));
        assert_eq!(ids(&found), ["doi:10.1000/own"]);
        // A reference list heading with a number or a trailing dot is recognised too.
        let found = find_identifiers(
            &[],
            &pages(&["doi:10.1000/own\n7. References.\ndoi:10.1000/cited"]),
        );
        assert_eq!(ids(&found), ["doi:10.1000/own"]);
    }

    #[test]
    fn ranks_labeled_metadata_and_repeated_and_earlier_ones_first() {
        let info = vec!["10.1000/from-info".to_owned()];
        let found = find_identifiers(
            &info,
            &pages(&[
                "doi:10.1000/header and again doi:10.1000/header",
                "see 10.1000/stray on page two",
            ]),
        );
        assert_eq!(
            ids(&found),
            [
                "doi:10.1000/header",
                "doi:10.1000/from-info",
                "doi:10.1000/stray"
            ]
        );
        assert_eq!(found[1].found_in, FoundIn::DocumentInfo);
        assert_eq!(found[0].found_in, FoundIn::PageText { page: 1 });
    }

    #[test]
    fn a_doi_is_ranked_above_an_arxiv_id_of_equal_standing() {
        let found = find_identifiers(&[], &pages(&["arXiv:2401.01234v1\nDOI 10.1000/published"]));
        assert_eq!(ids(&found), ["doi:10.1000/published", "arxiv:2401.01234v1"]);
    }

    #[test]
    fn text_without_identifiers_and_unusual_input_are_harmless() {
        assert!(find_identifiers(&[], &[]).is_empty());
        assert!(
            find_identifiers(&["".into()], &pages(&["", "日本語のみ。10. ", "10."])).is_empty()
        );
        // Multibyte text right after the prefix must not split a character.
        assert!(find_identifiers(&[], &pages(&["10.1000/日本語"])).len() <= 1);
    }
}

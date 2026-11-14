use crate::model::*;
use std::collections::HashSet;
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct BibtexOptions {
    pub include_abstract: bool,
    pub include_doi: bool,
    pub include_url: bool,
    pub include_keywords: bool,
}
impl Default for BibtexOptions {
    fn default() -> Self {
        Self {
            include_abstract: false,
            include_doi: true,
            include_url: true,
            include_keywords: false,
        }
    }
}
#[derive(Debug, Error)]
pub enum BibtexError {
    #[error("Duplicate citation key: {0}")]
    DuplicateCitationKey(CitationKey),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BibtexWarning {
    MissingRequiredField {
        paper_id: PaperId,
        field: &'static str,
    },
    FallbackEntryType {
        paper_id: PaperId,
    },
    InvalidStoredIdentifier {
        paper_id: PaperId,
        field: &'static str,
    },
    EmptyContributor {
        paper_id: PaperId,
        role: &'static str,
    },
}
#[derive(Clone, Debug)]
pub struct BibtexOutput {
    pub content: String,
    pub warnings: Vec<BibtexWarning>,
}

// Metadata is plain text, never executable TeX supplied by an external source.
// Balanced command groups also keep braces from changing BibTeX's field structure.
fn escape(text: &str) -> String {
    let mut out = String::new();
    for (i, word) in text.split_whitespace().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        for c in word.chars() {
            match c {
                '\\' => out.push_str("{\\textbackslash{}}"),
                '{' => out.push_str("{\\textbraceleft{}}"),
                '}' => out.push_str("{\\textbraceright{}}"),
                '&' | '%' | '$' | '#' | '_' => {
                    out.push('\\');
                    out.push(c);
                }
                '~' => out.push_str("{\\textasciitilde{}}"),
                '^' => out.push_str("{\\textasciicircum{}}"),
                c if c.is_control() => {}
                c => out.push(c),
            }
        }
    }
    out
}
fn names(
    people: &[Contributor],
    paper_id: PaperId,
    role: &'static str,
    warnings: &mut Vec<BibtexWarning>,
) -> String {
    let mut names = vec![];
    for c in people {
        let name = match &c.name {
            ContributorName::Person { given, family } if !family.trim().is_empty() => {
                let family = format!("{{{}}}", escape(family));
                match given.as_deref().filter(|s| !s.trim().is_empty()) {
                    Some(given) => format!("{family}, {{{}}}", escape(given)),
                    None => family,
                }
            }
            ContributorName::Organization(name) | ContributorName::Literal(name)
                if !name.trim().is_empty() =>
            {
                format!("{{{}}}", escape(name))
            }
            _ => {
                warnings.push(BibtexWarning::EmptyContributor { paper_id, role });
                continue;
            }
        };
        names.push(name);
    }
    names.join(" and ")
}
pub fn render_entry(paper: &Paper, options: &BibtexOptions) -> BibtexOutput {
    let b = paper.bibliography();
    let p = &b.publication;
    let ids = &b.identifiers;
    let id = paper.id();
    let mut warnings = vec![];
    let (kind, required): (&str, &[&str]) = match p.kind {
        PublicationType::JournalArticle => ("article", &["author", "title", "journal", "year"]),
        PublicationType::ConferencePaper => {
            ("inproceedings", &["author", "title", "booktitle", "year"])
        }
        PublicationType::Book => ("book", &["author/editor", "title", "publisher", "year"]),
        PublicationType::BookChapter => (
            "incollection",
            &["author", "title", "booktitle", "publisher", "year"],
        ),
        PublicationType::MastersThesis => ("mastersthesis", &["author", "title", "school", "year"]),
        PublicationType::DoctoralThesis => ("phdthesis", &["author", "title", "school", "year"]),
        PublicationType::TechnicalReport => {
            ("techreport", &["author", "title", "institution", "year"])
        }
        PublicationType::Preprint => ("unpublished", &["author", "title", "note"]),
        PublicationType::Thesis
        | PublicationType::Patent
        | PublicationType::Other
        | PublicationType::Unknown => {
            warnings.push(BibtexWarning::FallbackEntryType { paper_id: id });
            ("misc", &[])
        }
    };
    let mut fields: Vec<(&str, String)> = vec![];
    let author = names(&b.authors, id, "author", &mut warnings);
    let editor = names(&b.editors, id, "editor", &mut warnings);
    if !author.is_empty() {
        fields.push(("author", author));
    }
    if !editor.is_empty() {
        fields.push(("editor", editor));
    }
    if let Some(title) = b.title.as_deref().filter(|s| !s.trim().is_empty()) {
        fields.push(("title", format!("{{{}}}", escape(title))));
    }
    let mut add = |name, text: Option<&str>| {
        if let Some(text) = text.filter(|s| !s.trim().is_empty()) {
            fields.push((name, escape(text)));
        }
    };
    let container = if p.kind == PublicationType::JournalArticle {
        "journal"
    } else if matches!(
        p.kind,
        PublicationType::ConferencePaper | PublicationType::BookChapter
    ) {
        "booktitle"
    } else {
        "howpublished"
    };
    add(container, p.container_title.as_deref());
    add("publisher", p.publisher.as_deref());
    add("volume", p.volume.as_deref());
    add(
        if p.kind == PublicationType::JournalArticle {
            "number"
        } else {
            "issue"
        },
        p.issue.as_deref(),
    );
    add("pages", p.pages.as_deref());
    add("eid", p.article_number.as_deref());
    add("isbn", ids.isbn.as_deref());
    add("issn", ids.issn.as_deref());
    if options.include_doi {
        add("doi", ids.doi.as_ref().map(Doi::as_str));
    }
    if options.include_url {
        add("url", ids.url.as_deref());
    }
    if options.include_abstract {
        add("abstract", b.abstract_text.as_deref());
    }
    if let Some(arxiv) = &ids.arxiv_id {
        fields.push(("archivePrefix", "arXiv".into()));
        fields.push(("eprint", escape(arxiv.as_str())));
    }
    if ids.raw_doi.is_some() {
        warnings.push(BibtexWarning::InvalidStoredIdentifier {
            paper_id: id,
            field: "doi",
        });
    }
    if ids.raw_arxiv_id.is_some() {
        warnings.push(BibtexWarning::InvalidStoredIdentifier {
            paper_id: id,
            field: "arxiv_id",
        });
    }
    if let Some(year) = p.year {
        fields.push(("year", year.to_string()));
    }
    if let Some(month) = p.month {
        fields.push(("month", month.to_string()));
    }
    if p.kind == PublicationType::Preprint {
        fields.push(("note", "Preprint".into()));
    }
    if options.include_keywords && !b.keywords.is_empty() {
        fields.push(("keywords", escape(&b.keywords.join(", "))));
    }
    for &required in required {
        let present = if required == "author/editor" {
            fields.iter().any(|(k, _)| *k == "author" || *k == "editor")
        } else {
            fields.iter().any(|(k, _)| *k == required)
        };
        if !present {
            warnings.push(BibtexWarning::MissingRequiredField {
                paper_id: id,
                field: required,
            });
        }
    }
    let mut content = format!("@{kind}{{{},\n", paper.citation_key());
    for (name, value) in fields {
        content.push_str(&format!("  {name} = {{{value}}},\n"));
    }
    content.push_str("}\n");
    BibtexOutput { content, warnings }
}
pub fn export_bibtex(
    papers: &[Paper],
    options: &BibtexOptions,
) -> Result<BibtexOutput, BibtexError> {
    let mut seen = HashSet::new();
    for paper in papers {
        if !seen.insert(paper.citation_key().as_str().to_ascii_lowercase()) {
            return Err(BibtexError::DuplicateCitationKey(
                paper.citation_key().clone(),
            ));
        }
    }
    let mut content = String::new();
    let mut warnings = vec![];
    for paper in papers {
        let output = render_entry(paper, options);
        if !content.is_empty() {
            content.push('\n');
        }
        content.push_str(&output.content);
        warnings.extend(output.warnings);
    }
    Ok(BibtexOutput { content, warnings })
}

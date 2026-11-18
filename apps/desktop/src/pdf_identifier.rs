use crate::application::AppError;
use lopdf::{Document, LoadOptions, Object};
use paper_core::service::{FoundIdentifier, find_identifiers};
use std::{path::Path, sync::mpsc, thread, time::Duration};

/// Files larger than this are not read for identifiers (the whole file is loaded into memory).
const MAX_SCAN_BYTES: u64 = 128 * 1024 * 1024;
/// How many pages from the start are searched; a document's own identifier is on the first ones.
const SCANNED_PAGES: usize = 3;
/// Bound on what one page or stream may inflate to while reading.
const MAX_INFLATED_BYTES: usize = 32 * 1024 * 1024;
/// A damaged or hostile file must not keep the add screen waiting.
const SCAN_TIMEOUT: Duration = Duration::from_secs(15);

/// Reads the PDF's metadata and the text of its first pages and returns the DOIs and arXiv IDs
/// found, best first. Nothing leaves the computer. The file is parsed on its own thread, so a
/// panic or a stall inside the parser only fails this lookup; the thread is left to finish by
/// itself after a timeout.
pub fn scan_pdf(path: &Path) -> Result<Vec<FoundIdentifier>, AppError> {
    let size = std::fs::metadata(path)
        .map_err(|_| AppError::Input("PDFを読み込めません。".into()))?
        .len();
    if size > MAX_SCAN_BYTES {
        return Err(AppError::Input(
            "PDFが大きいため、識別子を探せません。識別子は手で入力してください。".into(),
        ));
    }
    let bytes = std::fs::read(path).map_err(|_| AppError::Input("PDFを読み込めません。".into()))?;
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(read_texts(&bytes));
    });
    match receiver.recv_timeout(SCAN_TIMEOUT) {
        Ok(Ok((info, pages))) => Ok(find_identifiers(&info, &pages)),
        Ok(Err(message)) => Err(AppError::Input(message.into())),
        Err(_) => Err(AppError::Input(
            "PDFの解析を完了できませんでした。識別子は手で入力してください。".into(),
        )),
    }
}

/// The metadata strings and the text of the first pages.
fn read_texts(bytes: &[u8]) -> Result<(Vec<String>, Vec<String>), &'static str> {
    let options = LoadOptions {
        max_decompressed_size: Some(MAX_INFLATED_BYTES),
        ..LoadOptions::default()
    };
    let document = Document::load_mem_with_options(bytes, options)
        .map_err(|_| "PDFを読み取れませんでした。識別子は手で入力してください。")?;
    if document.is_encrypted() {
        return Err("暗号化されたPDFのため、識別子を探せません。");
    }
    let mut info = Vec::new();
    collect_info(&document, &mut info);
    let pages = document
        .get_pages()
        .keys()
        .take(SCANNED_PAGES)
        .map(|number| {
            // A page that cannot be read (an unusual font, say) is skipped, not fatal.
            document
                .extract_text_with_limit(&[*number], MAX_INFLATED_BYTES)
                .unwrap_or_default()
        })
        .collect();
    Ok((info, pages))
}

fn collect_info(document: &Document, out: &mut Vec<String>) {
    // The information dictionary: every string value, since publishers use custom keys.
    if let Ok(reference) = document.trailer.get(b"Info")
        && let Ok((_, Object::Dictionary(info))) = document.dereference(reference)
    {
        for (_, value) in info.iter() {
            if let Object::String(raw, _) = value {
                out.push(decode_pdf_string(raw));
            }
        }
    }
    // The XMP packet is XML text; the identifier patterns stop at its markup.
    if let Ok(catalog) = document.catalog()
        && let Ok(metadata) = catalog.get(b"Metadata")
        && let Ok((_, Object::Stream(stream))) = document.dereference(metadata)
        && let Ok(content) = stream.decompressed_content_with_limit(MAX_INFLATED_BYTES)
    {
        out.push(String::from_utf8_lossy(&content).into_owned());
    }
}

/// A PDF text string is UTF-16BE with a byte order mark, or an 8-bit encoding close to Latin-1.
fn decode_pdf_string(raw: &[u8]) -> String {
    match raw {
        [0xfe, 0xff, rest @ ..] => {
            let units: Vec<u16> = rest
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u16::from_be_bytes(*pair))
                .collect();
            String::from_utf16_lossy(&units)
        }
        [0xef, 0xbb, 0xbf, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        _ => raw.iter().map(|b| char::from(*b)).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{Dictionary, Stream, StringFormat, dictionary};
    use paper_core::model::PaperIdentifier;

    /// A one-page PDF whose text is `text`, with optional Info entries.
    fn pdf(page_text: &[&str], info: &[(&str, &str)]) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font = doc.add_object(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
        });
        let resources = doc.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font },
        });
        let mut kids = Vec::new();
        for text in page_text {
            let content = format!("BT /F1 12 Tf 72 720 Td ({text}) Tj ET");
            let content_id = doc.add_object(Stream::new(Dictionary::new(), content.into_bytes()));
            let page = doc.add_object(dictionary! {
                "Type" => "Page", "Parent" => pages_id, "Contents" => content_id,
                "Resources" => resources, "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            });
            kids.push(Object::Reference(page));
        }
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages", "Kids" => kids.clone(), "Count" => kids.len() as i64,
            }),
        );
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog);
        if !info.is_empty() {
            let mut dictionary = Dictionary::new();
            for (key, value) in info {
                dictionary.set(
                    *key,
                    Object::String(value.as_bytes().to_vec(), StringFormat::Literal),
                );
            }
            let info_id = doc.add_object(dictionary);
            doc.trailer.set("Info", info_id);
        }
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }
    fn write(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("paper-pdf-id-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }
    fn names(found: &[FoundIdentifier]) -> Vec<String> {
        found
            .iter()
            .map(|f| match &f.identifier {
                PaperIdentifier::Doi(d) => format!("doi:{}", d.as_str()),
                PaperIdentifier::Arxiv(a) => format!("arxiv:{}", a.as_str()),
                PaperIdentifier::Url(u) => u.clone(),
            })
            .collect()
    }

    #[test]
    fn reads_the_doi_from_the_page_text_and_the_arxiv_id_from_the_stamp() {
        let path = write(
            "paper.pdf",
            &pdf(
                &["Title DOI: 10.1000/page-doi", "arXiv:2401.01234v2 [cs.LG]"],
                &[],
            ),
        );
        let found = scan_pdf(&path).unwrap();
        assert_eq!(
            names(&found),
            ["doi:10.1000/page-doi", "arxiv:2401.01234v2"]
        );
    }

    #[test]
    fn reads_identifiers_from_the_information_dictionary() {
        let path = write(
            "info.pdf",
            &pdf(
                &["No identifier in the text"],
                &[("Subject", "doi:10.1000/from-info")],
            ),
        );
        let found = scan_pdf(&path).unwrap();
        assert_eq!(names(&found), ["doi:10.1000/from-info"]);
    }

    #[test]
    fn a_pdf_without_identifiers_gives_an_empty_result_not_an_error() {
        let path = write("plain.pdf", &pdf(&["Nothing to find here"], &[]));
        assert!(scan_pdf(&path).unwrap().is_empty());
    }

    #[test]
    fn only_the_first_pages_are_searched() {
        let path = write(
            "long.pdf",
            &pdf(&["one", "two", "three", "doi:10.1000/too-late"], &[]),
        );
        assert!(scan_pdf(&path).unwrap().is_empty());
    }

    #[test]
    fn unreadable_input_is_an_error_with_a_reason() {
        let path = write("broken.pdf", b"%PDF-1.7 this is not a document");
        assert!(matches!(scan_pdf(&path), Err(AppError::Input(_))));
        let missing = path.with_file_name("missing.pdf");
        assert!(matches!(scan_pdf(&missing), Err(AppError::Input(_))));
    }

    #[test]
    fn pdf_strings_are_decoded_from_utf16_and_latin1() {
        assert_eq!(
            decode_pdf_string(&[0xfe, 0xff, 0x00, 0x41, 0x00, 0x42]),
            "AB"
        );
        assert_eq!(decode_pdf_string(b"caf\xe9"), "café");
    }
}

use i_slint_backend_testing::ElementHandle;
use paper_desktop::{AttachmentRow, DetailView, LinkTarget, MainWindow};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};

fn find(w: &MainWindow, label: &str) -> Option<ElementHandle> {
    ElementHandle::find_by_accessible_label(w, label).next()
}
fn element(w: &MainWindow, label: &str) -> ElementHandle {
    find(w, label).unwrap_or_else(|| panic!("missing {label}"))
}
fn set_identifiers(w: &MainWindow, doi: &str, arxiv: &str, url: &str) {
    w.set_doi(doi.into());
    w.set_arxiv_id(arxiv.into());
    w.set_url(url.into());
}

#[test]
fn the_action_row_opens_the_best_link_first_keeps_the_rest_in_a_menu_and_reflows() {
    i_slint_backend_testing::init_no_event_loop();
    let w = MainWindow::new().unwrap();
    w.set_ready(true);
    w.set_busy(false);
    w.set_citation_key("Smith2024".into());
    w.set_paper_bibtex("@article{Smith2024}".into());
    w.set_detail_view(DetailView::PaperDetail);
    let opened = Rc::new(RefCell::new(Vec::new()));
    let sink = opened.clone();
    w.on_open_link(move |target| sink.borrow_mut().push(target));
    w.show().unwrap();
    w.window().set_size(slint::LogicalSize::new(1360., 900.));

    // No identifiers and no PDF: only attaching, BibTeX and the key are offered.
    set_identifiers(&w, "", "", "");
    assert!(find(&w, "DOIを開く").is_none());
    assert!(find(&w, "arXivを開く").is_none());
    assert!(find(&w, "URLを開く").is_none());
    assert!(find(&w, "その他の操作").is_none());
    assert!(find(&w, "PDFを開く").is_none());
    assert_eq!(element(&w, "PDFを添付…").accessible_enabled(), Some(true));
    assert_eq!(
        element(&w, "BibTeX をコピー").accessible_enabled(),
        Some(true)
    );
    w.set_paper_bibtex("".into());
    assert_eq!(
        element(&w, "BibTeX をコピー").accessible_enabled(),
        Some(false)
    );
    w.set_paper_bibtex("@article{Smith2024}".into());

    // A DOI wins over arXiv and URL; the others are one menu away, with their values.
    set_identifiers(&w, "10.1000/a", "2401.12345", "https://example.test/p");
    element(&w, "DOIを開く").invoke_accessible_default_action();
    assert_eq!(*opened.borrow(), vec![LinkTarget::Doi]);
    assert!(
        find(&w, "arXivを開く").is_none(),
        "closed menu has no items"
    );
    element(&w, "その他の操作").invoke_accessible_default_action();
    for label in [
        "DOIをコピー",
        "arXivを開く",
        "arXiv IDをコピー",
        "URLを開く",
        "URLをコピー",
    ] {
        assert!(find(&w, label).is_some(), "menu item {label}");
    }
    element(&w, "arXivを開く").invoke_accessible_default_action();
    assert_eq!(opened.borrow().last(), Some(&LinkTarget::Arxiv));

    // Without a DOI, arXiv is the one-click link and is not repeated in the menu.
    set_identifiers(&w, "", "2401.12345", "https://example.test/p");
    assert!(find(&w, "DOIを開く").is_none());
    element(&w, "arXivを開く").invoke_accessible_default_action();
    assert_eq!(opened.borrow().last(), Some(&LinkTarget::Arxiv));
    element(&w, "その他の操作").invoke_accessible_default_action();
    assert!(find(&w, "DOIをコピー").is_none());
    assert!(find(&w, "arXiv IDをコピー").is_some());
    assert!(find(&w, "URLを開く").is_some());

    // With only a URL it is the one-click link.
    set_identifiers(&w, "", "", "https://example.test/p");
    element(&w, "URLを開く").invoke_accessible_default_action();
    assert_eq!(opened.borrow().last(), Some(&LinkTarget::Url));

    // A PDF turns the first button from "attach" into "open"; with none openable it is disabled.
    let row = |id: &str, missing: bool| AttachmentRow {
        id: id.into(),
        name: format!("{id}.pdf").into(),
        details: "1 KB".into(),
        missing,
    };
    w.set_attachment_rows(ModelRc::new(VecModel::from(vec![row("a", true)])));
    assert!(find(&w, "PDFを添付…").is_none());
    assert_eq!(
        element(&w, "PDFを開く").accessible_enabled(),
        Some(false),
        "every file is missing"
    );
    w.set_primary_attachment("a".into());
    assert_eq!(element(&w, "PDFを開く").accessible_enabled(), Some(true));

    // One row when the pane is wide, two rows when it is narrow; nothing leaves the window.
    set_identifiers(&w, "10.1000/a", "", "");
    let top = |label: &str| element(&w, label).absolute_position().y;
    assert!((top("PDFを開く") - top("BibTeX をコピー")).abs() < 2.);
    w.window().set_size(slint::LogicalSize::new(1080., 680.));
    assert!(top("BibTeX をコピー") > top("PDFを開く") + 10.);
    for label in [
        "PDFを開く",
        "DOIを開く",
        "その他の操作",
        "BibTeX をコピー",
        "引用キーをコピー",
    ] {
        let control = element(&w, label);
        assert!(
            control.absolute_position().x + control.size().width <= 1080.,
            "{label} reaches x={}",
            control.absolute_position().x + control.size().width
        );
    }
    w.hide().unwrap();
}

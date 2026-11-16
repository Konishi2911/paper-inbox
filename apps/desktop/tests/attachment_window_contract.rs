use chrono::Utc;
use i_slint_backend_testing::ElementHandle;
use paper_core::{
    command::LibraryCommand,
    model::{Bibliography, CitationKey, Paper, PaperId},
    ports::PaperRepository,
};
use paper_desktop::{DetailView, MainWindow, NavigationAction, config::Config, ui};
use paper_storage::SqliteLibrary;
use slint::{ComponentHandle, Model};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};

fn wait(window: &MainWindow) {
    let weak = window.as_weak();
    let start = Instant::now();
    let timer = slint::Timer::default();
    let completed = Rc::new(std::cell::Cell::new(false));
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(5),
        move || {
            let window = weak.upgrade().unwrap();
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "{}",
                window.get_status()
            );
            if !window.get_busy() && !completed.replace(true) {
                slint::quit_event_loop().unwrap();
            }
        },
    );
    slint::run_event_loop_until_quit().unwrap();
}
fn element(w: &MainWindow, label: &str) -> ElementHandle {
    let found: Vec<_> = ElementHandle::find_by_accessible_label(w, label).collect();
    assert_eq!(found.len(), 1, "unique visible control: {label}");
    found.into_iter().next().unwrap()
}
fn activate(w: &MainWindow, label: &str) {
    element(w, label).invoke_accessible_default_action();
}
fn absent(w: &MainWindow, label: &str) -> bool {
    ElementHandle::find_by_accessible_label(w, label)
        .next()
        .is_none()
}
fn stored(db: &Path, id: PaperId) -> usize {
    SqliteLibrary::open_local(db)
        .unwrap()
        .attachments(id)
        .unwrap()
        .len()
}
fn stored_files(files: &Path) -> usize {
    paper_storage::FileBlobStore::open(files)
        .unwrap()
        .hashes()
        .unwrap()
        .len()
}

struct Sandbox(PathBuf);
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn pdfs_are_attached_listed_protected_removed_and_reclaimed_from_the_window() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let sandbox = Sandbox(std::env::temp_dir().join(format!("paper-inbox-ui-{}", PaperId::new())));
    std::fs::create_dir_all(&sandbox.0).unwrap();
    let database = sandbox.0.join("library.sqlite");
    let files = sandbox.0.join("library-files");

    let paper = Paper::new(
        PaperId::new(),
        CitationKey::parse("Smith2026").unwrap(),
        Bibliography {
            title: Some("Attached".into()),
            ..Default::default()
        },
        Utc::now(),
    )
    .unwrap();
    let id = paper.id();
    SqliteLibrary::open_local(&database)
        .unwrap()
        .commit_at(LibraryCommand::CreatePaper { paper }, Utc::now())
        .unwrap();

    let good = sandbox.0.join("results.pdf");
    std::fs::write(&good, "%PDF-1.7\nsome pages").unwrap();
    let not_pdf = sandbox.0.join("notes.pdf");
    std::fs::write(&not_pdf, "plain text").unwrap();
    // What the file chooser returns next; `None` is the user cancelling.
    let choices: Rc<RefCell<Vec<Option<PathBuf>>>> = Rc::new(RefCell::new(vec![]));
    let queue = choices.clone();

    let w = MainWindow::new().unwrap();
    w.show().unwrap();
    w.window().set_size(slint::LogicalSize::new(1360., 900.));
    let controller = ui::attach_with_providers(
        &w,
        Config {
            database: database.clone(),
        },
        Box::new(|_| Ok(vec![])),
        Box::new(move || queue.borrow_mut().pop().flatten()),
    );
    wait(&w);
    w.invoke_request_navigation(NavigationAction::OpenPaper, id.to_string().into());
    wait(&w);
    assert_eq!(w.get_detail_view(), DetailView::PaperDetail);
    assert_eq!(w.get_attachment_rows().row_count(), 0);
    element(&w, "PDFは添付されていません。");

    // Cancelling the chooser changes nothing and starts no work.
    choices.borrow_mut().push(None);
    activate(&w, "PDFを添付…");
    assert!(!w.get_busy());
    assert_eq!(w.get_status(), "PDFの添付をやめました。");

    // A file that is not a PDF is refused with a reason and stores nothing.
    choices.borrow_mut().push(Some(not_pdf));
    activate(&w, "PDFを添付…");
    wait(&w);
    assert!(
        w.get_status().contains("PDFファイルではありません"),
        "{}",
        w.get_status()
    );
    assert_eq!(w.get_attachment_rows().row_count(), 0);
    assert_eq!(stored(&database, id), 0);
    assert_eq!(stored_files(&files), 0);

    // A real PDF appears at once, with its size, and is stored in the database.
    choices.borrow_mut().push(Some(good.clone()));
    activate(&w, "PDFを添付…");
    wait(&w);
    assert_eq!(w.get_attachment_rows().row_count(), 1, "{}", w.get_status());
    let row = w.get_attachment_rows().row_data(0).unwrap();
    assert_eq!(row.name, "results.pdf");
    assert!(
        row.details.contains(" B ") || row.details.contains("KB"),
        "{}",
        row.details
    );
    assert!(!row.missing);
    assert_eq!(stored(&database, id), 1);
    assert_eq!(stored_files(&files), 1);
    assert_eq!(
        element(&w, "results.pdfを開く").accessible_enabled(),
        Some(true)
    );

    // At the minimum window size the row's actions still fit.
    w.window().set_size(slint::LogicalSize::new(1080., 680.));
    for label in ["results.pdfを開く", "results.pdfを取り外す", "追加…"] {
        let control = element(&w, label);
        assert!(
            control.absolute_position().x + control.size().width <= 1080.,
            "{label} reaches x={}",
            control.absolute_position().x + control.size().width
        );
    }
    w.window().set_size(slint::LogicalSize::new(1360., 900.));

    // A file that vanished from the library is reported and cannot be opened.
    let blobs = paper_storage::FileBlobStore::open(&files).unwrap();
    let hash = blobs.hashes().unwrap()[0];
    let hex = paper_storage::content_hash_hex(&hash);
    let blob_path = files.join("blobs").join(&hex[..2]).join(&hex);
    let kept = std::fs::read(&blob_path).unwrap();
    std::fs::remove_file(&blob_path).unwrap();
    w.invoke_request_navigation(NavigationAction::OpenPaper, id.to_string().into());
    wait(&w);
    assert!(w.get_attachment_rows().row_data(0).unwrap().missing);
    assert_eq!(
        element(&w, "results.pdfを開く").accessible_enabled(),
        Some(false)
    );
    std::fs::write(&blob_path, kept).unwrap();
    w.invoke_request_navigation(NavigationAction::OpenPaper, id.to_string().into());
    wait(&w);
    assert!(!w.get_attachment_rows().row_data(0).unwrap().missing);

    // The same PDF twice on one paper is refused.
    choices.borrow_mut().push(Some(good));
    activate(&w, "追加…");
    wait(&w);
    assert!(w.get_status().contains("すでに"), "{}", w.get_status());
    assert_eq!(w.get_attachment_rows().row_count(), 1);

    // Attaching saves at once, so it waits while a note draft exists.
    w.set_dirty(true);
    assert_eq!(element(&w, "追加…").accessible_enabled(), Some(false));
    assert_eq!(
        element(&w, "results.pdfを取り外す").accessible_enabled(),
        Some(false)
    );
    w.set_dirty(false);

    // Removing asks once inside the row; backing out leaves everything as it was.
    activate(&w, "results.pdfを取り外す");
    element(&w, "取り外しますか？");
    assert!(absent(&w, "results.pdfを開く"));
    activate(&w, "results.pdfの取り外しをやめる");
    assert!(absent(&w, "取り外しますか？"));
    assert_eq!(stored(&database, id), 1);
    element(&w, "results.pdfを開く");

    activate(&w, "results.pdfを取り外す");
    activate(&w, "results.pdfの取り外しを実行");
    wait(&w);
    assert_eq!(w.get_attachment_rows().row_count(), 0, "{}", w.get_status());
    assert_eq!(stored(&database, id), 0);
    assert_eq!(stored_files(&files), 1, "removing never deletes the file");

    // The explicit cleanup shows what it will delete, then deletes only that.
    activate(&w, "未使用PDFの整理");
    wait(&w);
    assert_eq!(w.get_detail_view(), DetailView::AttachmentCleanup);
    assert_eq!(w.get_cleanup_count(), 1);
    activate(&w, "1 件を削除");
    wait(&w);
    assert_eq!(w.get_cleanup_count(), 0);
    assert!(w.get_status().contains("1 件"), "{}", w.get_status());
    assert_eq!(stored_files(&files), 0);
    assert_eq!(
        element(&w, "削除").accessible_enabled(),
        Some(false),
        "nothing left to delete"
    );

    // Closing returns to the paper that was open before.
    activate(&w, "閉じる");
    wait(&w);
    assert_eq!(w.get_detail_view(), DetailView::PaperDetail);
    assert_eq!(w.get_editing_id(), id.to_string());

    drop(controller);
    w.hide().unwrap();
}

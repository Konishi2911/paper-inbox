use chrono::Utc;
use paper_core::{
    command::LibraryCommand,
    model::{Bibliography, CitationKey, Paper, PaperId},
    ports::PaperRepository,
};
use paper_desktop::{MainWindow, NavigationAction, config::Config, ui};
use paper_storage::SqliteLibrary;
use slint::{ComponentHandle, Model};
use std::{
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
fn stored(db: &Path, id: PaperId) -> usize {
    SqliteLibrary::open_local(db)
        .unwrap()
        .attachments(id)
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
fn dropped_pdfs_attach_only_to_the_paper_on_screen() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let sandbox =
        Sandbox(std::env::temp_dir().join(format!("paper-inbox-drop-{}", PaperId::new())));
    std::fs::create_dir_all(&sandbox.0).unwrap();
    let database = sandbox.0.join("library.sqlite");
    let paper = Paper::new(
        PaperId::new(),
        CitationKey::parse("Drop2026").unwrap(),
        Bibliography {
            title: Some("Dropped".into()),
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
    let good = sandbox.0.join("Upper.PDF");
    std::fs::write(&good, "%PDF-1.7\nsome pages").unwrap();
    let text = sandbox.0.join("notes.txt");
    std::fs::write(&text, "%PDF-1.7").unwrap();

    let w = MainWindow::new().unwrap();
    w.show().unwrap();
    w.window().set_size(slint::LogicalSize::new(1360., 900.));
    let _controller = ui::attach_with_providers(
        &w,
        Config {
            database: database.clone(),
        },
        Box::new(|_| Ok(vec![])),
        Box::new(|| None),
    );
    wait(&w);
    let drop_file = |path: &Path| w.invoke_file_dropped(path.to_string_lossy().as_ref().into());

    // Nothing is open yet: the drop is explained and stores nothing.
    assert!(!w.get_can_drop_attach());
    drop_file(&good);
    assert!(w.get_status().contains("文献を開き"), "{}", w.get_status());
    assert!(!w.get_busy());
    assert_eq!(stored(&database, id), 0);

    w.invoke_request_navigation(NavigationAction::OpenPaper, id.to_string().into());
    wait(&w);
    assert!(w.get_can_drop_attach());
    // The extension decides first, whatever the content is.
    drop_file(&text);
    assert!(w.get_status().contains(".pdf"), "{}", w.get_status());
    assert_eq!(stored(&database, id), 0);

    // The extension is matched without regard to case, and the hover frame is cleared.
    w.set_drop_hover(true);
    drop_file(&good);
    assert!(!w.get_drop_hover());
    wait(&w);
    assert_eq!(stored(&database, id), 1, "{}", w.get_status());
    assert_eq!(
        w.get_attachment_rows().row_data(0).unwrap().name,
        "Upper.PDF"
    );

    // An unsaved edit blocks the drop, like the attach button.
    w.set_dirty(true);
    assert!(!w.get_can_drop_attach());
    drop_file(&good);
    assert!(!w.get_busy());
    assert_eq!(stored(&database, id), 1);
}

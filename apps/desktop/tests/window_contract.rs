use paper_desktop::{AuthorAction, NavigationAction};
use paper_desktop::{MainWindow, config::Config, ui};
use slint::{ComponentHandle, Model};
use std::time::{Duration, Instant};
fn wait(ui: &MainWindow) {
    let weak = ui.as_weak();
    let start = Instant::now();
    let timer = slint::Timer::default();
    let completed = std::rc::Rc::new(std::cell::Cell::new(false));
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(5),
        move || {
            let ui = weak.upgrade().unwrap();
            assert!(
                start.elapsed() < Duration::from_secs(15),
                "Timed out: {}",
                ui.get_status()
            );
            if !ui.get_busy() && !completed.replace(true) {
                slint::quit_event_loop().unwrap();
            }
        },
    );
    slint::run_event_loop_until_quit().unwrap();
}
#[test]
fn real_window_callbacks_cover_registration_validation_edit_restart_and_trash() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let path = std::env::temp_dir().join(format!(
        "paper-ui-window-{}.sqlite",
        paper_core::model::PaperId::new()
    ));
    let window = MainWindow::new().unwrap();
    let controller = ui::attach(
        &window,
        Config {
            database: path.clone(),
        },
    );
    wait(&window);
    assert!(window.get_ready());
    assert!(!window.get_busy());
    window.set_citation_key("Window2026".into());
    window.set_paper_title("Window test".into());
    window.set_year("2026".into());
    window.invoke_author_action(AuthorAction::Add, 0);
    window.invoke_author_edit(0, 0, "Lovelace".into());
    window.invoke_author_edit(0, 1, "Ada".into());
    window.invoke_save();
    wait(&window);
    assert!(window.get_saved_paper());
    assert!(!window.get_dirty());
    assert_eq!(window.get_total(), 1);
    let id = window.get_editing_id();
    window.set_doi("https://doi.org/10.1000/ABC".into());
    window.set_arxiv_id("arXiv:2401.12345v2".into());
    window.set_dirty(true);
    window.invoke_save();
    wait(&window);
    assert!(!window.get_dirty(), "{}", window.get_status());
    assert_eq!(window.get_doi(), "10.1000/abc");
    assert_eq!(window.get_arxiv_id(), "2401.12345v2");
    window.invoke_select_paper(id.clone(), true);
    assert_eq!(window.get_selected_count(), 1);
    let output = path.with_extension("bib");
    window.set_export_path(output.to_string_lossy().as_ref().into());
    window.set_paper_title("not saved".into());
    window.set_dirty(true);
    window.invoke_prepare_export();
    wait(&window);
    assert!(window.get_export_confirm());
    assert!(!window.get_export_replaces());
    assert!(!output.exists());
    assert!(window.get_export_preview().contains("Window test"));
    assert!(!window.get_export_preview().contains("not saved"));
    assert!(window.get_dirty());
    window.invoke_confirm_export();
    wait(&window);
    assert!(output.exists());
    assert!(!window.get_export_confirm());
    assert!(window.get_dirty());
    let original = std::fs::read(&output).unwrap();
    window.invoke_prepare_export();
    wait(&window);
    assert!(window.get_export_replaces());
    window.invoke_cancel_export();
    wait(&window);
    assert_eq!(std::fs::read(&output).unwrap(), original);
    window.invoke_prepare_export();
    wait(&window);
    std::fs::write(&output, "other writer").unwrap();
    window.invoke_confirm_export();
    wait(&window);
    assert!(window.get_status().contains("変更されています"));
    assert_eq!(std::fs::read_to_string(&output).unwrap(), "other writer");
    window.invoke_prepare_export();
    wait(&window);
    window.invoke_confirm_export();
    wait(&window);
    assert_ne!(std::fs::read(&output).unwrap(), b"other writer");
    std::fs::remove_file(output).unwrap();

    window.set_paper_title("Edited from UI".into());
    window.set_year("invalid".into());
    window.set_dirty(true);
    window.invoke_save();
    wait(&window);
    assert!(window.get_dirty());
    assert_eq!(window.get_paper_title(), "Edited from UI");
    assert_eq!(window.get_year(), "invalid");
    assert!(window.get_status().contains("出版年"));
    window.invoke_request_navigation(NavigationAction::CreatePaper, "".into());
    assert_eq!(window.get_editing_id(), id);
    assert!(window.get_dirty());
    // Cancel the pending discard and keep editing the same form.
    window.set_confirm_discard(false);
    window.set_year("2027".into());
    window.invoke_save();
    wait(&window);
    assert!(!window.get_dirty());
    window.set_search_text("Lovelace".into());
    window.invoke_search();
    wait(&window);
    assert_eq!(window.get_total(), 1);
    assert_eq!(
        window.get_papers().row_data(0).unwrap().title,
        "Edited from UI"
    );
    drop(controller);
    drop(window);
    let window = MainWindow::new().unwrap();
    let controller = ui::attach(
        &window,
        Config {
            database: path.clone(),
        },
    );
    wait(&window);
    assert_eq!(window.get_total(), 1);
    window.invoke_navigate(NavigationAction::OpenPaper, id.clone());
    wait(&window);
    assert_eq!(window.get_paper_title(), "Edited from UI");
    assert_eq!(window.get_year(), "2027");
    window.invoke_delete_paper(true);
    wait(&window);
    assert!(window.get_editor_deleted());
    assert_eq!(window.get_total(), 0);
    window.set_show_deleted(true);
    window.invoke_search();
    wait(&window);
    assert_eq!(window.get_total(), 1);
    window.invoke_delete_paper(false);
    wait(&window);
    assert!(!window.get_editor_deleted());
    assert_eq!(window.get_total(), 0);
    window.set_show_deleted(false);
    window.invoke_search();
    wait(&window);
    assert_eq!(window.get_total(), 1);
    {
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection.execute("UPDATE bibliographies SET doi='legacy invalid DOI' WHERE id=(SELECT bibliography_id FROM paper_snapshots WHERE id=(SELECT snapshot_id FROM papers WHERE id=?1))",[id.as_str()]).unwrap();
    }
    window.invoke_navigate(NavigationAction::OpenPaper, id.clone());
    wait(&window);
    assert!(!window.get_identifier_warning().is_empty());
    assert_eq!(window.get_doi(), "legacy invalid DOI");
    window.set_notes("notes preserve legacy identifier".into());
    window.set_dirty(true);
    window.invoke_save();
    wait(&window);
    assert!(!window.get_dirty());
    assert_eq!(window.get_doi(), "legacy invalid DOI");
    window.set_doi("DOI:10.1000/FIXED".into());
    window.set_dirty(true);
    window.invoke_save();
    wait(&window);
    assert!(!window.get_dirty());
    assert_eq!(window.get_doi(), "10.1000/fixed");
    assert!(window.get_identifier_warning().is_empty());
    drop(controller);
    drop(window);
    std::fs::remove_file(&path).unwrap();
    // A DB path that names a directory fails visibly, without enabling saves.
    let path = std::env::temp_dir().join(format!(
        "paper-ui-bad-{}",
        paper_core::model::PaperId::new()
    ));
    std::fs::create_dir(&path).unwrap();
    let window = MainWindow::new().unwrap();
    let controller = ui::attach(
        &window,
        Config {
            database: path.clone(),
        },
    );
    wait(&window);
    assert!(!window.get_ready());
    assert!(!window.get_busy());
    assert!(window.get_status().contains("保存先を開けません"));
    drop(controller);
    drop(window);
    std::fs::remove_dir(&path).unwrap();
}

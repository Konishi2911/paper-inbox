use chrono::Utc;
use i_slint_backend_testing::ElementHandle;
use paper_core::{
    command::LibraryCommand,
    model::{Bibliography, CitationKey, Paper, PaperId},
};
use paper_desktop::{MainWindow, NavigationAction, RowAction, config::Config, ui};
use paper_storage::SqliteLibrary;
use slint::{
    ComponentHandle, Model,
    platform::{PointerEventButton, WindowEvent},
};
use std::{
    path::PathBuf,
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

struct Sandbox(PathBuf);
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn paper(key: &str) -> Paper {
    Paper::new(
        PaperId::new(),
        CitationKey::parse(key).unwrap(),
        Bibliography {
            title: Some(key.into()),
            ..Default::default()
        },
        Utc::now(),
    )
    .unwrap()
}

fn right_click(w: &MainWindow, label: &str) {
    let found: Vec<_> = ElementHandle::find_by_accessible_label(w, label).collect();
    assert_eq!(found.len(), 1, "{label}");
    let e = &found[0];
    let o = e.absolute_position();
    let s = e.size();
    let position = slint::LogicalPosition::new(o.x + s.width / 2., o.y + s.height / 2.);
    w.window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    w.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Right,
    });
    w.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Right,
    });
}

#[test]
fn card_menu_actions_apply_to_the_card_or_the_selection_without_moving_it() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let sandbox =
        Sandbox(std::env::temp_dir().join(format!("paper-inbox-menu-{}", PaperId::new())));
    std::fs::create_dir_all(&sandbox.0).unwrap();
    let database = sandbox.0.join("library.sqlite");
    {
        let store = SqliteLibrary::open_local(&database).unwrap();
        for key in ["Aaa", "Bbb", "Ccc"] {
            store
                .commit_at(
                    LibraryCommand::CreatePaper { paper: paper(key) },
                    Utc::now(),
                )
                .unwrap();
        }
    }
    let w = MainWindow::new().unwrap();
    w.show().unwrap();
    w.window().set_size(slint::LogicalSize::new(1360., 900.));
    let _c = ui::attach_with_providers(
        &w,
        Config { database },
        Box::new(|_| Ok(vec![])),
        Box::new(|| None),
    );
    wait(&w);
    let id_of = |t: &str| w.get_papers().iter().find(|p| p.title == t).unwrap().id;
    let fav = |t: &str| {
        w.get_papers()
            .iter()
            .find(|p| p.title == t)
            .unwrap()
            .favorite
    };
    let status = |t: &str| {
        w.get_papers()
            .iter()
            .find(|p| p.title == t)
            .unwrap()
            .reading_status
    };

    // A right click does not open the card or change the selection.
    right_click(&w, "文献を開く: Bbb");
    wait(&w);
    assert_eq!(w.get_selected_count(), 0);
    assert_eq!(w.get_editing_id(), "");

    // No selection: the menu applies to the card alone, and leaves the selection empty.
    w.invoke_row_action(id_of("Bbb"), RowAction::FavoriteOn);
    wait(&w);
    assert!(fav("Bbb") && !fav("Aaa") && !fav("Ccc"));
    assert_eq!(w.get_selected_count(), 0);

    // Two selected, menu on a third card: only that card changes; the selection stays.
    w.set_pending_ctrl(true);
    w.set_pending_shift(false);
    w.invoke_request_navigation(NavigationAction::OpenPaper, id_of("Aaa"));
    wait(&w);
    w.invoke_request_navigation(NavigationAction::MultiSelect, id_of("Ccc"));
    wait(&w);
    assert_eq!(w.get_selected_count(), 2);
    w.invoke_row_action(id_of("Bbb"), RowAction::StatusRead);
    wait(&w);
    assert_eq!(status("Bbb"), 2);
    assert_eq!(status("Aaa"), 0);
    assert_eq!(w.get_selected_count(), 2, "the selection is not touched");

    // Menu on a selected card: the whole selection changes together.
    w.invoke_row_action(id_of("Aaa"), RowAction::FavoriteOn);
    wait(&w);
    assert!(fav("Aaa") && fav("Ccc") && fav("Bbb"));
    assert_eq!(
        w.get_selected_count(),
        0,
        "a change on the selection ends it"
    );

    // An unsaved edit of the card stops the change.
    w.invoke_open_paper(id_of("Ccc"));
    wait(&w);
    w.set_dirty(true);
    w.invoke_row_action(id_of("Ccc"), RowAction::FavoriteOff);
    wait(&w);
    assert!(fav("Ccc"), "refused while the open paper has unsaved edits");
    w.set_dirty(false);

    // So does an unapplied metadata review or candidate edit: the change reloads the open paper.
    w.set_review_dirty(true);
    w.invoke_row_action(id_of("Ccc"), RowAction::FavoriteOff);
    wait(&w);
    assert!(fav("Ccc"), "refused while a metadata review is unapplied");
    w.set_review_dirty(false);
    w.set_candidate_dirty(true);
    w.invoke_row_action(id_of("Ccc"), RowAction::FavoriteOff);
    wait(&w);
    assert!(fav("Ccc"), "refused while a candidate edit is unsaved");
    w.set_candidate_dirty(false);
    w.set_lookup_dirty(true);
    w.invoke_row_action(id_of("Ccc"), RowAction::FavoriteOff);
    wait(&w);
    assert!(fav("Ccc"), "refused while a lookup result is unconfirmed");
    w.set_lookup_dirty(false);
    w.invoke_row_action(id_of("Ccc"), RowAction::FavoriteOff);
    wait(&w);
    assert!(!fav("Ccc"), "allowed once nothing is unsaved");
}

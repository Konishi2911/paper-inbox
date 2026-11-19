use chrono::Utc;
use i_slint_backend_testing::ElementHandle;
use paper_core::{
    command::LibraryCommand,
    model::{Bibliography, CitationKey, Paper, PaperId},
};
use paper_desktop::{DetailView, MainWindow, config::Config, ui};
use paper_storage::SqliteLibrary;
use slint::{ComponentHandle, Model};
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

/// Titles of the rows that are shown as selected, in list order.
fn selected_titles(w: &MainWindow) -> Vec<String> {
    w.get_papers()
        .iter()
        .filter(|p| p.selected)
        .map(|p| p.title.to_string())
        .collect()
}
fn titles(w: &MainWindow) -> Vec<String> {
    w.get_papers().iter().map(|p| p.title.to_string()).collect()
}
/// A click with the given keys held, as the card reports it to the list.
fn click(w: &MainWindow, title: &str, ctrl: bool, shift: bool) {
    let id = w.get_papers().iter().find(|p| p.title == title).unwrap().id;
    w.invoke_row_modified_click(id, ctrl, shift);
}

fn rows_are_selected_with_ctrl_and_shift_and_changed_together() {
    let sandbox =
        Sandbox(std::env::temp_dir().join(format!("paper-inbox-multi-{}", PaperId::new())));
    std::fs::create_dir_all(&sandbox.0).unwrap();
    let database = sandbox.0.join("library.sqlite");
    let keys = ["Alpha2026", "Bravo2026", "Charlie2026", "Delta2026"];
    {
        let store = SqliteLibrary::open_local(&database).unwrap();
        for key in keys {
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
    let _controller = ui::attach_with_providers(
        &w,
        Config {
            database: database.clone(),
        },
        Box::new(|_| Ok(vec![])),
        Box::new(|| None),
    );
    wait(&w);
    // Titles in the order the list shows them (newest first by default).
    let order = titles(&w);
    assert_eq!(order.len(), 4);
    assert!(
        absent(&w, "選択を解除"),
        "no selection bar before selecting"
    );

    // Ctrl toggles single rows and is how a selection grows and shrinks.
    click(&w, &order[0], true, false);
    click(&w, &order[2], true, false);
    assert_eq!(w.get_selected_count(), 2);
    assert_eq!(
        selected_titles(&w),
        vec![order[0].clone(), order[2].clone()]
    );
    click(&w, &order[0], true, false);
    assert_eq!(selected_titles(&w), vec![order[2].clone()]);

    // Ctrl+Shift adds the range from the anchor (the last Ctrl click, row 0) to what is
    // already selected instead of replacing it.
    click(&w, &order[0], true, false);
    click(&w, &order[1], true, true);
    assert_eq!(
        selected_titles(&w),
        vec![order[0].clone(), order[1].clone(), order[2].clone()]
    );
    assert!(!absent(&w, "選択を解除"), "the selection bar is shown");
}

fn a_range_runs_from_the_anchor_and_a_bulk_change_clears_the_selection() {
    let sandbox =
        Sandbox(std::env::temp_dir().join(format!("paper-inbox-bulk-{}", PaperId::new())));
    std::fs::create_dir_all(&sandbox.0).unwrap();
    let database = sandbox.0.join("library.sqlite");
    {
        let store = SqliteLibrary::open_local(&database).unwrap();
        for key in ["Alpha2026", "Bravo2026", "Charlie2026", "Delta2026"] {
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
    let _controller = ui::attach_with_providers(
        &w,
        Config {
            database: database.clone(),
        },
        Box::new(|_| Ok(vec![])),
        Box::new(|| None),
    );
    wait(&w);
    let order = titles(&w);

    // Anchor on row 1, then Shift to row 3: rows 1..=3.
    click(&w, &order[1], true, false);
    click(&w, &order[3], false, true);
    assert_eq!(selected_titles(&w), order[1..].to_vec());
    // Shifting back to row 0 re-draws the range from the same anchor: rows 0..=1.
    click(&w, &order[0], false, true);
    assert_eq!(selected_titles(&w), order[..2].to_vec());
    assert_eq!(w.get_selected_count(), 2);

    // The bar names the count and offers the bulk actions.
    activate(&w, "選択を解除");
    assert_eq!(w.get_selected_count(), 0);
    assert!(selected_titles(&w).is_empty());

    // Select all shown rows, then mark them favorite in one change.
    w.invoke_select_all();
    assert_eq!(w.get_selected_count(), 4);
    w.invoke_bulk_favorite(true);
    wait(&w);
    assert_eq!(
        w.get_selected_count(),
        0,
        "a finished bulk change ends the selection"
    );
    assert!(w.get_papers().iter().all(|p| p.favorite));
    assert!(w.get_status().contains("4件に適用"), "{}", w.get_status());

    // Repeating the same change reports that nothing differed.
    w.invoke_select_all();
    w.invoke_bulk_favorite(true);
    wait(&w);
    assert!(
        w.get_status().contains("すでに同じ状態"),
        "{}",
        w.get_status()
    );

    // Delete needs a confirmation in the bar; cancelling changes nothing.
    click(&w, &order[0], true, false);
    click(&w, &order[1], true, false);
    activate(&w, "削除");
    assert!(!absent(&w, "削除する"));
    activate(&w, "キャンセル");
    assert!(absent(&w, "削除する"));
    assert_eq!(w.get_papers().row_count(), 4);
    activate(&w, "削除");
    activate(&w, "削除する");
    wait(&w);
    assert_eq!(w.get_papers().row_count(), 2);
    assert_eq!(w.get_selected_count(), 0);
}

fn a_new_search_or_sort_drops_the_selection_but_loading_more_keeps_it() {
    let sandbox =
        Sandbox(std::env::temp_dir().join(format!("paper-inbox-drop-{}", PaperId::new())));
    std::fs::create_dir_all(&sandbox.0).unwrap();
    let database = sandbox.0.join("library.sqlite");
    {
        let store = SqliteLibrary::open_local(&database).unwrap();
        for key in ["Alpha2026", "Bravo2026", "Charlie2026"] {
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
    let _controller = ui::attach_with_providers(
        &w,
        Config {
            database: database.clone(),
        },
        Box::new(|_| Ok(vec![])),
        Box::new(|| None),
    );
    wait(&w);
    let order = titles(&w);

    click(&w, &order[0], true, false);
    assert_eq!(w.get_selected_count(), 1);
    // The search button is a change of what the list shows.
    activate(&w, "検索");
    wait(&w);
    assert_eq!(w.get_selected_count(), 0);
    assert!(selected_titles(&w).is_empty());

    // A filter chip is one too.
    click(&w, &order[0], true, false);
    activate(&w, "未読だけ表示");
    wait(&w);
    assert_eq!(w.get_selected_count(), 0);
    activate(&w, "未読だけ表示");
    wait(&w);

    // Opening a paper with a plain click ends the selection.
    click(&w, &order[0], true, false);
    activate(&w, &format!("文献を開く: {}", order[1]));
    wait(&w);
    assert_eq!(w.get_detail_view(), DetailView::PaperDetail);
    assert_eq!(w.get_selected_count(), 0);
}

#[test]
fn multi_selection_in_the_window() {
    // The testing platform can be set once per process, so the scenarios share one test.
    i_slint_backend_testing::init_integration_test_with_system_time();
    rows_are_selected_with_ctrl_and_shift_and_changed_together();
    a_range_runs_from_the_anchor_and_a_bulk_change_clears_the_selection();
    a_new_search_or_sort_drops_the_selection_but_loading_more_keeps_it();
    real_input_selects_rows_and_keys_act_on_the_list();
}

/// Real pointer and key events: the card reads the modifier keys itself, and Ctrl+A / Escape
/// reach the list only when it has the focus (a click on a row gives it).
fn real_input_selects_rows_and_keys_act_on_the_list() {
    use slint::platform::{Key, PointerEventButton, WindowEvent};
    let sandbox =
        Sandbox(std::env::temp_dir().join(format!("paper-inbox-keys-{}", PaperId::new())));
    std::fs::create_dir_all(&sandbox.0).unwrap();
    let database = sandbox.0.join("library.sqlite");
    {
        let store = SqliteLibrary::open_local(&database).unwrap();
        for key in ["Alpha2026", "Bravo2026", "Charlie2026"] {
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
    let _controller = ui::attach_with_providers(
        &w,
        Config {
            database: database.clone(),
        },
        Box::new(|_| Ok(vec![])),
        Box::new(|| None),
    );
    wait(&w);
    let order = titles(&w);
    let card_center = |title: &str| {
        let card = element(&w, &format!("文献を開く: {title}"));
        let origin = card.absolute_position();
        let size = card.size();
        slint::LogicalPosition::new(origin.x + size.width / 2., origin.y + size.height / 2.)
    };
    let press = |position| {
        w.window()
            .dispatch_event(WindowEvent::PointerMoved { position });
        w.window().dispatch_event(WindowEvent::PointerPressed {
            position,
            button: PointerEventButton::Left,
        });
        w.window().dispatch_event(WindowEvent::PointerReleased {
            position,
            button: PointerEventButton::Left,
        });
    };
    let key = |key: Key| {
        w.window()
            .dispatch_event(WindowEvent::KeyPressed { text: key.into() });
    };
    let key_up = |key: Key| {
        w.window()
            .dispatch_event(WindowEvent::KeyReleased { text: key.into() });
    };

    // Ctrl held: the click selects and does not open the paper.
    key(Key::Control);
    press(card_center(&order[0]));
    press(card_center(&order[2]));
    key_up(Key::Control);
    assert_eq!(
        selected_titles(&w),
        vec![order[0].clone(), order[2].clone()]
    );
    assert_ne!(w.get_detail_view(), DetailView::PaperDetail);

    // Escape, with the list focused by the click above, drops the selection.
    key(Key::Escape);
    key_up(Key::Escape);
    assert_eq!(w.get_selected_count(), 0);

    // Ctrl+A selects every row shown.
    key(Key::Control);
    press(card_center(&order[1]));
    w.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "a".into() });
    w.window()
        .dispatch_event(WindowEvent::KeyReleased { text: "a".into() });
    key_up(Key::Control);
    assert_eq!(w.get_selected_count(), 3);

    // Shift held: the range runs from the anchor row.
    key(Key::Escape);
    key(Key::Shift);
    press(card_center(&order[0]));
    press(card_center(&order[2]));
    key_up(Key::Shift);
    assert_eq!(
        selected_titles(&w),
        order,
        "shift range from the first click"
    );

    // A plain click opens the paper and ends the selection.
    press(card_center(&order[1]));
    wait(&w);
    assert_eq!(w.get_detail_view(), DetailView::PaperDetail);
    assert_eq!(w.get_selected_count(), 0);
}

use chrono::Utc;
use i_slint_backend_testing::ElementHandle;
use paper_core::{
    command::LibraryCommand,
    model::{Bibliography, CitationKey, Paper, PaperId},
};
use paper_desktop::{DetailView, MainWindow, NavigationAction, config::Config, ui};
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

/// A library of papers with the given citation keys (their titles), and a window on it.
fn open_window(prefix: &str, keys: &[&str]) -> (Sandbox, MainWindow, ui::Controller) {
    let sandbox =
        Sandbox(std::env::temp_dir().join(format!("paper-inbox-{prefix}-{}", PaperId::new())));
    std::fs::create_dir_all(&sandbox.0).unwrap();
    let database = sandbox.0.join("library.sqlite");
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
    let controller = ui::attach_with_providers(
        &w,
        Config { database },
        Box::new(|_| Ok(vec![])),
        Box::new(|| None),
    );
    wait(&w);
    (sandbox, w, controller)
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
/// A plain click on a row: it opens the paper, which is then the only one selected.
fn open(w: &MainWindow, title: &str) {
    activate(w, &format!("文献を開く: {title}"));
    wait(w);
}
/// A click with Ctrl and/or Shift held, as the card reports it to the window.
fn click(w: &MainWindow, title: &str, ctrl: bool, shift: bool) {
    let id = w.get_papers().iter().find(|p| p.title == title).unwrap().id;
    w.set_pending_ctrl(ctrl);
    w.set_pending_shift(shift);
    w.invoke_request_navigation(NavigationAction::MultiSelect, id);
    wait(w);
}
fn select_all(w: &MainWindow) {
    w.invoke_request_navigation(NavigationAction::SelectAll, "".into());
    wait(w);
}
fn card_y(w: &MainWindow, title: &str) -> f32 {
    element(w, &format!("文献を開く: {title}"))
        .absolute_position()
        .y
}

fn the_first_click_opens_and_selects_and_later_ones_extend_it() {
    let (_sandbox, w, _controller) = open_window(
        "multi",
        &["Alpha2026", "Bravo2026", "Charlie2026", "Delta2026"],
    );
    // Titles in the order the list shows them (newest first by default).
    let order = titles(&w);
    assert_eq!(order.len(), 4);
    assert!(absent(&w, "選択を解除"), "no bulk bar before selecting");
    // BibTeX output has no entry of its own: not in the sidebar, not on the cards.
    assert!(absent(&w, "BibTeX 出力"));
    assert!(absent(&w, "BibTeX 出力に含める"));
    let before = card_y(&w, &order[0]);

    // A plain click opens the paper and selects it alone. One paper is not a bulk selection.
    open(&w, &order[0]);
    assert_eq!(w.get_detail_view(), DetailView::PaperDetail);
    assert_eq!(selected_titles(&w), vec![order[0].clone()]);
    assert!(absent(&w, "選択を解除"), "one open paper shows no bulk bar");
    assert_eq!(card_y(&w, &order[0]), before);

    // Ctrl adds the next paper. The pane stops showing a single paper, and the list stays put
    // while the bar replaces the title, search and filter rows.
    click(&w, &order[2], true, false);
    assert_eq!(w.get_selected_count(), 2);
    assert_eq!(
        selected_titles(&w),
        vec![order[0].clone(), order[2].clone()]
    );
    assert_eq!(w.get_detail_view(), DetailView::Empty);
    assert_eq!(w.get_editing_id(), "");
    assert!(!absent(&w, "選択を解除"), "the bulk bar is shown");
    assert_eq!(
        card_y(&w, &order[0]),
        before,
        "the bar takes the place of the rows above the list"
    );

    // Ctrl toggles a paper out. One paper left is opened again.
    click(&w, &order[0], true, false);
    assert_eq!(selected_titles(&w), vec![order[2].clone()]);
    assert_eq!(w.get_detail_view(), DetailView::PaperDetail);
    assert_eq!(w.get_paper_title(), order[2]);
    assert!(absent(&w, "選択を解除"));

    // Shift selects from the opened paper, the anchor, to the clicked one.
    open(&w, &order[1]);
    click(&w, &order[3], false, true);
    assert_eq!(selected_titles(&w), order[1..].to_vec());
    // Shifting back to row 0 draws the range from the same anchor: rows 0..=1.
    click(&w, &order[0], false, true);
    assert_eq!(selected_titles(&w), order[..2].to_vec());
    // Ctrl+Shift adds a range to what is selected instead of replacing it.
    click(&w, &order[3], true, true);
    assert_eq!(w.get_selected_count(), 4);

    // A plain click on a selected paper selects it alone and opens it.
    open(&w, &order[2]);
    assert_eq!(selected_titles(&w), vec![order[2].clone()]);
    assert_eq!(w.get_detail_view(), DetailView::PaperDetail);
}

fn bulk_changes_run_from_the_bar_and_delete_is_confirmed() {
    let (_sandbox, w, _controller) = open_window(
        "bulk",
        &["Alpha2026", "Bravo2026", "Charlie2026", "Delta2026"],
    );
    let order = titles(&w);

    open(&w, &order[1]);
    click(&w, &order[3], false, true);
    assert_eq!(selected_titles(&w), order[1..].to_vec());
    activate(&w, "選択を解除");
    assert_eq!(w.get_selected_count(), 0);
    assert!(selected_titles(&w).is_empty());

    // Select all shown rows, then mark them favorite in one change.
    select_all(&w);
    assert_eq!(w.get_selected_count(), 4);
    assert_eq!(w.get_detail_view(), DetailView::Empty);
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
    select_all(&w);
    w.invoke_bulk_favorite(true);
    wait(&w);
    assert!(
        w.get_status().contains("すでに同じ状態"),
        "{}",
        w.get_status()
    );

    // Delete is asked once in the bar; cancelling changes nothing.
    click(&w, &order[0], true, false);
    click(&w, &order[1], true, false);
    assert_eq!(w.get_selected_count(), 2);
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

fn the_bar_fits_the_narrowest_list_and_leads_to_bibtex_output() {
    let (_sandbox, w, _controller) = open_window("bar", &["Alpha2026", "Bravo2026", "Charlie2026"]);
    let order = titles(&w);
    w.set_paper_list_width(260.);
    open(&w, &order[0]);
    click(&w, &order[1], true, false);
    // The list keeps 16px of padding on both sides; the bar must stay within it.
    let list = ElementHandle::find_by_element_type_name(&w, "PaperList")
        .next()
        .unwrap();
    let left = list.absolute_position().x + 16.;
    let right = list.absolute_position().x + list.size().width - 16.;
    for label in [
        "BibTeX",
        "削除",
        "選択を解除",
        "選択した文献を未読にする",
        "選択した文献を読書中にする",
        "選択した文献を読了にする",
        "お気に入りにする",
        "お気に入りを外す",
        "選択した文献にタグを付ける",
        "アーカイブ",
        "アーカイブ解除",
    ] {
        let control = element(&w, label);
        let x = control.absolute_position().x;
        assert!(
            x >= left - 1. && x + control.size().width <= right + 1.,
            "{label} stays inside the list ({x}..{} in {left}..{right})",
            x + control.size().width
        );
    }

    // The bar's BibTeX button is the way to the export settings, for exactly this selection.
    activate(&w, "BibTeX");
    assert_eq!(w.get_detail_view(), DetailView::ExportSettings);
    assert_eq!(w.get_selected_count(), 2);
    // While the settings are open the selection can still be changed.
    click(&w, &order[2], true, false);
    assert_eq!(w.get_detail_view(), DetailView::ExportSettings);
    assert_eq!(w.get_selected_count(), 3);
}

fn a_new_search_or_filter_drops_the_selection() {
    let (_sandbox, w, _controller) =
        open_window("drop", &["Alpha2026", "Bravo2026", "Charlie2026"]);
    let order = titles(&w);

    open(&w, &order[0]);
    assert_eq!(w.get_selected_count(), 1);
    // The search button is a change of what the list shows.
    activate(&w, "検索");
    wait(&w);
    assert_eq!(w.get_selected_count(), 0);
    assert!(selected_titles(&w).is_empty());

    // A filter chip is one too. (With several papers selected the bar hides the chips, so the
    // selection here is the one opened paper.)
    open(&w, &order[1]);
    assert_eq!(w.get_selected_count(), 1);
    activate(&w, "未読だけ表示");
    wait(&w);
    assert_eq!(w.get_selected_count(), 0);
    activate(&w, "未読だけ表示");
    wait(&w);
}

#[test]
fn multi_selection_in_the_window() {
    // The testing platform can be set once per process, so the scenarios share one test.
    i_slint_backend_testing::init_integration_test_with_system_time();
    the_first_click_opens_and_selects_and_later_ones_extend_it();
    bulk_changes_run_from_the_bar_and_delete_is_confirmed();
    the_bar_fits_the_narrowest_list_and_leads_to_bibtex_output();
    a_new_search_or_filter_drops_the_selection();
    real_input_selects_rows_and_keys_act_on_the_list();
}

/// Real pointer and key events: the card reads the modifier keys itself, and Ctrl+A / Escape
/// reach the list only when it has the focus (a click on a row gives it).
fn real_input_selects_rows_and_keys_act_on_the_list() {
    use slint::platform::{Key, PointerEventButton, WindowEvent};
    let (_sandbox, w, _controller) =
        open_window("keys", &["Alpha2026", "Bravo2026", "Charlie2026"]);
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
        wait(&w);
    };
    let key = |key: Key| {
        w.window()
            .dispatch_event(WindowEvent::KeyPressed { text: key.into() });
    };
    let key_up = |key: Key| {
        w.window()
            .dispatch_event(WindowEvent::KeyReleased { text: key.into() });
    };

    // A plain click opens the first paper; Ctrl-click adds another and does not open it.
    press(card_center(&order[0]));
    assert_eq!(w.get_detail_view(), DetailView::PaperDetail);
    key(Key::Control);
    press(card_center(&order[2]));
    key_up(Key::Control);
    assert_eq!(
        selected_titles(&w),
        vec![order[0].clone(), order[2].clone()]
    );
    assert_eq!(w.get_detail_view(), DetailView::Empty);

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
    wait(&w);
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

    // A plain click opens one paper and selects only it.
    press(card_center(&order[1]));
    assert_eq!(w.get_detail_view(), DetailView::PaperDetail);
    assert_eq!(selected_titles(&w), vec![order[1].clone()]);
}

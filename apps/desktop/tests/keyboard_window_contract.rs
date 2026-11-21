use chrono::Utc;
use i_slint_backend_testing::ElementHandle;
use paper_core::{
    command::LibraryCommand,
    model::{Bibliography, CitationKey, Paper, PaperId},
};
use paper_desktop::{MainWindow, NavigationAction, config::Config, ui};
use paper_storage::SqliteLibrary;
use slint::{ComponentHandle, Model, SharedString, platform::Key, platform::WindowEvent};
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

fn open_window(keys: &[String]) -> (Sandbox, MainWindow, ui::Controller) {
    let sandbox =
        Sandbox(std::env::temp_dir().join(format!("paper-inbox-keys-{}", PaperId::new())));
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

fn titles(w: &MainWindow) -> Vec<String> {
    w.get_papers().iter().map(|p| p.title.to_string()).collect()
}
fn open(w: &MainWindow, title: &str) {
    activate(w, &format!("文献を開く: {title}"));
    wait(w);
}
/// A key typed into the window: it goes to whatever has the keyboard focus.
fn press(w: &MainWindow, text: impl Into<SharedString>) {
    let text = text.into();
    w.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    w.window().dispatch_event(WindowEvent::KeyReleased { text });
    wait(w);
}
fn key(w: &MainWindow, key: Key) {
    press(w, SharedString::from(key));
}
fn open_title(w: &MainWindow) -> String {
    w.get_paper_title().to_string()
}

#[test]
fn up_and_down_open_neighbors_and_slash_goes_to_search() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let keys: Vec<String> = (0..30).map(|i| format!("Key{i:02}")).collect();
    let (_sandbox, w, _controller) = open_window(&keys);
    let order = titles(&w);
    assert_eq!(order.len(), 30);

    // Up and Down open the neighbor, like a click, and stop at both ends.
    open(&w, &order[0]);
    assert_eq!(open_title(&w), order[0]);
    key(&w, Key::UpArrow);
    assert_eq!(open_title(&w), order[0], "no wrap above the first paper");
    key(&w, Key::DownArrow);
    assert_eq!(open_title(&w), order[1]);
    key(&w, Key::DownArrow);
    key(&w, Key::UpArrow);
    assert_eq!(open_title(&w), order[1]);
    assert_eq!(
        w.get_selected_count(),
        1,
        "the opened paper is the selection"
    );

    // The open paper scrolls into view while stepping down the whole list.
    let list = ElementHandle::find_by_element_type_name(&w, "PaperList")
        .next()
        .unwrap();
    let bottom = list.absolute_position().y + list.size().height;
    for (step, title) in order.iter().enumerate().skip(2) {
        key(&w, Key::DownArrow);
        assert_eq!(open_title(&w), *title);
        let card = element(&w, &format!("文献を開く: {}", title));
        let y = card.absolute_position().y;
        assert!(
            y >= list.absolute_position().y && y + card.size().height <= bottom,
            "step {step}: the open card stays inside the list ({y} in {}..{bottom})",
            list.absolute_position().y
        );
    }
    key(&w, Key::DownArrow);
    assert_eq!(open_title(&w), order[29], "no wrap below the last paper");
    for _ in 0..29 {
        key(&w, Key::UpArrow);
    }
    assert_eq!(open_title(&w), order[0]);
    let card = element(&w, &format!("文献を開く: {}", order[0]));
    assert!(card.absolute_position().y >= list.absolute_position().y);

    // "/" moves to the search box; what is typed there is text, not a command.
    press(&w, "/");
    press(&w, "x");
    assert_eq!(w.get_search_text(), "x");
    key(&w, Key::DownArrow);
    assert_eq!(
        open_title(&w),
        order[0],
        "arrow keys belong to the search box now"
    );
    press(&w, "/");
    assert_eq!(
        w.get_search_text(),
        "x/",
        "a slash typed in the box is only text"
    );
    w.set_search_text("".into());

    // Two or more selected: no paper is open, so the arrows do nothing until Esc.
    open(&w, &order[0]);
    w.set_pending_ctrl(true);
    w.set_pending_shift(false);
    let second = w
        .get_papers()
        .iter()
        .find(|p| p.title == order[2])
        .unwrap()
        .id;
    w.invoke_request_navigation(NavigationAction::MultiSelect, second);
    wait(&w);
    assert_eq!(w.get_selected_count(), 2);
    key(&w, Key::DownArrow);
    assert_eq!(
        w.get_selected_count(),
        2,
        "arrows leave a multi-selection alone"
    );

    // An unsaved edit is protected as with a click.
    open(&w, &order[0]);
    activate(&w, "編集");
    w.set_paper_title("Unsaved draft".into());
    w.set_dirty(true);
    activate(&w, &format!("文献を開く: {}", order[0]));
    assert!(
        !w.get_confirm_discard(),
        "clicking the open paper keeps the draft"
    );
    key(&w, Key::DownArrow);
    assert!(
        w.get_confirm_discard(),
        "Down asks before leaving the draft"
    );
    assert_eq!(open_title(&w), "Unsaved draft");
    activate(&w, "編集を続ける");
    assert!(!w.get_confirm_discard());
    assert_eq!(open_title(&w), "Unsaved draft");
}

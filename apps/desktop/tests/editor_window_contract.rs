use i_slint_backend_testing::ElementHandle;
use paper_desktop::{
    AuthorAction, MainWindow, NavigationAction,
    application::{Application, PaperForm},
    config::Config,
    ui,
};
use slint::{
    ComponentHandle, Model,
    platform::{PointerEventButton, WindowEvent},
};
use std::time::{Duration, Instant};
fn wait(window: &MainWindow) {
    let weak = window.as_weak();
    let start = Instant::now();
    let timer = slint::Timer::default();
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
            if !window.get_busy() {
                slint::quit_event_loop().unwrap();
            }
        },
    );
    slint::run_event_loop_until_quit().unwrap();
}

fn element(window: &MainWindow, label: &str) -> ElementHandle {
    let elements: Vec<_> = ElementHandle::find_by_accessible_label(window, label).collect();
    assert_eq!(elements.len(), 1, "unique visible control: {label}");
    elements.into_iter().next().unwrap()
}

fn click(window: &MainWindow, label: &str) {
    click_element(window, &element(window, label));
}

fn click_element(window: &MainWindow, element: &ElementHandle) {
    let origin = element.absolute_position();
    let size = element.size();
    let position =
        slint::LogicalPosition::new(origin.x + size.width / 2., origin.y + size.height / 2.);
    window
        .window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    window.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    window
        .window()
        .dispatch_event(WindowEvent::PointerReleased {
            position,
            button: PointerEventButton::Left,
        });
}

fn activate(w: &MainWindow, label: &str) {
    element(w, label).invoke_accessible_default_action();
}
fn names(w: &MainWindow) -> Vec<String> {
    w.get_author_rows()
        .iter()
        .map(|a| a.name.to_string())
        .collect()
}

#[test]
fn read_edit_cancel_save_errors_author_indices_and_new_navigation_keep_their_contracts() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let path = std::env::temp_dir().join(format!(
        "paper-editor-{}.sqlite",
        paper_core::model::PaperId::new()
    ));
    let w = MainWindow::new().unwrap();
    w.show().unwrap();
    w.window().set_size(slint::LogicalSize::new(1360., 900.));
    let controller = ui::attach(
        &w,
        Config {
            database: path.clone(),
        },
    );
    wait(&w);
    assert!(w.get_edit_mode());
    assert!(!w.get_saved_paper());
    w.set_citation_key("Step8".into());
    element(&w, "文献タイトル").set_accessible_value("Saved title");
    assert!(
        w.get_dirty(),
        "editing title through UI marks the shared draft dirty"
    );
    for (index, name) in ["Smith", "Group", "Literal"].into_iter().enumerate() {
        w.invoke_author_action(AuthorAction::Add, 0);
        w.invoke_author_edit(index as i32, 0, name.into());
        w.invoke_author_kind(index as i32, index as i32);
    }
    w.invoke_author_edit(0, 1, "Ada".into());
    let down = ElementHandle::find_by_accessible_label(&w, "↓").collect::<Vec<_>>();
    down[0].invoke_accessible_default_action();
    assert_eq!(names(&w), ["Group", "Smith", "Literal"]);
    let up = ElementHandle::find_by_accessible_label(&w, "↑").collect::<Vec<_>>();
    up[2].invoke_accessible_default_action();
    assert_eq!(names(&w), ["Group", "Literal", "Smith"]);
    let remove = ElementHandle::find_by_accessible_label(&w, "削除").collect::<Vec<_>>();
    remove[1].invoke_accessible_default_action();
    assert_eq!(names(&w), ["Group", "Smith"]);
    w.set_year("invalid".into());
    activate(&w, "文献を登録");
    wait(&w);
    assert!(w.get_edit_mode());
    assert!(w.get_dirty());
    assert!(!w.get_saved_paper());
    assert_eq!(w.get_total(), 0);
    w.set_year("2024".into());
    activate(&w, "文献を登録");
    wait(&w);
    assert!(
        !w.get_edit_mode(),
        "a successful save and reload opens read view"
    );
    assert_eq!(w.get_paper_authors(), "Group, Ada Smith");
    let id = w.get_editing_id();
    assert!(
        ElementHandle::find_by_accessible_label(&w, "文献タイトル")
            .next()
            .is_none()
    );
    click(&w, "編集");
    assert!(w.get_edit_mode());
    element(&w, "文献タイトル").set_accessible_value("Draft title");
    w.set_year("invalid".into());
    activate(&w, "変更を保存");
    wait(&w);
    assert!(w.get_edit_mode());
    assert!(w.get_dirty());
    assert_eq!(w.get_paper_title(), "Draft title");
    activate(&w, "編集をキャンセル");
    assert!(w.get_confirm_discard());
    click(&w, "編集を続ける");
    assert!(w.get_edit_mode());
    assert_eq!(w.get_paper_title(), "Draft title");
    activate(&w, "編集をキャンセル");
    click(&w, "破棄して移動");
    wait(&w);
    assert!(!w.get_edit_mode());
    assert!(!w.get_dirty());
    assert_eq!(w.get_paper_title(), "Saved title");
    assert_eq!(w.get_year(), "2024");
    click(&w, "編集");
    element(&w, "文献タイトル").set_accessible_value("Conflicting draft");
    {
        let other = Application {
            store: paper_storage::SqliteLibrary::open_local(&path).unwrap(),
        };
        let state = other
            .open(paper_core::model::PaperId::from_uuid(
                uuid::Uuid::parse_str(&id).unwrap(),
            ))
            .unwrap();
        let mut form = PaperForm::from_paper(&state.paper);
        form.notes = "Changed by another session".into();
        other.save(Some(&state), &form).unwrap();
    }
    activate(&w, "変更を保存");
    wait(&w);
    assert!(w.get_edit_mode());
    assert!(w.get_dirty());
    assert_eq!(w.get_paper_title(), "Conflicting draft");
    activate(&w, "編集をキャンセル");
    click(&w, "破棄して移動");
    wait(&w);
    assert!(!w.get_edit_mode());
    assert_eq!(w.get_notes(), "Changed by another session");
    assert_eq!(w.get_paper_title(), "Saved title");
    click(&w, "編集");
    activate(&w, "削除する…");
    assert!(w.get_confirm_delete());
    click(&w, "削除済みに移動");
    wait(&w);
    assert!(w.get_editor_deleted());
    assert!(!w.get_edit_mode());
    activate(&w, "復元する");
    wait(&w);
    assert!(!w.get_editor_deleted());
    assert!(!w.get_edit_mode());
    click(&w, "DOI / arXiv から登録");
    wait(&w);
    assert!(w.get_lookup_open());
    assert!(w.get_lookup_new());
    click(&w, "手動で登録");
    wait(&w);
    assert!(!w.get_lookup_open());
    assert!(w.get_edit_mode());
    assert!(!w.get_saved_paper());
    w.invoke_request_navigation(NavigationAction::OpenPaper, id);
    wait(&w);
    assert!(!w.get_edit_mode());
    w.hide().unwrap();
    drop(controller);
    drop(w);
    std::fs::remove_file(path).unwrap();
}

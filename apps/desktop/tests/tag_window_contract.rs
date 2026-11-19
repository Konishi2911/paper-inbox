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
fn names(rows: impl Model<Data = paper_desktop::TagRow>) -> Vec<String> {
    rows.iter().map(|r| r.name.to_string()).collect()
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

#[test]
fn tags_are_added_completed_filtered_and_removed_from_the_window() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let sandbox =
        Sandbox(std::env::temp_dir().join(format!("paper-inbox-tags-{}", PaperId::new())));
    std::fs::create_dir_all(&sandbox.0).unwrap();
    let database = sandbox.0.join("library.sqlite");
    let (first, second) = (paper("First2026"), paper("Second2026"));
    {
        let store = SqliteLibrary::open_local(&database).unwrap();
        for p in [&first, &second] {
            store
                .commit_at(LibraryCommand::CreatePaper { paper: p.clone() }, Utc::now())
                .unwrap();
        }
    }

    let w = MainWindow::new().unwrap();
    w.show().unwrap();
    w.window().set_size(slint::LogicalSize::new(1360., 900.));
    let controller = ui::attach_with_providers(
        &w,
        Config {
            database: database.clone(),
        },
        Box::new(|_| Ok(vec![])),
        Box::new(|| None),
    );
    wait(&w);
    w.invoke_request_navigation(NavigationAction::OpenPaper, first.id().to_string().into());
    wait(&w);
    assert_eq!(w.get_detail_view(), DetailView::PaperDetail);
    assert_eq!(w.get_paper_tag_rows().row_count(), 0);

    // Typing a name and pressing Enter creates and assigns the tag.
    let input = element(&w, "タグを追加");
    input.set_accessible_value("Machine Learning");
    w.invoke_add_tag("Machine Learning".into());
    wait(&w);
    assert_eq!(names(w.get_paper_tag_rows()), ["Machine Learning"]);
    assert_eq!(names(w.get_tag_rows()), ["Machine Learning"]);
    assert_eq!(w.get_tag_rows().row_data(0).unwrap().count, 1);
    element(&w, "Machine Learningを外す");

    // The same tag by another spelling is refused with a reason and changes nothing.
    w.invoke_add_tag("ＭＡＣＨＩＮＥ  learning".into());
    wait(&w);
    assert!(w.get_status().contains("すでに"), "{}", w.get_status());
    assert_eq!(w.get_paper_tag_rows().row_count(), 1);

    // On the other paper, typing offers the existing tag and picking it reuses it.
    w.invoke_request_navigation(NavigationAction::OpenPaper, second.id().to_string().into());
    wait(&w);
    assert_eq!(w.get_paper_tag_rows().row_count(), 0);
    w.invoke_tag_input_changed("mach".into());
    assert_eq!(names(w.get_tag_suggestions()), ["Machine Learning"]);
    activate(&w, "Machine Learningを付ける");
    wait(&w);
    assert_eq!(names(w.get_paper_tag_rows()), ["Machine Learning"]);
    assert_eq!(w.get_tag_rows().row_count(), 1, "no duplicate tag");
    assert_eq!(w.get_tag_rows().row_data(0).unwrap().count, 2);
    assert_eq!(w.get_tag_suggestions().row_count(), 0);

    // A note draft blocks tagging, which saves at once.
    w.set_dirty(true);
    assert_eq!(
        element(&w, "Machine Learningを外す").accessible_enabled(),
        Some(false)
    );
    w.set_dirty(false);

    // Remove from the second paper, then filter the list by the tag from the sidebar.
    activate(&w, "Machine Learningを外す");
    wait(&w);
    assert_eq!(w.get_paper_tag_rows().row_count(), 0);
    assert_eq!(w.get_tag_rows().row_data(0).unwrap().count, 1);
    assert_eq!(w.get_papers().row_count(), 2);
    activate(&w, "タグ Machine Learningで絞り込む");
    wait(&w);
    assert_eq!(w.get_papers().row_count(), 1);
    assert_eq!(w.get_papers().row_data(0).unwrap().title, "First2026");
    assert!(w.get_tag_rows().row_data(0).unwrap().selected);
    activate(&w, "タグ Machine Learningの絞り込みを解除");
    wait(&w);
    assert_eq!(w.get_papers().row_count(), 2);
    assert!(absent(&w, "タグ Machine Learningの絞り込みを解除"));

    // The tag stays on the first paper in the database.
    let store = SqliteLibrary::open_local(&database).unwrap();
    assert_eq!(store.paper_tags(first.id()).unwrap().len(), 1);
    assert!(store.paper_tags(second.id()).unwrap().is_empty());

    drop(controller);
    w.hide().unwrap();
}

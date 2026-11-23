use chrono::Utc;
use paper_core::{
    command::LibraryCommand,
    model::{Bibliography, CitationKey, Paper, PaperId, Tag, TagId},
};
use paper_desktop::{MainWindow, config::Config, ui};
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

struct Sandbox(PathBuf);
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn renaming_a_tag_keeps_the_dialog_open_on_a_clash_and_closes_it_on_success() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let sandbox =
        Sandbox(std::env::temp_dir().join(format!("paper-inbox-rename-{}", PaperId::new())));
    std::fs::create_dir_all(&sandbox.0).unwrap();
    let database = sandbox.0.join("library.sqlite");
    {
        let store = SqliteLibrary::open_local(&database).unwrap();
        let paper = Paper::new(
            PaperId::new(),
            CitationKey::parse("Key2026").unwrap(),
            Bibliography {
                title: Some("A paper".into()),
                ..Default::default()
            },
            Utc::now(),
        )
        .unwrap();
        let paper_id = paper.id();
        store
            .commit_at(LibraryCommand::CreatePaper { paper }, Utc::now())
            .unwrap();
        for name in ["ml", "nlp"] {
            let tag = Tag {
                id: TagId::new(),
                name: name.into(),
            };
            store
                .commit_at(LibraryCommand::CreateTag { tag: tag.clone() }, Utc::now())
                .unwrap();
            store
                .commit_at(
                    LibraryCommand::AddTag {
                        paper_id,
                        tag_id: tag.id,
                    },
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
        Config { database },
        Box::new(|_| Ok(vec![])),
        Box::new(|| None),
    );
    wait(&w);
    let id_of = |name: &str| w.get_tag_rows().iter().find(|t| t.name == name).unwrap().id;
    let names = || -> Vec<String> {
        w.get_tag_rows()
            .iter()
            .map(|t| t.name.to_string())
            .collect()
    };
    assert_eq!(names(), ["ml", "nlp"]);

    // The dialog is open (as the menu opens it); a name that is taken is refused and explained.
    w.set_rename_tag_id(id_of("ml"));
    w.set_rename_old_name("ml".into());
    w.set_rename_open(true);
    w.invoke_rename_tag(id_of("ml"), "NLP".into());
    wait(&w);
    assert!(w.get_rename_open(), "the dialog stays open after a refusal");
    assert!(
        w.get_rename_error().contains("すでにあります"),
        "{}",
        w.get_rename_error()
    );
    assert_eq!(names(), ["ml", "nlp"]);

    // A free name renames the tag, closes the dialog and refreshes the sidebar.
    w.invoke_rename_tag(id_of("ml"), "machine learning".into());
    wait(&w);
    assert!(!w.get_rename_open());
    assert_eq!(w.get_rename_error(), "");
    assert_eq!(names(), ["machine learning", "nlp"]);
    assert!(w.get_status().contains("machine learning"));
}

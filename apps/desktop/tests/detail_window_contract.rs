use i_slint_backend_testing::ElementHandle;
use paper_core::model::*;
use paper_desktop::{
    DetailView, MainWindow, NavigationAction, application::Application, config::Config, ui,
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
    let completed = std::rc::Rc::new(std::cell::Cell::new(false));
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

/// Pasting into the search box starts its 300 ms "search while typing" timer, and clearing the
/// text from code does not stop it. Without letting it run, the timer fires later inside some
/// unrelated click, whose search request then disables the button being clicked.
fn settle_search(window: &MainWindow) {
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::SingleShot,
        Duration::from_millis(400),
        || slint::quit_event_loop().unwrap(),
    );
    slint::run_event_loop_until_quit().unwrap();
    wait(window);
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
fn candidate() -> MetadataCandidate {
    MetadataCandidate {
        id: CandidateId::new(),
        bibliography: Bibliography {
            title: Some("Graph methods".into()),
            authors: vec![Contributor {
                name: ContributorName::Person {
                    family: "Smith".into(),
                    given: Some("Ada".into()),
                },
                orcid: None,
            }],
            identifiers: Identifiers {
                doi: Some(Doi::parse("10.1000/a").unwrap()),
                ..Default::default()
            },
            publication: Publication {
                year: Some(2024),
                ..Default::default()
            },
            ..Default::default()
        },
        present_fields: vec![
            BibliographyField::Title,
            BibliographyField::Authors,
            BibliographyField::Identifiers,
            BibliographyField::Year,
        ],
        provenance: Provenance {
            source: MetadataSource::Crossref,
            source_record_id: Some("10.1000/a".into()),
            observed_at: chrono::Utc::now(),
            source_updated_at: None,
        },
        version_kind: VersionKind::VersionOfRecord,
    }
}
fn activate(w: &MainWindow, label: &str) {
    element(w, label).invoke_accessible_default_action();
}
fn key(w: &MainWindow, text: slint::SharedString, pressed: bool) {
    w.window().dispatch_event(if pressed {
        WindowEvent::KeyPressed { text }
    } else {
        WindowEvent::KeyReleased { text }
    });
}
fn control_chord(w: &MainWindow, letter: &str) {
    let control: slint::SharedString = slint::platform::Key::Control.into();
    key(w, control.clone(), true);
    key(w, letter.into(), true);
    key(w, letter.into(), false);
    key(w, control, false);
}
fn stored(path: &std::path::Path, id: &str) -> paper_core::model::Paper {
    let app = Application {
        store: paper_storage::SqliteLibrary::open_local(path).unwrap(),
    };
    let id = paper_core::model::PaperId::from_uuid(uuid::Uuid::parse_str(id).unwrap());
    app.open(id).unwrap().paper
}

#[test]
fn detail_quick_actions_copy_notes_and_form_shortcuts_keep_their_contracts() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let path = std::env::temp_dir().join(format!(
        "paper-detail-{}.sqlite",
        paper_core::model::PaperId::new()
    ));
    let w = MainWindow::new().unwrap();
    w.show().unwrap();
    w.window().set_size(slint::LogicalSize::new(1360., 900.));
    let controller = ui::attach_with_lookup_provider(
        &w,
        Config {
            database: path.clone(),
        },
        Box::new(|_| {
            let mut c = candidate();
            c.bibliography.publication.volume = Some("30".into());
            c.bibliography.publication.pages = Some("1-10".into());
            c.bibliography.abstract_text = Some("x".repeat(400));
            c.present_fields.extend([
                paper_core::model::BibliographyField::Volume,
                paper_core::model::BibliographyField::Pages,
                paper_core::model::BibliographyField::Abstract,
            ]);
            Ok(vec![c])
        }),
    );
    wait(&w);
    w.invoke_request_navigation(NavigationAction::ImportPaper, "".into());
    wait(&w);
    w.set_lookup_identifier("10.1000/a".into());
    w.invoke_fetch_lookup();
    wait(&w);
    w.invoke_confirm_lookup();
    wait(&w);
    assert_eq!(w.get_detail_view(), DetailView::PaperDetail);
    let id = w.get_editing_id();

    // Fetched fields are visible in the read-only detail.
    assert_eq!(w.get_publication_summary(), "30, pp. 1-10, 2024");
    assert!(w.get_abstract_long());
    assert!(w.get_paper_bibtex().contains("Smith2024"));
    // A long unbreakable abstract must not push right-aligned actions out of view.
    element(&w, "DOIを開く");
    let key_copy = element(&w, "引用キーをコピー");
    assert!(key_copy.absolute_position().x + key_copy.size().width <= 1360.);
    // The identifiers are also on one selectable line, so their values stay visible.
    assert_eq!(w.get_identifiers_line(), "DOI 10.1000/a");
    element(&w, "識別子");
    // At the minimum window size the toolbar and action row still fit (two rows when narrow).
    w.window().set_size(slint::LogicalSize::new(1080., 680.));
    for label in [
        "編集",
        "引用キーをコピー",
        "BibTeX をコピー",
        "DOIを開く",
        "その他の操作",
        "お気に入りに追加",
    ] {
        let control = element(&w, label);
        assert!(
            control.absolute_position().x + control.size().width <= 1080.,
            "{label} at x={}",
            control.absolute_position().x + control.size().width
        );
    }
    w.window().set_size(slint::LogicalSize::new(1360., 900.));
    element(&w, "BibTeX をコピー");

    // Copy goes through the widget clipboard; paste it into the search box.
    activate(&w, "引用キーをコピー");
    click(&w, "文献を検索");
    control_chord(&w, "v");
    assert_eq!(w.get_search_text(), "Smith2024");
    w.set_search_text("".into());
    assert_eq!(w.get_status(), "引用キーをコピーしました。");
    settle_search(&w);

    // Less used copies are in the menu; they say what was copied and use the same clipboard.
    activate(&w, "その他の操作");
    activate(&w, "DOIをコピー");
    assert_eq!(w.get_status(), "DOIをコピーしました。");
    click(&w, "文献を検索");
    control_chord(&w, "v");
    assert_eq!(w.get_search_text(), "10.1000/a");
    w.set_search_text("".into());
    settle_search(&w);

    // Quick actions save one value and reload the detail.
    click(&w, "お気に入りに追加");
    wait(&w);
    assert!(w.get_favorite(), "{}", w.get_status());
    assert!(stored(&path, &id).metadata().favorite);
    element(&w, "お気に入りから外す");
    w.invoke_change_reading_status(2);
    wait(&w);
    assert_eq!(w.get_reading_status(), 2);
    assert_eq!(
        stored(&path, &id).metadata().reading_status,
        paper_core::model::ReadingStatus::Read
    );

    // Notes are edited in place and protected like any other draft.
    click(&w, "メモを書く");
    assert!(w.get_notes_editing());
    element(&w, "メモの入力").set_accessible_value("in-place note");
    assert!(w.get_dirty());
    w.invoke_request_navigation(NavigationAction::ImportPaper, "".into());
    assert!(w.get_confirm_discard(), "a note draft asks before leaving");
    click(&w, "編集を続ける");
    assert_eq!(w.get_notes(), "in-place note");
    click(&w, "メモを保存");
    wait(&w);
    assert!(!w.get_notes_editing(), "{}", w.get_status());
    assert!(!w.get_dirty());
    assert_eq!(stored(&path, &id).metadata().notes, "in-place note");

    // The form shows fetched fields, reports errors beside its actions and saves with Ctrl+S.
    click(&w, "編集");
    assert_eq!(w.get_detail_view(), DetailView::PaperEditor);
    assert_eq!(w.get_volume(), "30");
    element(&w, "出版年").set_accessible_value("invalid");
    click(&w, "ページ");
    control_chord(&w, "s");
    wait(&w);
    assert_eq!(w.get_detail_view(), DetailView::PaperEditor);
    assert!(
        w.get_form_error().contains("出版年"),
        "{}",
        w.get_form_error()
    );
    assert_eq!(w.get_year(), "invalid", "a failed save keeps the input");
    element(&w, "出版年").set_accessible_value("2025");
    element(&w, "ページ").set_accessible_value("11-20");
    control_chord(&w, "s");
    wait(&w);
    assert_eq!(
        w.get_detail_view(),
        DetailView::PaperDetail,
        "{}",
        w.get_status()
    );
    assert_eq!(w.get_form_error(), "");
    assert_eq!(
        stored(&path, &id)
            .bibliography()
            .publication
            .pages
            .as_deref(),
        Some("11-20")
    );

    // Manual entry: pasted authors become rows and an empty key is suggested.
    w.invoke_request_navigation(NavigationAction::CreatePaper, "".into());
    wait(&w);
    element(&w, "文献タイトル").set_accessible_value("Manual paper");
    activate(&w, "まとめて入力");
    element(&w, "著者をまとめて入力").set_accessible_value("Doe, Jane; John Roe");
    activate(&w, "一覧に追加");
    let names: Vec<_> = w
        .get_author_rows()
        .iter()
        .map(|r| r.name.to_string())
        .collect();
    assert_eq!(names, ["Doe", "Roe"]);
    element(&w, "著者2の姓");
    element(&w, "出版年").set_accessible_value("2023");
    assert_eq!(w.get_citation_key(), "");
    activate(&w, "文献を登録");
    wait(&w);
    assert_eq!(
        w.get_detail_view(),
        DetailView::PaperDetail,
        "{}",
        w.get_status()
    );
    assert_eq!(w.get_citation_key(), "Doe2023");
    assert_eq!(w.get_total(), 2);

    w.hide().unwrap();
    drop(controller);
    drop(w);
    std::fs::remove_file(path).unwrap();
}

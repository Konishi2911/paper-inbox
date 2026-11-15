use i_slint_backend_testing::ElementHandle;
use paper_desktop::NavigationAction;
use paper_desktop::{MainWindow, config::Config, ui};
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

#[test]
fn three_panes_support_direct_clicks_collapsing_export_and_protected_navigation() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let path = std::env::temp_dir().join(format!(
        "paper-layout-{}.sqlite",
        paper_core::model::PaperId::new()
    ));
    let window = MainWindow::new().unwrap();
    window.show().unwrap();
    window
        .window()
        .set_size(slint::LogicalSize::new(1360., 900.));
    let controller = ui::attach(
        &window,
        Config {
            database: path.clone(),
        },
    );
    wait(&window);
    for (key, title) in [("Smith2024", "First paper"), ("Jones2025", "Second paper")] {
        window.invoke_request_navigation(NavigationAction::CreatePaper, "".into());
        wait(&window);
        window.set_citation_key(key.into());
        window.set_paper_title(title.into());
        window.set_dirty(true);
        window.invoke_save();
        wait(&window);
    }
    let second_id = window.get_editing_id();
    window.set_paper_title("About dialog draft".into());
    window.set_dirty(true);
    click(&window, "このアプリについて");
    assert!(window.get_about_open());
    assert!(
        ElementHandle::find_by_accessible_label(&window, "#MadeWithSlint")
            .next()
            .is_some()
    );
    assert_eq!(window.get_paper_title(), "About dialog draft");
    click(&window, "閉じる");
    assert!(!window.get_about_open());
    assert!(window.get_dirty());
    assert_eq!(window.get_editing_id(), second_id);
    click(&window, "このアプリについて");
    window.window().dispatch_event(WindowEvent::KeyPressed {
        text: slint::platform::Key::Escape.into(),
    });
    assert!(
        !window.get_about_open(),
        "Escape closes the focused about dialog"
    );
    assert!(window.get_dirty());
    window.set_paper_title("Second paper".into());
    window.set_dirty(false);
    let card = element(&window, "文献を開く: First paper");
    let expanded_x = card.absolute_position().x;
    assert!(expanded_x >= 208.);
    assert!(card.size().width >= 300.);
    click(&window, "文献を開く: First paper");
    wait(&window);
    assert_eq!(window.get_paper_title(), "First paper");
    let first_id = window.get_editing_id();
    assert_ne!(first_id, second_id);
    assert_eq!(
        window.get_selected_count(),
        0,
        "opening is independent of export selection"
    );

    click(&window, "メニューを折りたたむ");
    assert!(window.get_sidebar_collapsed());
    assert!(
        element(&window, "文献を開く: First paper")
            .absolute_position()
            .x
            < expanded_x
    );
    assert_eq!(window.get_editing_id(), first_id);
    click(&window, "メニューを展開");
    assert!(!window.get_sidebar_collapsed());

    window.set_paper_title("Unsaved draft".into());
    window.set_dirty(true);
    click(&window, "文献を開く: First paper");
    assert!(
        !window.get_confirm_discard(),
        "clicking current paper keeps the draft"
    );
    click(&window, "文献を開く: Second paper");
    assert!(window.get_confirm_discard());
    assert_eq!(window.get_paper_title(), "Unsaved draft");
    click(&window, "編集を続ける");
    assert!(!window.get_confirm_discard());
    assert_eq!(window.get_editing_id(), first_id);
    click(&window, "文献を開く: Second paper");
    click(&window, "破棄して移動");
    wait(&window);
    assert_eq!(window.get_editing_id(), second_id);
    assert_eq!(window.get_paper_title(), "Second paper");

    window
        .window()
        .set_size(slint::LogicalSize::new(1080., 680.));
    let detail_action = element(&window, "候補・版を確認");
    assert!(
        detail_action.absolute_position().x
            > element(&window, "文献を開く: Second paper")
                .absolute_position()
                .x
    );
    assert!(detail_action.absolute_position().x + detail_action.size().width <= 1080.);
    let save = element(&window, "変更を保存");
    assert!(save.absolute_position().y + save.size().height <= 680.);
    window
        .window()
        .set_size(slint::LogicalSize::new(1360., 900.));
    click(&window, "BibTeX 出力 (0)");
    assert!(window.get_export_settings_open());
    assert!(
        element(&window, "出力内容を確認").absolute_position().x
            > element(&window, "文献を開く: First paper")
                .absolute_position()
                .x
    );
    assert_eq!(window.get_editing_id(), second_id);
    let checks: Vec<_> =
        ElementHandle::find_by_accessible_label(&window, "BibTeX 出力に含める").collect();
    assert_eq!(checks.len(), 2);
    for (index, checkbox) in checks.iter().enumerate() {
        let title = window.get_papers().row_data(index).unwrap().title;
        let card = element(&window, &format!("文献を開く: {title}"));
        assert!(checkbox.absolute_position().y >= card.absolute_position().y);
        assert!(
            checkbox.absolute_position().y + checkbox.size().height
                <= card.absolute_position().y + card.size().height,
            "export checkbox must fit inside its card"
        );
    }

    click_element(&window, &checks[0]);
    assert_eq!(window.get_selected_count(), 1);
    assert_eq!(
        window.get_editing_id(),
        second_id,
        "export checkbox does not open a paper"
    );

    window.invoke_request_navigation(NavigationAction::MetadataReview, first_id.clone());
    wait(&window);
    assert!(window.get_metadata_open());
    assert_eq!(window.get_papers().row_count(), 2);
    click(&window, "文献を開く: Second paper");
    wait(&window);
    assert!(!window.get_metadata_open());
    assert_eq!(window.get_editing_id(), second_id);
    click(&window, "DOI / arXiv から登録");
    wait(&window);
    assert!(window.get_lookup_open());
    assert_eq!(window.get_papers().row_count(), 2);
    click(&window, "文献を開く: First paper");
    wait(&window);
    assert!(!window.get_lookup_open());
    assert_eq!(window.get_editing_id(), first_id);

    window.hide().unwrap();
    drop(controller);
    drop(window);
    std::fs::remove_file(path).unwrap();
}

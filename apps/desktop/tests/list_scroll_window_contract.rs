use i_slint_backend_testing::ElementHandle;
use paper_desktop::{MainWindow, PaperRow};
use slint::{ComponentHandle, ModelRc, VecModel, platform::WindowEvent};

fn first_card_y(w: &MainWindow) -> f32 {
    ElementHandle::find_by_accessible_label(w, "文献を開く: Paper 3")
        .next()
        .expect("card")
        .absolute_position()
        .y
}

#[test]
fn the_paper_list_scrolls_with_the_wheel() {
    i_slint_backend_testing::init_no_event_loop();
    let w = MainWindow::new().unwrap();
    w.set_ready(true);
    w.set_busy(false);
    w.show().unwrap();
    w.window().set_size(slint::LogicalSize::new(1080., 680.));
    let rows: Vec<PaperRow> = (0..50)
        .map(|i| PaperRow {
            id: format!("id{i}").into(),
            title: format!("Paper {i}").into(),
            authors: "A. Author".into(),
            year: "2026".into(),
            citation_key: format!("Key{i}").into(),
            selected: false,
        })
        .collect();
    w.set_papers(ModelRc::new(VecModel::from(rows)));
    let _ = ElementHandle::find_by_accessible_label(&w, "文献を開く: Paper 3").count();
    w.set_total(120);
    let asked = std::rc::Rc::new(std::cell::Cell::new(0));
    let counter = asked.clone();
    w.on_search(move || counter.set(counter.get() + 1));
    let before = first_card_y(&w);
    let position = slint::LogicalPosition::new(300., before + 10.);
    w.window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    w.window().dispatch_event(WindowEvent::PointerScrolled {
        position,
        delta_x: 0.,
        delta_y: -100.,
    });
    let after = first_card_y(&w);
    assert!(after < before, "list did not scroll: {before} -> {after}");
    assert_eq!(asked.get(), 0, "nothing is requested far from the end");
    for _ in 0..40 {
        w.window().dispatch_event(WindowEvent::PointerScrolled {
            position,
            delta_x: 0.,
            delta_y: -400.,
        });
    }
    assert_eq!(w.get_limit(), 100, "reaching the end loads the next batch");
    assert!(asked.get() >= 1);
}

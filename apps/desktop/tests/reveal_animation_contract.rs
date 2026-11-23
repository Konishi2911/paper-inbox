use i_slint_backend_testing::{ElementHandle, mock_elapsed_time};
use paper_desktop::{DetailView, MainWindow};
use slint::ComponentHandle;
use std::time::Duration;

fn advance(ms: u64) {
    mock_elapsed_time(Duration::from_millis(ms));
    // Timers see the new time on the update after the one that moves the animation clock.
    slint::platform::update_timers_and_animations();
    slint::platform::update_timers_and_animations();
}
fn find(w: &MainWindow, label: &str) -> Option<ElementHandle> {
    ElementHandle::find_by_accessible_label(w, label).next()
}
/// The height of the box that holds the abstract text: the second `Reveal` on the detail.
fn abstract_height(w: &MainWindow) -> f32 {
    ElementHandle::find_by_element_type_name(w, "Reveal")
        .nth(1)
        .expect("the abstract has its box")
        .size()
        .height
}

#[test]
fn swapped_text_moves_to_its_new_height_over_time_but_follows_a_resize_at_once() {
    i_slint_backend_testing::init_no_event_loop();
    let w = MainWindow::new().unwrap();
    w.set_ready(true);
    w.set_busy(false);
    w.set_detail_view(DetailView::PaperDetail);
    w.set_paper_title("A paper".into());
    w.set_paper_authors("Smith, J.".into());
    w.set_paper_authors_short("Smith, J.".into());
    let long = "A long sentence about many things. ".repeat(40);
    w.set_abstract_text(long.as_str().into());
    w.set_abstract_preview("A long sentence about many things. …".into());
    w.set_abstract_long(true);
    w.show().unwrap();
    w.window().set_size(slint::LogicalSize::new(1360., 900.));
    advance(1);
    let short = abstract_height(&w);

    // Showing the rest: the box grows to the height of the full text, without jumping.
    find(&w, "続きを表示")
        .expect("the toggle is shown")
        .invoke_accessible_default_action();
    advance(1);
    let mut last = abstract_height(&w);
    assert!(
        last < short + 30.,
        "it starts near the short height: {last}"
    );
    for _ in 0..20 {
        advance(10);
        let height = abstract_height(&w);
        assert!(height >= last - 0.01, "grows without going back");
        last = height;
    }
    advance(300);
    let full = abstract_height(&w);
    assert!(
        full > short + 100.,
        "the full text is taller: {short} -> {full}"
    );

    // Closing it again moves back to the short height.
    find(&w, "折りたたむ")
        .expect("the toggle now folds")
        .invoke_accessible_default_action();
    advance(60);
    let mid = abstract_height(&w);
    assert!(mid < full && mid > short, "in between: {mid}");
    advance(300);
    assert!((abstract_height(&w) - short).abs() < 1.0);

    // A change of width (resizing the window or dragging the splitter) is not delayed.
    find(&w, "続きを表示")
        .unwrap()
        .invoke_accessible_default_action();
    // Let the "animating" window of the box close: its timer needs a few updates to fire.
    for _ in 0..16 {
        advance(50);
    }
    let wide = abstract_height(&w);
    w.window().set_size(slint::LogicalSize::new(1080., 900.));
    advance(1);
    let narrow = abstract_height(&w);
    assert!(
        narrow > wide,
        "narrower text is taller at once: {wide} -> {narrow}"
    );
    advance(400);
    let later = abstract_height(&w);
    assert!(
        (later - narrow).abs() < 1.0,
        "and stays there: {wide} -> {narrow} -> {later}"
    );
}

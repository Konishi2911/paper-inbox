use i_slint_backend_testing::{ElementHandle, mock_elapsed_time};
use paper_desktop::{MainWindow, Motion};
use slint::ComponentHandle;

/// Moves the mocked clock forward and lets Slint run its animations up to that time.
fn advance(ms: u64) {
    mock_elapsed_time(std::time::Duration::from_millis(ms));
    slint::platform::update_timers_and_animations();
}
fn sidebar_width(w: &MainWindow) -> f32 {
    ElementHandle::find_by_element_type_name(w, "Sidebar")
        .next()
        .unwrap()
        .size()
        .width
}
fn labels_shown(w: &MainWindow) -> bool {
    ElementHandle::find_by_accessible_label(w, "ライブラリ")
        .next()
        .is_some()
}

#[test]
fn the_sidebar_slides_and_its_labels_follow_the_width() {
    i_slint_backend_testing::init_no_event_loop();
    let w = MainWindow::new().unwrap();
    w.set_ready(true);
    w.set_busy(false);
    w.show().unwrap();
    w.window().set_size(slint::LogicalSize::new(1360., 900.));
    assert_eq!(sidebar_width(&w), 208.);
    assert!(labels_shown(&w));

    // Collapsing: the width moves over time, from 208 to 64 and monotonically; the labels go
    // while the bar is still wide enough to hold them, never squeezed into an ellipsis.
    w.set_sidebar_collapsed(true);
    advance(1);
    assert!(sidebar_width(&w) > 190., "starts from the open width");
    let mut last = sidebar_width(&w);
    for _ in 0..30 {
        advance(10);
        let width = sidebar_width(&w);
        assert!(width <= last + 0.01, "shrinks without going back");
        assert!(
            labels_shown(&w) == (width >= 190.),
            "labels follow the width ({width})"
        );
        last = width;
    }
    assert_eq!(sidebar_width(&w), 64.);
    assert!(!labels_shown(&w));

    // Expanding is the same movement the other way.
    w.set_sidebar_collapsed(false);
    advance(120);
    let middle = sidebar_width(&w);
    assert!(
        middle > 64. && middle < 208.,
        "in between at half time: {middle}"
    );
    advance(200);
    assert_eq!(sidebar_width(&w), 208.);
    assert!(labels_shown(&w));

    // With motion off the change lands at once.
    w.global::<Motion>().set_enabled(false);
    w.set_sidebar_collapsed(true);
    advance(1);
    assert_eq!(sidebar_width(&w), 64.);
}

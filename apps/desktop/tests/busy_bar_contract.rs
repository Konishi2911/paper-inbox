use i_slint_backend_testing::{ElementHandle, mock_elapsed_time};
use paper_desktop::MainWindow;
use slint::ComponentHandle;
use std::time::Duration;

fn advance(ms: u64) {
    mock_elapsed_time(Duration::from_millis(ms));
    // Timers see the new time on the update after the one that moves the animation clock.
    slint::platform::update_timers_and_animations();
    slint::platform::update_timers_and_animations();
}
fn bar_shown(w: &MainWindow) -> bool {
    ElementHandle::find_by_accessible_label(w, "処理の進行")
        .next()
        .is_some()
}

#[test]
fn the_busy_bar_shows_only_for_work_that_lasts_and_goes_when_it_ends() {
    i_slint_backend_testing::init_no_event_loop();
    let w = MainWindow::new().unwrap();
    w.set_ready(true);
    w.show().unwrap();
    w.window().set_size(slint::LogicalSize::new(1360., 900.));

    // The window starts busy (it is opening the library); the bar waits like for any work.
    advance(50);
    assert!(!bar_shown(&w), "not within the first 150ms");
    advance(200);
    assert!(bar_shown(&w), "shown once the work has lasted");

    // The end of the work takes the bar away.
    w.set_busy(false);
    advance(20);
    advance(20);
    assert!(!bar_shown(&w), "gone when the work is done");

    // Short work never shows it.
    w.set_busy(true);
    // (A `changed` handler runs on the update after the change, and starts the delay then.)
    advance(1);
    advance(100);
    assert!(!bar_shown(&w));
    w.set_busy(false);
    advance(300);
    assert!(!bar_shown(&w), "work that ended in time left no bar");

    // Work that lasts shows it, after 150ms.
    w.set_busy(true);
    // (A `changed` handler runs on the update after the change, and starts the delay then.)
    advance(1);
    advance(100);
    assert!(!bar_shown(&w));
    advance(100);
    assert!(bar_shown(&w));

    // The bar runs along the width: the running segment moves between updates.
    let before = segment_x(&w);
    advance(300);
    let after = segment_x(&w);
    assert!(before != after, "the segment runs: {before} -> {after}");
}

/// Horizontal position of the running segment: the one child of the bar.
fn segment_x(w: &MainWindow) -> f32 {
    ElementHandle::find_by_element_id(w, "BusyBar::segment")
        .next()
        .expect("the bar has its segment while active")
        .absolute_position()
        .x
}

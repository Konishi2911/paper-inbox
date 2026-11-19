use paper_desktop::{MainWindow, config::Config, ui};
use slint::{ComponentHandle, platform::WindowEvent};
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

/// Closing the window must ask first whenever any unsaved work exists, including decisions made
/// in the metadata diff review that were not applied yet.
#[test]
fn closing_the_window_asks_before_discarding_unapplied_review_decisions() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let path = std::env::temp_dir().join(format!(
        "paper-close-guard-{}.sqlite",
        paper_core::model::PaperId::new()
    ));
    let w = MainWindow::new().unwrap();
    w.show().unwrap();
    let controller = ui::attach(
        &w,
        Config {
            database: path.clone(),
        },
    );
    wait(&w);

    // Nothing unsaved: closing is not intercepted.
    assert!(!w.get_confirm_discard());
    w.window().dispatch_event(WindowEvent::CloseRequested);
    assert!(!w.get_confirm_discard());

    w.show().unwrap();
    w.set_review_dirty(true);
    w.window().dispatch_event(WindowEvent::CloseRequested);
    assert!(
        w.get_confirm_discard(),
        "unapplied review decisions are confirmed before the window closes"
    );
    assert!(
        w.window().is_visible(),
        "the window stays open until confirmed"
    );
    drop(controller);
    let _ = std::fs::remove_file(path);
}

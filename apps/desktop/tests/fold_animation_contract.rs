use i_slint_backend_testing::{ElementHandle, mock_elapsed_time};
use paper_desktop::{DetailView, MainWindow, Motion};
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
/// The height of the one `Fold` on screen (the bulk-input panel), or None when there is none.
fn fold_height(w: &MainWindow) -> Option<f32> {
    ElementHandle::find_by_element_type_name(w, "Fold")
        .next()
        .map(|fold| fold.size().height)
}

#[test]
fn a_folded_block_opens_and_closes_by_height() {
    i_slint_backend_testing::init_no_event_loop();
    let w = MainWindow::new().unwrap();
    w.set_ready(true);
    w.set_busy(false);
    w.set_detail_view(DetailView::PaperEditor);
    w.show().unwrap();
    w.window().set_size(slint::LogicalSize::new(1360., 900.));
    advance(1);

    // Closed: the block is not there at all.
    assert_eq!(fold_height(&w), None);

    // Opening: the height grows from 0 without ever going back, and ends at the content's height.
    find(&w, "まとめて入力")
        .expect("the toggle is shown")
        .invoke_accessible_default_action();
    advance(1);
    let mut last = fold_height(&w).expect("the block exists as soon as it opens");
    assert!(last < 5., "it starts closed: {last}");
    for _ in 0..20 {
        advance(10);
        let height = fold_height(&w).unwrap();
        assert!(
            height >= last - 0.01,
            "grows without going back: {last} -> {height}"
        );
        last = height;
    }
    assert!(last > 5., "it has grown by 200ms: {last}");
    advance(200);
    let open = fold_height(&w).unwrap();
    assert!(open > 50., "the open height holds the panel: {open}");
    assert!(
        find(&w, "一覧に追加").is_some(),
        "the button is shown in the open block"
    );

    // Closing: it shrinks the same way, is kept while it moves and is removed when done.
    find(&w, "まとめて入力を閉じる")
        .expect("the toggle now closes")
        .invoke_accessible_default_action();
    advance(80);
    let mid = fold_height(&w).expect("kept while it closes");
    assert!(mid < open && mid > 0., "in between: {mid} of {open}");
    advance(300);
    assert_eq!(fold_height(&w), None, "removed when closed");

    // With motion off the block is open at once.
    w.global::<Motion>().set_enabled(false);
    find(&w, "まとめて入力")
        .unwrap()
        .invoke_accessible_default_action();
    advance(1);
    let at_once = fold_height(&w).unwrap();
    assert!((at_once - open).abs() < 1.0, "{at_once} vs {open}");
}

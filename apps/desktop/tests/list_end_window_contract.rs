use i_slint_backend_testing::ElementHandle;
use paper_desktop::{MainWindow, PaperRow};
use slint::{ComponentHandle, ModelRc, VecModel, platform::WindowEvent};

/// Long titles used to wrap into taller cards than the ScrollView counted on, so the end of the
/// list could not be reached and the last cards stayed hidden.
#[test]
fn the_last_card_is_fully_reachable_even_when_titles_are_long() {
    i_slint_backend_testing::init_no_event_loop();
    for (width, height) in [(1080., 680.), (1360., 900.)] {
        let w = MainWindow::new().unwrap();
        w.set_ready(true);
        w.set_busy(false);
        w.show().unwrap();
        w.window().set_size(slint::LogicalSize::new(width, height));
        let rows: Vec<PaperRow> = (0..9)
            .map(|i| PaperRow {
                id: format!("id{i}").into(),
                title: if i % 2 == 0 {
                    format!("Paper {i}: A very long title about graph neural networks for molecular property prediction with attention and more words").into()
                } else {
                    format!("Paper {i}").into()
                },
                authors: "A. Author; B. Writer".into(),
                year: "2026".into(),
                citation_key: format!("Key{i}").into(),
                selected: false,
                reading_status: 1,
                favorite: true,
                has_pdf: true,
            })
            .collect();
        w.set_papers(ModelRc::new(VecModel::from(rows)));
        w.set_total(9);
        // The filters take few rows, so the cards start high and several fit at the minimum size.
        let first = ElementHandle::find_by_accessible_label(&w, "文献を開く: Paper 1")
            .next()
            .expect("second card")
            .absolute_position()
            .y;
        assert!(
            first <= 330.,
            "{width}x{height}: the cards start too low ({first})"
        );
        for label in ["並び順", "アーカイブを含める"] {
            let control = ElementHandle::find_by_accessible_label(&w, label)
                .next()
                .unwrap_or_else(|| panic!("{label}"));
            let list = ElementHandle::find_by_element_id(&w, "PaperList::scroll")
                .next()
                .unwrap();
            assert!(
                control.absolute_position().x + control.size().width
                    <= list.absolute_position().x + list.size().width + 16.,
                "{label} fits the list width"
            );
        }
        let position = slint::LogicalPosition::new(300., 450.);
        w.window()
            .dispatch_event(WindowEvent::PointerMoved { position });
        for _ in 0..80 {
            w.window().dispatch_event(WindowEvent::PointerScrolled {
                position,
                delta_x: 0.,
                delta_y: -400.,
            });
        }
        let viewport = ElementHandle::find_by_element_id(&w, "PaperList::scroll")
            .next()
            .expect("the list's scroll view");
        let viewport_bottom = viewport.absolute_position().y + viewport.size().height;
        let mut cards: Vec<(f32, f32)> = ElementHandle::find_by_element_type_name(&w, "PaperCard")
            .map(|c| (c.absolute_position().y, c.size().height))
            .collect();
        cards.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (y, h) = *cards.last().expect("cards");
        assert!(
            y + h <= viewport_bottom,
            "{width}x{height}: the last card ends at {} but the list ends at {viewport_bottom}",
            y + h
        );
        // The very last paper is the one that is shown last, not a hidden one.
        let last = ElementHandle::find_by_accessible_label(
            &w,
            "文献を開く: Paper 8: A very long title about graph neural networks for molecular property prediction with attention and more words",
        )
        .count();
        assert_eq!(last, 1, "{width}x{height}: Paper 8 is on screen");
        // Every card has the same height, whatever its title.
        assert!(cards.iter().all(|c| (c.1 - h).abs() < 0.5), "{cards:?}");
    }
}

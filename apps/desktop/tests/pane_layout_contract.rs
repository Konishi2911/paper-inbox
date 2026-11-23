use i_slint_backend_testing::ElementHandle;
use paper_desktop::{DetailView, MainWindow, PaperRow};
use slint::{
    ComponentHandle, ModelRc, VecModel,
    platform::{PointerEventButton, WindowEvent},
};

fn send(w: &MainWindow, event: WindowEvent) {
    w.window().dispatch_event(event);
}
fn splitter(w: &MainWindow) -> ElementHandle {
    ElementHandle::find_by_element_type_name(w, "Splitter")
        .next()
        .unwrap()
}
fn drag(w: &MainWindow, dx: f32) {
    let s = splitter(w);
    let p = s.absolute_position();
    let start = slint::LogicalPosition::new(p.x + 4., p.y + 150.);
    send(w, WindowEvent::PointerMoved { position: start });
    send(
        w,
        WindowEvent::PointerPressed {
            position: start,
            button: PointerEventButton::Left,
        },
    );
    for delta in [dx / 2., dx] {
        send(
            w,
            WindowEvent::PointerMoved {
                position: slint::LogicalPosition::new(start.x + delta, start.y),
            },
        );
    }
    send(
        w,
        WindowEvent::PointerReleased {
            position: slint::LogicalPosition::new(start.x + dx, start.y),
            button: PointerEventButton::Left,
        },
    );
}
#[test]
fn splitter_tracks_window_pointer_and_cards_fit_at_narrow_widths() {
    i_slint_backend_testing::init_no_event_loop();
    let w = MainWindow::new().unwrap();
    w.set_ready(true);
    w.set_busy(false);
    w.show().unwrap();
    w.window().set_size(slint::LogicalSize::new(1360., 900.));
    let initial = splitter(&w).absolute_position().x;
    drag(&w, 80.);
    assert!(
        (splitter(&w).absolute_position().x - initial - 80.).abs() < 2.,
        "drag follows pointer without feedback"
    );
    drag(&w, -2000.);
    assert!((w.get_paper_list_width() - 320.).abs() < 1.);
    drag(&w, 2000.);
    assert!(splitter(&w).absolute_position().x + 8. <= 1360. - 360.);
    w.window().set_size(slint::LogicalSize::new(1080., 680.));
    assert!(splitter(&w).absolute_position().x + 8. <= 1080. - 360.);
    w.set_sidebar_collapsed(true);
    assert!(splitter(&w).absolute_position().x + 8. <= 1080. - 360.);
    w.set_sidebar_collapsed(false);
    w.set_paper_list_width(320.);
    let title = "Long title with 日本語 and multiple words ".repeat(3);
    w.set_papers(ModelRc::new(VecModel::from(vec![PaperRow {
        id: "card".into(),
        title: title.clone().into(),
        authors: "Many authors ".repeat(20).into(),
        year: "2026".into(),
        citation_key: "Layout2026".into(),
        selected: false,
        reading_status: 0,
        favorite: false,
        has_pdf: false,
    }])));
    for export in [false, true] {
        w.set_detail_view(if export {
            DetailView::ExportSettings
        } else {
            DetailView::PaperDetail
        });
        let _ = splitter(&w).absolute_position();
        let card = ElementHandle::find_by_accessible_label(&w, &format!("文献を開く: {title}"))
            .next()
            .unwrap();
        for text in card.query_descendants().match_inherits("Text").find_all() {
            assert!(
                text.absolute_position().y + text.size().height
                    <= card.absolute_position().y + card.size().height + 1.,
                "all card text must fit"
            );
        }
    }
    w.set_detail_view(DetailView::PaperDetail);
    w.set_busy(true);
    let width = w.get_paper_list_width();
    drag(&w, 80.);
    assert_eq!(w.get_paper_list_width(), width);
    w.set_busy(false);
    w.set_confirm_delete(true);
    drag(&w, 80.);
    assert_eq!(w.get_paper_list_width(), width);
    w.hide().unwrap();
}

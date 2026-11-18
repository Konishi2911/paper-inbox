use i_slint_backend_testing::ElementHandle;
use paper_desktop::{MainWindow, NavigationAction, PaperRow};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{cell::RefCell, rc::Rc};

fn control(w: &MainWindow, label: &str) -> ElementHandle {
    ElementHandle::find_by_accessible_label(w, label)
        .next()
        .unwrap_or_else(|| panic!("missing {label}"))
}

#[test]
fn list_controls_share_query_values_and_notify_parent_without_owning_navigation() {
    i_slint_backend_testing::init_no_event_loop();
    let w = MainWindow::new().unwrap();
    w.set_ready(true);
    w.set_busy(false);
    w.show().unwrap();
    w.set_papers(ModelRc::new(VecModel::from(vec![PaperRow {
        id: "paper-id".into(),
        title: "A paper".into(),
        authors: "A. Author".into(),
        year: "2026".into(),
        citation_key: "Author2026".into(),
        selected: false,
        reading_status: 0,
        favorite: false,
        has_pdf: false,
    }])));
    let queries = Rc::new(RefCell::new(Vec::new()));
    let recorded = queries.clone();
    let weak = w.as_weak();
    w.on_search(move || {
        let w = weak.upgrade().unwrap();
        recorded.borrow_mut().push((
            w.get_search_text().to_string(),
            w.get_limit(),
            w.get_include_archived(),
        ));
    });
    w.set_limit(100);
    control(&w, "文献を検索").set_accessible_value("Graph");
    assert_eq!(w.get_search_text(), "Graph");
    control(&w, "検索").invoke_accessible_default_action();
    assert_eq!(
        queries.borrow().last().unwrap(),
        &("Graph".into(), 50, false)
    );
    control(&w, "アーカイブを含める").invoke_accessible_default_action();
    assert_eq!(
        queries.borrow().last().unwrap(),
        &("Graph".into(), 50, true)
    );
    for label in ["未読だけ表示", "お気に入りだけ表示", "PDFありだけ表示"] {
        w.set_limit(100);
        control(&w, label).invoke_accessible_default_action();
        assert_eq!(
            queries.borrow().last().unwrap().1,
            50,
            "{label} restarts the list"
        );
    }
    assert!(w.get_filter_unread() && w.get_filter_favorite() && w.get_filter_pdf());
    control(&w, "未読だけ表示").invoke_accessible_default_action();
    assert!(!w.get_filter_unread());
    let navigation = Rc::new(RefCell::new(Vec::new()));
    let received = navigation.clone();
    w.on_navigate(move |action, id| received.borrow_mut().push((action, id.to_string())));
    control(&w, "文献を開く: A paper").invoke_accessible_default_action();
    assert_eq!(
        *navigation.borrow(),
        vec![(NavigationAction::OpenPaper, "paper-id".into())]
    );
    w.set_busy(true);
    assert_eq!(control(&w, "検索").accessible_enabled(), Some(false));
    w.hide().unwrap();
}

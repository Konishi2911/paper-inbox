use paper_core::model::PaperId;
use paper_desktop::{
    DiffBulkAction, DiffFilter, DiffInputField, DiffResolution as R, MainWindow, NavigationAction,
    application::{Application, PaperForm},
    config::Config,
    ui,
};
use slint::{ComponentHandle, Model};
use std::time::{Duration, Instant};
fn wait(w: &MainWindow) {
    let weak = w.as_weak();
    let start = Instant::now();
    let done = std::rc::Rc::new(std::cell::Cell::new(false));
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(5),
        move || {
            let w = weak.upgrade().unwrap();
            assert!(
                start.elapsed() < Duration::from_secs(30),
                "{}",
                w.get_status()
            );
            if !w.get_busy() && !done.replace(true) {
                slint::quit_event_loop().unwrap();
            }
        },
    );
    slint::run_event_loop_until_quit().unwrap();
}
fn row(w: &MainWindow, label: &str) -> paper_desktop::ProposalRow {
    w.get_proposal_rows()
        .iter()
        .find(|r| r.label == label)
        .unwrap()
}
#[test]
fn staged_decisions_survive_filters_errors_and_conflicts_until_atomic_apply() {
    i_slint_backend_testing::init_integration_test_with_system_time();
    let path =
        std::env::temp_dir().join(format!("paper-resolution-window-{}.sqlite", PaperId::new()));
    let w = MainWindow::new().unwrap();
    w.show().unwrap();
    w.window().set_size(slint::LogicalSize::new(1600., 1400.));
    let controller = ui::attach(
        &w,
        Config {
            database: path.clone(),
        },
    );
    wait(&w);
    w.set_citation_key("Staged2026".into());
    w.set_paper_title("Manual title".into());
    w.set_year("2025".into());
    w.set_doi("10.1000/a".into());
    w.invoke_save();
    wait(&w);
    let id = w.get_editing_id();
    w.invoke_request_navigation(NavigationAction::MetadataReview, id.clone());
    wait(&w);
    w.invoke_begin_candidate();
    w.set_candidate_doi("10.1000/a".into());
    w.set_candidate_use_title(true);
    w.set_candidate_title("External title".into());
    w.set_candidate_use_year(true);
    w.set_candidate_year("2026".into());
    w.invoke_submit_candidate();
    wait(&w);
    let title = row(&w, "タイトル");
    let year = row(&w, "出版年");
    w.invoke_bulk_metadata(DiffBulkAction::AcceptSafe);
    assert!(
        !w.get_review_dirty(),
        "manual values never enter bulk adoption"
    );
    w.invoke_choose_metadata(title.id.clone(), R::AcceptProposed);
    w.invoke_choose_metadata(year.id.clone(), R::Custom);
    w.invoke_edit_metadata(year.id.clone(), DiffInputField::Value, "invalid".into());
    assert!(w.get_review_dirty());
    assert_eq!(w.get_paper_title(), "Manual title");
    for filter in [
        DiffFilter::Decided,
        DiffFilter::Unresolved,
        DiffFilter::Conflict,
        DiffFilter::Changed,
        DiffFilter::All,
    ] {
        w.set_diff_filter(filter);
        assert_eq!(row(&w, "出版年").custom_text, "invalid");
        assert_eq!(row(&w, "タイトル").decision, R::AcceptProposed);
    }
    w.invoke_apply_metadata();
    wait(&w);
    assert!(w.get_review_dirty());
    assert_eq!(row(&w, "出版年").custom_text, "invalid");
    assert_eq!(w.get_paper_title(), "Manual title");
    w.invoke_request_navigation(NavigationAction::OpenPaper, id.clone());
    assert!(w.get_confirm_discard());
    w.set_confirm_discard(false);
    // Keeping one undecided commits only the other choice.
    w.invoke_choose_metadata(year.id.clone(), R::Unresolved);
    w.invoke_apply_metadata();
    wait(&w);
    assert!(!w.get_review_dirty());
    assert_eq!(w.get_paper_title(), "External title");
    assert_eq!(w.get_year(), "2025");
    assert_eq!(w.get_proposal_rows().row_count(), 1);
    w.invoke_choose_metadata(year.id.clone(), R::Custom);
    w.invoke_edit_metadata(year.id.clone(), DiffInputField::Value, "2027".into());
    let apply = i_slint_backend_testing::ElementHandle::find_by_accessible_label(&w, "判断を適用")
        .next()
        .unwrap();
    apply.invoke_accessible_default_action();
    wait(&w);
    assert_eq!(w.get_year(), "2027");
    assert_eq!(w.get_proposal_rows().row_count(), 0);
    assert!(!w.get_review_dirty());
    w.invoke_begin_candidate();
    w.set_candidate_doi("10.1000/a".into());
    w.set_candidate_use_title(true);
    w.set_candidate_title("Later title".into());
    w.invoke_submit_candidate();
    wait(&w);
    let title = row(&w, "タイトル");
    w.invoke_bulk_metadata(DiffBulkAction::KeepAll);
    assert_eq!(row(&w, "タイトル").decision, R::KeepCurrent);
    let other = Application {
        store: paper_storage::SqliteLibrary::open_local(&path).unwrap(),
    };
    let paper_id = PaperId::from_uuid(uuid::Uuid::parse_str(&id).unwrap());
    let state = other.open(paper_id).unwrap();
    let mut form = PaperForm::from_paper(&state.paper);
    form.title = "Changed elsewhere".into();
    other.save(Some(&state), &form).unwrap();
    drop(other);
    w.invoke_apply_metadata();
    wait(&w);
    assert!(w.get_review_dirty());
    assert_eq!(row(&w, "タイトル").decision, R::KeepCurrent);
    w.invoke_request_navigation(NavigationAction::MetadataReview, id);
    assert!(w.get_confirm_discard());
    i_slint_backend_testing::ElementHandle::find_by_accessible_label(&w, "破棄して移動")
        .next()
        .unwrap()
        .invoke_accessible_default_action();
    wait(&w);
    assert!(!w.get_review_dirty());
    assert!(row(&w, "タイトル").stale);
    w.invoke_choose_metadata(title.id, R::AcceptProposed);
    assert!(
        !w.get_review_dirty(),
        "stale adoption disabled independently of the button"
    );
    w.invoke_begin_candidate();
    w.set_candidate_doi("10.1000/a".into());
    w.set_candidate_use_title(true);
    w.set_candidate_title("".into());
    w.invoke_submit_candidate();
    wait(&w);
    let deletion = w
        .get_proposal_rows()
        .iter()
        .find(|r| r.change_label == "削除")
        .unwrap();
    assert!(
        deletion.conflict && !deletion.bulk_safe,
        "explicit deletion of manual values always requires individual approval"
    );
    w.invoke_bulk_metadata(DiffBulkAction::AcceptSafe);
    assert_eq!(
        w.get_proposal_rows()
            .iter()
            .find(|r| r.id == deletion.id)
            .unwrap()
            .decision,
        R::Unresolved
    );
    let mut many = Vec::new();
    for index in 0..100 {
        let mut r = deletion.clone();
        r.id = PaperId::new().to_string().into();
        if index % 10 == 0 {
            r.change_label = "変更なし".into();
        }
        many.push(r);
    }
    let chosen = many[0].id.clone();
    w.set_proposal_rows(slint::ModelRc::new(slint::VecModel::from(many)));
    w.set_diff_filter(DiffFilter::Unresolved);
    w.invoke_refilter_metadata();
    assert_eq!(w.get_filtered_proposal_rows().row_count(), 100);
    w.invoke_choose_metadata(chosen.clone(), R::KeepCurrent);
    assert_eq!(w.get_proposal_rows().row_count(), 100);
    assert_eq!(w.get_filtered_proposal_rows().row_count(), 99);
    w.set_diff_filter(DiffFilter::Decided);
    w.invoke_refilter_metadata();
    assert_eq!(w.get_filtered_proposal_rows().row_count(), 1);
    w.set_diff_filter(DiffFilter::Changed);
    w.invoke_refilter_metadata();
    assert_eq!(w.get_filtered_proposal_rows().row_count(), 90);
    assert_eq!(
        w.get_proposal_rows()
            .iter()
            .find(|r| r.id == chosen)
            .unwrap()
            .decision,
        R::KeepCurrent
    );
    w.hide().unwrap();
    drop(controller);
    drop(w);
    std::fs::remove_file(path).unwrap();
}

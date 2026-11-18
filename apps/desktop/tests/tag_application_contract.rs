use paper_core::{command::LibraryCommand, model::*, query::TagCount};
use paper_desktop::application::{AppError, Application, tag_suggestions};
use paper_storage::SqliteLibrary;

fn setup() -> (Application, paper_core::command::PaperEditState) {
    let store = SqliteLibrary::in_memory(DeviceId::new()).unwrap();
    let paper = Paper::new(
        PaperId::new(),
        CitationKey::parse("Tagged2026").unwrap(),
        Bibliography {
            title: Some("t".into()),
            ..Default::default()
        },
        chrono::Utc::now(),
    )
    .unwrap();
    let id = paper.id();
    paper_core::ports::LibraryStore::commit(&store, LibraryCommand::CreatePaper { paper }).unwrap();
    let app = Application { store };
    let state = app.open(id).unwrap();
    (app, state)
}

#[test]
fn tags_are_found_by_folded_name_created_once_and_removed() {
    let (app, state) = setup();
    let first = app.add_tag_by_name(&state, "  ML ").unwrap();
    assert!(first.created);
    assert_eq!(first.tag.name, "ML");
    // The same tag by another spelling is reused, so it is already on the paper.
    for again in ["ml", "ＭＬ"] {
        assert!(matches!(
            app.add_tag_by_name(&state, again),
            Err(AppError::Input(m)) if m.contains("すでに")
        ));
    }
    assert_eq!(app.tag_counts(false).unwrap().len(), 1);
    assert!(matches!(
        app.add_tag_by_name(&state, "   "),
        Err(AppError::Input(_))
    ));
    let second = app.add_tag_by_name(&state, "graphs").unwrap();
    assert_eq!(
        app.paper_tags(state.paper.id())
            .unwrap()
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>(),
        ["graphs", "ML"]
    );
    app.remove_tag(&state, second.tag.id).unwrap();
    assert_eq!(app.paper_tags(state.paper.id()).unwrap().len(), 1);
    assert_eq!(
        app.tag_counts(false)
            .unwrap()
            .iter()
            .find(|c| c.tag.id == second.tag.id)
            .unwrap()
            .paper_count,
        0
    );
}

#[test]
fn suggestions_prefer_prefixes_and_skip_tags_the_paper_has() {
    let tag = |name: &str| TagCount {
        tag: Tag::new(TagId::new(), name).unwrap(),
        paper_count: 1,
    };
    let all = vec![tag("deep learning"), tag("learning theory"), tag("graphs")];
    let names = |typed: &str, taken: &[TagId]| -> Vec<String> {
        tag_suggestions(&all, taken, typed, 5)
            .into_iter()
            .map(|t| t.name)
            .collect()
    };
    assert_eq!(names("lea", &[]), ["learning theory", "deep learning"]);
    assert_eq!(names("ＧＲ", &[]), ["graphs"]);
    assert!(names("", &[]).is_empty());
    assert!(names("zzz", &[]).is_empty());
    assert_eq!(names("lea", &[all[1].tag.id]), ["deep learning"]);
}

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

#[test]
fn renaming_a_tag_changes_it_for_every_paper_and_refuses_taken_names() {
    let (app, state) = setup();
    let ml = app.add_tag_by_name(&state, "ml").unwrap().tag;
    let graphs = app.add_tag_by_name(&state, "graphs").unwrap().tag;

    let renamed = app.rename_tag(ml.id, "  Machine  Learning ").unwrap();
    assert_eq!(renamed.id, ml.id);
    assert_eq!(renamed.name, "Machine Learning");
    let names: Vec<_> = app
        .paper_tags(state.paper.id())
        .unwrap()
        .into_iter()
        .map(|t| t.name)
        .collect();
    assert!(names.contains(&"Machine Learning".to_string()) && names.len() == 2);

    // Adding by the new name finds the renamed tag, and the old name is free.
    assert!(matches!(
        app.add_tag_by_name(&state, "machine learning"),
        Err(AppError::Input(m)) if m.contains("すでに")
    ));

    // A name another tag has is refused, naming that tag; so are an unchanged name and a bad one.
    assert!(matches!(
        app.rename_tag(ml.id, "ＧＲＡＰＨＳ"),
        Err(AppError::Input(m)) if m.contains("graphs")
    ));
    assert!(matches!(
        app.rename_tag(ml.id, "Machine Learning"),
        Err(AppError::Input(m)) if m.contains("変わっていません")
    ));
    assert!(matches!(
        app.rename_tag(ml.id, " "),
        Err(AppError::Input(_))
    ));
    assert!(matches!(
        app.rename_tag(TagId::new(), "x"),
        Err(AppError::Input(m)) if m.contains("見つかりません")
    ));
    // Only case and spacing of its own name may change.
    assert_eq!(app.rename_tag(graphs.id, "Graphs").unwrap().name, "Graphs");
}

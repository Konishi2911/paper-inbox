use crate::{
    AuthorRow, MainWindow, PaperRow,
    application::{Application, AuthorInput, PaperForm},
    config::Config,
};
use paper_core::{command::PaperEditState, model::*, query::*};
use paper_storage::SqliteLibrary;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::{
    sync::mpsc::{self, Sender},
    thread,
};

enum Request {
    Initialize,
    List(PaperQuery),
    New,
    Open(PaperId),
    Save(PaperForm),
    Delete(bool),
    Shutdown,
}
struct EditorView {
    form: PaperForm,
    id: Option<PaperId>,
    deleted: bool,
}
#[derive(Default)]
struct Reply {
    editor: Option<EditorView>,
    list: Option<SearchResult>,
    message: String,
    ready: Option<bool>,
    saved: bool,
    load_required: bool,
    offset: Option<i32>,
}
struct Worker {
    config: Config,
    app: Option<Application>,
    editing: Option<PaperEditState>,
    query: PaperQuery,
}
impl Worker {
    fn handle(&mut self, request: Request) -> Reply {
        if matches!(request, Request::Initialize) {
            return match self.config.prepare().and_then(|_| {
                SqliteLibrary::open_local(&self.config.database)
                    .map_err(|e| format!("保存先を開けません: {e}"))
            }) {
                Ok(store) => {
                    self.app = Some(Application { store });
                    self.editing = None;
                    match self.app.as_ref().unwrap().list(&self.query) {
                        Ok(list) => Reply {
                            list: Some(list),
                            editor: Some(EditorView {
                                form: PaperForm::default(),
                                id: None,
                                deleted: false,
                            }),
                            ready: Some(true),
                            message: "準備ができました。".into(),
                            ..Default::default()
                        },
                        Err(e) => Reply {
                            message: e.to_string(),
                            ready: Some(false),
                            ..Default::default()
                        },
                    }
                }
                Err(message) => Reply {
                    message,
                    ready: Some(false),
                    ..Default::default()
                },
            };
        }
        let Some(app) = &self.app else {
            return Reply {
                message: "保存先を開けていません。再接続してください。".into(),
                ..Default::default()
            };
        };
        match request {
            Request::List(query) => match app.list(&query) {
                Ok(list) => {
                    self.query = query;
                    Reply {
                        offset: Some(self.query.pagination.offset as i32),
                        list: Some(list),
                        message: "一覧を更新しました。".into(),
                        ..Default::default()
                    }
                }
                Err(e) => Reply {
                    message: e.to_string(),
                    ..Default::default()
                },
            },
            Request::New => {
                self.editing = None;
                Reply {
                    editor: Some(EditorView {
                        form: PaperForm::default(),
                        id: None,
                        deleted: false,
                    }),
                    message: "新しい文献を登録できます。".into(),
                    ..Default::default()
                }
            }
            Request::Open(id) => match app.open(id) {
                Ok(state) => {
                    let view = view(&state);
                    self.editing = Some(state);
                    Reply {
                        editor: Some(view),
                        message: "文献を読み込みました。".into(),
                        ..Default::default()
                    }
                }
                Err(e) => Reply {
                    message: e.to_string(),
                    ..Default::default()
                },
            },
            Request::Save(form) => match app.save(self.editing.as_ref(), &form) {
                Ok(id) => self.after_save(id, "保存しました。"),
                Err(e) => Reply {
                    message: e.to_string(),
                    ..Default::default()
                },
            },
            Request::Delete(deleted) => {
                let Some(state) = &self.editing else {
                    return Reply {
                        message: "文献を選択してください。".into(),
                        ..Default::default()
                    };
                };
                let id = state.paper.id();
                match app.set_deleted(state, deleted) {
                    Ok(()) => self.after_save(
                        id,
                        if deleted {
                            "削除済みに移動しました。"
                        } else {
                            "復元しました。"
                        },
                    ),
                    Err(e) => Reply {
                        message: e.to_string(),
                        ..Default::default()
                    },
                }
            }
            Request::Initialize | Request::Shutdown => unreachable!(),
        }
    }
    fn after_save(&mut self, id: PaperId, message: &str) -> Reply {
        let app = self.app.as_ref().unwrap();
        let mut reply = Reply {
            saved: true,
            message: message.into(),
            ..Default::default()
        };
        match app.open(id) {
            Ok(state) => {
                reply.editor = Some(view(&state));
                self.editing = Some(state);
            }
            Err(e) => {
                self.editing = None;
                reply.load_required = true;
                reply.message = format!(
                    "{message} 再読込には失敗しました: {e} 保存を繰り返さず、再接続してください。"
                );
            }
        }
        match app.list(&self.query) {
            Ok(mut list) => {
                if self.query.pagination.offset >= list.total_count {
                    self.query.pagination.offset = if list.total_count == 0 {
                        0
                    } else {
                        (list.total_count - 1) / 20 * 20
                    };
                    match app.list(&self.query) {
                        Ok(page) => list = page,
                        Err(e) => reply
                            .message
                            .push_str(&format!(" 一覧の更新に失敗しました: {e}")),
                    }
                }
                reply.offset = Some(self.query.pagination.offset as i32);
                reply.list = Some(list);
            }
            Err(e) => reply
                .message
                .push_str(&format!(" 一覧の更新に失敗しました: {e}")),
        }
        reply
    }
}
fn view(state: &PaperEditState) -> EditorView {
    EditorView {
        form: PaperForm::from_paper(&state.paper),
        id: Some(state.paper.id()),
        deleted: state.paper.deleted_at().is_some(),
    }
}

pub struct Controller {
    sender: Sender<Request>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Drop for Controller {
    fn drop(&mut self) {
        let _ = self.sender.send(Request::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
pub fn attach(ui: &MainWindow, config: Config) -> Controller {
    ui.set_database_path(config.database.to_string_lossy().as_ref().into());
    let (sender, receiver) = mpsc::channel();
    let weak = ui.as_weak();
    let worker = thread::spawn(move || {
        let mut state = Worker {
            config,
            app: None,
            editing: None,
            query: PaperQuery {
                pagination: Pagination {
                    limit: 20,
                    offset: 0,
                },
                ..Default::default()
            },
        };
        while let Ok(request) = receiver.recv() {
            if matches!(request, Request::Shutdown) {
                break;
            }
            let reply = state.handle(request);
            if weak
                .upgrade_in_event_loop(move |ui| apply_reply(&ui, reply))
                .is_err()
            {
                break;
            }
        }
    });
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_search(move || {
        if let Some(ui) = weak.upgrade() {
            dispatch(&ui, &tx, Request::List(query_from_ui(&ui)));
        }
    });
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_navigate(move |action, id| {
        if let Some(ui) = weak.upgrade() {
            let request = match action {
                3 => {
                    let _ = slint::quit_event_loop();
                    return;
                }
                0 => Request::New,
                2 => {
                    ui.set_ready(false);
                    Request::Initialize
                }
                _ => match uuid::Uuid::parse_str(&id) {
                    Ok(id) => Request::Open(PaperId::from_uuid(id)),
                    Err(_) => {
                        ui.set_status("文献 ID を読み取れません。".into());
                        return;
                    }
                },
            };
            dispatch(&ui, &tx, request);
        }
    });
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_save(move || {
        if let Some(ui) = weak.upgrade() {
            dispatch(&ui, &tx, Request::Save(form_from_ui(&ui)));
        }
    });
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_delete_paper(move |deleted| {
        if let Some(ui) = weak.upgrade() {
            dispatch(&ui, &tx, Request::Delete(deleted));
        }
    });
    let weak = ui.as_weak();
    ui.on_author_edit(move |index, field, text| {
        if let Some(ui) = weak.upgrade() {
            edit_author(&ui, index, |row| match field {
                0 => row.name = text.clone(),
                1 => row.given = text.clone(),
                _ => row.orcid = text.clone(),
            });
        }
    });
    let weak = ui.as_weak();
    ui.on_author_kind(move |index, kind| {
        if let Some(ui) = weak.upgrade() {
            edit_author(&ui, index, |row| row.kind = kind);
        }
    });
    let weak = ui.as_weak();
    ui.on_author_action(move |index, action| {
        if let Some(ui) = weak.upgrade() {
            let model = ui.get_author_rows();
            let Some(rows) = model.as_any().downcast_ref::<VecModel<AuthorRow>>() else {
                return;
            };
            let index = index.max(0) as usize;
            match action {
                0 => rows.push(AuthorRow::default()),
                1 if index < rows.row_count() => {
                    rows.remove(index);
                }
                2 | 3 => {
                    let other = if action == 2 {
                        index.checked_sub(1)
                    } else {
                        Some(index + 1)
                    };
                    if let Some(other) = other.filter(|i| *i < rows.row_count())
                        && let (Some(a), Some(b)) = (rows.row_data(index), rows.row_data(other))
                    {
                        rows.set_row_data(index, b);
                        rows.set_row_data(other, a);
                    }
                }
                _ => {}
            }
            ui.set_dirty(true);
        }
    });
    let weak = ui.as_weak();
    ui.window().on_close_requested(move || {
        if let Some(ui) = weak.upgrade()
            && ui.get_dirty()
        {
            ui.invoke_request_navigation(3, "".into());
            return slint::CloseRequestResponse::KeepWindowShown;
        }
        slint::CloseRequestResponse::HideWindow
    });
    dispatch(ui, &sender, Request::Initialize);
    Controller {
        sender,
        worker: Some(worker),
    }
}
fn edit_author(ui: &MainWindow, index: i32, edit: impl FnOnce(&mut AuthorRow)) {
    let rows = ui.get_author_rows();
    let Some(rows) = rows.as_any().downcast_ref::<VecModel<AuthorRow>>() else {
        return;
    };
    if let Some(mut row) = rows.row_data(index.max(0) as usize) {
        edit(&mut row);
        rows.set_row_data(index.max(0) as usize, row);
        ui.set_dirty(true);
    }
}
fn dispatch(ui: &MainWindow, sender: &Sender<Request>, request: Request) {
    ui.set_busy(true);
    ui.set_status("処理しています…".into());
    if sender.send(request).is_err() {
        ui.set_busy(false);
        ui.set_status("処理を続けられません。アプリを再起動してください。".into());
    }
}
pub fn form_from_ui(ui: &MainWindow) -> PaperForm {
    PaperForm {
        citation_key: ui.get_citation_key().into(),
        title: ui.get_paper_title().into(),
        authors: ui
            .get_author_rows()
            .iter()
            .map(|r| AuthorInput {
                kind: r.kind,
                given: r.given.into(),
                name: r.name.into(),
                orcid: r.orcid.into(),
            })
            .collect(),
        publication_type: ui.get_publication_type().max(0) as usize,
        container_title: ui.get_container_title().into(),
        year: ui.get_year().into(),
        notes: ui.get_notes().into(),
        reading_status: ui.get_reading_status().max(0) as usize,
        favorite: ui.get_favorite(),
        archived: ui.get_archived(),
    }
}
fn query_from_ui(ui: &MainWindow) -> PaperQuery {
    PaperQuery {
        text: Some(ui.get_search_text().into()),
        filter: PaperFilter {
            include_archived: ui.get_show_deleted() || ui.get_include_archived(),
            deletion: if ui.get_show_deleted() {
                DeletionFilter::Deleted
            } else {
                DeletionFilter::Active
            },
            ..Default::default()
        },
        sort: match ui.get_sort_index() {
            1 => PaperSort {
                field: SortField::Title,
                order: SortOrder::Ascending,
            },
            2 => PaperSort {
                field: SortField::PublicationYear,
                order: SortOrder::Descending,
            },
            _ => PaperSort::default(),
        },
        pagination: Pagination {
            limit: 20,
            offset: ui.get_offset().max(0) as usize,
        },
    }
}
fn apply_reply(ui: &MainWindow, reply: Reply) {
    if let Some(offset) = reply.offset {
        ui.set_offset(offset);
    }
    if let Some(ready) = reply.ready {
        ui.set_ready(ready);
    }
    if let Some(editor) = reply.editor {
        let f = editor.form;
        ui.set_citation_key(f.citation_key.into());
        ui.set_paper_title(f.title.into());
        ui.set_container_title(f.container_title.into());
        ui.set_year(f.year.into());
        ui.set_publication_type(f.publication_type as i32);
        ui.set_notes(f.notes.into());
        ui.set_reading_status(f.reading_status as i32);
        ui.set_favorite(f.favorite);
        ui.set_archived(f.archived);
        let rows: Vec<_> = f
            .authors
            .into_iter()
            .map(|a| AuthorRow {
                kind: a.kind,
                given: a.given.into(),
                name: a.name.into(),
                orcid: a.orcid.into(),
            })
            .collect();
        ui.set_author_rows(ModelRc::new(VecModel::from(rows)));
        ui.set_editor_visible(true);
        ui.set_editor_deleted(editor.deleted);
        ui.set_saved_paper(editor.id.is_some());
        ui.set_editing_id(
            editor
                .id
                .map(|id| id.to_string())
                .unwrap_or_default()
                .into(),
        );
        ui.set_dirty(false);
        ui.set_load_required(false);
        ui.set_confirm_delete(false);
    }
    if reply.saved {
        ui.set_dirty(false);
    }
    if reply.load_required {
        ui.set_load_required(true);
    }
    if let Some(list) = reply.list {
        let rows: Vec<_> = list
            .papers
            .into_iter()
            .map(|p| PaperRow {
                id: p.id.to_string().into(),
                title: p.title.unwrap_or_default().into(),
                citation_key: p.citation_key.as_str().into(),
                year: p.year.map(|y| y.to_string()).unwrap_or_default().into(),
                authors: p
                    .authors
                    .iter()
                    .map(|a| match &a.name {
                        ContributorName::Person { given, family } => {
                            format!("{} {}", given.as_deref().unwrap_or(""), family)
                                .trim()
                                .to_owned()
                        }
                        ContributorName::Organization(s) | ContributorName::Literal(s) => s.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
                    .into(),
            })
            .collect();
        ui.set_total(list.total_count.min(i32::MAX as usize) as i32);
        ui.set_papers(ModelRc::new(VecModel::from(rows)));
    }
    ui.set_status(reply.message.into());
    ui.set_busy(false);
}

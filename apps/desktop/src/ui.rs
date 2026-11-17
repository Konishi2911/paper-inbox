use crate::export::{self, PreparedExport};
use crate::{
    AttachmentRow, AuthorAction, AuthorRow, DetailView, LinkTarget, LookupField, MainWindow,
    NavigationAction, PaperRow,
    application::{Application, AuthorInput, PaperForm, QuickChange},
    attachment::AttachmentFiles,
    config::Config,
    lookup::{LookupPreview, LookupUserInput},
};
use paper_core::service::{BibtexOptions, BibtexWarning};
use paper_core::{command::PaperEditState, model::*, query::*};
use paper_storage::SqliteLibrary;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use std::path::PathBuf;
use std::{
    sync::mpsc::{self, Sender},
    thread,
};

enum Request {
    BeginLookup(bool),
    FetchLookup(String),
    ConfirmLookup(String, String, LookupUserInput),
    CancelLookup,
    SuggestKey(PaperForm),
    Initialize,
    List(PaperQuery),
    New,
    Open(PaperId),
    Save(PaperForm),
    Quick(QuickChange),
    Delete(bool),
    ReviewMetadata(PaperId),
    SubmitCandidate(crate::metadata::CandidateForm),
    AssociateCandidate(CandidateId),
    ResolveMetadata(ProposalId, bool),
    ResolveMetadataBatch(Vec<crate::metadata_resolution::ResolutionInput>),
    ProposeCanonical(VersionId),
    PrepareExport {
        ids: Vec<PaperId>,
        path: PathBuf,
        options: BibtexOptions,
    },
    ConfirmExport,
    CancelExport,
    AttachPdf(PathBuf),
    OpenAttachment(AttachmentId),
    DetachAttachment(AttachmentId),
    PrepareCleanup,
    ReclaimUnused,
    Shutdown,
}
struct EditorView {
    form: PaperForm,
    id: Option<PaperId>,
    deleted: bool,
    identifier_warning: String,
    /// The saved entry with default export options, for the copy action.
    bibtex: String,
}
#[derive(Default)]
struct Reply {
    editor: Option<EditorView>,
    list: Option<SearchResult>,
    message: String,
    ready: Option<bool>,
    saved: bool,
    load_required: bool,
    export_preview: Option<(String, String, String, bool)>,
    clear_export: bool,
    metadata: Option<MetadataReview>,
    close_metadata: bool,
    close_candidate: bool,
    lookup_open: Option<bool>,
    lookup_new: Option<bool>,
    lookup_preview: Option<(LookupPreview, String)>,
    key_suggestion: Option<String>,
    form_error: Option<String>,
    /// Attachment rows of the paper in `editor`; sent together with every editor reload.
    attachments: Option<Vec<AttachmentView>>,
    /// Unused file count and total bytes; opens the cleanup screen.
    cleanup: Option<(usize, u64)>,
}
struct AttachmentView {
    id: String,
    name: String,
    details: String,
    missing: bool,
}
pub type LookupProvider = Box<
    dyn FnMut(&PaperIdentifier) -> Result<Vec<MetadataCandidate>, paper_metadata::LookupError>
        + Send,
>;
struct PendingLookup {
    candidate: MetadataCandidate,
    paper_id: PaperId,
    input: String,
}
struct Worker {
    config: Config,
    app: Option<Application>,
    files: Option<AttachmentFiles>,
    editing: Option<PaperEditState>,
    query: PaperQuery,
    pending_export: Option<PreparedExport>,
    review: Option<MetadataReview>,
    pending_lookup: Option<PendingLookup>,
    lookup_new: bool,
    lookup_provider: LookupProvider,
}
impl Worker {
    fn handle(&mut self, request: Request) -> Reply {
        let mut reply = self.handle_request(request);
        self.fill_attachments(&mut reply);
        reply
    }
    /// Every editor reload carries the paper's PDFs, so the list never lags behind.
    fn fill_attachments(&self, reply: &mut Reply) {
        let Some(editor) = &reply.editor else {
            return;
        };
        let mut rows = vec![];
        if let (Some(id), Some(app), Some(files)) = (editor.id, &self.app, &self.files) {
            match files.list(&app.store, id) {
                Ok(list) => {
                    for a in list {
                        let missing = !files.is_present(&a).unwrap_or(false);
                        rows.push(AttachmentView {
                            id: a.id.to_string(),
                            name: a.filename.clone(),
                            details: crate::detail::attachment_details(&a, missing),
                            missing,
                        });
                    }
                }
                Err(e) => reply
                    .message
                    .push_str(&format!(" PDFの一覧を読み込めません: {e}")),
            }
        }
        reply.attachments = Some(rows);
    }
    fn handle_request(&mut self, request: Request) -> Reply {
        if matches!(request, Request::Initialize) {
            return match self.config.prepare().and_then(|_| {
                SqliteLibrary::open_local(&self.config.database)
                    .map_err(|e| format!("保存先を開けません: {e}"))
            }) {
                Ok(store) => {
                    self.app = Some(Application { store });
                    let files = AttachmentFiles::open(
                        &self.config.files_dir(),
                        &std::env::temp_dir().join("paper-inbox-open"),
                    );
                    let files_note = files
                        .as_ref()
                        .err()
                        .map(|e| format!(" PDFの保存先を開けないため、PDF機能は使えません: {e}"))
                        .unwrap_or_default();
                    self.files = files.ok();
                    self.editing = None;
                    self.pending_export = None;
                    self.review = None;
                    self.pending_lookup = None;
                    match self.app.as_ref().unwrap().list(&self.query) {
                        Ok(list) => Reply {
                            list: Some(list),
                            editor: Some(EditorView {
                                form: PaperForm::default(),
                                id: None,
                                deleted: false,
                                identifier_warning: String::new(),
                                bibtex: String::new(),
                            }),
                            ready: Some(true),
                            close_metadata: true,
                            message: format!("準備ができました。{files_note}"),
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
            Request::BeginLookup(new) => {
                self.pending_lookup = None;
                self.lookup_new = new;
                if new {
                    self.editing = None;
                    self.review = None;
                }
                Reply {
                    editor: if new {
                        Some(EditorView {
                            form: PaperForm::default(),
                            id: None,
                            deleted: false,
                            identifier_warning: String::new(),
                            bibtex: String::new(),
                        })
                    } else {
                        None
                    },
                    lookup_open: Some(true),
                    lookup_new: Some(new),
                    message:
                        "識別子を入力してください。取得時に Crossref または arXiv へ送信します。"
                            .into(),
                    ..Default::default()
                }
            }
            Request::FetchLookup(input) => {
                self.pending_lookup = None;
                let fetched = paper_metadata::parse_identifier(&input)
                    .and_then(|id| (self.lookup_provider)(&id));
                match fetched {
                    Ok(mut values) if values.len() == 1 => {
                        let candidate = values.remove(0);
                        if let Err(e) = candidate.validate() {
                            return Reply {
                                message: e.to_string(),
                                ..Default::default()
                            };
                        }
                        let key = if self.lookup_new {
                            match app.suggest_key_for(&candidate.bibliography, None) {
                                Ok(key) => key,
                                Err(e) => {
                                    return Reply {
                                        message: e.to_string(),
                                        ..Default::default()
                                    };
                                }
                            }
                        } else {
                            String::new()
                        };
                        let details = crate::lookup::preview(&candidate);
                        self.pending_lookup = Some(PendingLookup {
                            candidate,
                            paper_id: PaperId::new(),
                            input: input.trim().into(),
                        });
                        Reply {
                            lookup_preview: Some((details, key)),
                            message: "取得結果を確認してください。まだ保存していません。".into(),
                            ..Default::default()
                        }
                    }
                    Ok(_) => Reply {
                        message: "取得先から一意な候補が得られませんでした。".into(),
                        ..Default::default()
                    },
                    Err(e) => Reply {
                        message: e.to_string(),
                        ..Default::default()
                    },
                }
            }
            Request::ConfirmLookup(input, key, user_input) => {
                let Some(pending) = &self.pending_lookup else {
                    return Reply {
                        message: "書誌情報を再取得してください。".into(),
                        ..Default::default()
                    };
                };
                if input.trim() != pending.input {
                    return Reply {
                        message: "識別子が変わっています。再取得してください。".into(),
                        ..Default::default()
                    };
                }
                if self.lookup_new {
                    match app.register_lookup(
                        pending.paper_id,
                        &pending.candidate,
                        &key,
                        &user_input,
                    ) {
                        Ok(id) => {
                            self.pending_lookup = None;
                            let mut reply = self.after_save(id, "取得した書誌情報を登録しました。");
                            reply.lookup_open = Some(false);
                            reply.close_metadata = true;
                            reply
                        }
                        Err(e) => Reply {
                            message: e.to_string(),
                            ..Default::default()
                        },
                    }
                } else {
                    let Some(review) = &self.review else {
                        return Reply {
                            message: "文献の候補画面を開き直してください。".into(),
                            ..Default::default()
                        };
                    };
                    let id = review.state.paper.id();
                    let result = app.store.commit_batch_at(
                        vec![
                            paper_core::command::LibraryCommand::SubmitMetadataCandidate {
                                paper_id: id,
                                candidate: pending.candidate.clone(),
                            },
                        ],
                        std::slice::from_ref(&review.state),
                        chrono::Utc::now(),
                    );
                    match result {
                        Ok(_) => {
                            self.pending_lookup = None;
                            let mut reply = self.after_metadata(
                                id,
                                "取得結果を候補として保存しました。",
                                false,
                            );
                            reply.lookup_open = Some(false);
                            reply
                        }
                        Err(e) => Reply {
                            message: crate::application::AppError::from(e).to_string(),
                            ..Default::default()
                        },
                    }
                }
            }
            Request::CancelLookup => {
                self.pending_lookup = None;
                Reply {
                    lookup_open: Some(false),
                    message: "取得画面を閉じました。".into(),
                    ..Default::default()
                }
            }
            Request::SuggestKey(form) => {
                match app.suggest_key(&form, self.editing.as_ref().map(|s| s.paper.id())) {
                    Ok(key) => Reply {
                        key_suggestion: Some(key),
                        message: "引用キーの候補を入力しました。保存前に変更できます。".into(),
                        ..Default::default()
                    },
                    Err(e) => Reply {
                        message: e.to_string(),
                        ..Default::default()
                    },
                }
            }
            Request::List(query) => match app.list(&query) {
                Ok(list) => {
                    self.query = query;
                    Reply {
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
                self.review = None;
                self.pending_lookup = None;
                Reply {
                    close_metadata: true,
                    editor: Some(EditorView {
                        form: PaperForm::default(),
                        id: None,
                        deleted: false,
                        identifier_warning: String::new(),
                        bibtex: String::new(),
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
                        close_metadata: true,
                        ..Default::default()
                    }
                }
                Err(e) => Reply {
                    message: e.to_string(),
                    ..Default::default()
                },
            },
            Request::Save(mut form) => {
                // A new paper without a key gets the usual suggestion; the
                // reloaded form shows it. Saved keys are never generated here.
                let mut key_note = "";
                if self.editing.is_none() && form.citation_key.trim().is_empty() {
                    match app.suggest_key(&form, None) {
                        Ok(key) => {
                            form.citation_key = key;
                            key_note = "引用キーは自動で提案した値を使いました。";
                        }
                        Err(e) => {
                            return Reply {
                                message: e.to_string(),
                                form_error: Some(e.to_string()),
                                ..Default::default()
                            };
                        }
                    }
                }
                match app.save(self.editing.as_ref(), &form) {
                    Ok(id) => self.after_save(id, &format!("保存しました。{key_note}")),
                    Err(e) => Reply {
                        message: e.to_string(),
                        form_error: Some(e.to_string()),
                        ..Default::default()
                    },
                }
            }
            Request::Quick(change) => {
                let Some(state) = self.editing.clone() else {
                    return Reply {
                        message: "文献を選択してください。".into(),
                        ..Default::default()
                    };
                };
                let id = state.paper.id();
                let message = match &change {
                    QuickChange::ReadingStatus(_) => "読書状態を更新しました。",
                    QuickChange::Favorite(true) => "お気に入りに追加しました。",
                    QuickChange::Favorite(false) => "お気に入りから外しました。",
                    QuickChange::Archived(true) => "アーカイブしました。",
                    QuickChange::Archived(false) => "アーカイブを解除しました。",
                    QuickChange::Notes(_) => "メモを保存しました。",
                };
                let keeps_input = matches!(change, QuickChange::Notes(_));
                match app.quick_update(&state, change) {
                    Ok(()) => self.after_save(id, message),
                    // A failed note keeps the typed draft. A failed toggle
                    // reloads so the control shows the stored value again.
                    Err(e) if keeps_input => Reply {
                        message: e.to_string(),
                        ..Default::default()
                    },
                    Err(e) => {
                        let mut reply = self.after_save(id, "");
                        reply.saved = false;
                        reply.message = format!("{e} 表示を保存済みの内容に戻しました。");
                        reply
                    }
                }
            }
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
            Request::ReviewMetadata(id) => match app.metadata_review(id) {
                Ok(review) => {
                    self.editing = Some(review.state.clone());
                    self.review = Some(review.clone());
                    Reply {
                        editor: Some(view(&review.state)),
                        close_candidate: true,
                        metadata: Some(review),
                        message: "候補・提案・版を読み込みました。".into(),
                        ..Default::default()
                    }
                }
                Err(e) => Reply {
                    message: e.to_string(),
                    ..Default::default()
                },
            },
            Request::SubmitCandidate(form) => {
                let Some(review) = &self.review else {
                    return Reply {
                        message: "文献の候補画面を開き直してください。".into(),
                        ..Default::default()
                    };
                };
                let id = review.state.paper.id();
                match app.submit_candidate(&review.state, &form) {
                    Ok(()) => self.after_metadata(id, "候補を保存しました。", true),
                    Err(e) => Reply {
                        message: e.to_string(),
                        ..Default::default()
                    },
                }
            }
            Request::AssociateCandidate(candidate) => {
                let Some(review) = &self.review else {
                    return Reply {
                        message: "候補画面を開き直してください。".into(),
                        ..Default::default()
                    };
                };
                let id = review.state.paper.id();
                match app.associate_candidate(&review.state, candidate) {
                    Ok(()) => self.after_metadata(
                        id,
                        "候補を同じ文献の版として登録しました。差分は提案を確認してください。",
                        false,
                    ),
                    Err(e) => Reply {
                        message: e.to_string(),
                        ..Default::default()
                    },
                }
            }
            Request::ResolveMetadataBatch(inputs) => {
                let Some(review) = &self.review else {
                    return Reply {
                        message: "候補画面を開き直してください。".into(),
                        ..Default::default()
                    };
                };
                let id = review.state.paper.id();
                match app.resolve_metadata_batch(review, &inputs) {
                    Ok(()) => {
                        self.after_metadata(id, "判断済みの項目をまとめて保存しました。", false)
                    }
                    Err(e) => Reply {
                        message: e.to_string(),
                        ..Default::default()
                    },
                }
            }
            Request::ResolveMetadata(proposal, accept) => {
                let Some(review) = &self.review else {
                    return Reply {
                        message: "候補画面を開き直してください。".into(),
                        ..Default::default()
                    };
                };
                let id = review.state.paper.id();
                match app.resolve_metadata(&review.state, proposal, accept) {
                    Ok(()) => self.after_metadata(
                        id,
                        if accept {
                            "提案を採用しました。"
                        } else {
                            "提案を拒否しました。"
                        },
                        false,
                    ),
                    Err(e) => Reply {
                        message: e.to_string(),
                        ..Default::default()
                    },
                }
            }
            Request::ProposeCanonical(version) => {
                let Some(review) = &self.review else {
                    return Reply {
                        message: "版の画面を開き直してください。".into(),
                        ..Default::default()
                    };
                };
                let id = review.state.paper.id();
                match app.propose_canonical(&review.state, version) {
                    Ok(()) => self.after_metadata(id, "正規版の切替提案を作成しました。", false),
                    Err(e) => Reply {
                        message: e.to_string(),
                        ..Default::default()
                    },
                }
            }
            Request::PrepareExport { ids, path, options } => {
                self.pending_export = None;
                match export::prepare(app, &ids, &options, &path, &self.config.database) {
                    Ok(prepared) => {
                        let summary = format!(
                            "{}件の保存済み文献 / 警告{}件 / {}",
                            prepared.count,
                            prepared.output.warnings.len(),
                            prepared.path.display()
                        );
                        let messages = warning_messages(&prepared);
                        let preview = (
                            prepared.output.content.clone(),
                            summary,
                            messages,
                            prepared.replaces_file(),
                        );
                        self.pending_export = Some(prepared);
                        Reply {
                            export_preview: Some(preview),
                            message:
                                "出力内容を確認してください。未保存の編集内容は含めていません。"
                                    .into(),
                            ..Default::default()
                        }
                    }
                    Err(e) => Reply {
                        message: e.to_string(),
                        clear_export: true,
                        ..Default::default()
                    },
                }
            }
            Request::ConfirmExport => {
                let Some(prepared) = self.pending_export.take() else {
                    return Reply {
                        message: "出力をもう一度準備してください。".into(),
                        clear_export: true,
                        ..Default::default()
                    };
                };
                match prepared.publish() {
                    Ok(()) => Reply {
                        message: format!(
                            "{}件を {} に出力しました。警告{}件。",
                            prepared.count,
                            prepared.path.display(),
                            prepared.output.warnings.len()
                        ),
                        clear_export: true,
                        ..Default::default()
                    },
                    Err(e) => Reply {
                        message: e.to_string(),
                        clear_export: true,
                        ..Default::default()
                    },
                }
            }
            Request::CancelExport => {
                self.pending_export = None;
                Reply {
                    message: "出力をキャンセルしました。ファイルは変更していません。".into(),
                    clear_export: true,
                    ..Default::default()
                }
            }
            Request::AttachPdf(path) => {
                let (Some(state), Some(files)) = (self.editing.as_ref(), self.files.as_ref())
                else {
                    return Reply {
                        message: "PDFを添付する文献を選択してください。".into(),
                        ..Default::default()
                    };
                };
                let id = state.paper.id();
                match files.attach(&app.store, id, &path) {
                    Ok(attachment) => {
                        self.after_save(id, &format!("PDFを添付しました: {}", attachment.filename))
                    }
                    Err(e) => Reply {
                        message: e.to_string(),
                        ..Default::default()
                    },
                }
            }
            Request::OpenAttachment(attachment_id) | Request::DetachAttachment(attachment_id) => {
                let detach = matches!(request, Request::DetachAttachment(_));
                let (Some(state), Some(files)) = (self.editing.as_ref(), self.files.as_ref())
                else {
                    return Reply {
                        message: "文献を選択してください。".into(),
                        ..Default::default()
                    };
                };
                let id = state.paper.id();
                // Only attachments of the paper on screen can be opened or removed.
                let found = files
                    .list(&app.store, id)
                    .map(|list| list.into_iter().find(|a| a.id == attachment_id));
                let attachment = match found {
                    Ok(Some(a)) => a,
                    Ok(None) => {
                        return Reply {
                            message: "このPDFは見つかりません。表示を更新してください。".into(),
                            ..Default::default()
                        };
                    }
                    Err(e) => {
                        return Reply {
                            message: e.to_string(),
                            ..Default::default()
                        };
                    }
                };
                if detach {
                    match files.detach(&app.store, attachment.id) {
                        Ok(()) => self.after_save(
                            id,
                            "PDFを取り外しました。ファイルは「未使用PDFの整理」で削除するまで残ります。",
                        ),
                        Err(e) => Reply {
                            message: e.to_string(),
                            ..Default::default()
                        },
                    }
                } else {
                    let opened = files
                        .prepare_open(&attachment)
                        .and_then(|path| AttachmentFiles::open_with_system(&path));
                    Reply {
                        message: match opened {
                            Ok(()) => format!("PDFを開きました: {}", attachment.filename),
                            Err(e) => e.to_string(),
                        },
                        ..Default::default()
                    }
                }
            }
            Request::PrepareCleanup => match self.files.as_ref().map(|f| f.unused(&app.store)) {
                Some(Ok(unused)) => Reply {
                    cleanup: Some((unused.len(), unused.iter().map(|(_, bytes)| bytes).sum())),
                    ..Default::default()
                },
                Some(Err(e)) => Reply {
                    message: e.to_string(),
                    ..Default::default()
                },
                None => Reply {
                    message: "PDFの保存先を開けていません。再接続してください。".into(),
                    ..Default::default()
                },
            },
            Request::ReclaimUnused => match self.files.as_ref().map(|f| f.reclaim(&app.store)) {
                Some(Ok(report)) => Reply {
                    cleanup: Some((0, 0)),
                    message: format!(
                        "未使用のPDFを {} 件（{}）削除しました。",
                        report.files,
                        crate::detail::format_size(report.bytes)
                    ),
                    ..Default::default()
                },
                Some(Err(e)) => Reply {
                    message: e.to_string(),
                    ..Default::default()
                },
                None => Reply {
                    message: "PDFの保存先を開けていません。再接続してください。".into(),
                    ..Default::default()
                },
            },
            Request::Initialize | Request::Shutdown => unreachable!(),
        }
    }
    fn after_metadata(&mut self, id: PaperId, message: &str, close_candidate: bool) -> Reply {
        let mut reply = Reply {
            message: message.into(),
            close_candidate,
            ..Default::default()
        };
        let app = self.app.as_ref().unwrap();
        match app.metadata_review(id) {
            Ok(review) => {
                self.editing = Some(review.state.clone());
                self.review = Some(review.clone());
                reply.editor = Some(view(&review.state));
                reply.metadata = Some(review);
            }
            Err(e) => {
                reply.load_required = true;
                reply.message = format!(
                    "{message} 再読込には失敗しました。保存を繰り返さず、表示を更新してください: {e}"
                );
            }
        }
        if let Ok(list) = app.list(&self.query) {
            reply.list = Some(list);
        }
        reply
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
            Ok(list) => {
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
        bibtex: paper_core::service::render_entry(&state.paper, &BibtexOptions::default()).content,
        deleted: state.paper.deleted_at().is_some(),
        identifier_warning: if state.paper.bibliography().identifiers.raw_doi.is_some()
            || state
                .paper
                .bibliography()
                .identifiers
                .raw_arxiv_id
                .is_some()
        {
            "以前の識別子に形式の問題があります。元の値は保持しています。識別子を変更するときは両方の入力を確認してください。不正な値は出力から除外します。".into()
        } else {
            String::new()
        },
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
    let mut resolver = None;
    attach_with_lookup_provider(
        ui,
        config,
        Box::new(move |id| {
            if resolver.is_none() {
                resolver = Some(paper_metadata::HttpMetadataResolver::new()?);
            }
            resolver.as_ref().expect("initialized").resolve_blocking(id)
        }),
    )
}
/// Chooses the PDF to attach. It runs on the UI thread and is replaced in tests.
pub type FilePicker = Box<dyn Fn() -> Option<PathBuf>>;
fn choose_pdf() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("添付するPDFを選択")
        .add_filter("PDF", &["pdf"])
        .pick_file()
}
/// Inject transport for deterministic integration tests; production uses fixed HTTPS hosts.
pub fn attach_with_lookup_provider(
    ui: &MainWindow,
    config: Config,
    lookup_provider: LookupProvider,
) -> Controller {
    attach_with_providers(ui, config, lookup_provider, Box::new(choose_pdf))
}
/// Like [`attach_with_lookup_provider`] with an injectable file chooser.
pub fn attach_with_providers(
    ui: &MainWindow,
    config: Config,
    lookup_provider: LookupProvider,
    file_picker: FilePicker,
) -> Controller {
    ui.set_database_path(config.database.to_string_lossy().as_ref().into());
    let (sender, receiver) = mpsc::channel();
    let weak = ui.as_weak();
    let worker = thread::spawn(move || {
        let mut state = Worker {
            config,
            app: None,
            files: None,
            editing: None,
            pending_export: None,
            review: None,
            pending_lookup: None,
            lookup_new: false,
            lookup_provider,
            query: PaperQuery {
                pagination: Pagination {
                    limit: 50,
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
                NavigationAction::Quit => {
                    let _ = slint::quit_event_loop();
                    return;
                }
                NavigationAction::ImportPaper => Request::BeginLookup(true),
                NavigationAction::CreatePaper => Request::New,
                NavigationAction::AttachmentCleanup => Request::PrepareCleanup,
                NavigationAction::Reconnect => {
                    ui.set_ready(false);
                    Request::Initialize
                }
                NavigationAction::OpenPaper | NavigationAction::MetadataReview => {
                    match uuid::Uuid::parse_str(&id) {
                        Ok(id) => {
                            if action == NavigationAction::MetadataReview {
                                Request::ReviewMetadata(PaperId::from_uuid(id))
                            } else {
                                Request::Open(PaperId::from_uuid(id))
                            }
                        }
                        Err(_) => {
                            ui.set_status("文献 ID を読み取れません。".into());
                            return;
                        }
                    }
                }
            };
            dispatch(&ui, &tx, request);
        }
    });
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_begin_lookup(move || {
        if let Some(ui) = weak.upgrade() {
            dispatch(&ui, &tx, Request::BeginLookup(false));
        }
    });
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_fetch_lookup(move || {
        if let Some(ui) = weak.upgrade() {
            ui.set_lookup_has_result(false);
            dispatch(
                &ui,
                &tx,
                Request::FetchLookup(ui.get_lookup_identifier().into()),
            );
        }
    });
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_confirm_lookup(move || {
        if let Some(ui) = weak.upgrade() {
            dispatch(
                &ui,
                &tx,
                Request::ConfirmLookup(
                    ui.get_lookup_identifier().into(),
                    ui.get_lookup_key().into(),
                    LookupUserInput {
                        notes: ui.get_lookup_notes().into(),
                        reading_status: match ui.get_lookup_reading_status() {
                            1 => ReadingStatus::Reading,
                            2 => ReadingStatus::Read,
                            _ => ReadingStatus::Unread,
                        },
                        favorite: ui.get_lookup_favorite(),
                    },
                ),
            );
        }
    });
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_cancel_lookup(move || {
        if let Some(ui) = weak.upgrade() {
            dispatch(&ui, &tx, Request::CancelLookup);
        }
    });
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_suggest_key(move || {
        if let Some(ui) = weak.upgrade() {
            dispatch(&ui, &tx, Request::SuggestKey(form_from_ui(&ui)));
        }
    });
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_save(move || {
        if let Some(ui) = weak.upgrade() {
            dispatch(&ui, &tx, Request::Save(form_from_ui(&ui)));
        }
    });
    // Detail quick actions save one personal value without the edit form.
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_change_reading_status(move |index| {
        if let Some(ui) = weak.upgrade() {
            let status = match index {
                1 => ReadingStatus::Reading,
                2 => ReadingStatus::Read,
                _ => ReadingStatus::Unread,
            };
            dispatch(&ui, &tx, Request::Quick(QuickChange::ReadingStatus(status)));
        }
    });
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_toggle_favorite(move || {
        if let Some(ui) = weak.upgrade() {
            let favorite = !ui.get_favorite();
            dispatch(&ui, &tx, Request::Quick(QuickChange::Favorite(favorite)));
        }
    });
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_toggle_archived(move || {
        if let Some(ui) = weak.upgrade() {
            let archived = !ui.get_archived();
            dispatch(&ui, &tx, Request::Quick(QuickChange::Archived(archived)));
        }
    });
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_save_notes(move || {
        if let Some(ui) = weak.upgrade() {
            let notes = ui.get_notes().to_string();
            dispatch(&ui, &tx, Request::Quick(QuickChange::Notes(notes)));
        }
    });
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_attach_pdf(move || {
        if let Some(ui) = weak.upgrade() {
            match file_picker() {
                Some(path) => dispatch(&ui, &tx, Request::AttachPdf(path)),
                None => ui.set_status("PDFの添付をやめました。".into()),
            }
        }
    });
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_open_attachment(move |id| {
        if let Some(ui) = weak.upgrade() {
            match uuid::Uuid::parse_str(&id) {
                Ok(id) => dispatch(
                    &ui,
                    &tx,
                    Request::OpenAttachment(AttachmentId::from_uuid(id)),
                ),
                Err(_) => ui.set_status("PDFの ID を読み取れません。".into()),
            }
        }
    });
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_detach_attachment(move |id| {
        if let Some(ui) = weak.upgrade() {
            match uuid::Uuid::parse_str(&id) {
                Ok(id) => dispatch(
                    &ui,
                    &tx,
                    Request::DetachAttachment(AttachmentId::from_uuid(id)),
                ),
                Err(_) => ui.set_status("PDFの ID を読み取れません。".into()),
            }
        }
    });
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_reclaim_unused(move || {
        if let Some(ui) = weak.upgrade() {
            dispatch(&ui, &tx, Request::ReclaimUnused);
        }
    });
    let weak = ui.as_weak();
    ui.on_open_link(move |target| {
        if let Some(ui) = weak.upgrade() {
            let url = match target {
                LinkTarget::Doi => crate::detail::doi_url(&ui.get_doi()),
                LinkTarget::Arxiv => crate::detail::arxiv_url(&ui.get_arxiv_id()),
                LinkTarget::Url => crate::detail::web_url(&ui.get_url()),
            };
            let message = match url {
                Some(url) => match crate::detail::open_in_browser(&url) {
                    Ok(()) => format!("ブラウザーで開きました: {url}"),
                    Err(e) => e,
                },
                None => "開ける形式のリンクではありません。値を確認してください。".into(),
            };
            ui.set_status(message.into());
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
    ui.on_select_paper(move |id, selected| {
        if let Some(ui) = weak.upgrade() {
            let mut ids: Vec<_> = ui.get_selected_ids().iter().collect();
            if selected {
                if !ids.contains(&id) {
                    ids.push(id.clone());
                }
            } else {
                ids.retain(|v| v != &id);
            }
            ui.set_selected_count(ids.len() as i32);
            ui.set_selected_ids(ModelRc::new(VecModel::from(ids)));
            let model = ui.get_papers();
            if let Some(rows) = model.as_any().downcast_ref::<VecModel<PaperRow>>() {
                for i in 0..rows.row_count() {
                    if let Some(mut row) = rows.row_data(i)
                        && row.id == id
                    {
                        row.selected = selected;
                        rows.set_row_data(i, row);
                    }
                }
            }
        }
    });
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_prepare_export(move || {
        if let Some(ui) = weak.upgrade() {
            let ids = ui
                .get_selected_ids()
                .iter()
                .map(|s| uuid::Uuid::parse_str(&s).map(PaperId::from_uuid))
                .collect::<Result<Vec<_>, _>>();
            let ids = match ids {
                Ok(ids) => ids,
                Err(_) => {
                    ui.set_status("選択内容を確認してください。".into());
                    return;
                }
            };
            ui.set_preview_return_view(ui.get_detail_view());
            let options = BibtexOptions {
                include_abstract: ui.get_export_abstract(),
                include_doi: ui.get_export_doi(),
                include_url: ui.get_export_url(),
                include_keywords: ui.get_export_keywords(),
            };
            dispatch(
                &ui,
                &tx,
                Request::PrepareExport {
                    ids,
                    path: PathBuf::from(ui.get_export_path().as_str()),
                    options,
                },
            );
        }
    });
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_confirm_export(move || {
        if let Some(ui) = weak.upgrade() {
            dispatch(&ui, &tx, Request::ConfirmExport);
        }
    });
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_cancel_export(move || {
        if let Some(ui) = weak.upgrade() {
            dispatch(&ui, &tx, Request::CancelExport);
        }
    });
    let weak = ui.as_weak();
    ui.on_begin_candidate(move || {
        if let Some(ui) = weak.upgrade() {
            crate::ui_metadata::begin_form(&ui);
        }
    });
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_submit_candidate(move || {
        if let Some(ui) = weak.upgrade() {
            dispatch(
                &ui,
                &tx,
                Request::SubmitCandidate(crate::ui_metadata::read_form(&ui)),
            );
        }
    });
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_associate_candidate(move |id| {
        if let Some(ui) = weak.upgrade() {
            match uuid::Uuid::parse_str(&id) {
                Ok(id) => dispatch(
                    &ui,
                    &tx,
                    Request::AssociateCandidate(CandidateId::from_uuid(id)),
                ),
                Err(_) => ui.set_status("候補の表示を更新してください。".into()),
            }
        }
    });
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_apply_metadata(move || {
        if let Some(ui) = weak.upgrade() {
            match crate::ui_metadata::read_decisions(&ui) {
                Ok(inputs) => dispatch(&ui, &tx, Request::ResolveMetadataBatch(inputs)),
                Err(e) => ui.set_status(e.to_string().into()),
            }
        }
    });
    crate::ui_metadata::attach_decisions(ui);
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_resolve_metadata(move |id, accept| {
        if let Some(ui) = weak.upgrade() {
            match uuid::Uuid::parse_str(&id) {
                Ok(id) => dispatch(
                    &ui,
                    &tx,
                    Request::ResolveMetadata(ProposalId::from_uuid(id), accept),
                ),
                Err(_) => ui.set_status("提案の表示を更新してください。".into()),
            }
        }
    });
    let tx = sender.clone();
    let weak = ui.as_weak();
    ui.on_propose_canonical(move |id| {
        if let Some(ui) = weak.upgrade() {
            match uuid::Uuid::parse_str(&id) {
                Ok(id) => dispatch(
                    &ui,
                    &tx,
                    Request::ProposeCanonical(VersionId::from_uuid(id)),
                ),
                Err(_) => ui.set_status("版の表示を更新してください。".into()),
            }
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
    ui.on_author_bulk(move |text| {
        if let Some(ui) = weak.upgrade() {
            let model = ui.get_author_rows();
            let Some(rows) = model.as_any().downcast_ref::<VecModel<AuthorRow>>() else {
                return;
            };
            let parsed = crate::application::parse_author_list(&text);
            if parsed.is_empty() {
                ui.set_status("追加できる著者名がありませんでした。".into());
                return;
            }
            let count = parsed.len();
            for author in parsed {
                rows.push(AuthorRow {
                    kind: author.kind,
                    given: author.given.into(),
                    name: author.name.into(),
                    orcid: author.orcid.into(),
                });
            }
            ui.set_dirty(true);
            ui.set_status(format!("著者を{count}名追加しました。各行を確認してください。").into());
        }
    });
    let weak = ui.as_weak();
    ui.on_author_action(move |action, index| {
        if let Some(ui) = weak.upgrade() {
            let model = ui.get_author_rows();
            let Some(rows) = model.as_any().downcast_ref::<VecModel<AuthorRow>>() else {
                return;
            };
            let index = index.max(0) as usize;
            match action {
                AuthorAction::Add => rows.push(AuthorRow::default()),
                AuthorAction::Remove => {
                    if index < rows.row_count() {
                        rows.remove(index);
                    }
                }
                AuthorAction::MoveUp | AuthorAction::MoveDown => {
                    let other = if action == AuthorAction::MoveUp {
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
            }
            ui.set_dirty(true);
        }
    });
    let weak = ui.as_weak();
    ui.window().on_close_requested(move || {
        if let Some(ui) = weak.upgrade()
            && (ui.get_dirty() || ui.get_candidate_dirty() || ui.get_lookup_dirty())
        {
            ui.invoke_request_navigation(NavigationAction::Quit, "".into());
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
        publisher: ui.get_publisher().into(),
        volume: ui.get_volume().into(),
        issue: ui.get_issue().into(),
        pages: ui.get_pages().into(),
        doi: ui.get_doi().into(),
        arxiv_id: ui.get_arxiv_id().into(),
        url: ui.get_url().into(),
        abstract_text: ui.get_abstract_text().into(),
        keywords: ui.get_keywords().into(),
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
            limit: ui.get_limit().max(1) as usize,
            offset: 0,
        },
    }
}
fn apply_reply(ui: &MainWindow, reply: Reply) {
    let mut next_view = None;
    if let Some(new) = reply.lookup_new {
        ui.set_lookup_new(new);
        if new {
            ui.set_candidate_dirty(false);
        }
        ui.set_lookup_identifier("".into());
        ui.set_lookup_source("".into());
        ui.set_lookup_fields(ModelRc::default());
        ui.set_lookup_key("".into());
        ui.set_lookup_notes("".into());
        ui.set_lookup_reading_status(0);
        ui.set_lookup_favorite(false);
        ui.set_lookup_has_result(false);
        ui.set_lookup_dirty(false);
    }
    if reply.lookup_open == Some(false) {
        ui.set_lookup_dirty(false);
        ui.set_lookup_has_result(false);
    }
    if let Some((preview, key)) = reply.lookup_preview {
        let fields: Vec<_> = preview
            .fields
            .into_iter()
            .map(|(label, value)| LookupField {
                label: label.into(),
                value: value.into(),
            })
            .collect();
        ui.set_lookup_source(preview.source.into());
        ui.set_lookup_fields(ModelRc::new(VecModel::from(fields)));
        ui.set_lookup_key(key.into());
        ui.set_lookup_has_result(true);
        ui.set_lookup_dirty(true);
    }
    if let Some(key) = reply.key_suggestion {
        ui.set_citation_key(key.into());
        ui.set_dirty(true);
    }

    if reply.close_metadata {
        ui.set_lookup_dirty(false);
        ui.set_candidate_dirty(false);
    }
    if let Some(ready) = reply.ready {
        ui.set_ready(ready);
    }
    if let Some(editor) = reply.editor {
        let f = editor.form;
        // A complete, successfully loaded editor is the only point at which
        // an existing paper returns to read mode. Error replies keep the draft.
        next_view = Some(if editor.id.is_none() || editor.deleted {
            DetailView::PaperEditor
        } else {
            DetailView::PaperDetail
        });
        // Read-only presentation derived from the same saved form.
        ui.set_paper_authors(crate::detail::authors_full(&f).into());
        ui.set_paper_authors_short(crate::detail::authors_short(&f).into());
        ui.set_publication_summary(crate::detail::publication_summary(&f).into());
        let (preview, long) = crate::detail::abstract_preview(&f.abstract_text);
        ui.set_abstract_preview(preview.into());
        ui.set_abstract_long(long);
        ui.set_keywords_line(crate::detail::keywords_line(&f).into());
        ui.set_paper_bibtex(editor.bibtex.into());
        ui.set_identifiers_line(crate::detail::identifiers_line(&f).into());
        ui.set_notes_editing(false);
        ui.set_form_error("".into());
        ui.set_citation_key(f.citation_key.into());
        ui.set_paper_title(f.title.into());
        ui.set_container_title(f.container_title.into());
        ui.set_year(f.year.into());
        ui.set_publisher(f.publisher.into());
        ui.set_volume(f.volume.into());
        ui.set_issue(f.issue.into());
        ui.set_pages(f.pages.into());
        ui.set_doi(f.doi.into());
        ui.set_arxiv_id(f.arxiv_id.into());
        ui.set_url(f.url.into());
        ui.set_abstract_text(f.abstract_text.into());
        ui.set_keywords(f.keywords.into());
        ui.set_identifier_warning(editor.identifier_warning.into());
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
        ui.set_review_dirty(false);
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
    if let Some(review) = reply.metadata {
        crate::ui_metadata::show_review(ui, &review);
        next_view = Some(DetailView::MetadataReview);
    }
    if reply.close_candidate {
        if ui.get_detail_view() == DetailView::MetadataCandidateEditor {
            next_view = Some(DetailView::MetadataReview);
        }
        ui.set_candidate_dirty(false);
    }
    if reply.saved {
        ui.set_dirty(false);
    }
    if let Some(error) = reply.form_error {
        ui.set_form_error(error.into());
    }
    if let Some(rows) = reply.attachments {
        let rows: Vec<_> = rows
            .into_iter()
            .map(|a| AttachmentRow {
                id: a.id.into(),
                name: a.name.into(),
                details: a.details.into(),
                missing: a.missing,
            })
            .collect();
        // The main button opens the first file that is still in the library.
        ui.set_primary_attachment(
            rows.iter()
                .find(|r| !r.missing)
                .map(|r| r.id.clone())
                .unwrap_or_default(),
        );
        ui.set_attachment_rows(ModelRc::new(VecModel::from(rows)));
    }
    if let Some((count, bytes)) = reply.cleanup {
        ui.set_cleanup_count(count.min(i32::MAX as usize) as i32);
        ui.set_cleanup_size(crate::detail::format_size(bytes).into());
        next_view = Some(DetailView::AttachmentCleanup);
    }

    if reply.load_required {
        ui.set_load_required(true);
    }
    if let Some(list) = reply.list {
        let rows: Vec<_> = list
            .papers
            .into_iter()
            .map(|p| PaperRow {
                selected: ui
                    .get_selected_ids()
                    .iter()
                    .any(|id| id.as_str() == p.id.to_string()),
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
    if let Some((content, summary, warnings, replaces)) = reply.export_preview {
        ui.set_export_preview(content.into());
        ui.set_export_summary(summary.into());
        ui.set_export_warnings(warnings.into());
        ui.set_export_replaces(replaces);
        next_view = Some(DetailView::ExportPreview);
    }
    if reply.clear_export && ui.get_detail_view() == DetailView::ExportPreview {
        next_view = Some(ui.get_preview_return_view());
    }
    if reply.lookup_open == Some(true) {
        next_view = Some(DetailView::Import);
    } else if reply.lookup_open == Some(false)
        && ui.get_detail_view() == DetailView::Import
        && next_view.is_none()
    {
        // Closing the unified add screen returns to the neutral view; manual
        // entry is an explicit navigation of its own.
        next_view = Some(if ui.get_lookup_new() {
            DetailView::Empty
        } else {
            DetailView::MetadataReview
        });
    }
    if let Some(view) = next_view {
        ui.set_detail_view(view);
    }
    ui.set_status(reply.message.into());
    ui.set_busy(false);
}

fn warning_messages(prepared: &PreparedExport) -> String {
    prepared
        .output
        .warnings
        .iter()
        .map(|warning| {
            let (id, text) = match warning {
                BibtexWarning::MissingRequiredField { paper_id, field } => (
                    *paper_id,
                    format!(
                        "必須項目がありません: {}",
                        match *field {
                            "author" => "著者",
                            "title" => "タイトル",
                            "journal" => "掲載誌",
                            "year" => "出版年",
                            "booktitle" => "書籍名・会議名",
                            "publisher" => "出版社",
                            "school" => "学校名（入力未対応）",
                            "institution" => "機関名（入力未対応）",
                            "author/editor" => "著者または編集者",
                            other => other,
                        }
                    ),
                ),
                BibtexWarning::FallbackEntryType { paper_id } => {
                    (*paper_id, "専用の種別がないため misc で出力します。".into())
                }
                BibtexWarning::InvalidStoredIdentifier { paper_id, field } => (
                    *paper_id,
                    format!("形式に問題のある既存の {field} は出力しません。"),
                ),
                BibtexWarning::EmptyContributor { paper_id, role } => {
                    (*paper_id, format!("名前が空の {role} は出力しません。"))
                }
            };
            let label = prepared
                .labels
                .iter()
                .find(|(key, _)| *key == id)
                .map(|(_, s)| s.as_str())
                .unwrap_or("文献");
            format!("{label}: {text}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod reply_contract {
    use super::{EditorView, Reply, apply_reply};
    use crate::{
        DetailView, MainWindow,
        application::{AuthorInput, PaperForm},
    };
    use paper_core::model::PaperId;

    #[test]
    fn only_a_complete_reload_leaves_existing_edit_mode() {
        i_slint_backend_testing::init_no_event_loop();
        let w = MainWindow::new().unwrap();
        w.set_detail_view(DetailView::PaperEditor);
        w.set_dirty(true);
        w.set_paper_title("draft".into());
        apply_reply(
            &w,
            Reply {
                message: "validation or conflict failure".into(),
                ..Default::default()
            },
        );
        assert!(w.get_edit_mode());
        assert!(w.get_dirty());
        assert_eq!(w.get_paper_title(), "draft");
        apply_reply(
            &w,
            Reply {
                saved: true,
                load_required: true,
                message: "saved, reload failed".into(),
                ..Default::default()
            },
        );
        assert!(w.get_edit_mode());
        assert!(!w.get_dirty());
        assert!(w.get_load_required());
        assert_eq!(w.get_paper_title(), "draft");
        apply_reply(
            &w,
            Reply {
                editor: Some(EditorView {
                    id: Some(PaperId::new()),
                    deleted: false,
                    identifier_warning: String::new(),
                    bibtex: String::new(),
                    form: PaperForm {
                        title: "loaded".into(),
                        authors: vec![
                            AuthorInput {
                                kind: 0,
                                given: "Ada".into(),
                                name: "Smith".into(),
                                orcid: String::new(),
                            },
                            AuthorInput {
                                kind: 1,
                                name: "Research Group".into(),
                                ..Default::default()
                            },
                            AuthorInput {
                                kind: 2,
                                name: "Literal Author".into(),
                                ..Default::default()
                            },
                        ],
                        ..Default::default()
                    },
                }),
                ..Default::default()
            },
        );
        assert!(!w.get_edit_mode());
        assert!(!w.get_load_required());
        assert_eq!(w.get_paper_title(), "loaded");
        assert_eq!(
            w.get_paper_authors(),
            "Ada Smith, Research Group, Literal Author"
        );
        apply_reply(
            &w,
            Reply {
                editor: Some(EditorView {
                    id: None,
                    deleted: false,
                    identifier_warning: String::new(),
                    bibtex: String::new(),
                    form: PaperForm::default(),
                }),
                ..Default::default()
            },
        );
        assert!(w.get_edit_mode());
        assert_eq!(w.get_paper_authors(), "");
    }
}

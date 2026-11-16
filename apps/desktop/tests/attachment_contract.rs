use chrono::Utc;
use paper_core::{
    command::LibraryCommand,
    model::{Bibliography, CitationKey, Paper, PaperId},
    ports::BlobStore,
};
use paper_desktop::{
    application::AppError,
    attachment::{AttachmentFiles, FileState},
};
use paper_storage::{FileBlobStore, SqliteLibrary, content_hash_hex};
use std::{fs, path::PathBuf};

struct Sandbox(PathBuf);
impl Sandbox {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("paper-inbox-app-files-{}", PaperId::new()));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn files(&self) -> PathBuf {
        self.0.join("library-files")
    }
    fn viewer(&self) -> PathBuf {
        self.0.join("viewer")
    }
    fn pdf(&self, name: &str, body: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, format!("%PDF-1.7\n{body}")).unwrap();
        path
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Fixture {
    sandbox: Sandbox,
    store: SqliteLibrary,
    files: AttachmentFiles,
}
impl Fixture {
    fn new() -> Self {
        let sandbox = Sandbox::new();
        let store = SqliteLibrary::open_local(sandbox.0.join("library.sqlite")).unwrap();
        let files = AttachmentFiles::open(&sandbox.files(), &sandbox.viewer()).unwrap();
        Self {
            sandbox,
            store,
            files,
        }
    }
    fn paper(&self, key: &str) -> PaperId {
        let paper = Paper::new(
            PaperId::new(),
            CitationKey::parse(key).unwrap(),
            Bibliography {
                title: Some("Paper".into()),
                ..Default::default()
            },
            Utc::now(),
        )
        .unwrap();
        let id = paper.id();
        self.store
            .commit_at(LibraryCommand::CreatePaper { paper }, Utc::now())
            .unwrap();
        id
    }
    fn stored_files(&self) -> usize {
        FileBlobStore::open(self.sandbox.files())
            .unwrap()
            .hashes()
            .unwrap()
            .len()
    }
}

fn input_message(error: AppError) -> String {
    match error {
        AppError::Input(message) => message,
        other => panic!("expected an input error, got {other:?}"),
    }
}

#[test]
fn a_pdf_is_stored_listed_with_its_real_size_and_survives_reopening() {
    let f = Fixture::new();
    let paper = f.paper("Alpha2026");
    let path = f.sandbox.pdf("日本語 paper.pdf", "body");
    let expected = fs::metadata(&path).unwrap().len();

    let attachment = f.files.attach(&f.store, paper, &path).unwrap();
    assert_eq!(attachment.filename, "日本語 paper.pdf");
    assert_eq!(attachment.media_type, "application/pdf");
    assert_eq!(attachment.size_bytes, expected);
    assert_eq!(f.files.list(&f.store, paper).unwrap().len(), 1);
    assert_eq!(f.stored_files(), 1);
    assert_eq!(
        content_hash_hex(&attachment.content_hash).len(),
        64,
        "stored under its SHA-256"
    );
    assert_eq!(f.files.check(&attachment).unwrap(), FileState::Present);

    let reopened = AttachmentFiles::open(&f.sandbox.files(), &f.sandbox.viewer()).unwrap();
    assert_eq!(reopened.list(&f.store, paper).unwrap().len(), 1);
}

#[test]
fn rejected_files_leave_neither_a_row_nor_a_stored_file() {
    let f = Fixture::new();
    let paper = f.paper("Beta2026");
    let text = f.sandbox.0.join("notes.pdf");
    fs::write(&text, "this is not a pdf").unwrap();
    let empty = f.sandbox.0.join("empty.pdf");
    fs::write(&empty, "").unwrap();

    for bad in [text, empty] {
        let message = input_message(f.files.attach(&f.store, paper, &bad).unwrap_err());
        assert!(message.contains("PDF"), "{message}");
    }
    assert!(
        f.files.attach(&f.store, paper, &f.sandbox.0).is_err(),
        "a folder"
    );
    assert!(
        f.files
            .attach(&f.store, paper, &f.sandbox.0.join("missing.pdf"))
            .is_err()
    );
    assert!(f.files.list(&f.store, paper).unwrap().is_empty());
    assert_eq!(f.stored_files(), 0);
    assert!(f.files.unused(&f.store).unwrap().is_empty());
}

#[test]
fn the_same_pdf_is_refused_twice_on_a_paper_but_shared_between_papers() {
    let f = Fixture::new();
    let (first, second) = (f.paper("Gamma2026"), f.paper("Delta2026"));
    let path = f.sandbox.pdf("same.pdf", "identical");
    f.files.attach(&f.store, first, &path).unwrap();

    let message = input_message(f.files.attach(&f.store, first, &path).unwrap_err());
    assert!(message.contains("すでに"), "{message}");
    assert_eq!(f.files.list(&f.store, first).unwrap().len(), 1);

    let copy = f.sandbox.pdf("renamed.pdf", "identical");
    f.files.attach(&f.store, second, &copy).unwrap();
    assert_eq!(
        f.stored_files(),
        1,
        "identical content keeps one stored file"
    );
    assert_eq!(
        f.files.list(&f.store, second).unwrap()[0].filename,
        "renamed.pdf"
    );
}

#[test]
fn opening_uses_a_named_temporary_copy_and_never_a_damaged_one() {
    let f = Fixture::new();
    let paper = f.paper("Epsilon2026");
    let path = f.sandbox.pdf("Smith: results?.pdf", "content");
    let attachment = f.files.attach(&f.store, paper, &path).unwrap();

    let copy = f.files.prepare_open(&attachment).unwrap();
    assert!(copy.starts_with(f.sandbox.viewer()));
    assert_eq!(copy.file_name().unwrap(), "Smith_ results_.pdf");
    assert_eq!(fs::read(&copy).unwrap(), fs::read(&path).unwrap());
    let second = f.files.prepare_open(&attachment).unwrap();
    assert_ne!(
        copy.parent(),
        second.parent(),
        "each open gets its own folder"
    );

    // Damage the stored file: opening is refused and leaves no copy behind.
    let hex = content_hash_hex(&attachment.content_hash);
    let stored = f.sandbox.files().join("blobs").join(&hex[..2]).join(&hex);
    fs::write(&stored, b"damaged").unwrap();
    assert_eq!(f.files.check(&attachment).unwrap(), FileState::Damaged);
    let before = fs::read_dir(f.sandbox.viewer()).unwrap().count();
    assert!(matches!(
        f.files.prepare_open(&attachment),
        Err(AppError::Storage(_))
    ));
    assert_eq!(fs::read_dir(f.sandbox.viewer()).unwrap().count(), before);

    fs::remove_file(&stored).unwrap();
    assert_eq!(f.files.check(&attachment).unwrap(), FileState::Missing);
    let message = input_message(f.files.prepare_open(&attachment).unwrap_err());
    assert!(message.contains("見つかりません"), "{message}");

    // The next start clears copies left behind.
    drop(f.files);
    let restarted = AttachmentFiles::open(&f.sandbox.files(), &f.sandbox.viewer()).unwrap();
    assert!(!f.sandbox.viewer().exists());
    drop(restarted);
}

#[test]
fn detaching_keeps_the_file_until_the_explicit_cleanup_removes_only_unused_ones() {
    let f = Fixture::new();
    let (first, second) = (f.paper("Zeta2026"), f.paper("Eta2026"));
    let shared = f.sandbox.pdf("shared.pdf", "shared");
    let own = f.sandbox.pdf("own.pdf", "own");
    let a = f.files.attach(&f.store, first, &shared).unwrap();
    let b = f.files.attach(&f.store, first, &own).unwrap();
    f.files.attach(&f.store, second, &shared).unwrap();

    f.files.detach(&f.store, a.id).unwrap();
    f.files.detach(&f.store, b.id).unwrap();
    assert_eq!(f.stored_files(), 2, "detaching never deletes a file");
    let unused = f.files.unused(&f.store).unwrap();
    assert_eq!(
        unused.len(),
        1,
        "the shared file is still used by the other paper"
    );
    assert_eq!(unused[0].0, b.content_hash);
    assert_eq!(unused[0].1, b.size_bytes);

    let report = f.files.reclaim(&f.store).unwrap();
    assert_eq!((report.files, report.bytes), (1, b.size_bytes));
    assert_eq!(f.stored_files(), 1);
    assert_eq!(
        f.files.reclaim(&f.store).unwrap().files,
        0,
        "nothing left to reclaim"
    );
    assert_eq!(f.files.list(&f.store, second).unwrap().len(), 1);
    let blobs = FileBlobStore::open(f.sandbox.files()).unwrap();
    assert!(!blobs.contains(&b.content_hash).unwrap());
    assert!(blobs.contains(&a.content_hash).unwrap());

    // A removed attachment can be attached again.
    f.files.attach(&f.store, first, &shared).unwrap();
    assert_eq!(f.files.list(&f.store, first).unwrap().len(), 1);
}

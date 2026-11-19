use chrono::{DateTime, TimeZone, Utc};
use paper_core::{
    command::LibraryCommand,
    model::*,
    ports::{BlobStore, LibraryStore, PaperRepository},
    query::*,
};
use paper_storage::{
    BlobError, FileBlobStore, SqliteLibrary, StorageError, content_hash_hex, parse_content_hash,
};
use std::{
    io::{self, Cursor, Read},
    path::PathBuf,
};

/// A unique directory removed when the test ends.
struct Sandbox {
    root: PathBuf,
}
impl Sandbox {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("paper-inbox-files-{}", PaperId::new()));
        std::fs::create_dir_all(&root).unwrap();
        Self { root }
    }
    fn files(&self) -> PathBuf {
        self.root.join("library-files")
    }
    fn database(&self) -> PathBuf {
        self.root.join("library.sqlite")
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn now(n: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(1_000 + n, 0).unwrap()
}
fn paper(key: &str) -> Paper {
    Paper::new(
        PaperId::new(),
        CitationKey::parse(key).unwrap(),
        Bibliography {
            title: Some("Attachments".into()),
            ..Default::default()
        },
        now(0),
    )
    .unwrap()
}
fn create(store: &SqliteLibrary, p: &Paper) {
    store
        .commit_at(LibraryCommand::CreatePaper { paper: p.clone() }, now(0))
        .unwrap();
}
fn put(blobs: &FileBlobStore, bytes: &[u8]) -> ContentHash {
    blobs.put(&mut Cursor::new(bytes.to_vec())).unwrap()
}
fn attachment(paper_id: PaperId, hash: ContentHash, name: &str) -> Attachment {
    Attachment {
        id: AttachmentId::new(),
        paper_id,
        version_id: None,
        content_hash: hash,
        filename: name.into(),
        media_type: "application/pdf".into(),
        size_bytes: 8,
        kind: AttachmentKind::FullText,
        created_at: now(1),
    }
}
fn add(store: &SqliteLibrary, a: &Attachment) -> Result<(), StorageError> {
    store
        .commit_at(
            LibraryCommand::AddAttachment {
                attachment: a.clone(),
            },
            now(2),
        )
        .map(|_| ())
}
fn file_count(blobs: &FileBlobStore) -> usize {
    blobs.hashes().unwrap().len()
}

const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

#[test]
fn files_are_stored_by_sha256_deduplicated_and_verified_on_read() {
    let sandbox = Sandbox::new();
    let blobs = FileBlobStore::open(sandbox.files()).unwrap();
    let hash = put(&blobs, b"abc");
    assert_eq!(content_hash_hex(&hash), ABC_SHA256);
    assert_eq!(parse_content_hash(ABC_SHA256), Some(hash));
    assert_eq!(parse_content_hash("ABC"), None);
    assert_eq!(parse_content_hash(&ABC_SHA256.to_uppercase()), None);
    assert_eq!(content_hash_hex(&put(&blobs, b"")), EMPTY_SHA256);

    assert!(blobs.contains(&hash).unwrap());
    assert_eq!(put(&blobs, b"abc"), hash, "same content, same hash");
    assert_eq!(file_count(&blobs), 2, "no second copy of identical content");
    let mut out = vec![];
    blobs.read(&hash, &mut out).unwrap();
    assert_eq!(out, b"abc");

    let unknown = parse_content_hash(&"0".repeat(64)).unwrap();
    assert!(!blobs.contains(&unknown).unwrap());
    assert!(matches!(
        blobs.read(&unknown, &mut io::sink()),
        Err(BlobError::NotFound(_))
    ));
    assert!(!blobs.remove(&unknown).unwrap());
}

#[test]
fn a_damaged_file_is_detected_and_never_reported_as_valid() {
    let sandbox = Sandbox::new();
    let blobs = FileBlobStore::open(sandbox.files()).unwrap();
    let hash = put(&blobs, b"original content");
    assert!(blobs.verify(&hash).unwrap());
    let hex = content_hash_hex(&hash);
    let path = sandbox.files().join("blobs").join(&hex[..2]).join(&hex);
    std::fs::write(&path, b"tampered").unwrap();
    assert!(!blobs.verify(&hash).unwrap());
    assert!(matches!(
        blobs.read(&hash, &mut io::sink()),
        Err(BlobError::HashMismatch(_))
    ));
    assert!(blobs.remove(&hash).unwrap());
    assert!(!blobs.contains(&hash).unwrap());
}

struct FailingReader(usize);
impl Read for FailingReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.0 == 0 {
            return Err(io::Error::other("disk unplugged"));
        }
        self.0 -= 1;
        buffer.fill(7);
        Ok(buffer.len().min(100))
    }
}

#[test]
fn a_failed_or_interrupted_write_leaves_no_blob_and_no_temporary_file() {
    let sandbox = Sandbox::new();
    let blobs = FileBlobStore::open(sandbox.files()).unwrap();
    assert!(blobs.put(&mut FailingReader(3)).is_err());
    assert_eq!(file_count(&blobs), 0);
    let temporary = |dir: &PathBuf| std::fs::read_dir(dir.join("tmp")).unwrap().count();
    assert_eq!(temporary(&sandbox.files()), 0);

    // A crash can leave a partial file; reopening removes it and keeps complete blobs.
    let kept = put(&blobs, b"complete");
    std::fs::write(sandbox.files().join("tmp").join("crashed.part"), b"partial").unwrap();
    drop(blobs);
    let reopened = FileBlobStore::open(sandbox.files()).unwrap();
    assert_eq!(temporary(&sandbox.files()), 0);
    assert!(reopened.contains(&kept).unwrap());
}

#[test]
fn attachments_persist_with_events_and_are_listed_per_paper() {
    let sandbox = Sandbox::new();
    let blobs = FileBlobStore::open(sandbox.files()).unwrap();
    let store = SqliteLibrary::open_local(sandbox.database()).unwrap();
    let (p, other) = (paper("Alpha2026"), paper("Beta2026"));
    create(&store, &p);
    create(&store, &other);
    let hash = put(&blobs, b"%PDF-1.7 first");
    let first = attachment(p.id(), hash, "paper.pdf");
    add(&store, &first).unwrap();

    // The same file may belong to another paper: one stored file, two references.
    let shared = attachment(other.id(), hash, "copy.pdf");
    add(&store, &shared).unwrap();
    assert_eq!(file_count(&blobs), 1);

    let listed = store.attachments(p.id()).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, first.id);
    assert_eq!(listed[0].content_hash, hash);
    assert_eq!(listed[0].filename, "paper.pdf");
    assert_eq!(listed[0].kind, AttachmentKind::FullText);
    assert_eq!(store.attachments(other.id()).unwrap().len(), 1);

    let events = store.pending_events(100).unwrap();
    let added = events
        .iter()
        .filter(|e| matches!(&e.operation, SyncOperation::AttachmentAdded(a) if a.id == first.id))
        .count();
    assert_eq!(added, 1, "the addition is recorded for synchronization");

    drop(store);
    let reopened = SqliteLibrary::open_local(sandbox.database()).unwrap();
    assert_eq!(reopened.attachments(p.id()).unwrap().len(), 1);
}

#[test]
fn invalid_attachments_are_rejected_without_leaving_rows_or_events() {
    let sandbox = Sandbox::new();
    let blobs = FileBlobStore::open(sandbox.files()).unwrap();
    let store = SqliteLibrary::open_local(sandbox.database()).unwrap();
    let p = paper("Gamma2026");
    create(&store, &p);
    let hash = put(&blobs, b"%PDF-1.7 content");
    let events_before = store.pending_events(100).unwrap().len();
    let good = attachment(p.id(), hash, "ok.pdf");
    add(&store, &good).unwrap();

    for name in [
        "",
        "   ",
        "dir/file.pdf",
        "dir\\file.pdf",
        "line\nbreak.pdf",
    ] {
        assert!(
            matches!(
                add(&store, &attachment(p.id(), hash, name)),
                Err(StorageError::InvalidOperation(_))
            ),
            "file name {name:?}"
        );
    }
    let duplicate = attachment(p.id(), hash, "again.pdf");
    assert!(matches!(
        add(&store, &duplicate),
        Err(StorageError::InvalidOperation(_))
    ));
    let unknown_paper = attachment(PaperId::new(), hash, "x.pdf");
    assert!(matches!(
        add(&store, &unknown_paper),
        Err(StorageError::NotFound(_))
    ));
    let mut unknown_version = attachment(p.id(), put(&blobs, b"other"), "v.pdf");
    unknown_version.version_id = Some(VersionId::new());
    assert!(matches!(
        add(&store, &unknown_version),
        Err(StorageError::VersionNotFound(_))
    ));

    assert_eq!(store.attachments(p.id()).unwrap().len(), 1);
    assert_eq!(store.pending_events(100).unwrap().len(), events_before + 1);

    // A batch is all or nothing: the valid first command is rolled back with the invalid one.
    let fresh = attachment(p.id(), put(&blobs, b"fresh"), "fresh.pdf");
    let result = store.commit_batch_at(
        vec![
            LibraryCommand::AddAttachment {
                attachment: fresh.clone(),
            },
            LibraryCommand::AddAttachment {
                attachment: duplicate,
            },
        ],
        &[],
        now(3),
    );
    assert!(result.is_err());
    assert_eq!(store.attachments(p.id()).unwrap().len(), 1);

    store
        .commit_at(LibraryCommand::DeletePaper { paper_id: p.id() }, now(4))
        .unwrap();
    assert!(matches!(
        add(&store, &fresh),
        Err(StorageError::InvalidOperation(_))
    ));
}

#[test]
fn removing_keeps_the_file_until_it_is_reclaimed_explicitly() {
    let sandbox = Sandbox::new();
    let blobs = FileBlobStore::open(sandbox.files()).unwrap();
    let store = SqliteLibrary::open_local(sandbox.database()).unwrap();
    let (p, other) = (paper("Delta2026"), paper("Epsilon2026"));
    create(&store, &p);
    create(&store, &other);
    let shared = put(&blobs, b"%PDF-1.7 shared");
    let only_here = put(&blobs, b"%PDF-1.7 only here");
    let orphan = put(&blobs, b"%PDF-1.7 never attached");
    let a = attachment(p.id(), shared, "shared.pdf");
    let b = attachment(p.id(), only_here, "mine.pdf");
    let c = attachment(other.id(), shared, "shared-too.pdf");
    for x in [&a, &b, &c] {
        add(&store, x).unwrap();
    }

    let unused = |store: &SqliteLibrary| {
        let used = store.referenced_attachment_hashes().unwrap();
        blobs
            .hashes()
            .unwrap()
            .into_iter()
            .filter(|h| !used.contains(h))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        unused(&store),
        vec![orphan],
        "an unattached file is reclaimable"
    );

    store
        .commit_at(
            LibraryCommand::RemoveAttachment {
                attachment_id: a.id,
            },
            now(5),
        )
        .unwrap();
    store
        .commit_at(
            LibraryCommand::RemoveAttachment {
                attachment_id: b.id,
            },
            now(5),
        )
        .unwrap();
    assert!(store.attachments(p.id()).unwrap().is_empty());
    assert!(
        blobs.contains(&only_here).unwrap(),
        "removal never deletes a file"
    );
    let mut reclaimable = unused(&store);
    reclaimable.sort_by_key(|h| *h.as_bytes());
    let mut expected = vec![only_here, orphan];
    expected.sort_by_key(|h| *h.as_bytes());
    assert_eq!(
        reclaimable, expected,
        "the file still used by another paper stays"
    );

    assert!(matches!(
        store.commit_at(
            LibraryCommand::RemoveAttachment {
                attachment_id: a.id
            },
            now(6)
        ),
        Err(StorageError::InvalidOperation(_))
    ));
    let removed = store
        .pending_events(100)
        .unwrap()
        .iter()
        .filter(|e| matches!(e.operation, SyncOperation::AttachmentRemoved { attachment_id } if attachment_id == a.id))
        .count();
    assert_eq!(removed, 1);

    // The same file can be attached again after it was removed.
    add(&store, &attachment(p.id(), shared, "again.pdf")).unwrap();
    assert_eq!(store.attachments(p.id()).unwrap().len(), 1);
}

#[test]
fn search_reports_and_filters_by_attachment_presence() {
    let sandbox = Sandbox::new();
    let blobs = FileBlobStore::open(sandbox.files()).unwrap();
    let store = SqliteLibrary::open_local(sandbox.database()).unwrap();
    let (with, without) = (paper("With2026"), paper("Without2026"));
    create(&store, &with);
    create(&store, &without);
    let a = attachment(with.id(), put(&blobs, b"%PDF-1.7 x"), "x.pdf");
    add(&store, &a).unwrap();

    let search = |flag: Option<bool>| {
        store
            .search(&PaperQuery {
                filter: PaperFilter {
                    has_attachment: flag,
                    ..Default::default()
                },
                ..Default::default()
            })
            .unwrap()
            .papers
    };
    let flags = |papers: Vec<PaperSummary>| {
        let mut v = papers
            .into_iter()
            .map(|p| (p.citation_key.as_str().to_owned(), p.has_attachment))
            .collect::<Vec<_>>();
        v.sort();
        v
    };
    assert_eq!(
        flags(search(None)),
        vec![("With2026".into(), true), ("Without2026".into(), false)]
    );
    assert_eq!(flags(search(Some(true))), vec![("With2026".into(), true)]);
    assert_eq!(
        flags(search(Some(false))),
        vec![("Without2026".into(), false)]
    );
    store
        .commit_at(
            LibraryCommand::RemoveAttachment {
                attachment_id: a.id,
            },
            now(5),
        )
        .unwrap();
    assert!(search(Some(true)).is_empty());
}

#[test]
fn a_version_4_database_is_upgraded_in_place_and_newer_ones_are_refused() {
    let sandbox = Sandbox::new();
    let p = paper("Upgrade2026");
    {
        let store = SqliteLibrary::open_local(sandbox.database()).unwrap();
        create(&store, &p);
    }
    let raw = rusqlite::Connection::open(sandbox.database()).unwrap();
    raw.execute_batch(
        "ALTER TABLE events DROP COLUMN attachment_id; DROP TABLE attachments; PRAGMA user_version=4;",
    )
    .unwrap();
    drop(raw);

    let store = SqliteLibrary::open_local(sandbox.database()).unwrap();
    assert!(
        store.get(p.id()).unwrap().is_some(),
        "existing papers are kept"
    );
    assert!(store.attachments(p.id()).unwrap().is_empty());
    let version: u32 = rusqlite::Connection::open(sandbox.database())
        .unwrap()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 6);
    drop(store);

    rusqlite::Connection::open(sandbox.database())
        .unwrap()
        .pragma_update(None, "user_version", 7)
        .unwrap();
    assert!(SqliteLibrary::open_local(sandbox.database()).is_err());
}

#[test]
fn storing_the_same_content_again_replaces_a_damaged_stored_file() {
    let sandbox = Sandbox::new();
    let blobs = FileBlobStore::open(sandbox.files()).unwrap();
    let hash = put(&blobs, b"original content");
    let hex = content_hash_hex(&hash);
    let path = sandbox.files().join("blobs").join(&hex[..2]).join(&hex);
    std::fs::write(&path, b"tampered").unwrap();
    assert!(!blobs.verify(&hash).unwrap());
    // Choosing the intact file again repairs the store instead of reporting a stored copy.
    assert_eq!(put(&blobs, b"original content"), hash);
    assert!(
        blobs.verify(&hash).unwrap(),
        "the damaged copy was replaced"
    );
    assert_eq!(file_count(&blobs), 1);
    // An intact copy is left alone and no temporary file remains.
    assert_eq!(put(&blobs, b"original content"), hash);
    assert!(blobs.verify(&hash).unwrap());
    let tmp = std::fs::read_dir(sandbox.files().join("tmp"))
        .unwrap()
        .count();
    assert_eq!(tmp, 0, "no temporary file is left behind");
}

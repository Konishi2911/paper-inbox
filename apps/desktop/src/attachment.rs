//! PDF attachments for the desktop app: importing a file, listing, opening through a
//! temporary copy, and reclaiming files no attachment uses any more.
//!
//! Ordering rule (see `docs/design.md`): the file is stored first and the database row is
//! added afterwards, so a row never points at a missing file. A failure in between only
//! leaves an unused file, which the explicit cleanup removes.
use crate::application::AppError;
use chrono::Utc;
use paper_core::{
    command::LibraryCommand,
    model::{Attachment, AttachmentId, AttachmentKind, ContentHash, PaperId},
    ports::{BlobStore, PaperRepository},
};
use paper_storage::{BlobError, FileBlobStore, SqliteLibrary};
use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
};

/// Largest file accepted. Papers are a few MB; this only stops accidental huge files.
pub const MAX_PDF_BYTES: u64 = 512 * 1024 * 1024;
const PDF_MEDIA_TYPE: &str = "application/pdf";
/// The PDF header may be preceded by a little junk; readers accept it within 1024 bytes.
const SIGNATURE_WINDOW: usize = 1024;

/// What is known about a stored file without opening a viewer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileState {
    Present,
    Missing,
    /// The stored bytes no longer match the recorded hash.
    Damaged,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReclaimReport {
    pub files: usize,
    pub bytes: u64,
}

impl From<BlobError> for AppError {
    fn from(error: BlobError) -> Self {
        match error {
            BlobError::NotFound(_) => AppError::Input(
                "PDFファイルが見つかりません。ライブラリのフォルダーから移動・削除された可能性があります。"
                    .into(),
            ),
            BlobError::HashMismatch(_) => AppError::Storage(
                "PDFファイルの内容が記録と一致しません。ファイルが破損している可能性があります。".into(),
            ),
            BlobError::Io(e) => AppError::Storage(format!("ファイル操作に失敗しました: {e}")),
        }
    }
}

pub struct AttachmentFiles {
    blobs: FileBlobStore,
    /// Folder for temporary copies handed to the PDF viewer.
    open_dir: PathBuf,
    /// Serializes "store file, add row" against cleanup so a new file is never reclaimed.
    guard: Mutex<()>,
}

impl AttachmentFiles {
    /// Opens (creating if needed) the file store. Temporary copies older than a day are
    /// removed; newer ones may belong to another running instance or an open viewer.
    pub fn open(files_dir: &Path, open_dir: &Path) -> Result<Self, AppError> {
        Self::open_with_retention(files_dir, open_dir, STALE_AFTER)
    }

    /// Like [`Self::open`] with an explicit age after which temporary copies are deleted.
    pub fn open_with_retention(
        files_dir: &Path,
        open_dir: &Path,
        retention: std::time::Duration,
    ) -> Result<Self, AppError> {
        sweep_stale_copies(open_dir, retention);
        Ok(Self {
            blobs: FileBlobStore::open(files_dir)?,
            open_dir: open_dir.to_owned(),
            guard: Mutex::new(()),
        })
    }

    /// Imports a PDF chosen by the user and attaches it to the paper.
    pub fn attach(
        &self,
        store: &SqliteLibrary,
        paper_id: PaperId,
        source: &Path,
    ) -> Result<Attachment, AppError> {
        let metadata = fs::metadata(source).map_err(|_| {
            AppError::Input("ファイルを読み込めません。パスを確認してください。".into())
        })?;
        if !metadata.is_file() {
            return Err(AppError::Input(
                "フォルダーではなくファイルを選択してください。".into(),
            ));
        }
        if metadata.len() > MAX_PDF_BYTES {
            return Err(AppError::Input(
                "ファイルが大きすぎます（上限 512 MB）。".into(),
            ));
        }
        let mut file = fs::File::open(source)
            .map_err(|e| AppError::Input(format!("ファイルを開けません: {e}")))?;
        let mut head = vec![0u8; SIGNATURE_WINDOW];
        let read = read_up_to(&mut file, &mut head)?;
        if !head[..read].windows(5).any(|w| w == b"%PDF-") {
            return Err(AppError::Input("PDFファイルではありません。".into()));
        }
        drop(file);

        let _lock = self
            .guard
            .lock()
            .map_err(|_| AppError::Storage("内部状態が壊れました。".into()))?;
        let mut counted = Counting {
            inner: fs::File::open(source)
                .map_err(|e| AppError::Input(format!("ファイルを開けません: {e}")))?,
            bytes: 0,
        };
        let hash = self.blobs.put(&mut counted)?;
        if store
            .attachments(paper_id)?
            .iter()
            .any(|a| a.content_hash == hash)
        {
            return Err(AppError::Input(
                "同じPDFがこの文献にすでに添付されています。".into(),
            ));
        }
        let attachment = Attachment {
            id: AttachmentId::new(),
            paper_id,
            version_id: None,
            content_hash: hash,
            filename: stored_name(source),
            media_type: PDF_MEDIA_TYPE.into(),
            size_bytes: counted.bytes,
            kind: AttachmentKind::FullText,
            created_at: Utc::now(),
        };
        store.commit_at(
            LibraryCommand::AddAttachment {
                attachment: attachment.clone(),
            },
            Utc::now(),
        )?;
        Ok(attachment)
    }

    pub fn list(
        &self,
        store: &SqliteLibrary,
        paper_id: PaperId,
    ) -> Result<Vec<Attachment>, AppError> {
        Ok(store.attachments(paper_id)?)
    }

    /// Removes the attachment from the paper. The file stays until [`Self::reclaim`].
    pub fn detach(
        &self,
        store: &SqliteLibrary,
        attachment_id: AttachmentId,
    ) -> Result<(), AppError> {
        store.commit_at(
            LibraryCommand::RemoveAttachment { attachment_id },
            Utc::now(),
        )?;
        Ok(())
    }

    /// Cheap presence test for list rows; it does not read the file.
    pub fn is_present(&self, attachment: &Attachment) -> Result<bool, AppError> {
        Ok(self.blobs.contains(&attachment.content_hash)?)
    }

    /// Checks the stored file; re-reads it fully, so use it on demand rather than per row.
    pub fn check(&self, attachment: &Attachment) -> Result<FileState, AppError> {
        if !self.blobs.contains(&attachment.content_hash)? {
            return Ok(FileState::Missing);
        }
        Ok(if self.blobs.verify(&attachment.content_hash)? {
            FileState::Present
        } else {
            FileState::Damaged
        })
    }

    /// Copies the file under its original name into the temporary folder and returns the
    /// path for the viewer. The hash is verified while copying; a damaged copy is deleted.
    pub fn prepare_open(&self, attachment: &Attachment) -> Result<PathBuf, AppError> {
        let directory = self.open_dir.join(AttachmentId::new().to_string());
        fs::create_dir_all(&directory)
            .map_err(|e| AppError::Storage(format!("一時フォルダーを作成できません: {e}")))?;
        let path = directory.join(viewer_name(&attachment.filename));
        let result = fs::File::create_new(&path)
            .map_err(|e| AppError::Storage(format!("一時ファイルを作成できません: {e}")))
            .and_then(|mut out| {
                self.blobs
                    .read(&attachment.content_hash, &mut out)
                    .map_err(AppError::from)
            });
        if let Err(error) = result {
            let _ = fs::remove_dir_all(&directory);
            return Err(error);
        }
        Ok(path)
    }

    /// Hands the path to the operating system's default application (no shell involved).
    pub fn open_with_system(path: &Path) -> Result<(), AppError> {
        let program = if cfg!(target_os = "windows") {
            "explorer.exe"
        } else if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        Command::new(program)
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|e| AppError::Storage(format!("PDFビューアーを起動できません: {e}")))
    }

    /// Stored files that no attachment refers to, with their sizes.
    pub fn unused(&self, store: &SqliteLibrary) -> Result<Vec<(ContentHash, u64)>, AppError> {
        let used = store.referenced_attachment_hashes()?;
        let mut unused = vec![];
        for hash in self.blobs.hashes()? {
            if !used.contains(&hash) {
                unused.push((hash, self.blobs.size(&hash)?));
            }
        }
        Ok(unused)
    }

    /// Deletes every unused file. The set is recomputed under the same lock that guards
    /// importing, so a file being attached right now is never removed.
    pub fn reclaim(&self, store: &SqliteLibrary) -> Result<ReclaimReport, AppError> {
        let _lock = self
            .guard
            .lock()
            .map_err(|_| AppError::Storage("内部状態が壊れました。".into()))?;
        let mut report = ReclaimReport::default();
        for (hash, bytes) in self.unused(store)? {
            if self.blobs.remove(&hash)? {
                report.files += 1;
                report.bytes += bytes;
            }
        }
        Ok(report)
    }
}

/// Copies handed to a viewer are kept for a day so that another running instance or a
/// viewer that is still open never loses its file; older ones are deleted.
const STALE_AFTER: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

fn sweep_stale_copies(open_dir: &Path, retention: std::time::Duration) {
    let Ok(entries) = fs::read_dir(open_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age > retention);
        if old {
            let _ = fs::remove_dir_all(entry.path());
        }
    }
}

struct Counting<R> {
    inner: R,
    bytes: u64,
}
impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buffer)?;
        self.bytes += read as u64;
        Ok(read)
    }
}

fn read_up_to(reader: &mut impl Read, buffer: &mut [u8]) -> Result<usize, AppError> {
    let mut total = 0;
    while total < buffer.len() {
        match reader.read(&mut buffer[total..]) {
            Ok(0) => break,
            Ok(n) => total += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(AppError::Input(format!("ファイルを読み込めません: {e}"))),
        }
    }
    Ok(total)
}

/// The name kept in the database: the source file name without control characters or
/// path separators, never empty.
fn stored_name(source: &Path) -> String {
    let name: String = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
        .chars()
        .map(|c| {
            if c.is_control() || c == '/' || c == '\\' {
                '_'
            } else {
                c
            }
        })
        .take(255)
        .collect();
    if name.trim().is_empty() {
        "document.pdf".into()
    } else {
        name
    }
}

/// A file name safe on every platform for the temporary copy, ending in `.pdf`.
fn viewer_name(name: &str) -> String {
    let mut safe: String = name
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .take(150)
        .collect();
    safe = safe.trim_matches([' ', '.']).to_owned();
    let stem = safe.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.ends_with(|c: char| c.is_ascii_digit()));
    if safe.is_empty() || reserved {
        safe = format!("document-{safe}");
    }
    if !safe.to_ascii_lowercase().ends_with(".pdf") {
        safe.push_str(".pdf");
    }
    safe
}

#[cfg(test)]
mod tests {
    use super::{stored_name, viewer_name};
    use std::path::Path;

    #[test]
    fn names_are_made_safe_without_losing_the_readable_part() {
        assert_eq!(viewer_name("論文 (final).pdf"), "論文 (final).pdf");
        assert_eq!(viewer_name("a:b*c?.PDF"), "a_b_c_.PDF");
        assert_eq!(viewer_name("notes"), "notes.pdf");
        assert_eq!(viewer_name("CON.pdf"), "document-CON.pdf");
        assert_eq!(viewer_name("..."), "document-.pdf");
        assert_eq!(viewer_name(&"x".repeat(400)).chars().count(), 154);
        assert_eq!(stored_name(Path::new("dir/a\u{7}b.pdf")), "a_b.pdf");
        assert_eq!(stored_name(Path::new("")), "document.pdf");
    }
}

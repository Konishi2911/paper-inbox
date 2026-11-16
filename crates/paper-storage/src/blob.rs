//! Content-addressed file store for attachments.
//!
//! Layout: `<root>/blobs/<first two hex digits>/<64 hex digits>` for stored files and
//! `<root>/tmp/*.part` for files being written. A file is first written to `tmp`, flushed to
//! disk and only then renamed into `blobs`, so a crash leaves either a complete blob or a
//! leftover `.part` file, never a half-written blob under its final name.
use paper_core::{model::AttachmentId, model::ContentHash, ports::BlobStore};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, thiserror::Error)]
pub enum BlobError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("Stored file does not match its content hash: {0}")]
    HashMismatch(String),
    #[error("Stored file not found: {0}")]
    NotFound(String),
}

/// Lowercase 64-digit hexadecimal form used for file names and the database.
pub fn content_hash_hex(hash: &ContentHash) -> String {
    hash.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

/// Parses the form written by [`content_hash_hex`]; anything else is rejected.
pub fn parse_content_hash(text: &str) -> Option<ContentHash> {
    if text.len() != 64 || !text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(ContentHash::from_bytes(bytes))
}

/// Removes the temporary file unless the write completed.
struct TempFile {
    path: PathBuf,
    keep: bool,
}
impl Drop for TempFile {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_file(&self.path);
        }
    }
}

pub struct FileBlobStore {
    root: PathBuf,
}
impl FileBlobStore {
    /// Creates the directories and deletes `.part` files left by an interrupted write.
    /// Call it before any write starts; the application opens the store once at startup.
    pub fn open(root: impl AsRef<Path>) -> Result<Self, BlobError> {
        let store = Self {
            root: root.as_ref().to_owned(),
        };
        fs::create_dir_all(store.blob_dir())?;
        fs::create_dir_all(store.tmp_dir())?;
        store.clean_temporary()?;
        Ok(store)
    }
    fn blob_dir(&self) -> PathBuf {
        self.root.join("blobs")
    }
    fn tmp_dir(&self) -> PathBuf {
        self.root.join("tmp")
    }
    fn path_of(&self, hash: &ContentHash) -> PathBuf {
        let hex = content_hash_hex(hash);
        self.blob_dir().join(&hex[..2]).join(hex)
    }
    fn clean_temporary(&self) -> Result<usize, BlobError> {
        let mut removed = 0;
        for entry in fs::read_dir(self.tmp_dir())? {
            let path = entry?.path();
            if path.extension().is_some_and(|e| e == "part") && fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
        Ok(removed)
    }
    /// Re-reads the stored file and compares it with its name.
    pub fn verify(&self, hash: &ContentHash) -> Result<bool, BlobError> {
        match self.read(hash, &mut io::sink()) {
            Ok(()) => Ok(true),
            Err(BlobError::HashMismatch(_)) => Ok(false),
            Err(error) => Err(error),
        }
    }
    /// Hashes of every stored file, found from the file names only.
    pub fn hashes(&self) -> Result<Vec<ContentHash>, BlobError> {
        let mut found = vec![];
        for directory in fs::read_dir(self.blob_dir())? {
            let directory = directory?.path();
            if !directory.is_dir() {
                continue;
            }
            for file in fs::read_dir(directory)? {
                let file = file?.path();
                if let Some(hash) = file
                    .file_name()
                    .and_then(|n| n.to_str())
                    .and_then(parse_content_hash)
                    && file.is_file()
                {
                    found.push(hash);
                }
            }
        }
        found.sort_by_key(|h| *h.as_bytes());
        Ok(found)
    }
    /// Deletes one stored file. Returns false when it was not there.
    pub fn remove(&self, hash: &ContentHash) -> Result<bool, BlobError> {
        match fs::remove_file(self.path_of(hash)) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }
}
impl BlobStore for FileBlobStore {
    type Error = BlobError;
    fn put(&self, reader: &mut dyn Read) -> Result<ContentHash, BlobError> {
        let mut temp = TempFile {
            path: self.tmp_dir().join(format!("{}.part", AttachmentId::new())),
            keep: false,
        };
        let mut file = fs::File::create_new(&temp.path)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 64 * 1024];
        loop {
            let read = match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
            };
            hasher.update(&buffer[..read]);
            file.write_all(&buffer[..read])?;
        }
        file.sync_all()?;
        drop(file);
        let hash = ContentHash::from_bytes(hasher.finalize().into());
        let destination = self.path_of(&hash);
        if destination.is_file() {
            return Ok(hash);
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        match fs::rename(&temp.path, &destination) {
            Ok(()) => temp.keep = true,
            // A concurrent writer of the same content may have won the rename.
            Err(_) if destination.is_file() => {}
            Err(e) => return Err(e.into()),
        }
        Ok(hash)
    }
    fn read(&self, hash: &ContentHash, writer: &mut dyn Write) -> Result<(), BlobError> {
        let name = content_hash_hex(hash);
        let mut file = match fs::File::open(self.path_of(hash)) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(BlobError::NotFound(name)),
            Err(e) => return Err(e.into()),
        };
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 64 * 1024];
        loop {
            let read = match file.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.into()),
            };
            hasher.update(&buffer[..read]);
            writer.write_all(&buffer[..read])?;
        }
        if hasher.finalize().as_slice() != hash.as_bytes() {
            return Err(BlobError::HashMismatch(name));
        }
        Ok(())
    }
    fn contains(&self, hash: &ContentHash) -> Result<bool, BlobError> {
        Ok(self.path_of(hash).is_file())
    }
}

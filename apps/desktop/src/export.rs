use crate::application::Application;
use paper_core::{
    model::PaperId,
    service::{BibtexError, BibtexOptions, BibtexOutput, export_bibtex},
};
use std::{
    collections::{HashSet, hash_map::DefaultHasher},
    fs::{self, File, Metadata, OpenOptions},
    hash::Hasher,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("出力する文献を選択してください。")]
    EmptySelection,
    #[error("出力先は拡張子 .bib のファイルで指定してください。")]
    InvalidPath,
    #[error("ライブラリ本体へは出力できません。別のファイルを指定してください。")]
    LibraryTarget,
    #[error("出力先には通常のファイルを指定してください。リンクやフォルダーは置き換えません。")]
    NotRegular,
    #[error("出力先が確認時から変更されています。もう一度出力を準備し、内容を確認してください。")]
    TargetChanged,
    #[error(transparent)]
    Storage(#[from] paper_storage::StorageError),
    #[error(transparent)]
    Bibtex(#[from] BibtexError),
    #[error("ファイルを出力できません: {0}")]
    Io(#[from] io::Error),
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum TargetState {
    Missing,
    Existing(Stamp),
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stamp {
    len: u64,
    modified: Option<SystemTime>,
    digest: u64,
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    ino: u64,
}
pub struct PreparedExport {
    pub path: PathBuf,
    pub output: BibtexOutput,
    pub count: usize,
    pub labels: Vec<(PaperId, String)>,
    target: TargetState,
}
impl PreparedExport {
    pub fn replaces_file(&self) -> bool {
        matches!(self.target, TargetState::Existing(_))
    }
    pub fn publish(&self) -> Result<(), ExportError> {
        self.publish_with(|f| f.write_all(self.output.content.as_bytes()))
    }
    fn publish_with(
        &self,
        write: impl FnOnce(&mut File) -> io::Result<()>,
    ) -> Result<(), ExportError> {
        let parent = self
            .path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let path = parent.join(format!(".paper-inbox-export-{}.tmp", PaperId::new()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path)?;
        let _cleanup = TempFile(path.clone());
        write(&mut file)?;
        file.sync_all()?;
        drop(file);
        if target_state(&self.path)? != self.target {
            return Err(ExportError::TargetChanged);
        }
        match self.target {
            TargetState::Missing => {
                fs::hard_link(&path, &self.path).map_err(|e| {
                    if e.kind() == io::ErrorKind::AlreadyExists {
                        ExportError::TargetChanged
                    } else {
                        ExportError::Io(e)
                    }
                })?;
            }
            TargetState::Existing(_) => fs::rename(&path, &self.path)?,
        }
        Ok(())
    }
}
struct TempFile(PathBuf);
impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
fn same_metadata(a: &Metadata, b: &Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if a.dev() != b.dev() || a.ino() != b.ino() {
            return false;
        }
    }
    a.len() == b.len() && a.modified().ok() == b.modified().ok()
}
fn target_state(path: &Path) -> Result<TargetState, ExportError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(TargetState::Missing),
        Err(e) => return Err(e.into()),
    };
    if !metadata.file_type().is_file() {
        return Err(ExportError::NotRegular);
    }
    let mut file = File::open(path)?;
    let opened = file.metadata()?;
    if !same_metadata(&metadata, &opened) {
        return Err(ExportError::TargetChanged);
    }
    let mut hasher = DefaultHasher::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.write(&buffer[..n]);
    }
    if !same_metadata(&opened, &file.metadata()?)
        || !same_metadata(&opened, &fs::symlink_metadata(path)?)
    {
        return Err(ExportError::TargetChanged);
    }
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    Ok(TargetState::Existing(Stamp {
        len: opened.len(),
        modified: opened.modified().ok(),
        digest: hasher.finish(),
        #[cfg(unix)]
        dev: opened.dev(),
        #[cfg(unix)]
        ino: opened.ino(),
    }))
}
pub fn prepare(
    app: &Application,
    ids: &[PaperId],
    options: &BibtexOptions,
    path: &Path,
    library: &Path,
) -> Result<PreparedExport, ExportError> {
    if ids.is_empty() {
        return Err(ExportError::EmptySelection);
    }
    if path.file_name().is_none()
        || !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("bib"))
    {
        return Err(ExportError::InvalidPath);
    }
    if let (Ok(target), Ok(db)) = (fs::metadata(path), fs::metadata(library)) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if target.dev() == db.dev() && target.ino() == db.ino() {
                return Err(ExportError::LibraryTarget);
            }
        }
        if let (Ok(target), Ok(source)) = (fs::canonicalize(path), fs::canonicalize(library)) {
            #[cfg(windows)]
            let same = target
                .to_string_lossy()
                .eq_ignore_ascii_case(&source.to_string_lossy());
            #[cfg(not(windows))]
            let same = target == source;
            if same {
                return Err(ExportError::LibraryTarget);
            }
        }
        let _ = (target, db);
    }
    let mut seen = HashSet::new();
    let ids: Vec<_> = ids.iter().copied().filter(|id| seen.insert(*id)).collect();
    let papers = app.store.get_many(&ids)?;
    let output = export_bibtex(&papers, options)?;
    let target = target_state(path)?;
    Ok(PreparedExport {
        path: path.into(),
        output,
        count: papers.len(),
        labels: papers
            .iter()
            .map(|p| (p.id(), p.citation_key().as_str().into()))
            .collect(),
        target,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(path: &Path) -> PreparedExport {
        PreparedExport {
            path: path.into(),
            output: BibtexOutput {
                content: "new output".into(),
                warnings: vec![],
            },
            count: 1,
            labels: vec![],
            target: target_state(path).unwrap(),
        }
    }
    #[test]
    fn late_failure_does_not_replace_existing_file_or_leave_temporary_files() {
        let dir = std::env::temp_dir().join(format!("paper-publish-{}", PaperId::new()));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("export.bib");
        fs::write(&path, b"original").unwrap();
        let prepared = fixture(&path);
        assert!(
            prepared
                .publish_with(|f| {
                    f.write_all(b"partial")?;
                    Err(io::Error::other("injected failure"))
                })
                .is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), b"original");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        prepared.publish().unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new output");
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn changed_or_newly_created_target_is_not_clobbered() {
        let dir = std::env::temp_dir().join(format!("paper-publish-race-{}", PaperId::new()));
        fs::create_dir(&dir).unwrap();
        let path = dir.join("export.bib");
        let prepared = fixture(&path);
        fs::write(&path, "other writer").unwrap();
        assert!(matches!(
            prepared.publish(),
            Err(ExportError::TargetChanged)
        ));
        let prepared = fixture(&path);
        fs::write(&path, "changed again").unwrap();
        assert!(matches!(
            prepared.publish(),
            Err(ExportError::TargetChanged)
        ));
        assert_eq!(fs::read_to_string(&path).unwrap(), "changed again");
        fs::remove_dir_all(dir).unwrap();
    }
}

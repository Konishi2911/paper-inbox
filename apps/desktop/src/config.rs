use std::{ffi::OsString, path::PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub struct Config {
    pub database: PathBuf,
}
impl Config {
    pub fn from_args(
        args: impl IntoIterator<Item = OsString>,
        env: impl Fn(&str) -> Option<OsString>,
    ) -> Result<Self, String> {
        let mut args = args.into_iter();
        let mut path = None;
        while let Some(arg) = args.next() {
            if arg != "--database" || path.is_some() {
                return Err("使い方: paper-desktop [--database 保存先.sqlite]".into());
            }
            let value = args
                .next()
                .filter(|s| !s.is_empty())
                .ok_or("--database の後に保存先を指定してください。")?;
            path = Some(PathBuf::from(value));
        }
        let database = match path {
            Some(p) => p,
            None => default_database(&env)?,
        };
        if database.as_os_str().is_empty() {
            return Err("保存先が空です。".into());
        }
        Ok(Self { database })
    }
    /// Folder of the attachment files: `library-files` next to the database file.
    pub fn files_dir(&self) -> PathBuf {
        self.database
            .parent()
            .unwrap_or(std::path::Path::new(""))
            .join("library-files")
    }
    pub fn prepare(&self) -> Result<(), String> {
        if let Some(parent) = self.database.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "保存先フォルダーを作成できません: {}: {e}",
                    parent.display()
                )
            })?;
        }
        Ok(())
    }
}
fn default_database(env: &impl Fn(&str) -> Option<OsString>) -> Result<PathBuf, String> {
    let directory = if cfg!(target_os = "windows") {
        env("LOCALAPPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|s| PathBuf::from(s).join("Library/Application Support"))
    } else {
        env("XDG_DATA_HOME")
            .filter(|s| PathBuf::from(s).is_absolute())
            .map(PathBuf::from)
            .or_else(|| env("HOME").map(|s| PathBuf::from(s).join(".local/share")))
    }
    .filter(|p| p.is_absolute())
    .ok_or("標準の保存先が見つかりません。--database で指定してください。")?;
    Ok(directory.join("paper-inbox/library.sqlite"))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn override_and_bad_arguments() {
        assert_eq!(
            Config::from_args(["--database".into(), "custom.sqlite".into()], |_| None)
                .unwrap()
                .database,
            PathBuf::from("custom.sqlite")
        );
        assert!(Config::from_args(["--database".into()], |_| None).is_err());
        assert!(Config::from_args(["--unknown".into()], |_| None).is_err());
        assert!(Config::from_args(["--database".into(), "".into()], |_| None).is_err());
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn xdg_path_and_relative_fallback() {
        assert_eq!(
            Config::from_args([], |k| match k {
                "XDG_DATA_HOME" => Some("/example/data".into()),
                _ => None,
            })
            .unwrap()
            .database,
            PathBuf::from("/example/data/paper-inbox/library.sqlite")
        );
        assert_eq!(
            Config::from_args([], |k| match k {
                "XDG_DATA_HOME" => Some("relative".into()),
                "HOME" => Some("/example/home".into()),
                _ => None,
            })
            .unwrap()
            .database,
            PathBuf::from("/example/home/.local/share/paper-inbox/library.sqlite")
        );
        assert!(Config::from_args([], |_| None).is_err());
    }
}

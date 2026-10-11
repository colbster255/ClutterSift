use crate::model::{now, Settings};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

/// Component-aware Windows comparison. Never use string-prefix checks for containment.
pub fn normalized(path: &str) -> String {
    path.trim_start_matches(r"\\?\")
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}
pub fn within(path: &str, parent: &str) -> bool {
    let p = normalized(path);
    let root = normalized(parent);
    !root.is_empty()
        && (p == root
            || p.strip_prefix(&root)
                .is_some_and(|rest| rest.starts_with('\\')))
}
pub fn is_reparse(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}
pub fn checked_path(path: &Path) -> Result<PathBuf, String> {
    // canonicalize emits an extended-length prefix on Windows; accept only local drive form.
    let value = path
        .to_str()
        .ok_or("Paths with non-Unicode names are unsupported.")?;
    let local = value.strip_prefix(r"\\?\");
    if let Some(local) = local {
        if local.as_bytes().get(1) != Some(&b':')
            || !local
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphabetic)
        {
            return Err("Only local drive paths are supported.".into());
        }
    }
    let path = local.map(Path::new).unwrap_or(path);
    if !path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err("Use an absolute path without parent traversal.".into());
    }
    let text = path.to_string_lossy();
    if text.starts_with(r"\\")
        || text.contains(':') && text.get(2..).is_some_and(|s| s.contains(':'))
    {
        return Err("Network, device, and alternate-stream paths are unsupported.".into());
    }
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetDriveTypeW(root: *const u16) -> u32;
        }
        let letter = *text.as_bytes().first().ok_or("Missing drive letter.")?;
        if !letter.is_ascii_alphabetic() || text.as_bytes().get(1) != Some(&b':') {
            return Err("Choose an absolute local drive path.".into());
        }
        let root = [letter as u16, b':' as u16, b'\\' as u16, 0];
        // SAFETY: root is a live, NUL-terminated UTF-16 array; GetDriveTypeW only reads it.
        let kind = unsafe { GetDriveTypeW(root.as_ptr()) };
        if !matches!(kind, 2 | 3 | 5 | 6) {
            return Err("Network, unavailable, and unknown drives are unsupported.".into());
        }
    }
    for ancestor in path.ancestors() {
        let meta =
            fs::symlink_metadata(ancestor).map_err(|e| format!("Cannot validate path: {e}"))?;
        if is_reparse(&meta) {
            return Err("Symbolic links and reparse points are excluded.".into());
        }
    }
    fs::canonicalize(path).map_err(|e| e.to_string())
}
pub struct Policy {
    pub settings: Settings,
    hard: Vec<String>,
    personal: Vec<String>,
    temp: PathBuf,
    patterns: GlobSet,
}
impl Policy {
    #[cfg(test)]
    pub fn fixture(settings: Settings, temp: PathBuf) -> Self {
        let mut policy = Self::new(settings).unwrap();
        policy.temp = temp;
        policy
    }
    pub fn new(settings: Settings) -> Result<Self, String> {
        let mut builder = GlobSetBuilder::new();
        for pattern in &settings.patterns {
            let glob = GlobBuilder::new(&pattern.replace('\\', "/"))
                .case_insensitive(true)
                .build()
                .map_err(|e| e.to_string())?;
            builder.add(glob);
        }
        let mut hard: Vec<String> = [
            "SystemRoot",
            "ProgramFiles",
            "ProgramFiles(x86)",
            "ProgramData",
        ]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .collect();
        hard.extend(
            [
                r"C:\Windows",
                r"C:\Program Files",
                r"C:\Program Files (x86)",
                r"C:\ProgramData",
            ]
            .map(str::to_owned),
        );
        let mut personal = vec![];
        if let Ok(home) = std::env::var("USERPROFILE") {
            personal.extend(
                [
                    "Documents",
                    "Desktop",
                    "Pictures",
                    "Videos",
                    "Music",
                    "Saved Games",
                    ".ssh",
                    ".aws",
                    ".azure",
                    ".config",
                    "OneDrive",
                ]
                .map(|p| Path::new(&home).join(p).to_string_lossy().into_owned()),
            );
        }
        Ok(Self {
            settings,
            hard,
            personal,
            temp: std::env::temp_dir(),
            patterns: builder.build().map_err(|e| e.to_string())?,
        })
    }
    pub fn excluded(&self, path: &Path) -> bool {
        self.settings
            .exclusions
            .iter()
            .any(|p| within(&path.to_string_lossy(), p))
    }
    pub fn protected(&self, path: &Path) -> bool {
        let value = path.to_string_lossy();
        self.hard
            .iter()
            .chain(self.personal.iter())
            .chain(self.settings.protected.iter())
            .any(|p| within(&value, p))
            || self.patterns.is_match(value.replace('\\', "/"))
            || path.file_name().is_some_and(|n| self.patterns.is_match(n))
            || path.components().any(|c| {
                matches!(
                    c.as_os_str().to_string_lossy().to_lowercase().as_str(),
                    ".git"
                        | "node_modules"
                        | ".venv"
                        | "steamapps"
                        | "windowsapps"
                        | "$recycle.bin"
                        | "system volume information"
                        | "appdata"
                )
            }) && !within(&value, &self.temp.to_string_lossy())
            || path.ancestors().any(|p| {
                p.join(".git").exists()
                    || p.join("Cargo.toml").exists()
                    || p.join("package.json").exists()
                    || p.join("pyproject.toml").exists()
                    || p.join(".sln").exists()
            })
    }
    pub fn classify(&self, path: &Path, bytes: u64, modified: u64) -> (String, bool, String) {
        if self.protected(path) {
            return (
                "Protected".into(),
                false,
                "Protected by system, personal-data, application, project, or saved rules.".into(),
            );
        }
        let ext = path
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        let age = now().saturating_sub(modified);
        // Only direct regular files in the OS temp folder are preview-eligible.
        if path.parent().is_some_and(|p| {
            normalized(&p.to_string_lossy()) == normalized(&self.temp.to_string_lossy())
        }) && ext == "tmp"
            && age > 7 * 86400
        {
            return ("Temporary".into(), true, "A .tmp file directly in the OS temporary folder, untouched for over 7 days. Review before removal.".into());
        }
        let category = if ["msi", "exe", "msix", "iso"].contains(&ext.as_str()) {
            "Installers"
        } else if ["zip", "7z", "rar", "tar", "gz"].contains(&ext.as_str()) {
            "Archives"
        } else if ext == "log" {
            "Logs"
        } else if ["rs", "ts", "tsx", "js", "py", "cpp", "cs", "sln"].contains(&ext.as_str()) {
            "Development"
        } else if within(
            &path.to_string_lossy(),
            &std::env::var("USERPROFILE")
                .map(|h| format!("{h}\\Downloads"))
                .unwrap_or_default(),
        ) && age > 90 * 86400
        {
            "Old downloads"
        } else if bytes >= 100 * 1024 * 1024 {
            "Large files"
        } else {
            "Other"
        };
        (
            category.into(),
            false,
            "Analysis only. Age, size, and duplication do not establish that a file is disposable."
                .into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_local_paths_can_be_revalidated() {
        let dir = tempfile::tempdir().unwrap();
        let canonical = fs::canonicalize(dir.path()).unwrap();
        assert_eq!(checked_path(&canonical).unwrap(), canonical);
    }
    #[test]
    fn temporary_preview_policy_is_narrow() {
        let dir = tempfile::tempdir().unwrap();
        let policy = Policy::fixture(Settings::default(), dir.path().to_owned());
        let old = now() - 8 * 86400;
        assert!(policy.classify(&dir.path().join("old.tmp"), 1, old).1);
        assert!(!policy.classify(&dir.path().join("recent.tmp"), 1, now()).1);
        assert!(!policy.classify(&dir.path().join("old.log"), 1, old).1);
        assert!(
            !policy
                .classify(&dir.path().join("nested").join("old.tmp"), 1, old)
                .1
        );
    }
    #[test]
    fn component_boundaries_and_case() {
        assert!(within(r"c:/WINDOWS/System32", r"C:\Windows"));
        assert!(!within(r"C:\WindowsBackup\x", r"C:\Windows"));
        assert!(!within("x", ""));
    }
    #[test]
    fn hard_protection() {
        let p = Policy::new(Settings::default()).unwrap();
        assert!(p.protected(Path::new(r"C:\Windows\System32\x.tmp")));
        assert!(p.protected(Path::new(r"C:\Program Files\Example\x.tmp")));
    }
    #[test]
    fn patterns_and_saved_protection() {
        let s = Settings {
            protected: vec![r"D:\Development".into()],
            patterns: vec!["*.save".into()],
            ..Default::default()
        };
        let p = Policy::new(s).unwrap();
        assert!(p.protected(Path::new(r"d:\development\x")));
        assert!(p.protected(Path::new("game.save")));
    }
    #[test]
    fn no_age_based_deletion() {
        let p = Policy::new(Settings::default()).unwrap();
        assert!(
            !p.classify(Path::new(r"D:\Downloads\old.zip"), u64::MAX, 0)
                .1
        );
    }
    #[test]
    fn project_protection() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), "").unwrap();
        let p = Policy::new(Settings::default()).unwrap();
        assert!(p.protected(&dir.path().join("a.tmp")));
    }
    #[test]
    fn traversal_and_missing_paths_rejected() {
        assert!(checked_path(Path::new("../x")).is_err());
        assert!(checked_path(Path::new(r"\\server\share")).is_err());
    }
    #[test]
    fn malformed_glob_rejected() {
        assert!(Policy::new(Settings {
            patterns: vec!["[".into()],
            ..Default::default()
        })
        .is_err());
    }
}

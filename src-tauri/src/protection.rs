use crate::{model::Settings, policy::checked_path};
use std::path::PathBuf;
/// Deliberately local: no goal or path is sent to a model to propose protection.
pub fn propose(settings: &Settings, goal: &str) -> Result<Settings, String> {
    if goal.len() > 4000 {
        return Err("Use at most 4,000 characters.".into());
    }
    let lower = goal.to_lowercase();
    if !["keep", "protect", "preserve"]
        .iter()
        .any(|word| lower.contains(word))
    {
        return Err(
            "Describe what to keep, protect, or preserve. Quote paths containing spaces.".into(),
        );
    }
    let mut paths: Vec<PathBuf> = vec![];
    for (index, part) in goal.split('"').enumerate() {
        if index % 2 == 1 {
            paths.push(PathBuf::from(part));
        } else {
            paths.extend(part.split_whitespace().filter_map(|token| {
                let token = token.trim_matches(|c: char| matches!(c, ',' | ';' | '\''));
                if token.as_bytes().get(1) == Some(&b':') {
                    Some(PathBuf::from(token))
                } else {
                    None
                }
            }));
        }
    }
    if let Ok(home) = std::env::var("USERPROFILE") {
        for (name, folder) in [
            ("documents", "Documents"),
            ("desktop", "Desktop"),
            ("pictures", "Pictures"),
            ("saved games", "Saved Games"),
        ] {
            if lower.contains(name) {
                let path = PathBuf::from(&home).join(folder);
                if path.exists() {
                    paths.push(path);
                }
            }
        }
    }
    for (name, base, folder) in [
        ("steam", "ProgramFiles(x86)", "Steam"),
        ("visual studio", "ProgramFiles", "Microsoft Visual Studio"),
    ] {
        if lower.contains(name) {
            if let Ok(base) = std::env::var(base) {
                let path = PathBuf::from(base).join(folder);
                if path.exists() {
                    paths.push(path);
                }
            }
        }
    }
    let mut result = settings.clone();
    let original = result.protected.len();
    for path in paths {
        let canonical = checked_path(&path)?;
        let value = canonical
            .to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_string();
        if !result
            .protected
            .iter()
            .any(|p| crate::policy::normalized(p) == crate::policy::normalized(&value))
        {
            result.protected.push(value);
        }
    }
    if result.protected.len() == original {
        return Err("No new existing paths recognized. Use quoted absolute paths, or add rules in Protected items. Existing protections are unchanged.".into());
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quoted_existing_path_is_proposed_without_saving() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings::default();
        let goal = format!("Keep everything in \"{}\"", dir.path().display());
        let proposal = propose(&settings, &goal).unwrap();
        assert_eq!(proposal.protected.len(), 1);
        assert!(settings.protected.is_empty());
        assert!(propose(&proposal, &goal).is_err());
    }
    #[test]
    fn commands_and_nonexistent_paths_do_not_become_rules() {
        assert!(propose(&Settings::default(), "delete everything").is_err());
        assert!(propose(
            &Settings::default(),
            "Keep \"C:\\nonexistent-cluttersift-fixture\\file\""
        )
        .is_err());
        assert!(propose(&Settings::default(), "Keep \"..\\Windows\"").is_err());
    }
}

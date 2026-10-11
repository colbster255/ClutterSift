use crate::{
    file_ops::{pin_directory, Identity, LockedFile},
    model::*,
    policy::{self, Policy},
    store,
};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Serialize, Deserialize)]
pub struct Approval {
    pub items: Vec<(Item, Identity)>,
}
#[cfg(test)]
pub fn prepare(db: &Connection, preview: &Preview) -> Result<(), String> {
    prepare_cancellable(db, preview, &AtomicBool::new(false))
}
pub fn prepare_cancellable(
    db: &Connection,
    preview: &Preview,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let mut items = Vec::new();
    for item in &preview.items {
        let mut file = LockedFile::open(Path::new(&item.path))?;
        let identity = file.identity_cancellable(cancel)?;
        if !crate::scanner::unchanged(item) {
            return Err("File changed during review. Rescan first.".into());
        }
        items.push((item.clone(), identity));
    }
    db.execute(
        "INSERT INTO approvals (id,created,json) VALUES (?1,?2,?3)",
        params![
            preview.id,
            now(),
            serde_json::to_string(&Approval { items }).map_err(|e| e.to_string())?
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
fn save(db: &Connection, record: &RecoveryRecord) -> Result<(), String> {
    db.execute(
        "INSERT OR REPLACE INTO recovery VALUES (?1,?2,?3)",
        params![
            record.id,
            record.created,
            serde_json::to_string(record).map_err(|e| e.to_string())?
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn folder(db_path: &Path) -> Result<PathBuf, String> {
    let root = db_path
        .parent()
        .ok_or("Missing application data folder.")?
        .join("quarantine");
    let _guards = pin_directory(root.parent().ok_or("Missing parent.")?)?;
    match std::fs::create_dir(&root) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(e.to_string()),
    }
    policy::checked_path(&root)
}
pub fn execute(
    db_path: &Path,
    preview: &str,
    approved: bool,
    policy: &Policy,
    cancel: &AtomicBool,
    progress: impl Fn(&CleanupResult),
) -> Result<CleanupResult, String> {
    if !approved {
        return Err("Explicit approval is required.".into());
    }
    let db = store::open(db_path)?;
    let json: String = db
        .query_row(
            "SELECT json FROM approvals WHERE id=?1 AND consumed=0 AND created>=?2",
            params![preview, now().saturating_sub(15 * 60)],
            |r| r.get(0),
        )
        .map_err(|_| "Preview expired or already used. Create a new review.".to_string())?;
    let approval: Approval = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    if approval.items.is_empty() || approval.items.len() > 200 {
        return Err("Invalid review size.".into());
    }
    let root = folder(db_path)?;
    let _root_guard = pin_directory(&root)?;
    if db
        .execute(
            "UPDATE approvals SET consumed=1 WHERE id=?1 AND consumed=0",
            [preview],
        )
        .map_err(|e| e.to_string())?
        != 1
    {
        return Err("Review already used.".into());
    }
    let mut result = CleanupResult::default();
    for (item, identity) in approval.items {
        if cancel.load(Ordering::Relaxed) {
            result.cancelled = true;
            break;
        }
        let id = uuid::Uuid::new_v4().to_string();
        let mut record = RecoveryRecord {
            id: id.clone(),
            preview: preview.into(),
            created: now(),
            item,
            stored: root
                .join(format!("{id}.held"))
                .to_str()
                .ok_or("Unsupported recovery path.")?
                .into(),
            identity,
            status: "pending".into(),
            error: None,
        };
        // Commit intent durably before the only filesystem mutation.
        save(&db, &record)?;
        let action = (|| {
            let path = Path::new(&record.item.path);
            let mut file = LockedFile::open(path)?;
            if !record.item.eligible
                || policy.excluded(path)
                || !policy
                    .classify(path, record.item.bytes, record.item.modified)
                    .1
                || file.identity_cancellable(cancel)? != record.identity
            {
                return Err("File changed or is now protected; left untouched.".into());
            }
            if cancel.load(Ordering::Relaxed) {
                return Err("Cancelled before moving this file.".into());
            }
            file.move_to(Path::new(&record.stored))
        })();
        match action {
            Ok(()) => {
                record.status = "quarantined".into();
                result.quarantined += 1;
            }
            Err(error) => {
                record.status = "failed".into();
                record.error = Some(error);
                result.failed += 1;
            }
        }
        // If the process stops here, pending records are reconciled by object identity.
        save(&db, &record)?;
        progress(&result);
    }
    result.cancelled |= cancel.load(Ordering::Relaxed);
    Ok(result)
}
fn matches(path: &Path, identity: &Identity) -> bool {
    LockedFile::open(path)
        .and_then(|mut f| f.identity())
        .is_ok_and(|i| &i == identity)
}
pub fn history(db_path: &Path, reconcile: bool) -> Result<Vec<RecoveryRecord>, String> {
    let db = store::open(db_path)?;
    let root = db_path
        .parent()
        .ok_or("Missing data folder.")?
        .join("quarantine");
    let mut statement = db
        .prepare("SELECT json FROM recovery ORDER BY created DESC, rowid DESC LIMIT 500")
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;
    drop(statement);
    let mut records = Vec::new();
    for json in rows {
        let mut record: RecoveryRecord = serde_json::from_str(&json).map_err(|e| e.to_string())?;
        validate_record(&record, &root)?;
        if reconcile && ["pending", "restore_pending"].contains(&record.status.as_str()) {
            let source = matches(Path::new(&record.item.path), &record.identity);
            let stored = matches(Path::new(&record.stored), &record.identity);
            record.status = match (source, stored, record.status.as_str()) {
                (false, true, _) => "quarantined",
                (true, false, "restore_pending") => "restored",
                (true, false, _) => "failed",
                _ => "conflict",
            }
            .into();
            record.error = Some("Recovered an interrupted operation. Verify the reported file location before continuing.".into());
            save(&db, &record)?;
        }
        records.push(record);
    }
    Ok(records)
}
fn validate_record(record: &RecoveryRecord, root: &Path) -> Result<(), String> {
    uuid::Uuid::parse_str(&record.id).map_err(|_| "Invalid recovery record ID.")?;
    if policy::normalized(&record.stored)
        != policy::normalized(&root.join(format!("{}.held", record.id)).to_string_lossy())
    {
        return Err("Invalid recovery location in journal.".into());
    }
    Ok(())
}
pub fn restore(db_path: &Path, id: &str, approved: bool) -> Result<(), String> {
    if !approved {
        return Err("Explicit restore approval is required.".into());
    }
    let db = store::open(db_path)?;
    let json: String = db
        .query_row("SELECT json FROM recovery WHERE id=?1", [id], |r| r.get(0))
        .map_err(|_| "Unknown recovery record.")?;
    let mut record: RecoveryRecord = serde_json::from_str(&json).map_err(|e| e.to_string())?;
    validate_record(
        &record,
        &db_path
            .parent()
            .ok_or("Missing data folder.")?
            .join("quarantine"),
    )?;
    if record.status != "quarantined" {
        return Err("This record is not available for automatic restore.".into());
    }
    let mut file = LockedFile::open(Path::new(&record.stored))?;
    if file.identity()? != record.identity {
        return Err("Recovery file changed. Automatic restore refused.".into());
    }
    let destination = Path::new(&record.item.path);
    let _parents = pin_directory(destination.parent().ok_or("Missing original directory.")?)?;
    if destination.try_exists().map_err(|e| e.to_string())? {
        return Err("Original path is occupied. Restore never overwrites files.".into());
    }
    record.status = "restore_pending".into();
    record.error = None;
    save(&db, &record)?;
    if let Err(error) = file.move_to(destination) {
        record.status = "quarantined".into();
        record.error = Some(error.clone());
        save(&db, &record)?;
        return Err(error);
    }
    record.status = "restored".into();
    save(&db, &record)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        _dir: tempfile::TempDir,
        db: PathBuf,
        source: PathBuf,
        policy: Policy,
        preview: Preview,
    }
    impl Fixture {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let source = dir.path().join("temp");
            std::fs::create_dir(&source).unwrap();
            let path = source.join("old.tmp");
            std::fs::write(&path, b"synthetic recovery bytes").unwrap();
            filetime::set_file_mtime(
                &path,
                filetime::FileTime::from_unix_time(now() as i64 - 8 * 86400, 0),
            )
            .unwrap();
            let meta = std::fs::metadata(&path).unwrap();
            let db_path = dir.path().join("inventory.db");
            let db = store::open(&db_path).unwrap();
            let preview = Preview {
                id: uuid::Uuid::new_v4().to_string(),
                items: vec![Item {
                    id: "fixture".into(),
                    path: path.to_str().unwrap().into(),
                    bytes: meta.len(),
                    modified: crate::scanner::modified(&meta),
                    eligible: true,
                    category: "Temporary".into(),
                    reason: "Synthetic fixture".into(),
                    duplicate_group: None,
                }],
                bytes: meta.len(),
                executable: true,
                notice: String::new(),
            };
            prepare(&db, &preview).unwrap();
            Self {
                policy: Policy::fixture(Settings::default(), source.clone()),
                _dir: dir,
                db: db_path,
                source,
                preview,
            }
        }
        fn run(&self) -> CleanupResult {
            execute(
                &self.db,
                &self.preview.id,
                true,
                &self.policy,
                &AtomicBool::new(false),
                |_| {},
            )
            .unwrap()
        }
        fn path(&self) -> PathBuf {
            self.source.join("old.tmp")
        }
    }
    #[test]
    fn quarantine_restore_roundtrip_and_no_overwrite() {
        let f = Fixture::new();
        assert_eq!(f.run().quarantined, 1);
        assert!(!f.path().exists());
        let records = history(&f.db, true).unwrap();
        assert_eq!(records[0].status, "quarantined");
        assert_eq!(
            std::fs::read(&records[0].stored).unwrap(),
            b"synthetic recovery bytes"
        );
        assert!(restore(&f.db, &records[0].id, false).is_err());
        let moved_parent = f._dir.path().join("moved-parent");
        std::fs::rename(&f.source, &moved_parent).unwrap();
        assert!(restore(&f.db, &records[0].id, true).is_err());
        std::fs::rename(&moved_parent, &f.source).unwrap();
        std::fs::write(f.path(), b"new user file").unwrap();
        assert!(restore(&f.db, &records[0].id, true).is_err());
        assert_eq!(std::fs::read(f.path()).unwrap(), b"new user file");
        // Only this test's replacement fixture is removed.
        std::fs::remove_file(f.path()).unwrap();
        restore(&f.db, &records[0].id, true).unwrap();
        assert_eq!(
            std::fs::read(f.path()).unwrap(),
            b"synthetic recovery bytes"
        );
        assert_eq!(history(&f.db, true).unwrap()[0].status, "restored");
        assert!(execute(
            &f.db,
            &f.preview.id,
            true,
            &f.policy,
            &AtomicBool::new(false),
            |_| {}
        )
        .is_err());
    }
    #[test]
    fn approval_expiry_cancellation_and_current_protection() {
        let mut f = Fixture::new();
        assert!(execute(
            &f.db,
            &f.preview.id,
            false,
            &f.policy,
            &AtomicBool::new(false),
            |_| {}
        )
        .is_err());
        assert!(execute(
            &f.db,
            "invented",
            true,
            &f.policy,
            &AtomicBool::new(false),
            |_| {}
        )
        .is_err());
        let db = store::open(&f.db).unwrap();
        db.execute("UPDATE approvals SET created=0", []).unwrap();
        assert!(execute(
            &f.db,
            &f.preview.id,
            true,
            &f.policy,
            &AtomicBool::new(false),
            |_| {}
        )
        .is_err());
        db.execute("UPDATE approvals SET created=?1", [now()])
            .unwrap();
        let result = execute(
            &f.db,
            &f.preview.id,
            true,
            &f.policy,
            &AtomicBool::new(true),
            |_| {},
        )
        .unwrap();
        assert!(result.cancelled);
        assert!(f.path().exists());
        f.preview.id = uuid::Uuid::new_v4().to_string();
        prepare(&db, &f.preview).unwrap();
        f.policy
            .settings
            .protected
            .push(f.source.to_str().unwrap().into());
        assert_eq!(f.run().failed, 1);
        assert!(f.path().exists());
    }
    #[test]
    fn content_change_with_preserved_size_and_time_is_refused() {
        let f = Fixture::new();
        let old_time =
            filetime::FileTime::from_last_modification_time(&std::fs::metadata(f.path()).unwrap());
        std::fs::write(f.path(), b"SYNTHETIC RECOVERY BYTES").unwrap();
        filetime::set_file_mtime(f.path(), old_time).unwrap();
        assert_eq!(f.run().failed, 1);
        assert!(f.path().exists());
        let g = Fixture::new();
        g.run();
        let r = history(&g.db, true).unwrap().remove(0);
        std::fs::write(&r.stored, b"corrupted quarantine").unwrap();
        assert!(restore(&g.db, &r.id, true).is_err());
        assert!(!g.path().exists());
    }
    #[test]
    fn interrupted_journal_reconciles_without_moving_files() {
        let f = Fixture::new();
        f.run();
        let db = store::open(&f.db).unwrap();
        let mut record = history(&f.db, true).unwrap().remove(0);
        record.status = "pending".into();
        save(&db, &record).unwrap();
        assert_eq!(history(&f.db, false).unwrap()[0].status, "pending");
        assert_eq!(history(&f.db, true).unwrap()[0].status, "quarantined");
        restore(&f.db, &record.id, true).unwrap();
        record.status = "restore_pending".into();
        save(&db, &record).unwrap();
        assert_eq!(history(&f.db, true).unwrap()[0].status, "restored");
        assert!(f.path().exists());
    }
    #[test]
    fn partial_failure_and_cancellation_leave_other_files_untouched() {
        let mut f = Fixture::new();
        let second = f.source.join("second.tmp");
        std::fs::copy(f.path(), &second).unwrap();
        filetime::set_file_mtime(
            &second,
            filetime::FileTime::from_unix_time(now() as i64 - 8 * 86400, 0),
        )
        .unwrap();
        let mut item = f.preview.items[0].clone();
        item.id = "second".into();
        item.path = second.to_str().unwrap().into();
        item.modified = crate::scanner::modified(&std::fs::metadata(&second).unwrap());
        f.preview.items.push(item);
        f.preview.id = uuid::Uuid::new_v4().to_string();
        let db = store::open(&f.db).unwrap();
        prepare(&db, &f.preview).unwrap();
        std::fs::write(f.path(), "changed").unwrap();
        let result = f.run();
        assert_eq!(result.failed, 1);
        assert_eq!(result.quarantined, 1);
        assert!(f.path().exists());
        let g = Fixture::new();
        let mut preview = g.preview.clone();
        let second = g.source.join("second.tmp");
        std::fs::copy(g.path(), &second).unwrap();
        filetime::set_file_mtime(
            &second,
            filetime::FileTime::from_unix_time(now() as i64 - 8 * 86400, 0),
        )
        .unwrap();
        let mut item = preview.items[0].clone();
        item.id = "second".into();
        item.path = second.to_str().unwrap().into();
        item.modified = crate::scanner::modified(&std::fs::metadata(&second).unwrap());
        preview.items.push(item);
        preview.id = uuid::Uuid::new_v4().to_string();
        prepare(&store::open(&g.db).unwrap(), &preview).unwrap();
        let cancel = AtomicBool::new(false);
        let result = execute(&g.db, &preview.id, true, &g.policy, &cancel, |_| {
            cancel.store(true, Ordering::Relaxed)
        })
        .unwrap();
        assert_eq!(result.quarantined, 1);
        assert!(result.cancelled);
        assert!(second.exists());
    }
}

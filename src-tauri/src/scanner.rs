use crate::{
    model::*,
    policy::{checked_path, is_reparse, Policy},
    store,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant, UNIX_EPOCH},
};
use walkdir::WalkDir;
pub fn modified(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(u64::MAX)
}
pub fn scan(
    db_path: &Path,
    root: &Path,
    policy: Policy,
    cancel: &AtomicBool,
    mut emit: impl FnMut(&Progress),
    id: String,
) -> Result<Progress, String> {
    let root = checked_path(root)?;
    if !root.is_dir() && !root.is_file() {
        return Err("Choose a file, directory, or local drive root.".into());
    }
    let db = store::open(db_path)?;
    let mut p = Progress {
        scan_id: id,
        status: "scanning".into(),
        ..Default::default()
    };
    store::progress(&db, &p)?;
    emit(&p);
    let mut last = Instant::now();
    let mut walk = WalkDir::new(&root)
        .follow_links(false)
        .max_open(16)
        .into_iter();
    db.execute_batch("BEGIN").map_err(|e| e.to_string())?;
    while let Some(entry) = walk.next() {
        if cancel.load(Ordering::Relaxed) {
            p.status = "cancelled".into();
            break;
        }
        let entry = match entry {
            Ok(e) => e,
            Err(_) => {
                p.errors += 1;
                continue;
            }
        };
        let meta = match fs::symlink_metadata(entry.path()) {
            Ok(m) => m,
            Err(_) => {
                p.errors += 1;
                continue;
            }
        };
        if entry.path().to_str().is_none() {
            if meta.is_dir() {
                walk.skip_current_dir();
            }
            p.skipped += 1;
            continue;
        }
        if is_reparse(&meta)
            || policy.excluded(entry.path())
            || crate::policy::within(
                &entry.path().to_string_lossy(),
                &db_path.parent().unwrap_or(db_path).to_string_lossy(),
            )
        {
            if meta.is_dir() {
                walk.skip_current_dir();
            }
            p.skipped += 1;
            continue;
        }
        if meta.is_file() {
            let (category, eligible, reason) =
                policy.classify(entry.path(), meta.len(), modified(&meta));
            let item = Item {
                id: uuid::Uuid::new_v4().to_string(),
                path: entry.path().to_string_lossy().into_owned(),
                bytes: meta.len(),
                modified: modified(&meta),
                category,
                eligible,
                reason,
                duplicate_group: None,
            };
            store::insert(&db, &p.scan_id, &item)?;
            p.files += 1;
            p.bytes = p.bytes.saturating_add(item.bytes);
            if eligible {
                p.candidates += 1;
                p.candidate_bytes = p.candidate_bytes.saturating_add(item.bytes);
            }
        }
        if last.elapsed() > Duration::from_millis(300) {
            store::progress(&db, &p)?;
            db.execute_batch("COMMIT; BEGIN")
                .map_err(|e| e.to_string())?;
            emit(&p);
            last = Instant::now();
        }
    }
    if p.status != "cancelled" {
        p.status = "complete".into();
    }
    store::progress(&db, &p)?;
    db.execute_batch("COMMIT").map_err(|e| e.to_string())?;
    emit(&p);
    Ok(p)
}
fn hash(path: &Path, full: bool, cancel: &AtomicBool) -> Result<String, String> {
    checked_path(path)?;
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    let mut buffer = [0u8; 65536];
    if full {
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err("Cancelled".into());
            }
            let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            h.update(&buffer[..n]);
        }
    } else {
        let n = file.read(&mut buffer[..8192]).map_err(|e| e.to_string())?;
        h.update(&buffer[..n]);
        let len = file.metadata().map_err(|e| e.to_string())?.len();
        file.seek(SeekFrom::Start(len.saturating_sub(8192)))
            .map_err(|e| e.to_string())?;
        let n = file.read(&mut buffer[..8192]).map_err(|e| e.to_string())?;
        h.update(&buffer[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}
/// Explicit opt-in pass; bounded to 20,000 files and groups <=1,000 to limit disk activity.
pub fn duplicates(
    db_path: &Path,
    scan: &str,
    cancel: &AtomicBool,
    mut tick: impl FnMut(u64),
) -> Result<u64, String> {
    let db = store::open(db_path)?;
    db.execute(
        "UPDATE items SET duplicate_group=NULL WHERE scan=?1",
        [scan],
    )
    .map_err(|e| e.to_string())?;
    db.execute_batch(
        "CREATE TEMP TABLE hashes (id TEXT PRIMARY KEY, size INTEGER, partial TEXT, full TEXT);",
    )
    .map_err(|e| e.to_string())?;
    let mut query=db.prepare("SELECT * FROM items WHERE scan=?1 AND bytes>0 AND bytes IN (SELECT bytes FROM items WHERE scan=?1 GROUP BY bytes HAVING COUNT(*) BETWEEN 2 AND 1000) ORDER BY bytes DESC LIMIT 20000").map_err(|e|e.to_string())?;
    let rows = query
        .query_map([scan], store::row)
        .map_err(|e| e.to_string())?;
    let mut count = 0;
    for row in rows {
        if cancel.load(Ordering::Relaxed) {
            return Err(
                "Duplicate pass cancelled; existing confirmed groups remain review-only.".into(),
            );
        }
        let item = row.map_err(|e| e.to_string())?;
        if !unchanged(&item) {
            continue;
        }
        if let Ok(partial) = hash(Path::new(&item.path), false, cancel) {
            db.execute(
                "INSERT INTO hashes VALUES (?1,?2,?3,NULL)",
                rusqlite::params![item.id, item.bytes, partial],
            )
            .map_err(|e| e.to_string())?;
        }
        count += 1;
        if count % 100 == 0 {
            tick(count);
        }
    }
    let mut query=db.prepare("SELECT items.* FROM items JOIN hashes ON items.id=hashes.id WHERE (hashes.size,hashes.partial) IN (SELECT size,partial FROM hashes GROUP BY size,partial HAVING COUNT(*)>1)").map_err(|e|e.to_string())?;
    for row in query.query_map([], store::row).map_err(|e| e.to_string())? {
        if cancel.load(Ordering::Relaxed) {
            return Err("Duplicate pass cancelled.".into());
        }
        let item = row.map_err(|e| e.to_string())?;
        if let Ok(full) = hash(Path::new(&item.path), true, cancel) {
            if unchanged(&item) {
                db.execute(
                    "UPDATE hashes SET full=?1 WHERE id=?2",
                    rusqlite::params![full, item.id],
                )
                .map_err(|e| e.to_string())?;
            }
        }
    }
    let changed=db.execute("UPDATE items SET duplicate_group=(SELECT full FROM hashes WHERE hashes.id=items.id) WHERE scan=?1 AND id IN (SELECT id FROM hashes WHERE full IN (SELECT full FROM hashes WHERE full IS NOT NULL GROUP BY size,full HAVING COUNT(*)>1))",[scan]).map_err(|e|e.to_string())?;
    Ok(changed as u64)
}
pub fn unchanged(item: &Item) -> bool {
    checked_path(Path::new(&item.path)).is_ok()
        && fs::symlink_metadata(&item.path).is_ok_and(|m| {
            m.is_file() && !is_reparse(&m) && m.len() == item.bytes && modified(&m) == item.modified
        })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn non_unicode_names_cannot_alias_a_valid_inventory_path() {
        use std::os::windows::ffi::OsStringExt;
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        std::fs::create_dir(&data).unwrap();
        let invalid = data.join(std::ffi::OsString::from_wide(&[
            0xd800,
            b'.' as u16,
            b't' as u16,
            b'm' as u16,
            b'p' as u16,
        ]));
        std::fs::write(&invalid, "synthetic invalid-name fixture").unwrap();
        std::fs::write(data.join("�.tmp"), "synthetic valid-name fixture").unwrap();
        assert!(checked_path(&invalid).is_err());
        std::fs::create_dir(dir.path().join("appdata")).unwrap();
        let db_path = dir.path().join("appdata").join("inventory.db");
        let p = scan(
            &db_path,
            &data,
            Policy::fixture(Settings::default(), data.clone()),
            &AtomicBool::new(false),
            |_| {},
            "unicode".into(),
        )
        .unwrap();
        assert_eq!(p.files, 1);
        assert_eq!(p.skipped, 1);
        assert_eq!(
            store::view(&store::open(&db_path).unwrap(), "unicode", 0, "")
                .unwrap()
                .items
                .len(),
            1
        );
    }
    #[test]
    fn database_directory_is_excluded_even_with_canonical_windows_prefixes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("fixture");
        let internal = root.join("app-internal");
        fs::create_dir_all(&internal).unwrap();
        fs::write(root.join("visible.zip"), "fixture").unwrap();
        let p = scan(
            &internal.join("inventory.db"),
            &root,
            Policy::new(Settings::default()).unwrap(),
            &AtomicBool::new(false),
            |_| {},
            "self-exclusion".into(),
        )
        .unwrap();
        assert_eq!(p.files, 1);
        assert_eq!(p.skipped, 1);
    }
    #[test]
    fn exclusions_skip_subtrees_without_excluding_similar_siblings() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("fixture");
        let excluded = root.join("skip");
        let sibling = root.join("skip-sibling");
        fs::create_dir_all(&excluded).unwrap();
        fs::create_dir_all(&sibling).unwrap();
        fs::write(excluded.join("hidden.zip"), "x").unwrap();
        fs::write(sibling.join("visible.zip"), "y").unwrap();
        let db = d.path().join("database").join("test.db");
        fs::create_dir(db.parent().unwrap()).unwrap();
        let settings = Settings {
            exclusions: vec![excluded.to_string_lossy().into_owned()],
            ..Default::default()
        };
        let p = scan(
            &db,
            &root,
            Policy::new(settings).unwrap(),
            &AtomicBool::new(false),
            |_| {},
            "exclude".into(),
        )
        .unwrap();
        assert_eq!(p.files, 1);
        assert_eq!(p.skipped, 1);
    }
    #[test]
    fn scan_cancel_change_and_duplicates() {
        let d = tempfile::tempdir().unwrap();
        let data = d.path().join("data");
        fs::create_dir(&data).unwrap();
        fs::write(data.join("one.zip"), b"same data").unwrap();
        fs::write(data.join("two.zip"), b"same data").unwrap();
        fs::write(data.join("three.zip"), b"different").unwrap();
        let db = d.path().join("db").join("inventory.db");
        fs::create_dir(db.parent().unwrap()).unwrap();
        let cancel = AtomicBool::new(false);
        let p = scan(
            &db,
            &data,
            Policy::new(Settings::default()).unwrap(),
            &cancel,
            |_| {},
            "scan".into(),
        )
        .unwrap();
        assert_eq!(p.files, 3);
        assert_eq!(duplicates(&db, "scan", &cancel, |_| {}).unwrap(), 2);
        let c = store::open(&db).unwrap();
        let v = store::view(&c, "scan", 0, "").unwrap();
        assert_eq!(
            v.items
                .iter()
                .filter(|i| i.duplicate_group.is_some())
                .count(),
            2
        );
        let item = v
            .items
            .iter()
            .find(|i| i.path.ends_with("one.zip"))
            .unwrap();
        fs::write(&item.path, "changed contents").unwrap();
        assert!(!unchanged(item));
        cancel.store(true, Ordering::Relaxed);
        let p = scan(
            &db,
            &data,
            Policy::new(Settings::default()).unwrap(),
            &cancel,
            |_| {},
            "cancel".into(),
        )
        .unwrap();
        assert_eq!(p.status, "cancelled");
    }
    #[test]
    fn missing_root_is_error() {
        let d = tempfile::tempdir().unwrap();
        assert!(scan(
            &d.path().join("db"),
            &d.path().join("absent"),
            Policy::new(Settings::default()).unwrap(),
            &AtomicBool::new(false),
            |_| {},
            "x".into()
        )
        .is_err());
    }
    #[cfg(windows)]
    #[test]
    #[ignore = "Requires Windows symlink privilege; run explicitly in a disposable Windows VM"]
    fn symlink_root_is_rejected() {
        use std::os::windows::fs::symlink_dir;
        let d = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret"), "x").unwrap();
        symlink_dir(outside.path(), d.path().join("link")).unwrap();
        assert!(checked_path(&d.path().join("link")).is_err());
    }
}

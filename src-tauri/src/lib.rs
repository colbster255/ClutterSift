mod ai;
mod file_ops;
mod local_setup;
mod model;
mod planner;
mod policy;
mod protection;
mod recovery;
mod scanner;
mod store;
use model::*;
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tauri::{Emitter, Manager, State};
struct AppState {
    _instance: std::fs::File,
    db: PathBuf,
    busy: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
}
#[tauri::command]
fn open_setup_link(url: String) -> Result<(), String> {
    local_setup::open_link(&url)
}
#[tauri::command]
async fn local_llm_status() -> Result<local_setup::LocalStatus, String> {
    tauri::async_runtime::spawn_blocking(local_setup::status)
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn setup_local_llm(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    approved: bool,
) -> Result<local_setup::LocalStatus, String> {
    let guard = job(&state)?;
    let cancel = state.cancel.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        local_setup::setup(approved, &cancel, |message| {
            let _ = app.emit("local-setup-progress", message);
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn propose_protections(state: State<AppState>, goal: String) -> Result<Settings, String> {
    protection::propose(&store::settings(&store::open(&state.db)?)?, &goal)
}
struct Job(Arc<AtomicBool>);
impl Drop for Job {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}
fn job(state: &AppState) -> Result<Job, String> {
    state
        .busy
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .map_err(|_| "Another filesystem operation is already running.".to_string())?;
    state.cancel.store(false, Ordering::SeqCst);
    Ok(Job(state.busy.clone()))
}
#[tauri::command]
fn get_settings(state: State<AppState>) -> Result<Settings, String> {
    store::settings(&store::open(&state.db)?)
}
#[tauri::command]
fn save_settings(state: State<AppState>, settings: Settings) -> Result<(), String> {
    let _guard = job(&state)?;
    if settings.protected.len() + settings.exclusions.len() + settings.patterns.len() > 200 {
        return Err("Use at most 200 protection/exclusion rules.".into());
    }
    if !["offline", "ollama", "openrouter"].contains(&settings.provider.as_str())
        || !["dark", "light", "system"].contains(&settings.theme.as_str())
    {
        return Err("Invalid provider or theme.".into());
    }
    for path in settings.protected.iter().chain(settings.exclusions.iter()) {
        policy::checked_path(std::path::Path::new(path))?;
    }
    policy::Policy::new(settings.clone())?;
    store::save_settings(&store::open(&state.db)?, &settings)
}
#[tauri::command]
fn save_api_key(key: String) -> Result<(), String> {
    if key.len() > 4096 {
        return Err("Invalid key length.".into());
    }
    let entry = keyring::Entry::new("org.cluttersift.desktop", "openrouter")
        .map_err(|_| "Windows Credential Manager unavailable.".to_string())?;
    if key.is_empty() {
        entry
            .delete_credential()
            .map_err(|_| "Could not remove credential (it may not exist).".to_string())
    } else {
        entry
            .set_password(&key)
            .map_err(|_| "Could not save key in Windows Credential Manager.".to_string())
    }
}
#[tauri::command]
fn start_scan(
    app: tauri::AppHandle,
    state: State<AppState>,
    root: String,
) -> Result<String, String> {
    let root = policy::checked_path(std::path::Path::new(&root))?;
    let policy = policy::Policy::new(store::settings(&store::open(&state.db)?)?)?;
    let guard = job(&state)?;
    let id = uuid::Uuid::new_v4().to_string();
    let p = Progress {
        scan_id: id.clone(),
        status: "scanning".into(),
        ..Default::default()
    };
    store::progress(&store::open(&state.db)?, &p)?;
    let db = state.db.clone();
    let cancel = state.cancel.clone();
    let scan_id = id.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        if let Err(error) = scanner::scan(
            &db,
            &root,
            policy,
            &cancel,
            |p| {
                let _ = app.emit("scan-progress", p);
            },
            scan_id.clone(),
        ) {
            let mut p = Progress {
                scan_id,
                status: "failed".into(),
                ..Default::default()
            };
            if let Ok(db) = store::open(&db) {
                if let Ok(json) =
                    db.query_row("SELECT json FROM scans WHERE id=?1", [&p.scan_id], |r| {
                        r.get::<_, String>(0)
                    })
                {
                    if let Ok(previous) = serde_json::from_str::<Progress>(&json) {
                        p = Progress {
                            status: "failed".into(),
                            ..previous
                        };
                    }
                }
                let _ = store::progress(&db, &p);
            }
            let _ = app.emit("scan-progress", &p);
            let _ = app.emit("operation-error", error);
        }
    });
    Ok(id)
}
#[tauri::command]
fn cancel_scan(state: State<AppState>) {
    state.cancel.store(true, Ordering::Relaxed);
}
#[tauri::command]
fn scan_view(
    state: State<AppState>,
    scan: String,
    offset: u32,
    filter: String,
) -> Result<ScanView, String> {
    store::view(&store::open(&state.db)?, &scan, offset, &filter)
}
#[tauri::command]
fn scan_history(state: State<AppState>) -> Result<Vec<Progress>, String> {
    let db = store::open(&state.db)?;
    let mut stmt = db
        .prepare("SELECT json FROM scans ORDER BY rowid DESC LIMIT 20")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    rows.map(|r| serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string()))
        .collect()
}
#[tauri::command]
async fn find_duplicates(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    scan: String,
) -> Result<u64, String> {
    let guard = job(&state)?;
    let db = state.db.clone();
    let cancel = state.cancel.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        scanner::duplicates(&db, &scan, &cancel, |n| {
            let _ = app.emit("duplicate-progress", n);
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn ask_assistant(
    state: State<'_, AppState>,
    scan: String,
    goal: String,
) -> Result<Advice, String> {
    let (settings, items) = {
        let db = store::open(&state.db)?;
        let policy = policy::Policy::new(store::settings(&db)?)?;
        let items = store::view(&db, &scan, 0, "eligible")?
            .items
            .into_iter()
            .filter(|i| {
                policy
                    .classify(std::path::Path::new(&i.path), i.bytes, i.modified)
                    .1
            })
            .collect::<Vec<_>>();
        (policy.settings, items)
    };
    ai::advise(&settings, &goal, &items).await
}
#[tauri::command]
async fn cleanup_preview(
    state: State<'_, AppState>,
    scan: String,
    ids: Vec<String>,
) -> Result<Preview, String> {
    let guard = job(&state)?;
    let path = state.db.clone();
    let cancel = state.cancel.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let db = store::open(&path)?;
        planner::preview_cancellable(
            &db,
            &scan,
            &ids,
            &policy::Policy::new(store::settings(&db)?)?,
            &cancel,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn execute_cleanup(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    preview: String,
    approved: bool,
) -> Result<CleanupResult, String> {
    let guard = job(&state)?;
    let path = state.db.clone();
    let cancel = state.cancel.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let db = store::open(&path)?;
        let policy = policy::Policy::new(store::settings(&db)?)?;
        recovery::execute(&path, &preview, approved, &policy, &cancel, |p| {
            let _ = app.emit("cleanup-progress", p);
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn recovery_history(state: State<'_, AppState>) -> Result<Vec<RecoveryRecord>, String> {
    let path = state.db.clone();
    let busy = state.busy.clone();
    tauri::async_runtime::spawn_blocking(move || {
        // Reconciliation must not run while another operation can be moving a file.
        let guard = busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| Job(busy));
        recovery::history(&path, guard.is_some())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn restore_item(
    state: State<'_, AppState>,
    id: String,
    approved: bool,
) -> Result<(), String> {
    let guard = job(&state)?;
    let path = state.db.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        recovery::restore(&path, &id, approved)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn review_history(state: State<AppState>) -> Result<Vec<Preview>, String> {
    let db = store::open(&state.db)?;
    let mut stmt = db
        .prepare("SELECT json FROM previews ORDER BY created DESC LIMIT 20")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    rows.map(|r| serde_json::from_str(&r.map_err(|e| e.to_string())?).map_err(|e| e.to_string()))
        .collect()
}
#[tauri::command]
fn drives() -> Vec<serde_json::Value> {
    sysinfo::Disks::new_with_refreshed_list().iter().map(|d|serde_json::json!({"path":d.mount_point().to_string_lossy(),"name":d.name().to_string_lossy(),"total":d.total_space(),"available":d.available_space(),"removable":d.is_removable()})).collect()
}
#[tauri::command]
fn installed_apps() -> Vec<serde_json::Value> {
    let mut apps = vec![];
    #[cfg(windows)]
    {
        use winreg::{enums::*, RegKey};
        for hive in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
            for flags in [KEY_READ | KEY_WOW64_64KEY, KEY_READ | KEY_WOW64_32KEY] {
                if let Ok(key) = RegKey::predef(hive).open_subkey_with_flags(
                    r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
                    flags,
                ) {
                    for name in key.enum_keys().flatten() {
                        if let Ok(app) = key.open_subkey(name) {
                            if let Ok(name) = app.get_value::<String, _>("DisplayName") {
                                let bytes = app.get_value::<u32, _>("EstimatedSize").unwrap_or(0)
                                    as u64
                                    * 1024;
                                apps.push(serde_json::json!({"name":name,"bytes":bytes}));
                            }
                        }
                    }
                }
            }
        }
    }
    apps.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    apps.dedup_by(|a, b| a["name"] == b["name"]);
    apps
}
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let folder = app.path().app_local_data_dir()?;
            std::fs::create_dir_all(&folder)?;
            // Windows packaged hosts can redirect LocalAppData. Resolve the actual
            // storage location once, before locking it or deriving journal paths.
            let folder = policy::checked_path(&folder).map_err(std::io::Error::other)?;
            use std::os::windows::fs::OpenOptionsExt;
            let instance = std::fs::OpenOptions::new().create(true).truncate(false).read(true).write(true).share_mode(0).open(folder.join("instance.lock"))
                .map_err(|e| std::io::Error::new(e.kind(), format!("ClutterSift is already running or its data directory cannot be locked: {e}")))?;
            let db = folder.join("cluttersift.db");
            let conn = store::open(&db).map_err(std::io::Error::other)?;
            // Interrupted inventories remain available, but cannot masquerade as complete scans.
            let mut q = conn.prepare("SELECT json FROM scans")?;
            let scans = q
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            drop(q);
            for json in scans {
                if let Ok(mut p) = serde_json::from_str::<Progress>(&json) {
                    if p.status == "scanning" {
                        p.status = "interrupted".into();
                        store::progress(&conn, &p).map_err(std::io::Error::other)?;
                    }
                }
            }
            app.manage(AppState {
                _instance: instance,
                db,
                busy: Arc::new(AtomicBool::new(false)),
                cancel: Arc::new(AtomicBool::new(false)),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            local_llm_status,
            open_setup_link,
            setup_local_llm,
            save_settings,
            propose_protections,
            save_api_key,
            start_scan,
            cancel_scan,
            scan_view,
            scan_history,
            find_duplicates,
            ask_assistant,
            cleanup_preview,
            execute_cleanup,
            recovery_history,
            restore_item,
            review_history,
            drives,
            installed_apps
        ])
        .run(tauri::generate_context!())
        .expect("Could not start ClutterSift");
}

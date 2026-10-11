use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub protected: Vec<String>,
    pub patterns: Vec<String>,
    pub exclusions: Vec<String>,
    pub theme: String,
    pub provider: String,
    pub model: String,
    pub cloud_consent: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            protected: vec![],
            patterns: vec![],
            exclusions: vec![],
            theme: "light".into(),
            provider: "offline".into(),
            model: String::new(),
            cloud_consent: false,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub path: String,
    pub bytes: u64,
    pub modified: u64,
    pub category: String,
    pub eligible: bool,
    pub reason: String,
    pub duplicate_group: Option<String>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Progress {
    pub scan_id: String,
    pub files: u64,
    pub bytes: u64,
    pub candidates: u64,
    pub candidate_bytes: u64,
    pub errors: u64,
    pub skipped: u64,
    pub status: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Breakdown {
    pub category: String,
    pub bytes: u64,
    pub files: u64,
}
#[derive(Debug, Serialize)]
pub struct ScanView {
    pub progress: Progress,
    pub categories: Vec<Breakdown>,
    pub items: Vec<Item>,
    pub total: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suggestion {
    pub id: String,
    pub explanation: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Advice {
    pub summary: String,
    pub candidates: Vec<Suggestion>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preview {
    pub id: String,
    pub items: Vec<Item>,
    pub bytes: u64,
    pub executable: bool,
    pub notice: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryRecord {
    pub id: String,
    pub preview: String,
    pub created: u64,
    pub item: Item,
    pub stored: String,
    pub identity: crate::file_ops::Identity,
    pub status: String,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CleanupResult {
    pub quarantined: u64,
    pub failed: u64,
    pub cancelled: bool,
}
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

# Changelog

## 0.1.0 — Unreleased utility beta

- Initial Tauri 2 / React / Rust storage analysis app.
- Compact CCleaner-inspired Cleaner layout with a category sidebar, results list, and persistent Analyze / Review cleanup actions; light theme by default.
- Theme-aware native dropdown popups and clearing incompatible model names when switching AI providers.
- Explicit automated Ollama/model setup with progress, cancellation and readiness checks; guided OpenRouter setup with official account/key/model links.
- Background scans with cancellation, SQLite inventory, category breakdowns and file pagination.
- Conservative protection rules, exclusions and confirmed custom-rule changes.
- Optional bounded, staged SHA-256 duplicate analysis and installed-app metadata.
- Offline guidance and privacy-minimized Ollama/OpenRouter structured recommendations.
- Dry-run preview history and explicit one-use quarantine approvals with 15-minute expiry.
- Windows handle-based no-replace moves, locked identity/hash revalidation, durable per-item journal, interruption reconciliation and conflict-aware restore.
- Individual-file scan roots and cancellation during quarantine verification.
- Resolve Windows application-data redirection before deriving recovery paths, so packaged-host launches use the actual storage location.
- Windows CI/release configuration, synthetic-fixture tests and public-project documentation.

Quarantine does not free disk space. Recycle Bin integration and permanent deletion are unavailable. Provided as is under MIT; users are responsible for their choices and backups. See the checklist for verification status and incomplete features.

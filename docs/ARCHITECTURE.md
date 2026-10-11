# Architecture

The repository was empty when implementation began. Tauri 2 keeps filesystem decisions in Rust, with React providing a webview UI. SQLite holds disk-backed inventories so scans do not retain every file in RAM. There is no web server, frontend filesystem bridge, shell plugin, or arbitrary provider endpoint.

| Module           | Responsibility                                                                                         |
| ---------------- | ------------------------------------------------------------------------------------------------------ |
| `model.rs`       | IPC types and strict structured response types                                                         |
| `scanner.rs`     | Read-only walk, streaming progress, cancellation, staged duplicate hashing                             |
| `policy.rs`      | Local-path checks, reparse rejection, component-aware case-insensitive protection, classification      |
| `store.rs`       | SQLite schema, settings, inventory pages and scan status                                               |
| `ai.rs`          | Provider interface, fixed destinations, consent, bounded HTTP response, OS credentials                 |
| `planner.rs`     | Strict ID validation, current-policy and metadata revalidation, persisted dry runs                     |
| `file_ops.rs`    | Pinned Windows directory/file handles, identity/content hashing and no-replace handle renames          |
| `recovery.rs`    | Expiring one-use approvals, per-item quarantine journal, interruption reconciliation and restore       |
| `lib.rs`         | Tauri IPC boundary, one active filesystem job, single-instance data lock and registry inventory        |
| `src/App.tsx`    | Five screens, review selection, confirmation, setup guides and failure states                          |
| `local_setup.rs` | Explicit Ollama installation/model pull, bounded status parsing, cancellation, allowlisted setup links |

## Scan lifecycle

`protection.rs` interprets keep/protect/preserve goals locally, recognizing standard folder names and existing absolute paths. It only proposes additive rules; the user must confirm the complete list. No model call is involved. Custom application locations and redirected personal folders require explicit paths. This is a deliberately limited parser, not general dependency discovery.

An absolute local directory is validated, a UUID inventory is created, then a blocking worker walks with `follow_links(false)` and at most 16 open directory handles. Worker concurrency is **one** per app instance to avoid disk saturation. Progress and database batches are published about every 300 ms. Cancellation is checked per entry; a blocking OS I/O call cannot be instantly cancelled. Access errors increment counters. Files are stored individually in SQLite with size/category indexes. UI pages are limited to 200 rows. Interrupted scans are marked at next app startup.

Duplicate analysis is a separate user-triggered pass, limited to 20,000 files in size groups between 2 and 1,000. Partial hashes use first/last 8 KB, then full SHA-256 confirms matching candidates. Full reads use a 64 KB buffer and check cancellation. Metadata is checked before and after hashing; this is not an adversarial identity guarantee. Confirmed groups are informational, never deletion permission.

## Planning boundary

AI receives only the first 200 eligible candidate IDs and metadata, plus the goal. Outputs use strict schemas, bounded lengths, and exact membership validation. No path/action field is accepted. The planner reloads the same inventory, checks scan status and current policy, and captures file identity/content hashes under a locked handle. A preview changes no files. Its opaque approval expires after 15 minutes and is atomically consumed once; explicit approval is a separate IPC action. Historical previews cannot be executed from the UI. Bytes describe file sizes, not reclaimed space.

## File operations and recovery

Public packaging uses `scripts/build-desktop.cjs` to supply Rust path remapping through `CARGO_ENCODED_RUSTFLAGS` and check the native executable/default NSIS installer for the build machine's home/workspace path. CI and tag releases use this wrapper. This removes a confirmed development-path disclosure from raw local release builds; it is not a general binary secret scanner or a signing mechanism.

Only direct, old `.tmp` files allowed by backend policy are eligible. The executor opens the reviewed file with read/delete rights and read sharing only. Ancestor directories are opened from root downward without write/delete sharing; each is checked for reparse attributes. The final handle's resolved path must match the inventory path. Multiple hard links, unsupported Unicode names and reparse points are refused. Volume serial, file index, exact last-write time, size and streamed SHA-256 must match the reviewed snapshot. Directory handles stay live through a `SetFileInformationByHandle(FileRenameInfo)` rename with replacement disabled. The destination chain is also pinned. Cross-volume moves fail without copy/delete fallback.

Files move into the application data directory's `quarantine` folder. They still occupy disk space. Intent is committed to SQLite before each move, then its outcome is recorded. WAL uses synchronous FULL. On history refresh while idle, pending/interrupted records are reconciled by matching identity and bytes at the original and stored locations. Ambiguous records are labelled conflict; reconciliation never moves files. Restore opens the stored object, verifies it, pins the original parent, and renames without replacement. Missing parents, changed bytes or occupied destinations fail visibly. No permanent purge or Recycle Bin interface exists.

The data directory has an exclusive process-lifetime file lock, preventing concurrent instances sharing a journal. Within the instance, preview, moves, restores, settings updates, scans and duplicate jobs share the operation guard. History reads can run while busy but reconciliation cannot. Cancellation is checked during approval/execution hashing and between items; a synchronous Windows call cannot be instantly interrupted. Restore currently verifies a single file without cancellation. Hostile memory-mapped writers, drivers and compromised accounts are not covered by the locking claims.

## Optional local setup

`setup_local_llm` shares the operation guard with scans and cleanup, requires `approved=true`, and runs on a blocking worker. The installer command uses fixed arguments and no shell. The Ollama pull request uses a fixed local endpoint and model. Streamed JSON status lines are capped at 8 KB; the model list is capped at 128 KB. Failed, truncated or cancelled streams never report readiness. Setup rechecks the installed model before the UI saves preferences. A stalled network read times out, and cancellation is checked between reads and stages. The server intentionally remains running. An existing Ollama server is reused without changing its configuration; a newly started server receives loopback binding and `OLLAMA_NO_CLOUD=1`. Setup verifies reachability and installed-model presence, not inference quality or hardware suitability. No installation happens from selecting an AI provider.

Cloud setup is a guide, with exact allowlisted links to official account/key/model pages. Credentials still use Windows Credential Manager, cloud consent remains explicit, and provider switches clear the previous provider's model name. Native select controls declare the active color scheme and option colors for readable Windows menus.

## Persistence and performance limits

Database schema creation is idempotent, with WAL and a busy timeout. Preferences use a single JSON row; inventories/history remain local. Schema-versioned migrations and retention controls are still required before future schema changes. Directory project-marker detection currently performs repeated metadata checks; million-file performance has not been benchmarked. Logical inventory size grows with scanned file count. There is no automatic retention or purge feature. Installed application inventory reads Windows uninstall registry entries, de-duplicates display names, and does not execute uninstallers or infer orphaned directories.

# Security policy

Version 0.1.0 is an experimental open-source utility, provided as is under the MIT license. Users choose actions and are responsible for their use and backups. Explicitly approved quarantine and restore are exposed; permanent deletion and arbitrary commands are not. Do not interpret estimates or AI text as proof a file is disposable.

## Reporting

Report vulnerabilities through [GitHub's private vulnerability reporting](https://github.com/colbster255/ClutterSift/security/advisories/new), which is enabled for this repository. If unavailable, open an issue requesting a private contact without including exploit details, personal paths, or secrets. No response-time guarantee is made.

## Trust boundaries and limitations

- File and directory names and AI responses are untrusted. React renders them as text. AI input contains opaque IDs, category and size only; typed goals are untrusted and may contain sensitive information.
- Serde denies unknown model output fields. Backend planning rejects unknown, ineligible, repeated IDs and oversized responses. Text explanations are display-only.
- User rules are additive to built-in system/application/personal-data protection. Known directories and project markers cannot enumerate all dependencies.
- Scans skip reparse points, symlinks, exclusions, network/device roots and the app's data directory. A malicious process can change path components during read-only traversal. Scanning is not a security sandbox. Duplicate detection and quarantine verification read file bytes locally.
- Quarantine opens an ordinary single-link file with write/delete sharing denied, pins ancestor directories, validates its opened path, and compares volume/file identity, exact last-write time, size and SHA-256 against a one-use expiring approval. The move uses `SetFileInformationByHandle` without replacement, never a path-based delete fallback. Unsupported volumes/permissions/locks fail per item. Hostile drivers, memory-mapped writers, compromised accounts and arbitrary modifications of the journal are outside the protection guarantee.
- Intent is committed before each move with SQLite WAL and synchronous FULL. Interrupted operations are reconciled from matching object identity and bytes. Ambiguous states require manual inspection. Restore refuses changed bytes, occupied original paths, missing parents and reparse/hard-link paths. No automatic permanent purge exists. Losing the database or recovery folder can prevent in-app recovery; preserve both.
- SQLite stores inventory paths and preferences in plaintext in the current user's app-local data directory. File ownership and OS account protections are relied upon. Do not share this database.
- API keys are held by Windows Credential Manager and transient process memory. Keys and HTTP response bodies are not logged. Cloud calls require saved consent. Goals are transmitted verbatim.
- Loopback Ollama is an external service controlled by its operator. Rejecting cloud-named models is not proof of offline operation; disable Ollama remote features and verify its configuration.
- Local setup requires an explicit install action. Rust invokes fixed Windows Package Manager arguments for `Ollama.Ollama`, starts the default user-installed Ollama executable when necessary, and requests only the fixed `qwen2.5:1.5b` model from a loopback endpoint. No model output supplies commands, arguments or download destinations. Windows Package Manager validates its installer through its package manifest; third-party software/model downloads are separate from file cleanup. The installer may finish after cancellation. Installation and models are not rolled back automatically. Setup links open the default browser through an exact HTTPS allowlist; arbitrary URLs and local paths are rejected.
- No silent elevation, telemetry, filesystem plugin, shell plugin, or frontend network permissions are enabled. This cannot protect against an already-compromised Windows account or webview/runtime vulnerability.
- Tauri, WebView2, SQLite and other dependencies need ongoing patching. Unsigned artifacts are not verified publisher identities. Code signing is not currently configured.

Use the Windows checklist to validate changes to file operations. Recycle Bin integration and disk reclamation remain deferred; no blanket safety or dependency-discovery guarantee is made.

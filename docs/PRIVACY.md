# Privacy

No analytics or telemetry are included. Scanning and deterministic guidance work without network calls. The frontend's CSP prevents direct external connections; optional model calls originate in Rust at fixed endpoints.

Explicit local setup contacts the Windows Package Manager source and Ollama's model registry to install software and download the selected fixed local model. No file inventory or typed cleanup goal is included in setup requests. A server started by ClutterSift uses `OLLAMA_NO_CLOUD=1`; an existing service retains its configuration. Downloads and installed software remain after cancellation, and the local service remains available after setup. Software/model licenses are linked before installation. Setup guide links open official pages in the user's default browser only when clicked.

| Data                                       | Local storage                                       | Sent to AI                              |
| ------------------------------------------ | --------------------------------------------------- | --------------------------------------- |
| Full file paths and local candidate IDs    | SQLite inventory                                    | Only IDs, never paths                   |
| Size and category                          | SQLite inventory                                    | Up to 200 eligible candidate records    |
| File contents                              | Approved files retained in local quarantine         | Never sent                              |
| File bytes for duplicate analysis / review | Read locally; review hashes stored in local journal | Never sent                              |
| Protection/exclusion rules                 | SQLite settings                                     | Never sent                              |
| Typed goal                                 | Transient application/process memory                | Verbatim for selected AI provider       |
| Model explanation / dry-run plan           | Displayed; previews stored locally                  | Provider already has its response       |
| OpenRouter key                             | Windows Credential Manager                          | Authorization header to OpenRouter only |

Offline mode performs no inference request. Ollama uses `127.0.0.1:11434` with redirects and proxies disabled, and cloud-named models are rejected. A separately configured local service can itself forward data; verify that Ollama is configured for local inference only. Cloud inference uses OpenRouter only after explicit saved opt-in. OpenRouter and its model providers have their own privacy terms and retention policies. Do not include sensitive filenames or personal details in a typed goal. There is no option to upload filenames, paths, or contents in this version.

Database location is the per-user Tauri app-local data directory for `org.cluttersift.desktop`. Windows may redirect that directory when launched from a packaged host; ClutterSift resolves the actual location at startup before deriving journal and recovery paths. It includes sensitive path metadata, object identities and content hashes; it is not encrypted. Approved files are moved into the adjacent `quarantine` folder on the same volume. Closing the app does not erase inventories or quarantine. Preserve this folder and database together, and restore wanted files before removing app data or uninstalling. Uninstall may leave data and credentials behind. There is no automatic purge or in-app permanent deletion. Dry-run reviews are not backups; quarantine does not protect against disk failure or external deletion.

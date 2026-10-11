# ClutterSift

An open-source Windows storage utility, with optional AI guidance and protections enforced in Rust.

**0.1.0 is an experimental utility beta.** Scanning, duplicate analysis, protection rules, AI planning, dry-run reviews, explicitly approved quarantine, and restore are implemented. Quarantine moves eligible files into local recovery storage; **it does not reclaim disk space**. Windows Recycle Bin integration and permanent deletion are unavailable.

MIT licensed, provided **as is, without warranty**. You decide which files to move and are responsible for using the utility and keeping backups. Open source allows inspection and modification; it does not guarantee that a recommendation is correct or a file is unnecessary.

![Cleaner interface, actual light-theme browser preview without native storage data](docs/screenshots/cleaner-light-browser-preview.png)

Screenshot: actual rendered browser preview of the interface. Native storage access is disabled in a browser; no sample scan results are shown.

## What works

- Tauri 2 desktop app with React/TypeScript, light/dark/system themes, and five focused screens. The Cleaner combines drive selection, category filters, file selection, and Analyze / Review cleanup actions in one place.
- Background local-drive/folder scanning, cancellation, progress events, error counts, SQLite inventory, category totals, and paged largest-file results.
- Analysis of installers, archives, logs, old downloads, large files, and development files. These categories do **not** imply removability.
- Opt-in staged duplicate analysis: equal size → first/last 8 KB SHA-256 → full SHA-256. Matches stay review-only.
- Built-in system, application, known personal-folder, application-data, and project protections; confirmed custom paths, filename globs, and scan exclusions.
- Local natural-language protection proposals for recognized folders and existing absolute paths, with confirmation before saving. Quote paths with spaces; custom and relocated locations need explicit paths.
- Offline deterministic guidance, optional loopback Ollama and opt-in OpenRouter integrations with strict response validation.
- Windows Credential Manager API-key storage. No file contents, filenames, or paths are transmitted to AI providers.
- Registry-based installed-app inventory with reported sizes where available.
- Per-item dry-run review and one-use, 15-minute approvals for quarantine; backend policy, Windows object identity, precise timestamp, size, and content-hash revalidation.
- Durable per-file recovery records, cancellation between files/during verification, and restore to the original path without overwriting.

Only old `.tmp` files **directly inside** the OS temporary folder, older than seven days and allowed by current policy, are preview candidates. This does not certify that they are safe to delete.

## Run on Windows

Requirements: Windows 10/11 x64 with Microsoft Edge WebView2 Runtime, Node.js 24 LTS, stable Rust MSVC, and Visual Studio 2022 Build Tools with Desktop development with C++ and a Windows SDK.

```powershell
npm ci
npm run tauri -- dev
```

Start with a synthetic folder containing copies of test files. In **Cleaner**, choose a local drive, folder, or individual file and click **Analyze**. Explore categories and select files, then click **Review cleanup**. A separate checkbox and approval button authorize quarantine. **Recovery** shows outcomes and requires confirmation to restore. **Protected items** requires confirmation before saving rule changes. Settings need **Save preferences** to persist provider and theme changes. Only one instance can use the same application data directory at a time.

Quarantine is stored beside the SQLite database in the app-local `quarantine` folder. A move must stay on the same volume; unsupported or locked files are left in place. There is no copy/delete fallback, automatic purge, or in-app permanent deletion. Keep the journal and quarantine folder together and restore files before deleting app data or uninstalling. Missing original directories, changed recovery files, hard links, and reparse points stop automatic restoration. Quarantine is not a backup against disk failure or external deletion.

```powershell
npm run check
cargo test --manifest-path src-tauri/Cargo.toml --locked
npm run desktop:build
```

The NSIS installer is written to `src-tauri/target/release/bundle/nsis/`. It installs per-user without silent elevation. Unsigned builds may trigger Windows reputation warnings. Do not bypass warnings without verifying the source and artifact.

Use `desktop:build` for distributable artifacts. It remaps home/workspace paths in Rust diagnostics and rejects executable/installer output containing those original paths in UTF-8 or UTF-16. Raw Tauri builds can embed private development paths. The wrapper supports native optimized builds; custom profiles/targets require their own artifact validation. Remapping does not configure code signing.

After building, `npm run test:native` runs a Windows/WebView2 integration test with synthetic fixtures. It launches its own app process with a temporary loopback debugging port, exercises real IPC, scanning, duplicates, protection proposals, and an approved quarantine/restore round trip on a uniquely created old `.tmp` file. It closes that process and removes only its fixtures. It does not change saved preferences or call cloud AI. The port is not enabled in normal app launches. Close other app instances first. An interactive Windows session is required; this test is not part of headless CI. Synthetic history records remain in the local journal.

## Optional AI

Scanning needs no AI. In **Settings → AI provider → Ollama**, choose **Install Ollama & download model** for automated setup. It uses Windows Package Manager to install Ollama for the current user if needed, starts a loopback server if none is running, and downloads `qwen2.5:1.5b` (about 1 GB). Allow at least 4 GB for Ollama plus model storage. The button accepts the linked software/model terms. Setup verifies the installed model before saving Ollama preferences; it does not run an inference test. Progress, cancellation, a read-only connection/model check, and manual fallback instructions are included. Cancellation retains installed software and model data; a separate installer may finish. The server remains available after setup. No setup runs merely from selecting a provider.

The endpoint is fixed at `http://127.0.0.1:11434`; redirects and HTTP proxies are disabled. A server started by ClutterSift receives `OLLAMA_NO_CLOUD=1`; an already-running server keeps its existing configuration. For an existing service, set that user environment variable and restart Ollama to disable its cloud features. Cloud-named models are rejected. A loopback endpoint alone cannot prove a separately configured service is offline. See the [Ollama Windows instructions](https://docs.ollama.com/windows), [cloud configuration](https://docs.ollama.com/faq#how-do-i-disable-ollama-cloud-features), and [model information/license](https://ollama.com/library/qwen2.5:1.5b).

For OpenRouter, the Settings panel guides you through account/credits, [creating a limited API key](https://openrouter.ai/settings/keys), choosing a model with [structured-output support](https://openrouter.ai/docs/guides/features/structured-outputs), saving the key in Windows Credential Manager, explicit consent, and a first request. Provider changes clear incompatible model names and reset cloud consent. Your **typed goal is transmitted verbatim**: do not include private information. Provider failures are surfaced without affecting scanning. Recommendations reference at most 200 eligible candidate IDs, never executable paths or commands.

## Limits and release status

Disk-space reclamation, Recycle Bin integration, application-uninstall actions, orphan detection, saved protection profiles, empty-directory cleanup, largest-folder aggregation, and persistent metadata-reuse caching are not implemented. Duplicate analysis is capped at 20,000 files and size groups of 2–1,000; files outside the cap are not checked. No million-file performance or HDD throughput claims have been validated. Scan error messages are aggregated as counts; cleanup outcomes include per-item errors. Non-Unicode paths are skipped rather than stored as ambiguous replacement characters; execution also rejects paths it cannot match to the opened object.

Windows paths can change during read-only scans. The move layer separately opens files without sharing write/delete access, pins their directory chain, rejects reparse points and multiple hard links, verifies the opened path, and renames the verified handle without replacement. This has synthetic test coverage, not a guarantee against every Windows filesystem/filter or adversarial condition. Protection cannot identify every dependency, relocated personal folder, or development environment. Local inventories and recovery metadata are not encrypted.

See [architecture](docs/ARCHITECTURE.md), [privacy](docs/PRIVACY.md), [security limitations](SECURITY.md), [Windows testing](docs/WINDOWS_TESTING.md), [optional signing](docs/SIGNING.md), and [release readiness](docs/RELEASE_READINESS.md). No telemetry, no analytics, no fabricated benchmarks. Native Windows screenshots and the manual integration checklist are still needed before a cleaner release.

## Contribute

MIT licensed. Read [CONTRIBUTING.md](CONTRIBUTING.md) and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). Never test cleanup against real user files. Do not submit private inventories, logs, API keys, or personal paths in issues.

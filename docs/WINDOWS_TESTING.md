# Manual Windows integration checklist

Use a disposable VM and synthetic data only. Never test cleanup against a real profile. Record Windows/WebView2 version, app version, exact fixture and observed results. Automated unit tests do not replace these checks.

- [ ] Install/uninstall unsigned NSIS per-user build on Windows 10 and Windows 11 x64 without requesting elevation.
- [ ] Launch the packaged build; verify all screens, keyboard navigation, scrolling, dark/light/system themes, reduced motion, DPI scaling (100–200%), empty states, and error messages.
- [ ] Scan a synthetic folder with temporary files, archives, logs, installers, large/sparse files, and project markers. Compare file counts and logical sizes with fixture definitions.
- [ ] Scan a local drive read-only; confirm access-denied entries, long paths, unreadable/removed volumes, root paths and non-Unicode names are handled without crashes. Network and device roots must be rejected.
- [ ] Make junctions, symbolic links and mount points inside fixtures. Verify they are skipped. Mutate parent directories during scanning and document residual read-only traversal races.
- [ ] Cancel a scan and duplicate pass. Confirm the UI stays responsive, partial results are labelled, and a new scan can start.
- [ ] Restart during scanning. Verify old status changes to interrupted and partial inventory cannot be previewed as complete.
- [ ] Confirm Windows, Program Files, ProgramData, personal folders and detected projects remain protected; test similarly prefixed sibling names and case differences.
- [ ] Confirm each saved path/glob/exclusion must be reviewed and confirmed, persists over restart, and is enforced by preview revalidation.
- [ ] Compare exact and near-duplicate fixtures, including same size/different tails, large files, files changing during hashing and hard links. No duplicate should be auto-selected for cleanup.
- [ ] Simulate SQLite write failures/full disk and access-denied files. Verify visible failure with no success claim.
- [ ] Use a local Ollama model. Stop the service, request an invalid model, return malformed/oversized output, unknown IDs and injected commands. Verify failure and offline scanning availability.
- [ ] With test credentials, inspect OpenRouter payload: only goal plus IDs/categories/sizes. Verify no call before saved consent and no keys in logs/UI/history.
- [ ] Confirm keys are saved/removed in Windows Credential Manager and settings/database contain no key.
- [ ] Compare installed-app entries against Windows settings; treat missing/approximate sizes as reported metadata only.
- [ ] Verify preview leaves files unchanged; quarantine requires a separate approval, expires after 15 minutes, cannot be replayed, and shows per-item outcomes.
- [ ] Independently audit every packaged dependency, license, installer behavior and embedded resource before public distribution.

## File-operation verification (automated synthetic coverage plus manual checks)

- [ ] Security review of handle identity, reparse/hard-link race resistance and ancestor locking.
- [ ] Immediate policy/identity revalidation, explicit final approval bound to exact items, expiration and replay protection.
- [ ] Verify same-volume quarantine and restore, locked files, changed bytes with preserved timestamps, hard links, junctions, unsupported volumes and disk/SQLite failures, with **no permanent deletion fallback**. Recycle Bin integration remains absent.
- [ ] Durable per-item journaling, safe partial failures, cancel/restart behavior, no silent elevation or permission bypass.
- [ ] Item-specific restoration, naming conflicts, missing original folders and honest unsupported-recovery states.
- [ ] Fault injection between each validation and action; unrelated files must remain untouched.
- [ ] Launch two instances against the same data directory; the second must fail to acquire its lifetime lock.
- [ ] Cancel during preview/execution hashing and between files; completed moves remain restorable.
- [ ] Kill the child test process after intent commit and after rename, and inspect reconciliation. Never run this against real files.
- [ ] Confirm quarantine still consumes disk space; removing app data or quarantine externally can lose recovery files. Restore wanted files first.

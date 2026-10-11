# Local validation — 2026-10-11

The repository initially contained only an empty Git repository, with no existing application or commits. No existing code was removed. A stable Rust MSVC toolchain was installed to build and test the application using the existing Visual Studio C++ tools.

## Results

| Check                                     | Result                                                                                                                                |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| TypeScript and Vite production build      | Passed                                                                                                                                |
| Frontend workflow tests                   | 15 passed                                                                                                                             |
| Rust tests                                | 34 passed, 1 explicitly ignored                                                                                                       |
| Rust formatting                           | Passed                                                                                                                                |
| Prettier formatting                       | Passed                                                                                                                                |
| Clippy with warnings denied               | Passed                                                                                                                                |
| Windows optimized build and NSIS bundle   | Passed: dropdowns/AI setup build; executable and NSIS development-path checks passed                                                  |
| Native executable launch                  | Passed: process stayed alive and exposed a ClutterSift window                                                                         |
| Browser rendering check                   | Passed: five screens, light/dark themes, minimum desktop width without horizontal overflow, zero renderer exceptions                  |
| Browser screenshots                       | Actual rendered previews captured; no native storage data or fabricated scan results                                                  |
| npm dependency audit                      | No vulnerabilities reported                                                                                                           |
| RustSec dependency audit                  | No vulnerability errors; two transitive warnings retained, described below                                                            |
| Gitleaks source snapshot                  | Passed source snapshot including dropdowns and AI setup                                                                               |
| Native WebView2 synthetic end-to-end test | Passed: real IPC, scan/restore fixtures, dropdowns, local status, unapproved install rejection, cloud guide; saved settings unchanged |

Rust tests exercise component/case path comparisons, canonical-path revalidation, built-in/custom protections, glob validation, narrow temporary-file eligibility, exclusions, project detection, invalid/missing paths, scan cancellation, staged duplicate hashing, file changes, strict and oversized AI output, unknown/repeated/ineligible IDs, cloud-consent gates, offline guidance, SQLite persistence/pagination, local protection proposals and read-only preview revalidation. File-operation tests cover expiring/replayed approvals, explicit approval rejection, current protections, locked files, hard links, pinned parent directories, Unicode names and non-Unicode inventory rejection, SHA-256 tampering with preserved timestamps, per-item failures/cancellation, no-overwrite restore, missing original parents, interrupted records, and 32 variable-length Windows rename buffers. UI tests exercise explicit selection/preview, protected selections, error states, confirmed custom rules and natural-language protection proposals, cloud consent remaining off, separate quarantine/restore approval, and uncertain outcomes after execution errors.

The ignored test requires Windows symlink privilege and must be run deliberately in a disposable VM. Quarantine/restore have synthetic coverage. Junction/mount-point races and access-control/disk-failure injection still require manual testing.

## Dependency warnings

`cargo audit` reported `RUSTSEC-2024-0370` (unmaintained `proc-macro-error` 1.0.4) and `RUSTSEC-2024-0429` (unsound `glib` 0.18.5 iterator implementation). These transitive lockfile warnings were not suppressed. `cargo tree --target x86_64-pc-windows-msvc -i` for each crate reported no dependency tree: neither is reachable for the Windows target. Review non-Windows dependencies before supporting other platforms; a successful exit code is not a claim that all dependencies are risk-free. The npm audit initially found vulnerable development test tooling; it was updated and the audit then reported zero vulnerabilities.

## Remaining verification and utility work

- The unsigned NSIS installer was built and checked for embedded development paths. Installing/uninstalling it in disposable Windows VMs remains unverified.
- The full manual Windows 10/11 integration matrix, installer installation/uninstallation, DPI/accessibility matrix and fault injection.
- Live Ollama/OpenRouter inference, secure credential-store round trips and cloud payload inspection with actual test credentials.
- Full Ollama installation through Windows Package Manager, model-weight download, installer cancellation and restart/resume in a disposable Windows VM. These operations have not been executed on the development machine.
- Recycle Bin integration and disk reclamation are absent. Quarantine and restore are implemented; independent adversarial review and the full VM fault-injection matrix remain incomplete.
- Million-file/HDD performance, metadata cache reuse, detailed per-path scan errors, orphan detection, folder aggregation, empty-directory analysis and saved protection profiles.
- GitHub-hosted CI, tag release workflow, signing and public publishing. No repository remote or GitHub release was created.
- Third-party license-obligation review and private maintainer/reporting contacts.

The earlier read-only native smoke test passed against the release app and WebView2: native IPC, two-file scan, staged duplicate hashing, rejection of an ineligible preview, disabled removal, local protection proposal/cancellation, offline guidance, unchanged fixtures and zero renderer exceptions. Its temporary debugging port was limited to the child test process and closed afterward. The synthetic fixture was removed; its local scan-history record remains. Native UI helper automation initially timed out; this independent WebView2 test then verified the actual native flow.

The redesigned Cleaner has five screens, a drive/path selector, category filters and file results in one workspace, and persistent Analyze / Review cleanup controls. Browser checks covered light/dark themes, every screen, and the 900-pixel minimum desktop width without horizontal overflow or renderer errors. Updated screenshots show actual browser rendering without invented inventories. The final optimized executable passed the full native WebView2 scan/duplicate/approval/quarantine/restore test. That test exposed Windows packaged-host LocalAppData redirection; startup now canonicalizes the actual data folder before deriving database and recovery paths, while directory pinning remains strict. The synthetic file was restored with its original bytes and then removed by the fixture teardown. No renderer errors occurred.

No real user files were cleaned, moved or deleted. Tests move/restore only synthetic fixtures. Dry-run records are not backups, and no storage-recovery claim is made. An automatic approval review blocked removal of unused generated mobile icon assets; those assets remain.

## Dropdowns and AI setup update

Native select controls now declare light/dark color schemes and explicit option colors. Browser verification exercised provider selection by keyboard, theme/provider/category/rule choices, the local/cloud guides and the 900-pixel minimum width, with no horizontal overflow or renderer errors. Screenshots of expanded menus and setup panels are retained in ignored local artifacts. Frontend tests verify that choosing Ollama does not invoke installation, verified setup saves the local model with cloud consent off, incomplete setup leaves preferences unsaved, provider changes clear incompatible models, and setup links use the native allowlisted opener. Rust tests cover cancelled, invalid, truncated and oversized download-status streams and rejection of untrusted setup URLs. A regression test also covers completion arriving before an older initial scan response: final progress is retained and the result list refreshes independently of the scanning flag. Full live installation/model download and cloud inference remain unverified.

The final updated executable passed native WebView2 verification of theme/provider dropdown selection, the read-only local Ollama status command, rejection of `approved=false` setup, the cloud guide and unchecked consent, and unchanged saved settings. The existing synthetic scan, duplicate, approved quarantine and restore checks also passed. No installer or model-weight download was executed. The optimized executable and 4.05 MiB unsigned NSIS bundle passed development-path checks.

# 0.1.0 utility verification checklist

This is an **unreleased open-source utility beta**, provided as is under MIT. Users choose actions and are responsible for their use and backups. This checklist records evidence and remaining work, rather than product certification. A tag workflow builds a prerelease draft; publishing remains a maintainer decision.

## Implemented

- [x] Native app foundation and five focused UI screens.
- [x] AI-independent read-only scans with progress and cancellation.
- [x] SQLite inventory, settings and dry-run history.
- [x] Backend protection/exclusion policy and per-item preview revalidation.
- [x] Explicit, expiring, one-use quarantine approval; Windows handle moves; durable per-item journal and conflict-aware restore.
- [x] Structured, bounded optional Ollama/OpenRouter clients with cloud opt-in and credential-store support.
- [x] Explicit automated Ollama setup, progress and connection/model checks; guided OpenRouter setup.
- [x] Paged file analysis, category summaries, optional staged duplicate hashes, registry application inventory.
- [x] Public repository documentation and CI/release workflow definitions.

## Verification still to complete

- [ ] Windows installer built and installation tested on supported Windows versions.
- [x] Native launch and synthetic end-to-end scan verified locally.
- [ ] Rust and frontend tests, formatting, lint and dependency/secret checks pass on the release commit.
- [ ] Live Ollama and OpenRouter integration tested without private data.
- [ ] Full Windows Package Manager installation, model download and cancellation tested in a disposable Windows VM.
- [ ] Full manual checklist executed in a disposable Windows VM.
- [ ] Dependency license notices reviewed and packaged as required.
- [x] Actual browser-rendered screenshots captured and explicitly labelled; no native data or sample results claimed.
- [ ] Private security-reporting and community contact configured in GitHub.
- [ ] Code signing provisioned if trusted-publisher distribution is desired (currently absent).
- [ ] No absolute personal paths, inventories, credentials or generated build artifacts committed.

## Remaining utility work

Quarantine and restore are implemented, but quarantine does not reclaim space. Windows Recycle Bin integration, disk reclamation and permanent deletion are absent. The handle layer has synthetic tests; the full Windows fault-injection/VM matrix and independent review remain incomplete. Also missing: largest-folder aggregation, empty-directory analysis/cleanup, evidence-based orphan detection, saved protection profiles, detailed scan errors, versioned migrations/retention controls, lossless non-Unicode inventory paths, million-file scaling tests and cached metadata reuse. Local natural-language protection proposals use a limited parser; arbitrary dependency discovery is unsupported.

Validation results for the current local implementation are recorded in `VALIDATION.md` after checks complete. Workflow files have not run on GitHub until pushed; local success does not imply hosted CI or release publication.

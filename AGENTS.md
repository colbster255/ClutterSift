# Project conventions

ClutterSift is a community-maintained, open-source Windows utility, not a commercial product. Keep implementation and documentation practical. Users choose operations and take responsibility for their use and backups; the MIT license supplies the as-is/no-warranty terms. Do not invent product certification, service guarantees, or product launch processes.

Keep the utility's behavior explicit: scanning is read-only, previews change no files, moves require user approval, quarantine retains bytes and does not free disk space, and restore never overwrites. User responsibility does not replace accurate reporting or backend enforcement of selections and protection rules.

Test filesystem mutations only against synthetic fixtures created by the test. Never clean arbitrary real user files during development. Do not hide incomplete or untested features. Follow README development commands and record material verification in docs/VALIDATION.md.

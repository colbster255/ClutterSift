# Contributing

Use Node 24 and stable Rust MSVC on Windows, plus the C++ tools and WebView2 prerequisites in the README. Run `npm ci`, then `npm run tauri -- dev`.

Before a pull request:

```powershell
npm run format
npm run check
cargo fmt --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --locked
npm audit --audit-level=moderate
cargo audit --file src-tauri/Cargo.lock
```

Install `cargo-audit` if needed (`cargo install cargo-audit --locked`). CI also checks secrets with Gitleaks. Use temporary synthetic fixtures; never point cleanup tests at a real user profile. Keep filesystem decisions in Rust. React and model responses cannot authorize actions. Unknown model fields and IDs must fail closed.

Changes enabling deletion need independent security review, handle-based object identity validation, reparse-race tests, reliable recoverable-only Windows operations, per-item approval, operation journaling, partial-failure and cancellation testing, and a verified restore story. Path-based metadata checks alone are insufficient. Do not turn dry-run records into purported recovery backups.

Describe the problem, user-visible behavior, tests run, and untested Windows scenarios. Do not include personal inventories or credentials. Dependency licenses must remain compatible with MIT distribution. Review the release checklist before cutting a tag.

# AGENTS.md

Guidance for coding agents working on this repository. See
[AGENTS.RELEASE.md](AGENTS.RELEASE.md) for release automation context
(GitHub Actions workflow, current status, open points).

## Project

`1folder` is a macOS background agent (Rust) that watches configurable folders
(Downloads, Desktop, …), avoids duplicates via SHA-256 content hashes, classifies
files by rules (extension, file-name keywords, PDF text of the first 3 pages) and
moves everything into one structured target folder. Notifies via the macOS
Notification Center.

## Commands

```sh
cargo check          # fast compile check
cargo test           # unit tests
cargo clippy --all-targets
cargo build --release
```

There is no CI pipeline yet; run `cargo test` and `cargo clippy` before committing.

## Layout

- `src/main.rs` – CLI (`--config`, `--once`, `--dry-run`, `--init-config`)
- `src/config.rs` – TOML config, tilde expansion; `config.example.toml` is the
  default config (included at compile time, keep both in sync)
- `src/watch.rs` – filesystem watcher (notify), debounce, close/rename handling
- `src/organize.rs` – pipeline: stability check → hash → classify → dedupe → move;
  `HashIndex` caches content hashes of the target folder
- `src/i18n.rs` – `Language` and `Messages` tables (EN/DE) for user-facing strings
- `src/classify.rs` – rule matching and PDF text extraction (lopdf)
- `src/journal.rs` – JSONL action log in the target folder (the history that
  powers `--undo`)
- `src/report.rs` – optional `--report <FILE>` JSONL operation log
- `src/reorganize.rs` – `--reorganize` re-applies current rules to files already
  in the target folder, journaling moves as `reclassified` (undoable)
- `src/undo.rs` – `--undo [N|all]` with `--within`/`--src-prefix` filters;
  restores journal entries and marks them undone
- `src/notify.rs` – macOS notifications via `osascript`, log-only elsewhere
- `src/tags.rs` – Finder tags via `com.apple.metadata:_kMDItemUserTags` xattr
  (binary plist), no-op on non-macOS; configured in `[tags]`, per-rule `color`

## Conventions

- Language: code, identifiers and all documentation (README, AGENTS.md, config
  comments) in English. User-facing log and notification strings go through
  `src/i18n.rs` (`Messages` templates, filled with `i18n::fmt`); the default
  language is English, German is available via `[general] language = "de"` in the
  config. Add new user-facing strings to both `EN` and `DE` tables.
- No code comments unless strictly necessary; prefer self-explanatory names.
- Keep changes minimal and focused; add/extend unit tests for behavior changes
  (tests use `tempfile`, no filesystem assumptions outside the temp dir).
- Never delete user files — move only, and keep the journal append-only.
- Commits are authored by the repository owner (repo-local git config:
  Kristof Korwisi); the coding agent adds itself as co-author via the trailer
  `Co-authored-by: Vibe Code <vibe@users.noreply.github.com>`. Other AI tools
  use their own `Co-authored-by` trailer instead.
- `hooks/commit-msg` appends `Co-authored-by: korwisi <korwisi@users.noreply.github.com>`
  to commits authored by anyone else (e.g. coding agents committing under their
  own identity) and skips the trailer when the author already is the repository
  owner. Activate it after cloning with `git config core.hooksPath hooks`.
- macOS-only behavior (notifications) must stay `#[cfg]`-gated so `cargo check`
  and tests pass on Linux.

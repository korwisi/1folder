# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- macOS Finder tags: sorted files are tagged with their category (e.g. `Papers`,
  `Rechnungen`), optionally colored per rule (`rules[].color`, Finder color index
  1–7) and with the root tag `1folder`; configured in the new `[tags]` section.
  Tags make files findable via the Finder sidebar, Spotlight (`tag:Papers`) and
  Smart Folders, even after manually moving files. No-op on non-macOS. (#2)

## [0.1.0] - 2026-11-13

### Added
- Background agent that watches configurable folders (Downloads, Desktop, …) and
  consolidates everything into one structured target folder
- Duplicate prevention via SHA-256 content hashes, detecting duplicates even under
  different file names; configurable handling: `skip` (default), `keep_both`,
  `move_to_duplicates`
- Thematic sorting via ordered TOML rules: file extension, file-name keywords and
  content analysis of the first 3 PDF pages (e.g. papers vs. forms)
- Download-safe processing: browser temp files (`*.crdownload`, `*.part`, Safari
  `*.download` bundles) are ignored; files are picked up on close/rename events
  plus a size/mtime stability check
- macOS Notification Center notifications for every action
- Append-only JSONL journal in the target folder; `--dry-run` and `--once` modes
- Localized user-facing log and notification strings: English default, German via
  `[general] language = "de"`

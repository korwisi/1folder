# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `--report <FILE>`: appends one JSON line per operation (action, source,
  destination, category, matching rule or folder-classification reason,
  dry-run flag); useful with `--dry-run` to preview and audit sorting.
- `--undo [N|all]` with `--within <duration>` and `--src-prefix <dir>` filters:
  restores files and folders moved by 1folder back to their original locations
  (original structure is recreated), based on the journal. Undone items are
  marked in the journal, collisions at the original location are renamed, and
  `--dry-run` previews the restoration.
- Folder handling (`[folders]` section, default `mode = "move_as_unit"`): each
  top-level folder of a watched directory is classified and moved as one unit,
  preserving its internal structure. Classification order: rules with the new
  `folder_keywords` condition → project markers (`.git`, `package.json`,
  `Cargo.toml`, …) → homogeneous content (an images-only folder goes to
  `Bilder`) → mixed content goes to `folders.subdir` (default `Projekte`).
  macOS bundles (`.app`, `.bundle`, `.framework`) are always moved as units;
  Safari `.download` bundles, empty and hidden folders are left alone.
  `folders.mode = "ignore"` restores the previous per-file sorting.
- `folders.markers`: user-defined project markers (e.g. `.decker` for decker
  presentation projects) detected in addition to the built-in list; folders
  containing them are moved as units to `folders.subdir`.

- `general.preserve_source_paths` option to keep the source folder context of
  sorted files: `structure` recreates the relative path from the watched folder
  under the category subfolder, `prefix` prepends the parent folder name to the
  file name (e.g. `Abschlussfeier - foto.jpg`); default `off` keeps the previous
  flat behavior.
- Disc images (`iso`, `img`, `bin`) are sorted into `Archive` and camera RAW
  formats (`cr2`, `cr3`, `nef`, `arw`, `dng`, `raf`, `orf`, `rw2`) into `Bilder`
  in the default rules.

### Fixed
- `[dedupe]` missing from the config no longer yields an empty
  `duplicates_subdir` (would have moved duplicates to the target root).

## [0.2.1] - 2026-11-13

### Fixed
- Compilation on macOS: encode Finder tag plists via `plist::to_writer_binary`
  (`Value::to_binary` does not exist in `plist` 1.x). The tagged `v0.2.0` did
  not build on macOS. (#2)

## [0.2.0] - 2026-11-13

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

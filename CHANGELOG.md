# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.6.0] - 2026-11-27

### Added
- Default document classification now includes `.docm`, `.potx`, and `.pub`.
- Default 3D classification now includes Blender `.blend` and FBX `.fbx` files.
- Executable and binary formats now use a dedicated `Binaries` category.
- Private keys and certificates remain in the fallback category; `.key` files are
  distinguished by private-key content markers so Keynote presentations are still
  classified as documents.
- Added an ignored direct large-target hash-index refresh benchmark.
- Added `--rename-category OLD NEW` for end-to-end category renaming.
- `--reorganize` now recursively reclassifies nested files while preserving
  their relative subdirectories.

## [0.5.0] - 2026-11-24

### Added
- Pending-operation recovery state for moves and undo operations, so interrupted
  filesystem and journal work can be reconciled safely on the next run.
- Fault-injection unit coverage for cross-filesystem copy, rename, source removal,
  and journal append failures.

### Fixed
- Cross-filesystem file and directory moves now copy to a temporary destination,
  verify the copy, publish it with a rename, and only then remove the source.
- Dry-run, category subdirectory validation, tool-owned index files, stability
  timeout handling, watched-directory enumeration, and malformed journal handling
  are now enforced consistently across CLI and watch workflows.
- Release workflow `v0.5.0` run 45422234588 was verified end to end: the
  universal macOS archive was uploaded, downloaded, extracted, installed, and
  confirmed to contain both `x86_64` and `arm64` support.

## [0.4.1] - 2026-11-16

### Added
- **Persistent SHA-256 index**: target folder now caches file hashes in
  `.1folder_index.json`, eliminating the need to rehash all files on startup.
  The index stores path, size, modification time, and hash for each file; hashes
  are reused only when file metadata matches, and new/changed files are rehashed.
  On first run with a large collection, indexing can take several minutes; all
  subsequent runs load the index from disk and respond immediately to changes.
  Index reconciliation handles missing files automatically, and atomic writes
  (temp file + rename) prevent corruption from manual changes or interruptions.

### Changed
- Persistent index now skips `.DS_Store` and other system metadata files to keep
  index size minimal and avoid indexing macOS-specific cruft.

### Fixed
- README now documents that the first run with a large target folder can take
  several minutes during initial indexing; clarifies that this is a one-time
  cost and subsequent runs are much faster.

## [0.4.0] - 2026-11-16

### Added
- **Interactive language selection**: `--init-config` now prompts users to choose
  between German and English during first setup, automatically configuring folder
  names and UI language accordingly. English users no longer need to manually edit
  the config to switch from the German default.
- **Universal binary support for Apple Silicon**: new `build-universal.sh` script
  builds a single Mach-O universal binary combining both x86_64 (Intel) and
  aarch64 (Apple Silicon) architectures using `lipo`. Enables distributing one
  asset that runs natively on both Intel and Apple Silicon Macs. See README for
  setup requirements and build instructions.

### Fixed
- Clippy linting warning: replaced `assert_eq!(x, true)` with `assert!(x)` in
  language tests for better idiomatic Rust.
- Infinite loop in `free_destination()`: collision handling now uses a bounded
  loop (10,000 iterations max) with a timestamp-based fallback filename
  (`collision-{timestamp_millis}.ext`) to ensure termination and prevent panics
  on pathological filesystem states.
- HashIndex state loss during duplicate handling: the deduplication index is now
  properly passed through `handle_duplicate()` instead of creating temporary
  instances, ensuring that new files added during duplicate processing are
  tracked correctly to avoid re-detecting them on the next run.

## [0.3.0] - 2026-11-16

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
  `Bilder`) → mixed content goes to `folders.subdir` (default `Sammlungen`).
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
- New default rules: 3D printing (`3D-Druck`: `stl`, `scad`, `3mf`, `gcode`,
  `curaprofile`, `ipt`, `f3d`, `obj`, `step`, `stp`), videos (`Videos`) and
  audio (`Audio`), plus a PDF catch-all into `Dokumente` after the keyword
  rules.
- Default `folders.subdir` renamed from `Projekte` to `Sammlungen`: the bucket
  mostly holds mixed-content folders (model/font/app downloads), not only
  projects. Existing configs keep their own `subdir` value.

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

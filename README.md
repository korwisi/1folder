# 1folder

A small macOS background agent (Rust) that keeps **one folder** organized: it watches
folders such as *Downloads*, *Desktop* or *Documents*, avoids duplicates, analyzes
documents and moves everything into a single, structured target folder.

## Features

- **Watch one or more folders** (configurable, e.g. Downloads, Desktop, Documents) and
  consolidate everything into a single target folder.
- **Duplicate prevention** via SHA-256 content hashes. On a duplicate you choose:
  skip the file (default), keep both (`Name (2).ext`), or move it to a `Duplikate`
  folder.
- **Thematic sorting** through configurable rules: file extension, file-name keywords,
  and content analysis of the first PDF pages (e.g. detect scientific papers via
  *abstract*, *DOI*, *arXiv* or forms via *Formular*, *Antrag*).
- **macOS Notification Center** notifications for every action (moves, skipped
  duplicates).
- **Finder tags**: sorted files are tagged with their category (optionally colored
  per rule and with a common root tag), making them findable via the Finder
  sidebar, Spotlight (`tag:Papers`) and Smart Folders — even after manually
  moving files elsewhere.
- **Safe by design**: files are only moved, never deleted; every action is appended to
  a JSONL journal (`<target>/.1folder-journal.jsonl`); a `--dry-run` mode shows what
  would happen.
- **Download-safe**: files are only processed once their size and modification time
  have been stable for a configurable period. Browser temp files (`*.crdownload`,
  `*.part`, Safari's `*.download` bundles) are ignored, and a file is picked up as
  soon as its handle is closed or it is renamed to its final name.

## Build & setup

### Prerequisites

- Rust toolchain (edition 2021), e.g. via [rustup](https://rustup.rs):
  `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- macOS for the full feature set (folder watching works everywhere;
  Notification Center notifications fall back to log output on other platforms)

### Build and install

```sh
git clone https://github.com/korwisi/1folder
cd 1folder
cargo build --release
# binary: target/release/onefolder
sudo cp target/release/onefolder /usr/local/bin/
```

### Setup

Continue with the quick start below: initialize the config, review it, preview with
`--dry-run`, then run `--once` or start the background watch mode. To start
automatically at login, install the LaunchAgent described under
[Running as a background agent](#running-as-a-background-agent-launchagent) and grant
the required permissions under [macOS permissions (TCC)](#macos-permissions-tcc).

## Quick start

```sh
# 1. Write the default config to ~/Library/Application Support/1folder/config.toml
onefolder --init-config

# 2. Adjust target_dir, watch.dirs and rules to your liking
$EDITOR ~/Library/Application\ Support/1folder/config.toml

# 3. Preview what would happen
onefolder --once --dry-run

# 4. Sort existing files once
onefolder --once

# 5. Run continuously in the background
onefolder
```

## Configuration

`onefolder --init-config` writes [`config.example.toml`](config.example.toml) to
`~/Library/Application Support/1folder/config.toml`. Key settings:

| Setting | Meaning |
| --- | --- |
| `general.language` | Language for log output and notifications: `en` (default) or `de`. |
| `general.preserve_source_paths` | How to keep the source folder context: `off` (default), `structure` or `prefix`. |
| `target_dir` | The one folder everything is consolidated into. |
| `watch.dirs` | Folders to watch (tilde-expanded). |
| `watch.stability_wait_ms` | How long a file must stay unchanged before it is processed. |
| `watch.debounce_ms` | Quiet period after the last filesystem event. |
| `dedupe.on_duplicate` | `skip`, `keep_both` or `move_to_duplicates`. |
| `folders.mode` | Folder handling in watched folders: `move_as_unit` (default) or `ignore`. |
| `folders.subdir` | Category for folders with project markers or mixed content (default `Projekte`). |
| `notify.enabled` | Toggle Notification Center notifications. |
| `tags.enabled` | Apply macOS Finder tags to sorted files (no-op elsewhere). |
| `tags.root_tag` | Extra tag on every sorted file; empty disables it. |
| `rules[].color` | Optional Finder tag color: 1=gray 2=green 3=purple 4=blue 5=yellow 6=red 7=orange. |
| `rules` | Ordered list; first match wins. All specified conditions must match. |
| `fallback_subdir` | Subfolder for files that match no rule. |

A rule matches when **all** specified conditions are met:

- `extensions` – file extension, case-insensitive
- `filename_keywords` – substrings of the file name, case-insensitive
- `pdf_keywords` – substrings of the text extracted from the first 3 PDF pages

Example: keep scientific papers before generic PDF rules by listing the `Papers`
rule first.

### Preserving the source folder context

By default a file like `~/Desktop/alter-desktop/bilder/Abschlussfeier/foto.jpg`
ends up flat as `Bilder/foto.jpg` and the folder context is lost. With
`general.preserve_source_paths` you can keep it:

- `structure` recreates the relative path from the watched folder under the
  category: `Bilder/alter-desktop/bilder/Abschlussfeier/foto.jpg`
- `prefix` keeps the category flat but prepends the parent folder name to the
  file name: `Bilder/Abschlussfeier - foto.jpg`
- `off` (default) ignores the source path

### Folders are moved as units

By default (`folders.mode = "move_as_unit"`) each top-level folder of a watched
directory is classified and moved as one unit — internal structure is preserved.
Classification per folder, first match wins:

1. a rule whose `folder_keywords` match the folder name
2. project markers (`.git`, `package.json`, `Cargo.toml`, `pyproject.toml`,
   `go.mod`, `Makefile`, …) → `folders.subdir` (default `Projekte`)
3. homogeneous content — if every file in the folder falls into the same
   category, the folder goes there (an images-only folder → `Bilder`)
4. mixed content (e.g. extracted repositories without markers) → `folders.subdir`

macOS bundles (`.app`, `.bundle`, `.framework`) are always moved as units.
Safari `.download` bundles, empty folders and hidden folders are left alone.
Loose files directly in a watched folder are sorted per file as before. Set
`folders.mode = "ignore"` to restore pure per-file sorting.

## Running as a background agent (LaunchAgent)

Create `~/Library/LaunchAgents/de.korwisi.onefolder.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>de.korwisi.onefolder</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/onefolder</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/onefolder.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/onefolder.log</string>
</dict>
</plist>
```

Then load it:

```sh
launchctl load ~/Library/LaunchAgents/de.korwisi.onefolder.plist
```

## macOS permissions (TCC)

Access to *Downloads*, *Desktop* and *Documents* is protected by macOS privacy
controls. On first run, macOS shows a prompt ("Files and Folders"); accept it, or grant
the binary Full Disk Access under *System Settings → Privacy & Security → Full Disk
Access*. When started from a terminal, the terminal app needs this permission.

## Limitations / roadmap

- PDF analysis uses the text layer only; scanned PDFs without OCR text fall back to
  file-name/extension rules.
- Notifications use `osascript` (`display notification`), which works from plain
  binaries and LaunchAgents without bundling.
- No OCR and no cloud/classifier integration yet.

## Assets

Brand assets live in [`assets/`](assets/):

- `logo/onefolder-logo-*.png` – full logo (folder + documents) for websites and
  GitHub; transparent background on a square canvas
- `icon/onefolder-folder-icon-*.png` – complete motif (documents flowing into
  the one folder), compact icon layout (16–1024 px)
- `icon/onefolder-folder-icon-simple-*.png` – pure black silhouette of the
  complete motif (single color, ideal for small sizes and monochrome contexts)
- `icon/onefolder-folder-icon-bw-*.png` – black-and-white (grayscale) version
- `icon/onefolder*.icns` – ready-to-use macOS icon sets (color, grayscale and
  silhouette)

To use it as a macOS folder icon: select the folder in Finder → `Cmd+I` → drag
`onefolder-folder-icon-simple-512.png` (or an `.icns` file) onto the icon at the
top left of the info panel.

## License

Released under the [GNU General Public License v3.0](LICENSE) (GPL-3.0-or-later).

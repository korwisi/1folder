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

#### For your machine

```sh
git clone https://github.com/korwisi/1folder
cd 1folder
cargo build --release
# binary: target/release/onefolder
sudo cp target/release/onefolder /usr/local/bin/
```

#### Universal binary (Intel + Apple Silicon)

Build a single binary that runs natively on both architectures:

```sh
./build-universal.sh
# binary: target/universal-release/onefolder
sudo cp target/universal-release/onefolder /usr/local/bin/
```

Requirements:
- Rust installed via [rustup](https://rustup.rs) (not Homebrew; enables cross-compilation)
- Both targets installed: `rustup target add x86_64-apple-darwin aarch64-apple-darwin`
- Xcode Command Line Tools (includes `lipo`): `xcode-select --install`

### Setup

Continue with the quick start below: initialize the config, review it, preview with
`--dry-run`, then run `--once` or start the background watch mode. To start
automatically at login, install the LaunchAgent described under
[Running as a background agent](#running-as-a-background-agent-launchagent) and grant
the required permissions under [macOS permissions (TCC)](#macos-permissions-tcc).

The **experimental** native macOS desktop prototype provides an alternative interface
for inspecting status and configuration. Build and launch it with:

```sh
./desktop/build.sh
open desktop/1folder.app
```

The prototype uses the same config file and Rust core as the CLI and LaunchAgent.
It can edit and validate the TOML config, save it atomically, restart the
LaunchAgent, preview operations, run one shot, show activity, undo the latest
operation, and set the target-folder icon.

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

Set the target folder's Finder icon to the 1folder monochrome icon:

```sh
onefolder --set-target-icon
```


# 5. Run continuously in the background
onefolder
```

### ⚠️ First run: initial indexing

The first time you run `onefolder --once` or start the background agent, it will
build a persistent SHA-256 index of your entire target folder. This index is used
to detect duplicates and avoid rehashing on subsequent runs.

**For large target folders (thousands of files), the initial indexing can take
several minutes** as every file is read and hashed. This is a one-time operation:
subsequent runs will load the index from disk and only rehash changed files, which
is significantly faster.

You can monitor progress by watching the process:
```sh
ps aux | grep onefolder
```

Once indexing completes, the index file (`.1folder_index.json`) will appear in your
target folder, and the agent will respond immediately to new files.

## Configuration

`onefolder --init-config` writes [`config.example.toml`](config.example.toml) to
`~/Library/Application Support/1folder/config.toml`. Key settings:

| Setting | Meaning |
| --- | --- |
| `general.language` | Language for log output and notifications: `de` (default) or `en`. |
| `general.preserve_source_paths` | How to keep the source folder context: `off` (default), `structure` or `prefix`. |
| `target_dir` | The one folder everything is consolidated into. |
| `watch.dirs` | Folders to watch (tilde-expanded). |
| `watch.stability_wait_ms` | How long a file must stay unchanged before it is processed. |
| `watch.debounce_ms` | Quiet period after the last filesystem event. |
| `dedupe.on_duplicate` | `skip`, `keep_both` or `move_to_duplicates`. |
| `folders.mode` | Folder handling in watched folders: `move_as_unit` (default) or `ignore`. |
| `folders.subdir` | Category for folders with project markers or mixed content (default `Sammlungen`). |
| `folders.markers` | Extra project markers (files/folders like `.decker`) detected in addition to the built-in list. |
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

Note: with the default `folders.mode = "move_as_unit"`, folders are moved as
units and per-file naming never kicks in — `structure`/`prefix` only take
effect with `folders.mode = "ignore"`. The two settings are alternatives, not
additives.

### Previewing operations as JSON

`--report <FILE>` appends one JSON line per operation (works in dry-run and
real runs, `--once` and watch mode):

```sh
onefolder --once --dry-run --report /tmp/plan.jsonl
```

Each line contains `ts`, `action` (`moved`, `moved_folder`, …), `src`, `dst`,
`category`, `rule` (the matching rule name, or why a folder was classified:
`homogeneous`, `mixed-content`, `project-markers`, `bundle`) and `dry_run`.

### Folders are moved as units

By default (`folders.mode = "move_as_unit"`) each top-level folder of a watched
directory is classified and moved as one unit — internal structure is preserved.
Classification per folder, first match wins:

1. a rule whose `folder_keywords` match the folder name
2. project markers (`.git`, `package.json`, `Cargo.toml`, `pyproject.toml`,
   `go.mod`, `Makefile`, …, plus your own via `folders.markers`, e.g.
   `.decker` for decker presentation projects) → `folders.subdir`
   (default `Sammlungen`)
3. homogeneous content — if every file in the folder falls into the same
   category, the folder goes there (an images-only folder → `Bilder`)
4. mixed content (e.g. extracted repositories without markers) → `folders.subdir`

macOS bundles (`.app`, `.bundle`, `.framework`) are always moved as units.
Safari `.download` bundles, empty folders and hidden folders are left alone.
Loose files directly in a watched folder are sorted per file as before. Set
`folders.mode = "ignore"` to restore pure per-file sorting.

### Executable and sensitive files

The default rules route common executable and Windows binary formats to the
`Binaries` category (`.exe`, `.dll`, `.msi`, `.msix`, `.appimage`). Adjust or
remove this rule if those formats should remain elsewhere.

Certificates and private key containers are intentionally not moved: `.pem`,
`.p12`, `.pfx`, `.p8`, `.der`, `.crt`, `.cer`, `.jks`, and `.keystore` remain
in `fallback_subdir`. Files with the `.key` extension are inspected for private
key content markers. A private `.key` file remains in `fallback_subdir`, while
an Apple Keynote presentation (`.key`) continues to match the documents rule.

### Renaming categories

Quit the background agent, then rename a category end-to-end:

```sh
onefolder --rename-category Bilder Photos --dry-run
onefolder --rename-category Bilder Photos
```

The command validates the names, refuses destination collisions, updates
`rules[].subdir`, `fallback_subdir`, `folders.subdir`, and
`dedupe.duplicates_subdir` references in `config.toml`, renames the category
folder, retags contained files on macOS, and records the action in the journal.

## Reorganizing the target folder

Changing `rules`, `fallback_subdir` or elsewhere-referenced category names in the
config only affects newly sorted files — everything already in the target folder
keeps its place. To re-apply the current rules to it, run:

```sh
onefolder --reorganize --dry-run      # preview what would move
onefolder --reorganize                # actually re-sort
```

`--reorganize` walks every category folder, re-classifies each file directly in
it (including `pdf_keywords` rules) and each folder there as a unit, then moves
anything whose category changed. Stray files in the target root are sorted too,
name collisions get a ` (2)` suffix, emptied category folders are removed, and
Finder tags are re-applied. The `Duplikate` folder is never touched, and nested
paths deeper than one level inside a category (e.g. from
`preserve_source_paths = "structure"`) are reclassified too while preserving
their relative subdirectories.

Every move is journaled as `reclassified`, so it behaves like any other move:

```sh
onefolder --undo all --dry-run
onefolder --undo all                  # put everything back where it was
```

Quit the background agent before reorganizing, as with undo.

## Undo

Every action is recorded in an append-only journal at
`<target_dir>/.1folder-journal.jsonl` (one JSON line per action, including the
original and destination path). The journal is the history — plain text, no
database — and powers undo:

```sh
onefolder --undo                # restore the last moved item
onefolder --undo 10             # restore the last 10 items
onefolder --undo all            # restore everything still reversible
onefolder --undo all --within 30m     # only items moved in the last 30 minutes
onefolder --undo all --src-prefix ~/Desktop/alter-desktop   # only items from there
onefolder --undo all --dry-run  # preview what would be restored
```

Undo moves files and folders back to their original location, recreating the
original folder structure. If something now occupies the original spot, the
restored item is saved next to it as `name (2).ext` — nothing is ever
overwritten. Restored items are marked in the journal and cannot be undone a
second time. Items that were only detected as duplicates (`skip`) were never
moved and are ignored.

Note: quit the background agent before undoing (`launchctl unload`), otherwise
restored files in watched folders may trigger the sorter again. With the
default `dedupe.on_duplicate = "skip"` they would stay put, but other settings
could create copies.

Note: moving or renaming previously sorted items inside the target folder is
safe, but it invalidates their journal entries — undo then skips them with a
warning. See [Renaming categories](#renaming-categories) for the safe rename
procedure.

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

After installing a new release binary, restart the LaunchAgent so the running
process uses the new version:

```sh
sudo install -m 0755 onefolder /usr/local/bin/onefolder
launchctl kickstart -k gui/$(id -u)/de.korwisi.onefolder
/usr/local/bin/onefolder --version
```

In the example above, `onefolder` refers to the downloaded or extracted binary.
Run `launchctl list de.korwisi.onefolder` to confirm that the agent is loaded and
running.

## macOS permissions (TCC)

Access to *Downloads*, *Desktop* and *Documents* is protected by macOS privacy
controls. The LaunchAgent and desktop app are separate processes and have separate macOS
permission identities.

To grant permanent access:

1. Open *System Settings → Privacy & Security → Files and Folders*.
2. Add `/usr/local/bin/onefolder` for the LaunchAgent and enable access to **Desktop**
   and **Documents** (and any other configured watched or target folders).
3. Open *1folder.app* in Finder. Use **Open** once, then add the app itself under
   *Privacy & Security → Files and Folders*.
4. If macOS does not show the folders in the Files and Folders list, also add both
   `/usr/local/bin/onefolder` and `1folder.app` under *Privacy & Security → Full Disk
   Access*. Full Disk Access is broader than necessary; prefer Files and Folders where
   macOS offers it.
5. Remove stale entries for old app or binary paths after moving or rebuilding the app.

Stable, consistently code-signed builds preserve these grants across updates. The
`desktop/build.sh` development build uses the fixed ad-hoc identity
`de.korwisi.onefolder.desktop`, so the app itself remains distinguishable across local
rebuilds. The installed `/usr/local/bin/onefolder` binary must have a stable signature
or be an Apple-signed Developer ID build for persistent LaunchAgent grants. Repeated
prompts after every file are not normal with a consistent, stable executable identity;
they indicate that the binary or app bundle has changed identity or has not been granted
the required folders.

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

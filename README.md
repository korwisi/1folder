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
- **Safe by design**: files are only moved, never deleted; every action is appended to
  a JSONL journal (`<target>/.1folder-journal.jsonl`); a `--dry-run` mode shows what
  would happen.
- **Download-safe**: files are only processed once their size and modification time
  have been stable for a configurable period. Browser temp files (`*.crdownload`,
  `*.part`, Safari's `*.download` bundles) are ignored, and a file is picked up as
  soon as its handle is closed or it is renamed to its final name.

## Build

```sh
cargo build --release
# binary: target/release/onefolder
```

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
| `target_dir` | The one folder everything is consolidated into. |
| `watch.dirs` | Folders to watch (tilde-expanded). |
| `watch.stability_wait_ms` | How long a file must stay unchanged before it is processed. |
| `watch.debounce_ms` | Quiet period after the last filesystem event. |
| `dedupe.on_duplicate` | `skip`, `keep_both` or `move_to_duplicates`. |
| `notify.enabled` | Toggle Notification Center notifications. |
| `rules` | Ordered list; first match wins. All specified conditions must match. |
| `fallback_subdir` | Subfolder for files that match no rule. |

A rule matches when **all** specified conditions are met:

- `extensions` – file extension, case-insensitive
- `filename_keywords` – substrings of the file name, case-insensitive
- `pdf_keywords` – substrings of the text extracted from the first 3 PDF pages

Example: keep scientific papers before generic PDF rules by listing the `Papers`
rule first.

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

## License

MIT

# 1folder native desktop prototype

This prototype is a minimal native macOS AppKit application for validating the
Rust core through a C ABI. It is not a packaged or signed application.

Build requirements:

- macOS with Xcode Command Line Tools
- Rust toolchain
- Swift 6

Build:

```sh
./desktop/build.sh
```

This creates `desktop/1folder.app` with the 1folder icon, executable, resources,
and Rust library bundled together.

Run:

```sh
cd desktop
swift run
```

The prototype loads the default configuration from
`~/Library/Application Support/1folder/config.toml`. It can show status,
preview a one-shot run, run once, show recent journal activity, and undo the
last operation. The **Set target icon** action applies the monochrome
1folder icon to the configured target folder.
The configuration editor loads the same TOML file as the LaunchAgent. **Save &
restart agent** validates and atomically replaces the file, then restarts
`de.korwisi.onefolder` so the background process adopts the new settings.

The editor is populated when the prototype opens. If no text appears, verify that
the config path in the top field exists and is readable. The prototype and LaunchAgent
need separate macOS Files and Folders or Full Disk Access grants; see the
[main macOS permissions section](../README.md#macos-permissions-tcc).

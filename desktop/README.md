# 1folder native desktop prototype

**Experimental and work in progress.** This prototype is a native macOS AppKit
application for validating the Rust core through a C ABI. It is not a packaged
or signed application, and some workflows may change.

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
`~/Library/Application Support/1folder/config.toml`. It uses the native
macOS controls to edit folders, behavior, notifications, and Finder tags,
and also provides a TOML editor for advanced settings.

Available actions:

- **Refresh** reloads status and recent activity.
- **Preview** shows one-shot operations without changing files.
- **Run once** executes a one-shot run.
- **Set target icon** applies the 1folder monochrome icon to the target folder.
- **Undo last** restores the latest reversible operation.
- **Save & restart agent** validates and atomically saves the TOML file, then
  restarts `de.korwisi.onefolder` so the LaunchAgent adopts the new settings.

The editor is populated when the prototype opens. If no text appears, verify that
the config path in the top field exists and is readable. The prototype and LaunchAgent
need separate macOS Files and Folders or Full Disk Access grants; see the
[main macOS permissions section](../README.md#macos-permissions-tcc).

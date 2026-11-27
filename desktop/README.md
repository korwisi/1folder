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

Run:

```sh
cd desktop
swift run
```

The prototype loads the default configuration from
`~/Library/Application Support/1folder/config.toml`. It can show status,
preview a one-shot run, run once, show recent journal activity, and undo the
last operation.

# AGENTS.RELEASE.md

Handoff notes for the release automation work (continuation context; see
`.github/workflows/release.yml` for the source of truth).

## Status

- [x] `release.yml` created and syntax-checked (YAML parses).
- [x] First real run: verified with tag `v0.5.0` (run 45422234588). The
      workflow built, packaged, created the GitHub Release, and uploaded
      `onefolder-v0.5.0-macos-universal.tar.gz`.
- [ ] Code signing / notarization: separate open TODO in `TODO.md`.

## How the workflow works

- Trigger: `push: tags: ["v*"]` — no manual dispatch. Tags must be
  `vMAJOR.MINOR.PATCH` (current version at time of writing: 0.5.0).
- Single job on `macos-latest` (ARM runner; irrelevant since both architectures
  are built explicitly).
- Steps:
  1. `actions/checkout@v5`.
  2. `dtolnay/rust-toolchain@stable` with targets
     `x86_64-apple-darwin,aarch64-apple-darwin` (satisfies the prerequisites of
     `build-universal.sh`, which then runs unchanged).
  3. `cargo test`.
  4. `cargo clippy --all-targets -- -D warnings`.
  5. `bash build-universal.sh` → universal binary at
     `target/universal-release/onefolder` (`lipo` of both arches).
  6. Packaging: tars the binary together with `config.example.toml`,
     `README.md`, `LICENSE` into
     `onefolder-<tag>-macos-universal.tar.gz`.
  7. Release step: uses the built-in `GITHUB_TOKEN` (needs
     `permissions: contents: write`, already set; no secrets required). Creates
     the release with `--generate-notes` if it does not exist yet, then uploads
     the asset with `--clobber` so workflow re-runs (e.g. re-pushed tag) cannot
     fail on an existing asset.

## Open points

- The uploaded binary is unsigned, so downloads still hit Gatekeeper
  quarantine. Notarization would slot into the same workflow
  (`codesign` + `notarytool`); needs an Apple Developer ID and stored secrets
  (App Store Connect API key).
- Repo has no CI beyond this release workflow.

## Verified `v0.5.0` release

- Workflow run: 45422234588, successful.
- Asset:
  `onefolder-v0.5.0-macos-universal.tar.gz`.
- Asset contents: `onefolder`, `config.example.toml`, `README.md`, `LICENSE`.
- Universal binary verified with `lipo` as `x86_64 arm64`.
- Downloaded archive extracted and installed as `/usr/local/bin/onefolder`.
- Installed executable reports `1folder 0.5.0`.
- LaunchAgent `de.korwisi.onefolder` was restarted after installation.

## Why this file exists

This file carries the context across machines and records the verified release
procedure and remaining signing/notarization work.

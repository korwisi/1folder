# AGENTS.RELEASE.md

Handoff notes for the release automation work (continuation context; see
`.github/workflows/release.yml` for the source of truth).

## Status

- [x] `release.yml` created and syntax-checked (YAML parses).
- [ ] First real run: not yet verified. Trigger by pushing a `v*` tag, e.g.
      `git tag v0.4.2 && git push origin v0.4.2`. Only tick the Automate release
      artifacts item in `TODO.md` (Builds and releases section) after the first
      successful run.
- [ ] Code signing / notarization: separate open TODO in `TODO.md`.

## How the workflow works

- Trigger: `push: tags: ["v*"]` — no manual dispatch. Tags must be
  `vMAJOR.MINOR.PATCH` (current version at time of writing: 0.4.1).
- Single job on `macos-latest` (ARM runner; irrelevant since both architectures
  are built explicitly).
- Steps:
  1. `dtolnay/rust-toolchain@stable` with targets
     `x86_64-apple-darwin,aarch64-apple-darwin` (satisfies the prerequisites of
     `build-universal.sh`, which then runs unchanged).
  2. `bash build-universal.sh` → universal binary at
     `target/universal-release/onefolder` (`lipo` of both arches).
  3. Packaging: tars the binary together with `config.example.toml`,
     `README.md`, `LICENSE` into
     `onefolder-<tag>-macos-universal.tar.gz`.
  4. Release step: uses the built-in `GITHUB_TOKEN` (needs
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

## Why this file exists

Xcode on the original machine was too outdated to install `gh`; work was
continued elsewhere. This file carries the context across machines.

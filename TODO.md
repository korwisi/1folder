# TODO

Ideas and open items, roughly in the order they came up. Nothing here is
committed to a release; pick freely.

## Rules and classification

- [ ] More Office formats in `dokumente`: `.docm`, `.potx`, `.pub`
      (came up during the second dry-run analysis).
- [ ] 3D/modeling formats in `3d-druck`: `.blend`, `.fbx` (Blender/Fbx are
      modeling rather than printing; maybe a separate `3d` or `modelle`
      category instead).
- [ ] `.exe` handling: currently falls through to the fallback category.
      Windows binaries could go to `Archive`, a dedicated category, or stay
      in the fallback — decide once we know how often they appear.
- [ ] Certificates and keys (`.pem`, `.key`, `.p12`, …): deliberately **not**
      routed anywhere for now. They contain private material and must stay
      where the user put them; revisit only if there is a real need, and
      never move them silently.

## Target folder maintenance

- [ ] `onefolder rename-category <old> <new>`: rename a category end-to-end —
      update every config reference (`rules[].subdir`, `fallback_subdir`,
      `folders.subdir`, `dedupe.duplicates_subdir`) in place while preserving
      comments, rename the folder in the target (with collision validation),
      retag the files with the new category tag (macOS-gated), journal the
      action, and remind the user to restart the agent. An explicit command,
      not automatic detection of manual renames; see the "Renaming categories"
      section in the README for the manual procedure this would automate.

## Builds and releases

- [x] Release builds for Apple Silicon: added `build-universal.sh` script
      to create universal binaries (`lipo` of both Intel and Apple Silicon arches)
      so one asset covers both Intel and Apple Silicon Macs. See README for setup.
- [ ] Automate release artifacts with GitHub Actions: build on tag push
      (`macos-latest` runner, universal binary via `build-universal.sh`),
      and attach the result to the GitHub Release automatically.
- [ ] Consider code signing / notarization so macOS Gatekeeper does not
      quarantine downloaded binaries (needs an Apple Developer ID).

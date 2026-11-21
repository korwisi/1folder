# TODO

Ideas and open items, roughly in the order they came up. Nothing here is
committed to a release; pick freely.

The reliability items below have priority: they implement Phase 1 ("A
dependable daily tool") from [LONGTERM-GOALS.md](LONGTERM-GOALS.md) and should
take precedence over everything further down.

## Reliability and correctness

These items address failure modes identified during a review of the current
implementation. They should take precedence over expanding the feature set.
User-facing strings introduced by these items belong in the EN/DE message
tables in `src/i18n.rs`.

- [ ] Make dry-run strictly side-effect-free. A dry-run must not create or
      update the persistent hash index (or any other file, journal, tag, or
      notification state). Add an integration test that snapshots the target
      and relevant state before and after dry-run, including when new files
      would otherwise be indexed.
- [ ] Validate all configured category and destination subdirectory names
      before processing. Values such as absolute paths, path separators,
      `..`, or other paths escaping the target directory must be rejected.
      Cover `fallback_subdir`, `rules[].subdir`, `folders.subdir`, and
      `dedupe.duplicates_subdir`.
- [ ] Do not index 1folder's own artifacts in the target folder. The persistent
      index already skips `.1folder_index.json` and `.DS_Store`
      (`src/index.rs`), but the journal `.1folder-journal.jsonl` written into
      the target folder is still hashed into the index and can appear as a
      duplicate candidate. Exclude the journal (and any future tool-owned
      files) from `refresh()`.
- [ ] Do not silently discard malformed journal records. Report invalid lines
      with line numbers, distinguish a truncated final record from corruption
      in the middle of the journal where possible, and prevent undo from
      proceeding as though an incomplete history were complete.
- [ ] Surface input-directory enumeration errors. Failures to open a watched
      directory or read individual entries must be reported and reflected in
      the command's result/exit status; do not silently treat an unreadable
      directory as empty.
- [ ] Make cross-filesystem moves failure-safe. When a rename cannot be performed
      atomically because source and destination are on different filesystems,
      copy to a temporary file in the destination directory, ensure the copy is
      complete (and preferably verify its size/hash), rename the temporary file
      to its final destination, and only then remove the source. Clean up
      temporary files on failure and preserve enough state to recover from
      interruptions. Apply equivalent care to directory moves.
- [ ] Increase hash-index metadata precision. The current metadata key uses
      file size and modification time in whole seconds. Preserve sub-second
      modification-time precision where available and consider additional file
      identity metadata so a same-size replacement within one second cannot
      incorrectly reuse a stale hash.
- [ ] Make undo journal updates resilient to partial failure. In particular,
      the restore operation currently performs the filesystem restore before
      appending the undo record. Define and implement recoverable operation
      states so a successful restore followed by a journal-write failure is
      detectable and can be reconciled safely on the next run.
- [ ] Review all filesystem mutations for interruption and error handling:
      destination collisions, permissions, full disks, disappearing volumes,
      source changes during processing, and failures while writing reports or
      journal entries must not result in silent data loss or misleading state.
- [ ] Define a retry/recovery policy for transient watch-mode processing
      failures. Currently, failed events can be removed from the pending set;
      ensure files are not silently abandoned and make repeated failures
      visible without creating an unbounded retry loop.

## Reliability test coverage

Each item references the corresponding reliability item above; implement the
tests together with the fix rather than maintaining parallel descriptions.

- [ ] Integration tests proving dry-run leaves the filesystem and persistent
      state unchanged (see "Make dry-run strictly side-effect-free").
- [ ] Rejection of unsafe category/subdirectory values, including absolute
      paths and traversal attempts (see the validation item).
- [ ] Malformed journal handling: invalid middle lines, a truncated final
      line, and undo behavior when the journal is incomplete.
- [ ] Unreadable or disappearing input directories and individual
      directory-entry errors where they can be reproduced reliably.
- [ ] Interrupted or failed cross-filesystem moves, including failures during
      copy, final rename, and source removal (see "Make cross-filesystem moves
      failure-safe").
- [ ] Hash-index invalidation when a file is replaced by another file of the
      same size with a modification time that falls within the same second.
- [ ] Undo behavior when the filesystem restore succeeds but journal writing
      fails, and that the state can be reconciled.
- [ ] Concurrent changes to watched files, large files and directories, stale
      or damaged indexes, destination collisions, symlinks, and
      removable/external volumes where applicable.
- [ ] Filesystem tests stay isolated in temporary directories; avoid assumptions
      about the developer's real home directory or mounted volumes.

## Rules and classification

- [ ] More Office formats in the documents category: .docm, .potx, .pub
      (came up during the second dry-run analysis).
- [ ] 3D/modeling formats in the 3d-print category: .blend, .fbx (Blender/Fbx are
      modeling rather than printing; maybe a separate 3d or models category
      instead).
- [ ] .exe handling: currently falls through to the fallback category.
      Windows binaries could go to Archive, a dedicated category, or stay
      in the fallback — decide once we know how often they appear.
- [ ] Certificates and keys (.pem, .key, .p12, …): deliberately **not**
      routed anywhere for now. They contain private material and must stay
      where the user put them; revisit only if there is a real need, and
      never move them silently.

## Target folder maintenance

- [x] Persist the target folder's SHA-256 index so startup and each incoming file
      do not require rehashing every target file. Store each file's path, size,
      modification time, and hash in a small index; reuse a hash only when the
      file metadata still matches, and hash new or changed files. Reconcile
      missing files and write updates atomically so manual changes and
      interruptions do not leave the index stale or corrupt.
- [ ] Avoid a full recursive target-folder scan for every incoming file.
      The persistent index currently avoids re-hashing unchanged files, but
      repeated refresh scans can still make batches of incoming files
      unnecessarily expensive. Measure this with large targets first, then
      consider refreshing once per batch or using incremental updates.
- [ ] onefolder rename-category <old> <new>: rename a category end-to-end —
      update every config reference (rules[].subdir, fallback_subdir,
      folders.subdir, dedupe.duplicates_subdir) in place while preserving
      comments, rename the folder in the target (with collision validation),
      retag the files with the new category tag (macOS-gated), journal the
      action, and remind the user to restart the agent. An explicit command,
      not automatic detection of manual renames; see the "Renaming categories"
      section in the README for the manual procedure this would automate.
- [ ] --reorganize (shipped): recursively re-classify files nested deeper than
      one category level (currently skipped, mostly preserve_source_paths =
      "structure"), and offer a category-rename mode that reuses it.

## Notifications

- [ ] Clickable notifications that reveal the moved file in Finder. The current
      osascript display notification approach cannot handle clicks at all.
      Plan: a small Swift helper (~60 lines, UNUserNotificationCenter + delegate
      calling NSWorkspace.shared.activateFileViewerSelecting, which opens
      Finder and selects the file), compiled at build time via build.rs +
      swiftc on macOS and called like osascript is today. No external user
      dependency (unlike shelling out to terminal-notifier). Caveats: ad-hoc
      signing of the helper (ties into the code-signing / notarization item
      below), and one short-lived helper process per notification until it is
      dismissed or clicked.

## Builds and releases

- [x] Release builds for Apple Silicon: added build-universal.sh script
      to create universal binaries (lipo of both Intel and Apple Silicon
      arches) so one asset covers both Intel and Apple Silicon Macs. See README
      for setup.
- [ ] Release automation with GitHub Actions: workflow and universal-build
      script are prepared to build on tag push and attach the artifact to a
      GitHub Release. **Not yet tested end to end**; verify the first real
      release run, uploaded asset, archive contents, and download/install
      instructions before treating this as complete.
- [ ] Consider code signing / notarization so macOS Gatekeeper does not
      quarantine downloaded binaries (needs an Apple Developer ID).

## Desktop application readiness

These are preparatory items, not a commitment to start a GUI implementation
immediately. See LONGTERM-GOALS.md for product direction.

- [ ] Keep the organization engine independent of presentation and CLI concerns
      so a future native macOS app can reuse the same Rust core.
- [ ] Before choosing a GUI framework, define a small, stable interface between
      the core and its clients (configuration, preview/dry-run, execution
      status, activity history, and undo).
- [ ] Preserve a fully usable CLI and configuration-file workflow; a GUI must
      be an additional interface, not a prerequisite for using the core.

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

- [x] Make dry-run strictly side-effect-free. A dry-run must not create or
      update the persistent hash index (or any other file, journal, tag, or
      notification state). Add an integration test that snapshots the target
      and relevant state before and after dry-run, including when new files
      would otherwise be indexed.
- [x] Validate all configured category and destination subdirectory names
      before processing. Values such as absolute paths, path separators,
      `..`, or other paths escaping the target directory must be rejected.
      Cover `fallback_subdir`, `rules[].subdir`, `folders.subdir`, and
      `dedupe.duplicates_subdir`.
- [x] Do not index 1folder's own artifacts in the target folder. The persistent
      index already skips `.1folder_index.json` and `.DS_Store`
      (`src/index.rs`), but the journal `.1folder-journal.jsonl` written into
      the target folder is still hashed into the index and can appear as a
      duplicate candidate. Exclude the journal (and any future tool-owned
      files) from `refresh()`.
- [x] Do not silently discard malformed journal records. Report invalid lines
      with line numbers, distinguish a truncated final record from corruption
      in the middle of the journal where possible, and prevent undo from
      proceeding as though an incomplete history were complete.
- [x] Surface input-directory enumeration errors. Failures to open a watched
      directory or read individual entries must be reported and reflected in
      the command's result/exit status; do not silently treat an unreadable
      directory as empty.
- [x] Make cross-filesystem moves failure-safe. When a rename cannot be performed
      atomically because source and destination are on different filesystems,
      copy to a temporary file in the destination directory, ensure the copy is
      complete (and preferably verify its size/hash), rename the temporary file
      to its final destination, and only then remove the source. Clean up
      temporary files on failure and preserve enough state to recover from
      interruptions. Apply equivalent care to directory moves.
- [x] Increase hash-index metadata precision. The current metadata key uses
      file size and modification time in whole seconds. Preserve sub-second
      modification-time precision where available and consider additional file
      identity metadata so a same-size replacement within one second cannot
      incorrectly reuse a stale hash.
- [x] Make every filesystem mutation that depends on a journal write recoverable
      from partial failure. Two concrete instances share one root cause — the
      filesystem change happens first and the journal record only afterwards,
      with no recoverable state in between:

      - A move that succeeds but whose journal entry cannot be appended leaves
        the file undocumented and not undoable.
      - The undo restore currently performs the filesystem restore before
        appending the undo record, so a journal-write failure after a successful
        restore is likewise undetectable.

      Define one recoverable operation-state model and apply it to both cases
      (e.g. `pending → filesystem-done → journaled`), recorded before the
      filesystem change, so the next run can detect the intermediate state and
      reconcile it safely. Do not implement the two fixes independently.
- [x] Make file stability detection fail safely. Currently the result of
      `wait_until_stable()` is ignored at both call sites (`src/organize.rs`
      `process_file` and `process_folder`), and on timeout expiry the function
      reports a continuously changing file as processable as long as it still
      exists. Instead: never process a file that has not stabilized; keep it for
      a later attempt and make the timeout visible to the user. This needs a
      decision per mode — the watch-mode pending map can retry, but `--once` has
      no retry mechanism, so there the file must stay in place and the run must
      surface a visible error/exit status instead of skipping silently.
- [x] Review all filesystem mutations for interruption and error handling:
      destination collisions, permissions, full disks, disappearing volumes,
      source changes during processing, and failures while writing reports or
      journal entries must not result in silent data loss or misleading state.
- [x] Define a retry/recovery policy for transient watch-mode processing
      failures. Currently, failed events can be removed from the pending set;
      ensure files are not silently abandoned and make repeated failures
      visible without creating an unbounded retry loop.
- [x] Make file stability detection fail safely. If a file is still changing
      when the configured stability timeout expires, do not process it as
      stable. Keep it pending for a later retry and make the timeout visible
      to the user.

## Reliability test coverage

Each item references the corresponding reliability item above; implement the
tests together with the fix rather than maintaining parallel descriptions.

- [x] Integration tests proving dry-run leaves the filesystem and persistent
      state unchanged, including an end-to-end test of the actual CLI/run-once
      workflow rather than only unit tests of `process_file()` (see
      "Make dry-run strictly side-effect-free").
- [x] Rejection of unsafe category/subdirectory values, including absolute
      paths and traversal attempts (see the validation item).
- [x] Malformed journal handling: invalid middle lines, a truncated final
      line, and undo behavior when the journal is incomplete.
- [x] Unreadable or disappearing input directories and individual
      directory-entry errors where they can be reproduced reliably.
- [x] Interrupted or failed cross-filesystem moves, including failures during
      copy, final rename, and source removal (see "Make cross-filesystem moves
      failure-safe").
- [x] Hash-index invalidation when a file is replaced by another file of the
      same size with a modification time that falls within the same second.
- [x] Deleted target files are removed from the hash index and are not reported
      as duplicates if the same content is later placed back in a source folder.
- [x] Undo behavior when the filesystem restore succeeds but journal writing
      fails, and that the state can be reconciled (part of the shared
      recovery-state model above; cover both the move and the undo direction).
- [x] A move whose journal entry cannot be appended; verify the operation remains
      detectable and recoverable per the recovery-state model above.
- [x] Stability timeout while a watched file is still changing; verify it is
      not processed prematurely, stays in place in `--once` mode with a visible
      error/exit status, and remains eligible for retry in watch mode.
- [x] Concurrent changes to watched files, large files and directories, stale
      or damaged indexes, destination collisions, symlinks, and
      removable/external volumes where applicable.
- [x] Filesystem tests stay isolated in temporary directories; avoid assumptions
      about the developer's real home directory or mounted volumes.

## Rules and classification

- [x] More Office formats in the documents category: .docm, .potx, .pub
      (came up during the second dry-run analysis).
- [x] 3D/modeling formats in the 3d-print category: .blend, .fbx (Blender/Fbx are
      modeling rather than printing; maybe a separate 3d or models category
      instead).
- [x] .exe handling: currently falls through to the fallback category.
      Windows binaries could go to Archive, a dedicated category, or stay
      in the fallback — Windows executables and common binary formats now go to
      a dedicated Binaries category.
- [x] Certificates and keys (.pem, .key, .p12, …): deliberately **not**
      routed anywhere for now. Certificate/key extensions remain in the fallback
      category. `.key` is classified by content: private-key markers keep the file
      in place, while non-private Keynote `.key` documents are routed to Dokumente.

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
      unnecessarily expensive. A direct `PersistentIndex::refresh()` benchmark
      is available in `src/index.rs`; run it with the documented release build
      command, then consider refreshing once per batch or using incremental updates.
      Latest direct benchmark: 100 files cold 3.5 ms / warm 0.38 ms; 1,000 files
      cold 22.3 ms / warm 3.6 ms; 5,000 files cold 117.3 ms / warm 19.1 ms.
- [x] onefolder rename-category <old> <new>: rename a category end-to-end —
      update every config reference (rules[].subdir, fallback_subdir,
      folders.subdir, dedupe.duplicates_subdir) in place while preserving
      comments, rename the folder in the target (with collision validation),
      retag the files with the new category tag (macOS-gated), journal the
      action, and remind the user to restart the agent. An explicit command,
      not automatic detection of manual renames; see the "Renaming categories"
      section in the README for the manual procedure this would automate.
- [x] --reorganize (shipped): recursively re-classify files nested deeper than
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
- [x] Release automation with GitHub Actions: workflow and universal-build
      script build on tag push and attach the artifact to a GitHub Release.
      Verified end to end with `v0.5.0` run 45422234588: workflow success,
      uploaded archive, universal architecture contents, download/extraction,
      installation, and LaunchAgent restart.
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

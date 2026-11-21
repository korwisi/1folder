# Long-Term Goals

This document captures the intended direction of 1folder. It is a product
north star, not a binding release plan. Near-term implementation priorities
belong in [TODO.md](TODO.md); current behavior and usage belong in
[README.md](README.md).

## 1. Product intent

1folder exists first and foremost to make the maintainer's own day-to-day file
handling easier. Colleagues have also expressed interest, which suggests that
the underlying problem is shared and that the project may be useful beyond one
person.

The initial target audience is deliberately narrow: **computer-literate users
who are comfortable configuring a tool and understanding the basics of their
file system**. This is a conscious scope decision, not a statement that 1folder
should be difficult to use.

The project should solve its core problem exceptionally well before attempting
to serve every kind of user. If less technical users can benefit from it later,
that is welcome, but it should not drive the first implementation.

## 2. Product philosophy

### Help users stay in control

1folder organizes incoming files; it should not take ownership of the user's
filing system. The user remains in charge of the target folder, its structure,
and the rules that shape it.

Automation must be understandable, predictable, and reversible. Users should
be able to inspect what happened and recover from mistakes.

### Reliability before feature breadth

Moving files is consequential. Correctness, failure handling, and recoverability
matter more than adding many classification rules or convenience features.
A file must never silently disappear because an operation was interrupted or
a log could not be written.

### Local-first and privacy-conscious

The core workflow should remain local and useful without an account, cloud
service, subscription, or remote processing. File contents should not need to
leave the user's Mac for ordinary organization.

### Useful without AI

The essential classification and organization workflow should remain
deterministic and understandable. AI-assisted recognition may be explored in
the future, but it must be optional and must not become a dependency for the
core product.

### Keep the tool focused

1folder is a file-organization utility, not a general-purpose file manager,
cloud drive, workflow automation suite, or document management platform.
New capabilities should strengthen its central promise rather than expand the
product in unrelated directions.

## 3. Initial product scope

The first useful product should focus on a small, coherent workflow:

- Watch user-selected input folders.
- Classify incoming files using transparent, configurable rules.
- Move files into a user-defined target folder and structure.
- Detect duplicates using content hashes.
- Preserve useful context where configured, including meaningful folder
  structures and Finder tags.
- Record actions in a journal and provide a dependable undo mechanism.
- Offer dry-run/preview and machine-readable reports so users can understand
  the consequences before or after a run.
- Run locally in the background on macOS.

The CLI and TOML configuration are appropriate interfaces for the initial
audience. They should remain supported even if a graphical interface is added.

## 4. Product evolution

### Phase 1 — A dependable daily tool

The immediate goal is to make the existing Rust application safe and dependable
for regular personal use and for technically confident early adopters.

Priorities:

- Make file moves safe across filesystem boundaries and interruptions.
- Ensure the hash index cannot reuse stale hashes based on insufficient
  metadata.
- Make journal and undo behavior recoverable when filesystem or journal
  operations fail independently.
- Expand integration and failure-mode testing.
- Keep classification behavior explicit and configuration-driven.
- Improve installation and operational documentation where real users need it.

Success means users can trust 1folder with real incoming files and can explain
what it did.

A concrete exit criterion for this phase: all reliability items in
[TODO.md](TODO.md) are done, and the maintainer has used 1folder daily on real
incoming files for a sustained period without data loss or unrecoverable
states. Phase 2 (the native macOS interface) should not start before this
bar is met.

### Phase 2 — A native macOS interface

A standalone macOS application is a plausible next step because it could make
setup, monitoring, and recovery more convenient for the maintainer and
interested colleagues.

The preferred direction is to **retain the Rust organization engine and build
a native macOS interface around it**, rather than rewriting the core in a GUI
technology.

An initial interface could provide:

- Overview: running state and recent activity.
- Folders: input folders and target folder configuration.
- Rules: inspect and edit classification rules.
- Activity: a readable history of organization actions.
- Recovery: inspect and undo supported operations.
- Settings: background behavior, permissions, and diagnostics.

A menu-bar presence may be useful for status and quick access, but it is not a
requirement for the first GUI milestone.

The GUI should be an additional client of the same core. The CLI and
configuration-file workflow should remain first-class.

### Phase 3 — Broader accessibility, if justified

Once the core and technical-user experience are mature, usability improvements
may make 1folder approachable to less technical users:

- sensible defaults and guided setup;
- clear explanations of folder permissions and automation;
- previews and confirmation for consequential configuration changes;
- accessible activity and recovery views;
- fewer mandatory technical decisions.

This phase should be driven by observed user needs, not by an assumption that
every application must target everyone.

## 5. Architectural direction

Keep the organization engine independent of the user interface.

Conceptually, the product should have three layers:

1. **Rust core** — watching, classification, hashing, deduplication, moving,
   indexing, journaling, and undo.
2. **Clients** — the existing CLI first, and potentially a native macOS app
   later. Clients should use a clearly defined interface to the core.
3. **macOS and filesystem integration** — file-system events, Finder metadata,
   notifications, permissions, and other platform-specific behavior.

Before a GUI is started, define the smallest stable boundary needed for
configuration, dry-run/preview, execution, status, activity history, and undo.
Avoid designing a large plugin or workflow framework prematurely.

## 6. Explicit non-goals for the initial scope

To keep the project manageable, the following are not goals for the initial
product:

- Cloud synchronization or remote file processing.
- Mandatory accounts, subscriptions, or hosted services.
- AI as a requirement for classification or organization.
- Support for multiple operating systems.
- A general-purpose visual workflow editor.
- A comprehensive replacement for Finder or other file managers.
- Hiding every technical option from the initial target audience.

These ideas are not necessarily ruled out forever; they simply do not justify
the added complexity at this stage.

## 7. How to evaluate future ideas

A proposed feature should be evaluated against these questions:

1. Does it make incoming-file organization safer, clearer, or more useful?
2. Does it preserve user control and make actions understandable and reversible?
3. Can it work locally without introducing an unnecessary service dependency?
4. Does it fit the initial audience and current project scope?
5. Is the benefit worth the ongoing implementation and maintenance cost?

Prefer a small, reliable feature over a broad feature that weakens the central
workflow.

## 8. Relationship to the project roadmap

This document describes direction and constraints. It does not imply that every
idea above is approved for implementation.

Use [TODO.md](TODO.md) for actionable engineering tasks and their current
status. Revisit this document when real-world use by the maintainer and early
users provides evidence that the target audience, priorities, or product
boundaries should change.

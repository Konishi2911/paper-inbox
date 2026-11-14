# Desktop manual library milestone

Run from the workspace:

```sh
cargo run -p paper-desktop
cargo run -p paper-desktop -- --database /path/to/library.sqlite
```

On Linux/WSL the default database is `$XDG_DATA_HOME/paper-inbox/library.sqlite`, falling back to `~/.local/share/paper-inbox/library.sqlite`. Windows uses `%LOCALAPPDATA%/paper-inbox/library.sqlite`, and macOS uses `~/Library/Application Support/paper-inbox/library.sqlite`. An explicit relative database path is resolved from the launch directory. The application creates the parent directory, shows the active path, and reports connection failures in the window. `--database` is the only command-line option.

Supported screen operations: manual registration; title/author/citation-key substring search; sorting and 20-row pagination; detail/edit; notes, reading status, favorite and archive changes; soft deletion and restore. Deleted papers are a separate list and include archived papers. Author order, person/organization/literal names, and ORCID are preserved. DOI and arXiv inputs accept identifier bodies or official URLs. Invalid legacy values remain editable with a warning. Editor names, keywords and other publication fields not shown in the form remain intact when other fields are edited.

A worker thread owns database operations. The window remains responsive while work runs, and controls are disabled until the result arrives. A failed validation/save keeps the input and never displays success. Navigation, reconnect and window close ask before discarding dirty input. Deletion has a separate confirmation. A successful save followed by a reload failure is explicitly reported as saved, and further saves are disabled until reload/reconnect to prevent duplicate creation.

Every form save is one SQLite transaction. Before any command runs, the complete original paper and all field heads are compared with a consistent current read. Any document/head change rejects the whole save, including citation key and user metadata changes. Only form fields changed by the user generate commands; untouched bibliography fields are not rewritten. No-op saves produce no events. The expected heads still protect explicit clears.

Schema version 2 introduces a local-settings table holding the persistent local device ID. Version-1 databases are upgraded transactionally without rewriting paper/history/event rows. The explicit-device storage API remains available; the desktop uses `open_local` so a restart does not create another device identity. Newer schemas are rejected without downgrading them.

Verification:

```sh
cargo test --workspace
cargo check --workspace --all-targets
```

The desktop tests cover configuration parsing and fallback paths, real-file application round trips, preservation of fields not shown in the form, invalid/duplicate input, stale edit sessions and delete/restore. The headless Slint integration test runs the real callbacks and worker/event loop through registration, validation error with retained input, editing, restart, author search, deletion, restoration and startup failure. A bounded wait prevents a stalled worker from hanging the test indefinitely. The normal WSLg window was also checked with Japanese text entry, save and restart.

Select saved documents with the export checkbox, enter a .bib target path and prepare BibTeX output. Review warnings and the exact preview before confirming. Existing-file replacement is explicitly marked; cancel retains the original. Unsaved edits are not exported. See ../../docs/identifiers-and-export.md for identifier rules, compatibility, format mappings, file-publication guarantees and tests.

External lookup, versions, attachments, tags/collections and remote synchronization remain future milestones. Search currently uses SQLite's ASCII case folding; full Unicode case folding/relevance ranking are not implemented.

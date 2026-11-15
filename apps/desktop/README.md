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

Schema version 4 adds normalized candidates, proposals and version snapshots. The local-settings table holds the persistent local device ID. Version-1 databases are upgraded transactionally without rewriting paper/history/event rows. The explicit-device storage API remains available; the desktop uses `open_local` so a restart does not create another device identity. Newer schemas are rejected without downgrading them.

Verification:

```sh
cargo test --workspace
cargo check --workspace --all-targets
```

The desktop tests cover configuration parsing and fallback paths, real-file application round trips, preservation of fields not shown in the form, invalid/duplicate input, stale edit sessions and delete/restore. The headless Slint integration test runs the real callbacks and worker/event loop through registration, validation error with retained input, editing, restart, author search, deletion, restoration and startup failure. A bounded wait prevents a stalled worker from hanging the test indefinitely. The normal WSLg window was also checked with Japanese text entry, save and restart.

Select saved documents with the export checkbox, enter a .bib target path and prepare BibTeX output. Review warnings and the exact preview before confirming. Existing-file replacement is explicitly marked; cancel retains the original. Unsaved edits are not exported. See ../../docs/identifiers-and-export.md for identifier rules, compatibility, format mappings, file-publication guarantees and tests.

Attachments, tags/collections and remote synchronization remain future milestones. Search currently uses SQLite's ASCII case folding; full Unicode case folding/relevance ranking are not implemented.

Open a saved active paper and choose 「候補・版を確認」 to review field proposals, held candidates and independent versions. 「候補を入力」 records external information manually; it does not contact a provider. Enter the candidate's own DOI/arXiv identifiers as identity evidence. They are not copied from the current paper and are not submitted as identifier replacement. The screen currently accepts title, year, container, publisher and abstract updates; the storage API accepts all bibliography fields. Check only supplied fields: a checked empty field proposes deletion, and an unchecked field leaves the current value untouched.

Exact identifier matches can fill missing values. Manual values, deletions, uncertain freshness and lower-priority versions remain proposals. Related, conflicting and unknown candidates stay held until explicit association. Association records an independent version and proposes all differences. Publisher versions create a canonical-switch proposal. Approving a switch changes the canonical pointer and runs the usual field update policy: manual changes remain separate approvals. Other registered versions can also be proposed as canonical. The window lists source record/update time and current/proposed values, supports adopt/reject, and retains draft inputs and their stable candidate ID after errors. Custom resolutions are available through the storage API; edit the paper manually for a custom value in the desktop.

Additional contracts: metadata_form_contract checks explicit field presence and validation; metadata_window_contract exercises the real worker/callback path through failed input, discarded navigation, missing-value completion, field adoption/rejection, held association, canonical approval and restart. See ../../docs/metadata-candidates.md.

Identifier lookup is available through 「DOI / arXiv から登録」 and the saved-paper metadata review screen. Crossref/arXiv results are previewed before confirmation, with source provenance preserved on import and manual values protected on updates. New imports propose first-author/year citation keys; title words and numeric suffixes disambiguate collisions, including deleted keys. The manual editor offers 「引用キーを提案」. Saved keys are never automatically renamed. See [metadata-lookup-and-keys.md](../../docs/metadata-lookup-and-keys.md) for provider coverage, missing-value rules, name fallbacks, communication limits and tests.


The desktop uses three persistent panes: a collapsible left navigation rail, a searchable central paper list, and a right detail/registration workspace. Click a paper card to open it directly; the active paper is highlighted. Clicking the already open paper preserves its draft. Switching to another paper asks before discarding unsaved editor, lookup, or candidate input. Lookup and metadata review stay in the right pane so the list remains available. The left BibTeX action opens export settings on the right and separate checkboxes in the list; choosing papers for export does not change the detail selection. Save actions distinguish new registration from saving changes; deletion and restoration appear only for applicable papers. Minimum window size is 1080 × 680 logical pixels.

`layout_window_contract` dispatches real pointer events at controls located from Slint accessibility information. It verifies direct card selection, collapsed navigation geometry, independent export checkboxes, draft protection, and switching from metadata/lookup workspaces. Development builds emit the Slint information needed for these UI tests; release builds omit it.

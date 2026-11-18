# SQLite persistence contracts

The SQLite implementation stores bibliography, ordered contributors and keywords as relational rows. There are no JSON payloads. `papers` points to an immutable `paper_snapshots` row; revisions and outbox events refer to the corresponding historical snapshot. Historical data therefore stays unchanged when the current paper is edited. Publication and identifier scalars are columns of `bibliographies` (one-to-one with a bibliography snapshot).

Implemented commands: CreatePaper, EditBibliography for paper-level and version-level fields, ChangeCitationKey, UpdateNotes, SetReadingStatus, SetFavorite, SetArchived, DeletePaper and RestorePaper. Creation seeds manual revisions only for supplied bibliography fields. None, blank strings, empty lists, Unknown publication type and an empty identifier set are initially unset and have no heads, allowing later external completion. Clients must read active revisions before editing; an initially unset field uses an empty expected-head list. Every explicit edit, including clearing a value, creates a protected manual revision. Identifiers remain a single field: supplying any identifier protects the initial identifier set as a whole. Existing historical revisions are never retroactively removed. Expected heads are compared as a set; duplicate expected IDs are rejected. Edits to deleted papers are rejected.

Citation keys are case-sensitive and unique across all papers, including deleted papers. Deletion is soft. Get returns deleted papers; search defaults to active papers and also supports Deleted and All via DeletionFilter. A restore keeps identity and history. CreatePaper is an explicit manual creation API, not an external metadata import API.

Each commit uses BEGIN IMMEDIATE to serialize writers and atomically update the current paper, revisions, field heads, device sequence and pending events. Immutable snapshots are retained; compaction is not implemented. Clock values are passed explicitly by commit_at for tests; commit uses Utc::now. Stored timestamps use UTC with fixed nanosecond precision. A timestamp earlier than the previous paper update is rejected.

Search supports title, citation key and author substring matching; publication type, reading status, year, favorite and archive filters; deterministic sorting with an ID tie-breaker; and offset pagination. Text matching is literal (SQL parameters, no LIKE wildcard interpretation). SQLite lower() currently provides ASCII case folding, not full Unicode case folding. Count and page are read in one transaction. Tag filters (all or any of the given tags) are supported; relevance ranking and collection filters explicitly return Unsupported.

Collections, duplicate merges and remote event application remain unsupported; their repository lists and existence filters return no entities. Attachments are persisted (see below). Versions, metadata candidates and field/canonical proposals are persisted; pending-proposal search filters use actual pending rows. Pending events and upload acknowledgements persist locally, but this is not a working synchronization transport. Event sequence and parent chains are currently per device; cross-device causal dependencies must be designed before remote application is enabled.

Schema version 6 is installed transactionally. Version 5 added the attachments table and an events.attachment_id column; version 6 adds the tags and paper_tags tables and an events.tag_id column. Nothing existing is rewritten. Existing version-1/2 databases gain normalized version snapshots, candidates, present-field lists, proposals and proposal heads without rewriting existing history. Version 1 also adds local_settings, which retains the desktop local device ID across restarts. Existing paper, revision and event data is preserved. Newer versions are rejected without modifying their schema version.

Verification: `cargo test -p paper-core -p paper-storage`. Integration tests use isolated real SQLite files, reopen them, and inject an SQL trigger failure after state changes to verify rollback. They also cover independent/concurrent connections, history, event immutability, upload acknowledgements, every bibliography field and unsupported operations.

Atomic form saves use load_edit_state and commit_batch_at. The expected full document and every field head are checked under BEGIN IMMEDIATE before any command is applied. A mismatch rejects the entire batch. All command results, history and outbox changes roll back together on failure. Empty unchanged batches create no events.

DOI/arXiv legacy strings use compatibility restoration: invalid strings are retained as raw values with warnings, and unrelated saves preserve them. New inputs and identifier edits use validated types. No schema migration or automatic historical rewrite is required. get_many reads selected documents in a single transaction for export.

See [metadata-candidates.md](metadata-candidates.md) for candidate identity, explicit deletions, approval, version selection and retry contracts.

RegisterMetadataCandidate atomically records a confirmed new-paper acquisition, source-backed field adoption and a separate pending canonical proposal. citation_keys reads all current keys, including deleted papers, with an optional edited-paper exclusion. See [metadata-lookup-and-keys.md](metadata-lookup-and-keys.md).

## Attachments (schema 5)

`attachments` rows hold paper, optional version, SHA-256 (64 lowercase hex digits), file name, media type, size, kind, creation time and `removed_at`. The files are kept by `FileBlobStore` under `library-files/` (see [design](design.md)); this crate never deletes a file when a row is removed.

- `AddAttachment` fails with `InvalidOperation` for an empty, overlong, control-character or path-separator file name, an invalid media type, a deleted paper, or the same hash already attached (and not removed) to that paper. An unknown paper is `NotFound` and a version of another paper is `VersionNotFound`. The same hash may belong to several papers.
- `RemoveAttachment` sets `removed_at`; removing twice is `InvalidOperation`. The same file can be attached again later.
- Both commands append an `AttachmentAdded` / `AttachmentRemoved` event in the same transaction, so a batch containing a failing command stores nothing.
- `attachments(paper)` lists rows that were not removed. `referenced_attachment_hashes()` returns the hashes of such rows; files outside that set are unused and may be deleted by an explicit cleanup.
- Search `has_attachment` filters and `PaperSummary.has_attachment` use the non-removed rows.
- Callers must store the file first and add the row afterwards. The store checks neither the file nor the media type; the application layer validates the PDF signature.

## Tags (schema 6)

Flat tags. A tag has an id and a display name; its uniqueness key is the name with surrounding whitespace removed, inner whitespace runs collapsed, full-width ASCII and the ideographic space folded to half-width, and case ignored (`ML`, `ml`, `ＭＬ` and ` ML ` are one tag). This is not full Unicode NFKC (half-width katakana stay distinct). Names are 1–64 characters with no control characters. The display name is the first spelling created.

- `CreateTag` fails when the key or id already exists; the application layer looks up by name first. `AddTag` fails for a missing paper or tag, a deleted paper, or an assignment that exists; `RemoveTag` fails when the paper does not have the tag. Each success appends a `TagCreated`, `TagAdded` or `TagRemoved` event in the same transaction. `TagCreated` has no paper snapshot.
- Assignments live in `paper_tags`, outside the immutable paper snapshots, so `Paper::metadata().tags` is **not** filled. Read `paper_tags(paper_id)` or the `tags` of a search summary. Creating a paper with preset tags is still Unsupported.
- Assignments are kept when a paper is deleted or archived; restoring brings them back. `tags(include_archived)` lists every tag by name with the number of papers carrying it, never counting deleted papers and counting archived ones only when asked; tags nobody carries are listed with 0, and the UI hides them.
- Search filters by tag with `TagMatch::All` (every tag) or `Any`.
- Renaming and deleting tags are not implemented (decided 2026-11-18); neither are tag events applied from other devices.

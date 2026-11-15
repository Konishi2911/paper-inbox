# Metadata candidates and independent versions

Candidate ingestion is local and transactional. This milestone supplies the planner, normalized SQLite storage and desktop review; network providers and automatic duplicate merging are outside its scope.

## Explicit input and identity

`MetadataCandidate` lives in bibliography.rs and carries a stable CandidateId, bibliography, provenance, version kind and explicit present_fields. Missing fields do not change curated metadata. A present empty field proposes deletion. Duplicate field names, manual sources, blank source names/record IDs and invalid new identifiers are rejected. Identifiers outside present_fields may serve as identity evidence without replacing the current identifier set. The complete original payload is retained.

The same ID, paper and payload is a no-op, including after association. Reusing an ID for different content or another paper fails. A distinct ID records a distinct acquisition. Automatic processing requires an Exact identifier assessment without unvalidated legacy evidence; related versions, conflicts and unknown identity remain held. No identifier existence lookup is performed and no match probability is invented.

Explicit association requires the expected paper and all field heads to match. It creates an independent version; every difference remains a proposal, including completion of an empty field. It does not claim identifier equality. Once a publisher version is canonical, lower-priority acquisitions cannot automatically fill or replace curated fields.

## Planning and approval

Initially missing values have no manual head and can be completed automatically for exact candidates. Explicit manual clears remain protected. Same-source/same-record newer source timestamps can update external values; older timestamps are ignored. Equal timestamps with different values, unknown freshness, multiple active heads, manual values and explicit external deletions require proposals. Only supplied fields participate.

Proposals retain the expected heads, original value, proposed revision, provenance and immutable source-version snapshot. Acceptance rechecks heads, source snapshot and identity (unless explicitly associated). Canonical-origin field proposals also check the selected canonical pointer. A mismatch fails without partial writes. Rejection safely dismisses a stale proposal. Custom values must match the target field and create manual history. Repeating the same resolution is a no-op; a different resolution after completion fails. Superseded proposals and their events remain in history.

## Versions and canonical selection

Versions have separate bibliography snapshots and revision heads owned by one paper. Editing a version leaves the curated bibliography unchanged. Publisher-version registration produces a switch proposal; it never automatically changes the canonical pointer. Selecting another version also creates a proposal. Approval checks both the expected canonical pointer and source-version snapshot.

A canonical switch updates the pointer, then feeds only the version's supplied fields through ordinary update planning. It does not blanket-copy the version bibliography. Missing curated fields may be completed under the usual policy; protected manual values remain separate field proposals. Choosing a manually entered version proposes differences. The paper citation key and user metadata are unchanged.

## Atomic persistence and verification

Schema 4 uses relational rows for candidates, ordered present fields, immutable version snapshots, proposals and expected heads; no JSON bibliography payload is introduced. Version-1/2 upgrades preserve paper/revision/event rows and run in a single transaction. A failed migration leaves the original schema version and columns intact. Foreign keys enforce stored references; commands additionally check version ownership.

Each submission, association, resolution or batch atomically commits current data, history, heads, proposal state and outbox. Event payloads point to historical snapshots rather than mutable current values. Metadata review reads the paper, heads, candidates, proposals and versions in one transaction. Desktop actions also check their complete original paper state. Remote event application and synchronization transport remain unsupported.

Core tests cover pure planning. SQLite tests cover identity holds, replay, manual protection, freshness/deletion, lower-version priority, version ownership, immutable events, stale approvals, custom resolution, restart, migration and injected failure rollback. Desktop tests cover form validation and actual Slint callbacks/worker behavior. Run cargo test --workspace, cargo check --workspace --all-targets and cargo clippy --workspace --all-targets -- -D warnings.

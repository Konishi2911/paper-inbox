# Identifier-based identity assessment

`paper_core::service::assess_identity` returns an `IdentityAssessment` with an outcome and directional evidence. `compare_identity` remains the verdict-only entry point. Both are pure functions of two bibliographies and no longer panic as placeholders. There are no database writes, network requests, candidate attachment, automatic merging or field updates in this milestone.

## Rules

Only validated DOI and arXiv identifiers participate. DOI values already use the approved parser normalization. Complete equal arXiv IDs, including their version suffixes (or both omitted suffixes), match. Common arXiv bases with different version suffixes, or with one suffix omitted, establish a related-version link. Version digits are treated as text, so there is no integer overflow or guessed latest version. Old archive names containing 'v' are preserved.

Outcome precedence:

1. A positive match or related-version link together with any comparable identifier mismatch produces NeedsReview.
2. A DOI match or a complete arXiv match produces Exact if there is no mismatch. Thus a matching DOI plus a same-base arXiv version difference is Exact with both pieces of evidence.
3. A version relation with no exact match or mismatch produces RelatedVersion.
4. Otherwise the result is Unknown. Unequal DOI values, unrelated arXiv bases, missing values and disjoint identifier coverage do not alone prove different intellectual works.

Exact means identifier-level agreement, not proof of equal PDF contents or a fully reconciled publication version. Title, authors, URL, ISBN, ISSN, date and keywords do not affect this stage. Different and ProbableDuplicate remain reserved enum variants for future separately approved policies; this matcher never emits them and never invents confidence values.

## Evidence and legacy data

Evidence is returned in stable DOI-then-arXiv order, with unavailable-side information before a comparison. Matches carry the validated shared identifier; mismatches and version relations carry both directional values. Missing values report identifier kind and side. Every raw legacy value reports UnvalidatedIdentifier and is excluded from comparisons, even if a caller constructs a public Identifiers value containing both raw and typed representations. The assessment does not reinterpret or repair raw strings.

Repair requirements are independent of the outcome. For example, matching validated arXiv IDs plus invalid legacy DOI strings produce Exact based on arXiv, with both DOI repair warnings retained. This is not permission to merge documents or overwrite fields. Future candidate ingestion must inspect the evidence, source provenance, version relationship and existing field revision/proposal contracts before applying anything.

Swapping arguments preserves the outcome. Directional evidence swaps sides/values; its semantic content is symmetric, while the vector's ordering remains stable for each input order. Inputs remain unchanged.

## Tests

The core contract suite covers DOI normalization, exact arXiv references, omitted and differing versions, conflicting identifiers, no-link mismatches, evidence values, legacy/ambiguous fields, ignored bibliographic text, old archive names and arbitrary-length version strings. A 24-by-24 input matrix checks symmetric outcomes/evidence, determinism, input immutability and absence of reserved verdicts.

The SQLite suite checks typed outcomes/evidence across save, close and reopen; legacy invalid values; normalized presentation forms; and unchanged persisted strings, rows and outbox events after reads/assessments. No schema change is needed.

Verification:

```sh
cargo test -p paper-core --test identity_contract
cargo test -p paper-storage --test identity_contract
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

External candidate ingestion, update proposals and their approval UI remain the next milestone. Title-based duplicate suggestions and automatic document merge are not implemented here.

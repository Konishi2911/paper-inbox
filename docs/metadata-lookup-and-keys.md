# Identifier lookup and citation-key proposals

## User flow

Choose 「＋ 文献を追加」 (the single add entry point), paste a DOI/arXiv ID or supported official URL and press Enter or 「書誌情報を取得」. Manual entry without an identifier is offered on the same screen as 「手動で入力」. Only the explicit fetch sends an identifier to its provider. The result (field by field), source, source record, source update date when known, version kind and editable suggested citation key are previewed before any library write. 「登録」 (or Enter in the focused citation-key field) adopts the supplied fields, preserving external provenance; fetched fields are read-only there. Reading status, favorite and notes entered on the same screen are personal metadata and are committed in the same transaction, so a rejected key stores nothing. Cancel leaves the library unchanged. Errors retain the identifier and preview/key when applicable; changing the identifier requires refetching. Fetching again creates a new acquisition ID, while retrying the same confirmation retains its original candidate and paper IDs.

For existing papers, open 「候補・版を確認」 then 「DOI / arXiv で取得」. Confirmation submits an ordinary candidate against the original paper/head snapshot. Existing identity, manual-value protection, held association, deletion and freshness rules apply. Saved citation keys are never changed by lookup. Canonical-version switches remain separate proposals, including on initial import.

Registration uses RegisterMetadataCandidate: create a blank paper, record the candidate, explicitly associate it, and accept supplied field proposals under one transaction. This association is the user's preview confirmation, not inferred identity with another library paper. No manually authored bibliography revisions are invented for imported values. Paper, candidate, version, revisions, proposals and outbox roll back together if any step fails. A matching completed retry produces no events. Duplicate-document detection and automatic merging are not included; a distinct acquisition can be intentionally registered as a separate paper.

## Providers and mapping

paper-metadata adapts the core MetadataResolver port. The desktop runs blocking HTTP on its existing dedicated worker, outside library transactions. DOI uses the fixed HTTPS Crossref works endpoint; arXiv uses the fixed HTTPS Atom API with id_list and a single result. URL parameters/path segments are encoded, redirects are rejected, connect/total timeouts are 8/20 seconds, responses are capped at 2 MiB, and requests within the process are serialized at least three seconds apart. There are no background requests or automatic retries. Not-found, rate-limit, server, malformed response, oversize and transport failures are reported without saving anything. The request includes a PaperInbox/version user agent; no personal contact address is fabricated.

Crossref maps available title, ordered authors/editors and ORCID, publication type, container, publisher, year/month, volume, issue, page, article number, DOI/URL/ISBN/ISSN, abstract text and subjects. Missing or empty provider values are omitted from present_fields, never interpreted as deletion. A malformed contributor fails the response rather than partially shortening the author list. Dates use published, published-print, published-online, issued in that order. Source freshness uses deposited.date-time, not retrieval/indexing time. Supported published types become publisher-version candidates; posted-content is a preprint; unknown types keep Unknown.

arXiv validates the returned base ID and any explicitly requested version. An unversioned request may resolve to the returned current version, which is retained; existing unversioned paper identifiers may therefore produce a held related-version candidate. The entry's published date supplies the original posting year/month and updated supplies source freshness. Names remain Literal because the Atom feed does not separate family/given names. The publisher DOI and unstructured journal_ref are not copied into the preprint's identifiers/publication: they may describe another version, and no relation is fabricated from them.

This release only covers Crossref-registered DOIs, not DataCite or arbitrary publisher scraping. It does not retrieve PDFs. Metadata remains provider-supplied information, not verified paper contents.

Official specifications: [Crossref REST API](https://www.crossref.org/documentation/retrieve-metadata/rest-api/), [arXiv API manual](https://info.arxiv.org/help/api/user-manual.html), [arXiv API terms/rate limits](https://info.arxiv.org/help/api/tou.html).

## Citation-key rule

The pure core service proposes first-author name + year: Smith2024. If occupied, it appends the first available title word excluding common English stop words: Smith2024Graph. If occupied again, it adds a number starting at 2: Smith2024Graph2, Smith2024Graph3. Empty/unusable title words use Work. Existing current keys across all papers, including deleted papers, participate. Comparison for proposals ignores ASCII case to avoid common BibTeX collisions. The edited paper's own key is excluded when explicitly requesting a new proposal; database saves retain the existing case-sensitive uniqueness contract and reject a concurrent exact-key collision without overwriting.

Person names use the first contributor's family field. Organization/Literal names use their complete label; thus an arXiv literal Ada Smith yields AdaSmith2024, without guessing a surname. Keep ASCII letters/digits only, capped at 64 author characters and 24 title characters. No automatic transliteration or name-reading inference is performed. An unusable author becomes Author; an absent year becomes Undated. All proposals pass the existing CitationKey validator.

New lookup previews propose keys automatically. Manual editors provide 「引用キーを提案」, using current unsaved bibliography fields; this is an explicit draft change and must still be saved. Neither metadata updates nor ordinary saves regenerate an existing key. Users can always edit the draft before confirmation.

## Verification

Offline contracts cover deterministic key precedence/fallbacks, case-insensitive and deleted-key collisions, parsers, missing fields, identifier mismatch, arXiv versions, hostile/malformed XML, source provenance, HTTP statuses, redirects, response size, input retention, preview confirmation, existing manual protection and transaction rollback after adoption. The Slint test uses the real worker/callback path with an injected transport. Normal tests never contact providers.

Run cargo test --workspace --offline, cargo check --workspace --all-targets --offline, cargo fmt --all --check, and cargo clippy --workspace --all-targets --offline -- -D warnings. The separate opt-in live test is cargo test -p paper-metadata --test live_contract -- --ignored; it uses public DOI/arXiv samples and was checked against both providers.

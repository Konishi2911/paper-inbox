# Identifiers and BibTeX export

DOI and arXiv identifiers now use validated constructors, FromStr and TryFrom<String>; Serde calls the same TryFrom implementation. Parsing does not make network requests or prove that a record exists.

Input normalization strips surrounding whitespace, a DOI/arXiv prefix, or an official HTTP(S) URL. DOI hostnames are doi.org and dx.doi.org; arXiv hostnames are arxiv.org and www.arxiv.org, with /abs/ and /pdf/ paths. PDF URL suffix .pdf is removed for arXiv. URL percent escapes are decoded once as UTF-8. Userinfo, nondefault ports, query strings, fragments and malformed escapes are rejected. DOI suffixes are not truncated or normalized as filesystem/URL path segments, and trailing punctuation is not guessed away. DOI comparison/storage folds ASCII letters to lowercase; other Unicode codepoints are preserved.

DOI validation checks the numeric 10. prefix (including numeric subdivisions), slash and a nonempty suffix without whitespace or controls. It deliberately does not use a restrictive ASCII-only suffix expression. arXiv validation supports the old archive/YYMMNNN form with optional two-letter uppercase subject class and the newer YYMM.NNNN/NNNNN forms, their date boundaries, nonzero sequence and optional positive vN version. Versions are retained; omitted versions are not invented. Archive membership and identifier existence are not checked remotely.

Legacy SQLite DOI/arXiv strings are read with a compatibility path. Values that fail validation are retained in raw_doi/raw_arxiv_id alongside the model rather than injected into validated types. No SQL columns or existing rows are rewritten during reads. Unrelated saves retain those raw values in the new snapshot too. Historical snapshots are left intact. New paper creation and identifier edits reject raw values. The editor displays the original values and a warning; changing either DOI or arXiv requires the resulting pair to validate (or be explicitly cleared). ISBN, ISSN and URL are retained. Raw and typed values for the same identifier cannot coexist.

## Output behavior

The core renderer emits deterministic traditional BibTeX entries in UTF-8. It renders articles, inproceedings, books, incollection, master's and doctoral theses, techreport and unpublished preprints. Generic thesis, patent, other and unknown kinds fall back to misc with a warning. School/institution are not inferred from unrelated publication fields: missing required fields are warned about. Journal issue becomes number; for other types the issue extension field preserves the value without introducing a volume/number conflict. Article numbers use eid. DOI, URL, ISBN, ISSN and arXiv archivePrefix/eprint are extension fields and may be ignored by a bibliography style.

Author/editor order is retained. Person family/given parts and organization/literal names are grouped so embedded commas or 'and' do not split people. Metadata is treated as plain text: TeX metacharacters, backslashes and literal braces are escaped with balanced groups. Existing raw TeX markup is not executed or preserved as markup. Titles are grouped to retain capitalization. Unicode text is retained; rendering support depends on the downstream TeX/tool configuration. The real local BibTeX engine test verifies parsing and .bbl production, not every downstream typesetting engine.

Duplicate citation keys are rejected case-insensitively for export, including distinct library keys that differ only in ASCII case. This is stricter than the case-sensitive library key constraint and avoids ambiguous traditional BibTeX keys. Invalid stored identifiers are omitted with warnings. Author/empty-name and required-field warnings are explicit; missing metadata is not fabricated. Abstract and keywords are optional and off by default; DOI and URL default on. Notes and other private user metadata are not included.

## Desktop workflow

Select stored papers with '出力に選択'; selection remains across pages and search filters. Enter a .bib output path (the first version uses a path field rather than a native file chooser). Choose the optional fields and prepare export. A separate preview shows the exact stored-document snapshot, target and warnings. Unsaved edits remain in the editor and are excluded. Existing targets are explicitly marked as replacements. Confirm to publish or cancel without changing the target.

Selected documents are read in one SQLite transaction. Export does not mutate the library or its events. The output is prepared in memory and written to a unique same-directory temporary file, synced before publication. New files use a no-clobber hard link; confirmed existing targets use rename. Temporary files are removed on errors. Target content/metadata is checked again before publication; changed targets require re-preparation and confirmation. Symlinks, directories and the active library file (including Unix hard-link aliases) are rejected. Parent directories are not created by export. The target filesystem must support the required same-directory hard-link/rename operation; unsupported operations report an error and retain the existing file. Other processes are not locked: the final existing-target check and rename are not a filesystem compare-and-swap. No guarantee against a concurrent writer in that final interval or a power failure is claimed.

## Verification

Core tests cover normalized constructors/Serde, old/new arXiv variants, invalid inputs, publication mappings, escaped fields, warnings and duplicate keys. An integration test invokes the actual BibTeX engine if installed; it logs a skip otherwise. SQLite tests verify legacy retention, repair and historical preservation. Desktop tests cover selection-to-file, protected paths, cancel, changed targets, injected partial-write failure, and real Slint callbacks through preview, confirmation and repair. The UI wait helper guards against duplicate quit events between consecutive event-loop runs.

Sources consulted:
- DOI Handbook: https://www.doi.org/doi-handbook/DOI_Handbook_Final.pdf
- arXiv identifiers: https://info.arxiv.org/help/arxiv_identifier.html
- BibTeX manual: https://mirrors.ctan.org/biblio/bibtex/base/btxdoc.pdf

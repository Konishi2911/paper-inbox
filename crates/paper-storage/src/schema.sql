PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS bibliographies (
 id TEXT PRIMARY KEY, title TEXT, kind TEXT NOT NULL,
 container_title TEXT, publisher TEXT, year INTEGER CHECK(year BETWEEN 1 AND 65535),
 month INTEGER CHECK(month BETWEEN 1 AND 12), volume TEXT, issue TEXT, pages TEXT,
 article_number TEXT, doi TEXT, arxiv_id TEXT, isbn TEXT, issn TEXT, url TEXT, abstract_text TEXT
);
CREATE TABLE IF NOT EXISTS contributors (
 bibliography_id TEXT NOT NULL REFERENCES bibliographies(id), role TEXT NOT NULL CHECK(role IN ('author','editor')),
 position INTEGER NOT NULL CHECK(position >= 0), kind TEXT NOT NULL CHECK(kind IN ('Person','Organization','Literal')),
 given_name TEXT, name TEXT NOT NULL, orcid TEXT, PRIMARY KEY(bibliography_id,role,position)
);
CREATE TABLE IF NOT EXISTS keywords (
 bibliography_id TEXT NOT NULL REFERENCES bibliographies(id), position INTEGER NOT NULL CHECK(position >= 0),
 value TEXT NOT NULL, PRIMARY KEY(bibliography_id,position)
);
CREATE TABLE IF NOT EXISTS paper_snapshots (
 id TEXT PRIMARY KEY, paper_id TEXT NOT NULL, bibliography_id TEXT NOT NULL REFERENCES bibliographies(id),
 citation_key TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT,
 notes TEXT NOT NULL, reading_status TEXT NOT NULL, favorite INTEGER NOT NULL CHECK(favorite IN (0,1)),
 archived INTEGER NOT NULL CHECK(archived IN (0,1))
);
CREATE TABLE IF NOT EXISTS papers (
 id TEXT PRIMARY KEY, citation_key TEXT NOT NULL UNIQUE,
 snapshot_id TEXT NOT NULL REFERENCES paper_snapshots(id)
);
CREATE TABLE IF NOT EXISTS revisions (
 id TEXT PRIMARY KEY, paper_id TEXT NOT NULL REFERENCES papers(id), field TEXT NOT NULL,
 bibliography_id TEXT NOT NULL REFERENCES bibliographies(id), source TEXT NOT NULL,
 source_record_id TEXT, observed_at TEXT NOT NULL, source_updated_at TEXT, created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS revision_parents (
 revision_id TEXT NOT NULL REFERENCES revisions(id), parent_id TEXT NOT NULL REFERENCES revisions(id),
 PRIMARY KEY(revision_id,parent_id)
);
CREATE TABLE IF NOT EXISTS field_heads (
 paper_id TEXT NOT NULL REFERENCES papers(id), field TEXT NOT NULL,
 revision_id TEXT NOT NULL REFERENCES revisions(id), PRIMARY KEY(paper_id,field,revision_id)
);
CREATE TABLE IF NOT EXISTS devices (id TEXT PRIMARY KEY, sequence INTEGER NOT NULL CHECK(sequence >= 0));
CREATE TABLE IF NOT EXISTS events (
 id TEXT PRIMARY KEY, device_id TEXT NOT NULL REFERENCES devices(id), sequence INTEGER NOT NULL,
 timestamp TEXT NOT NULL, schema_version INTEGER NOT NULL, kind TEXT NOT NULL,
 snapshot_id TEXT REFERENCES paper_snapshots(id), revision_id TEXT REFERENCES revisions(id),
 uploaded INTEGER NOT NULL DEFAULT 0 CHECK(uploaded IN (0,1)), UNIQUE(device_id,sequence)
);
CREATE TABLE IF NOT EXISTS event_parents (
 event_id TEXT NOT NULL REFERENCES events(id), parent_id TEXT NOT NULL REFERENCES events(id),
 PRIMARY KEY(event_id,parent_id)
);
CREATE INDEX IF NOT EXISTS revisions_target ON revisions(paper_id,field);
CREATE INDEX IF NOT EXISTS events_pending ON events(uploaded,device_id,sequence);
PRAGMA user_version = 1;

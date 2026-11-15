CREATE TABLE IF NOT EXISTS metadata_candidates (
 id TEXT PRIMARY KEY,paper_id TEXT NOT NULL REFERENCES papers(id),bibliography_id TEXT NOT NULL REFERENCES bibliographies(id),
 source TEXT NOT NULL,source_detail TEXT,source_record_id TEXT,observed_at TEXT NOT NULL,source_updated_at TEXT,version_kind TEXT NOT NULL,
 received_at TEXT NOT NULL,status TEXT NOT NULL CHECK(status IN ('Held','Processed','Associated')),
 associated_at TEXT,version_id TEXT REFERENCES paper_versions(id)
);
CREATE TABLE IF NOT EXISTS candidate_fields (
 candidate_id TEXT NOT NULL REFERENCES metadata_candidates(id),position INTEGER NOT NULL,field TEXT NOT NULL,
 PRIMARY KEY(candidate_id,field),UNIQUE(candidate_id,position)
);
CREATE TABLE IF NOT EXISTS update_proposals (
 id TEXT PRIMARY KEY,paper_id TEXT NOT NULL REFERENCES papers(id),candidate_id TEXT REFERENCES metadata_candidates(id),
 kind TEXT NOT NULL CHECK(kind IN ('FieldUpdate','CanonicalVersionChange')),reason TEXT NOT NULL,
 status TEXT NOT NULL CHECK(status IN ('Pending','Accepted','Rejected','Superseded')),created_at TEXT NOT NULL,resolved_at TEXT,
 field TEXT,candidate_revision_id TEXT REFERENCES revisions(id),candidate_version_id TEXT REFERENCES paper_versions(id),
 expected_canonical_version_id TEXT REFERENCES paper_versions(id),expected_version_snapshot TEXT REFERENCES version_snapshots(id),
 expected_bibliography_id TEXT REFERENCES bibliographies(id),resolution_kind TEXT,resolution_bibliography_id TEXT REFERENCES bibliographies(id)
);
CREATE TABLE IF NOT EXISTS proposal_heads (
 proposal_id TEXT NOT NULL REFERENCES update_proposals(id),revision_id TEXT NOT NULL REFERENCES revisions(id),PRIMARY KEY(proposal_id,revision_id)
);
CREATE INDEX IF NOT EXISTS candidates_paper ON metadata_candidates(paper_id,received_at,id);
CREATE INDEX IF NOT EXISTS proposals_pending ON update_proposals(paper_id,status);

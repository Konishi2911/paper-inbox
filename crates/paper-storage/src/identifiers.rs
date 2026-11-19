//! Looking up saved papers by DOI or arXiv ID, to warn before registering the same work twice.
use super::{Result, sql};
use paper_core::{
    model::{ArxivId, Doi},
    query::IdentifierMatch,
};
use rusqlite::{Connection, params};

/// The condition that a bibliography row `b` carries the DOI (`?1`) or arXiv base ID (`?2`).
/// DOIs are stored in their normalized lower-case form. An arXiv ID matches whatever version
/// suffix the saved one has.
const CARRIES: &str = "(?1 IS NOT NULL AND b.doi = ?1) OR (?2 IS NOT NULL AND \
    (b.arxiv_id = ?2 OR substr(b.arxiv_id, 1, length(?2) + 1) = ?2 || 'v'))";

pub(super) fn find(
    db: &Connection,
    doi: Option<&Doi>,
    arxiv: Option<&ArxivId>,
) -> Result<Vec<IdentifierMatch>> {
    if doi.is_none() && arxiv.is_none() {
        return Ok(vec![]);
    }
    let sql = format!(
        "WITH hits(paper_id) AS (
            SELECT s.paper_id FROM papers p
              JOIN paper_snapshots s ON s.id = p.snapshot_id
              JOIN bibliographies b ON b.id = s.bibliography_id
             WHERE {CARRIES}
            UNION
            SELECT pv.paper_id FROM paper_versions pv
              JOIN version_snapshots vs ON vs.id = pv.snapshot_id
              JOIN bibliographies b ON b.id = vs.bibliography_id
             WHERE {CARRIES}
         )
         SELECT p.id, p.citation_key, b.title, s.deleted_at IS NOT NULL
           FROM hits h
           JOIN papers p ON p.id = h.paper_id
           JOIN paper_snapshots s ON s.id = p.snapshot_id
           JOIN bibliographies b ON b.id = s.bibliography_id
          ORDER BY s.deleted_at IS NOT NULL, p.citation_key"
    );
    let mut statement = db.prepare(&sql)?;
    let rows = statement.query_map(
        params![doi.map(Doi::as_str), arxiv.map(ArxivId::base_id)],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, bool>(3)?,
            ))
        },
    )?;
    let mut found = Vec::new();
    for row in rows {
        let (id, citation_key, title, deleted) = row?;
        found.push(IdentifierMatch {
            paper_id: sql::decode(id)?,
            citation_key,
            title,
            deleted,
        });
    }
    Ok(found)
}

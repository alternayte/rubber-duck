use std::collections::HashMap;

use rusqlite::{params, Connection};

use crate::error::AppResult;
use crate::rag::model::RetrievedChunk;

const RRF_K: f64 = 60.0;
const TOP_K: usize = 8;

pub fn extract_search_terms(text: &str) -> Vec<String> {
    let mut terms = Vec::new();

    // CamelCase identifiers
    let camel_re = regex::Regex::new(r"\b[A-Z][a-z]+(?:[A-Z][a-z]+)+\b").unwrap();
    for m in camel_re.find_iter(text) {
        terms.push(m.as_str().to_string());
    }

    // snake_case identifiers
    let snake_re = regex::Regex::new(r"\b[a-z]+(?:_[a-z]+)+\b").unwrap();
    for m in snake_re.find_iter(text) {
        terms.push(m.as_str().to_string());
    }

    // Quoted strings
    let quote_re = regex::Regex::new(r#""([^"]+)""#).unwrap();
    for cap in quote_re.captures_iter(text) {
        if let Some(m) = cap.get(1) {
            terms.push(m.as_str().to_string());
        }
    }

    // @mentions (strip the @)
    let mention_re = regex::Regex::new(r"@([\w.\-]+/[\w.\-/]+)").unwrap();
    for cap in mention_re.captures_iter(text) {
        if let Some(m) = cap.get(1) {
            terms.push(m.as_str().to_string());
        }
    }

    terms.dedup();
    terms
}

pub fn build_expanded_fts_query(user_text: &str) -> String {
    let terms = extract_search_terms(user_text);
    let words: Vec<&str> = user_text
        .split_whitespace()
        .filter(|w| w.len() > 3)
        .take(10)
        .collect();

    let mut fts_parts: Vec<String> = terms.iter().map(|t| format!("\"{t}\"")).collect();
    for w in words {
        let cleaned = w.trim_matches(|c: char| !c.is_alphanumeric());
        if cleaned.len() > 3 && !fts_parts.iter().any(|p| p.contains(cleaned)) {
            fts_parts.push(format!("\"{cleaned}\""));
        }
    }

    if fts_parts.is_empty() {
        return format!("\"{}\"", user_text.replace('"', "\"\""));
    }

    fts_parts.join(" OR ")
}

pub fn adaptive_top_k(text: &str) -> usize {
    let terms = extract_search_terms(text);
    if terms.len() > 3 { 15 } else { TOP_K }
}

struct RankedChunk {
    chunk_id: i64,
    rrf_score: f64,
}

fn fuse_rrf(semantic: &[(i64, f64)], keyword: &[i64]) -> Vec<RankedChunk> {
    let mut scores: HashMap<i64, f64> = HashMap::new();

    for (rank, &(chunk_id, _distance)) in semantic.iter().enumerate() {
        *scores.entry(chunk_id).or_default() += 1.0 / (RRF_K + rank as f64 + 1.0);
    }

    for (rank, &chunk_id) in keyword.iter().enumerate() {
        *scores.entry(chunk_id).or_default() += 1.0 / (RRF_K + rank as f64 + 1.0);
    }

    let mut ranked: Vec<RankedChunk> = scores
        .into_iter()
        .map(|(chunk_id, rrf_score)| RankedChunk { chunk_id, rrf_score })
        .collect();

    ranked.sort_by(|a, b| b.rrf_score.partial_cmp(&a.rrf_score).unwrap_or(std::cmp::Ordering::Equal));
    ranked
}

fn embedding_to_bytes(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

pub fn hybrid_search(
    conn: &Connection,
    query_embedding: &[f32],
    query_text: &str,
    repo_ids: &[String],
) -> AppResult<Vec<RetrievedChunk>> {
    if repo_ids.is_empty() {
        return Ok(vec![]);
    }

    // Build set of valid chunk IDs for these repos
    let placeholders: String = repo_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");

    // Semantic search: top 40 from sqlite-vec
    let emb_bytes = embedding_to_bytes(query_embedding);
    let mut semantic_stmt = conn.prepare(
        "SELECT chunk_id, distance FROM code_chunks_vec WHERE embedding MATCH ?1 AND k = 40",
    )?;
    let semantic_raw: Vec<(i64, f64)> = semantic_stmt
        .query_map(params![emb_bytes], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, f64>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    // Filter to session repos
    let valid_query = format!("SELECT id FROM code_chunks WHERE repo_id IN ({placeholders})");
    let mut valid_stmt = conn.prepare(&valid_query)?;
    let valid_ids: std::collections::HashSet<i64> = valid_stmt
        .query_map(rusqlite::params_from_iter(repo_ids.iter()), |row| row.get::<_, i64>(0))?
        .collect::<Result<std::collections::HashSet<_>, _>>()?;

    let semantic: Vec<(i64, f64)> = semantic_raw
        .into_iter()
        .filter(|(id, _)| valid_ids.contains(id))
        .take(20)
        .collect();

    // Keyword search via FTS5 — gracefully handle FTS syntax errors
    let expanded_query = build_expanded_fts_query(query_text);
    let fts_query = format!(
        "SELECT cf.rowid FROM code_chunks_fts cf
         JOIN code_chunks cc ON cc.id = cf.rowid
         WHERE cf.content MATCH ?1 AND cc.repo_id IN ({placeholders})
         ORDER BY cf.rank
         LIMIT 20"
    );
    let mut fts_params: Vec<Box<dyn rusqlite::types::ToSql>> =
        vec![Box::new(expanded_query)];
    for rid in repo_ids {
        fts_params.push(Box::new(rid.clone()));
    }
    let keyword: Vec<i64> = match conn.prepare(&fts_query) {
        Ok(mut fts_stmt) => {
            match fts_stmt
                .query_map(rusqlite::params_from_iter(fts_params.iter().map(|p| p.as_ref())), |row| {
                    row.get::<_, i64>(0)
                }) {
                Ok(rows) => rows.collect::<Result<Vec<_>, _>>().unwrap_or_default(),
                Err(_) => vec![],
            }
        }
        Err(_) => vec![],
    };

    // RRF fusion
    let ranked = fuse_rrf(&semantic, &keyword);
    let top_k = adaptive_top_k(query_text);

    // Fetch chunk details for top results
    let top_ids: Vec<i64> = ranked.iter().take(top_k).map(|r| r.chunk_id).collect();
    if top_ids.is_empty() {
        return Ok(vec![]);
    }

    let id_placeholders: String = top_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let detail_query = format!(
        "SELECT cc.id, cc.file_path, cc.start_line, cc.end_line, cc.content,
                r.name as repo_name
         FROM code_chunks cc
         JOIN repos r ON r.id = cc.repo_id
         WHERE cc.id IN ({id_placeholders})"
    );
    let mut detail_stmt = conn.prepare(&detail_query)?;
    let details: HashMap<i64, RetrievedChunk> = detail_stmt
        .query_map(rusqlite::params_from_iter(top_ids.iter()), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                RetrievedChunk {
                    file_path: row.get(1)?,
                    repo_name: row.get(5)?,
                    start_line: row.get::<_, i64>(2)? as usize,
                    end_line: row.get::<_, i64>(3)? as usize,
                    content: row.get(4)?,
                    score: 0.0,
                },
            ))
        })?
        .collect::<Result<HashMap<_, _>, _>>()?;

    let results: Vec<RetrievedChunk> = ranked
        .iter()
        .take(top_k)
        .filter_map(|r| {
            details.get(&r.chunk_id).map(|chunk| {
                let mut c = chunk.clone();
                c.score = r.rrf_score;
                c
            })
        })
        .collect();

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::rag::store;

    // --- RRF unit tests ---

    #[test]
    fn rrf_fusion_combines_lists() {
        let semantic = vec![(1, 0.1), (2, 0.2), (3, 0.3)];
        let keyword = vec![2, 4, 1];
        let fused = fuse_rrf(&semantic, &keyword);
        assert_eq!(fused[0].chunk_id, 2); // appears in both at good positions
        assert_eq!(fused[1].chunk_id, 1); // also in both
        assert_eq!(fused.len(), 4);
    }

    #[test]
    fn rrf_fusion_empty_lists() {
        let fused = fuse_rrf(&[], &[]);
        assert!(fused.is_empty());
    }

    #[test]
    fn rrf_fusion_single_list() {
        let semantic = vec![(10, 0.1), (20, 0.2)];
        let fused = fuse_rrf(&semantic, &[]);
        assert_eq!(fused.len(), 2);
        assert_eq!(fused[0].chunk_id, 10);
    }

    // --- Integration tests ---

    fn test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn setup_session_and_repo(conn: &rusqlite::Connection) {
        conn.execute(
            "INSERT INTO sessions (id, title) VALUES (?1, ?2)",
            params!["s1", "Test"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO repos (id, session_id, name, source, local_path) VALUES (?1, ?2, ?3, ?4, ?5)",
            params!["r1", "s1", "my-repo", "/path", "/path"],
        )
        .unwrap();
    }

    #[test]
    fn hybrid_search_finds_by_keyword() {
        let db = test_db();
        let conn = db.conn().unwrap();
        setup_session_and_repo(&conn);
        let emb = vec![0.1_f32; 384];
        store::insert_chunk(
            &conn,
            "r1",
            "auth.rs",
            1,
            10,
            "fn authenticate_user() { check_password(); }",
            "rust",
            &emb,
        )
        .unwrap();
        store::insert_chunk(
            &conn,
            "r1",
            "db.rs",
            1,
            10,
            "fn connect_database() { pool.get(); }",
            "rust",
            &emb,
        )
        .unwrap();
        let query_emb = vec![0.1_f32; 384];
        let results =
            hybrid_search(&conn, &query_emb, "authenticate", &["r1".to_string()]).unwrap();
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.file_path == "auth.rs"));
    }

    #[test]
    fn hybrid_search_empty_when_no_repos() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let query_emb = vec![0.1_f32; 384];
        let results = hybrid_search(&conn, &query_emb, "test", &[]).unwrap();
        assert!(results.is_empty());
    }
}

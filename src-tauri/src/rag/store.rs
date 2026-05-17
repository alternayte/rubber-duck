use rusqlite::{params, Connection};

use crate::error::AppResult;

fn embedding_to_bytes(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

pub fn insert_chunk(
    conn: &Connection,
    repo_id: &str,
    file_path: &str,
    start_line: usize,
    end_line: usize,
    content: &str,
    language: &str,
    embedding: &[f32],
) -> AppResult<i64> {
    conn.execute(
        "INSERT INTO code_chunks (repo_id, file_path, start_line, end_line, content, language)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![repo_id, file_path, start_line as i64, end_line as i64, content, language],
    )?;
    let chunk_id = conn.last_insert_rowid();
    let emb_bytes = embedding_to_bytes(embedding);
    conn.execute(
        "INSERT INTO code_chunks_vec (chunk_id, embedding) VALUES (?1, ?2)",
        params![chunk_id, emb_bytes],
    )?;
    Ok(chunk_id)
}

pub fn count_by_repo(conn: &Connection, repo_id: &str) -> AppResult<usize> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM code_chunks WHERE repo_id = ?1",
        params![repo_id],
        |row| row.get(0),
    )?;
    Ok(count as usize)
}

pub fn delete_by_repo(conn: &Connection, repo_id: &str) -> AppResult<()> {
    conn.execute(
        "DELETE FROM code_chunks_vec WHERE chunk_id IN (SELECT id FROM code_chunks WHERE repo_id = ?1)",
        params![repo_id],
    )?;
    conn.execute("DELETE FROM code_chunks WHERE repo_id = ?1", params![repo_id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn setup_session_and_repo(conn: &Connection) {
        conn.execute(
            "INSERT INTO sessions (id, title) VALUES (?1, ?2)",
            params!["s1", "Test"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO repos (id, session_id, name, source, local_path) VALUES (?1, ?2, ?3, ?4, ?5)",
            params!["r1", "s1", "test-repo", "/path", "/path"],
        )
        .unwrap();
    }

    fn dummy_embedding() -> Vec<f32> {
        vec![0.1; 384]
    }

    #[test]
    fn insert_and_count() {
        let db = test_db();
        let conn = db.conn().unwrap();
        setup_session_and_repo(&conn);
        let id = insert_chunk(
            &conn,
            "r1",
            "src/main.rs",
            1,
            10,
            "fn main() {}",
            "rust",
            &dummy_embedding(),
        )
        .unwrap();
        assert!(id > 0);
        let count = count_by_repo(&conn, "r1").unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn delete_by_repo_removes_chunks_and_vectors() {
        let db = test_db();
        let conn = db.conn().unwrap();
        setup_session_and_repo(&conn);
        insert_chunk(
            &conn,
            "r1",
            "a.rs",
            1,
            5,
            "fn a() {}",
            "rust",
            &dummy_embedding(),
        )
        .unwrap();
        insert_chunk(
            &conn,
            "r1",
            "b.rs",
            1,
            5,
            "fn b() {}",
            "rust",
            &dummy_embedding(),
        )
        .unwrap();
        delete_by_repo(&conn, "r1").unwrap();
        assert_eq!(count_by_repo(&conn, "r1").unwrap(), 0);
        let vec_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM code_chunks_vec", [], |row| row.get(0))
            .unwrap();
        assert_eq!(vec_count, 0);
    }

    #[test]
    fn fts5_synced_on_insert_and_delete() {
        let db = test_db();
        let conn = db.conn().unwrap();
        setup_session_and_repo(&conn);
        insert_chunk(
            &conn,
            "r1",
            "main.rs",
            1,
            5,
            "fn calculate_total() {}",
            "rust",
            &dummy_embedding(),
        )
        .unwrap();
        let found: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM code_chunks_fts WHERE code_chunks_fts MATCH ?1",
                ["calculate_total"],
                |row| row.get(0),
            )
            .unwrap();
        assert!(found);
        delete_by_repo(&conn, "r1").unwrap();
        let found_after: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM code_chunks_fts WHERE code_chunks_fts MATCH ?1",
                ["calculate_total"],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!found_after);
    }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct CodeChunk {
    pub file_path: String,
    pub start_line: usize,
    pub end_line: usize,
    pub content: String,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedChunk {
    pub file_path: String,
    pub repo_name: String,
    pub start_line: usize,
    pub end_line: usize,
    pub content: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexProgress {
    pub repo_id: String,
    pub repo_name: String,
    pub files_done: usize,
    pub files_total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexStatus {
    pub indexed: bool,
    pub chunk_count: usize,
}

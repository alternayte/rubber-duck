use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoContext {
    pub id: String,
    pub session_id: String,
    pub name: String,
    pub source: String,
    pub local_path: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Vec<FileNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSearchResult {
    pub repo_id: String,
    pub repo_name: String,
    pub relative_path: String,
    pub display: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoFileContext {
    pub display: String,
    pub content: String,
}

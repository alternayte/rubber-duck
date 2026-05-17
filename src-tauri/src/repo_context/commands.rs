use tauri::{AppHandle, Manager, State};

use crate::db::Database;
use super::clone;
use super::model::{FileNode, FileSearchResult, RepoContext};
use super::store;
use super::tree;

#[tauri::command]
pub async fn attach_repo(
    app: AppHandle,
    db: State<'_, Database>,
    session_id: String,
    source: String,
) -> Result<RepoContext, String> {
    let (name, local_path) = if clone::is_git_url(&source) {
        let repos_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| e.to_string())?
            .join("repos");
        let path = clone::clone_repo(&source, &repos_dir).map_err(|e| e.to_string())?;
        let name = clone::extract_repo_name(&source);
        (name, path.to_string_lossy().to_string())
    } else {
        let path = std::path::Path::new(&source);
        if !path.is_dir() {
            return Err(format!("Not a directory: {source}"));
        }
        let canonical = path.canonicalize().map_err(|e| e.to_string())?;
        let name = canonical
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "repo".to_string());
        (name, canonical.to_string_lossy().to_string())
    };

    let conn = db.conn().map_err(|e| e.to_string())?;
    store::attach(&conn, &session_id, &name, &source, &local_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn detach_repo(db: State<Database>, repo_id: String) -> Result<(), String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::detach(&conn, &repo_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_repos(db: State<Database>, session_id: String) -> Result<Vec<RepoContext>, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::list_by_session(&conn, &session_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_repo_tree(db: State<Database>, repo_id: String) -> Result<Vec<FileNode>, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    let repo = store::get(&conn, &repo_id).map_err(|e| e.to_string())?;
    tree::walk_tree(std::path::Path::new(&repo.local_path)).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn read_repo_file(db: State<Database>, repo_id: String, path: String) -> Result<String, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    let repo = store::get(&conn, &repo_id).map_err(|e| e.to_string())?;
    tree::read_file(std::path::Path::new(&repo.local_path), &path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn search_repo_files(
    db: State<Database>,
    session_id: String,
    query: String,
) -> Result<Vec<FileSearchResult>, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    let repos = store::list_by_session(&conn, &session_id).map_err(|e| e.to_string())?;

    let mut all_results = Vec::new();
    for repo in &repos {
        match tree::search_files(std::path::Path::new(&repo.local_path), &query, &repo.id, &repo.name) {
            Ok(results) => all_results.extend(results),
            Err(e) => tracing::warn!("Search failed for repo {}: {e}", repo.name),
        }
    }

    all_results.sort_by(|a, b| a.display.cmp(&b.display));
    all_results.truncate(20);

    Ok(all_results)
}

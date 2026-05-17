use tauri::{AppHandle, State};

use crate::db::Database;
use crate::rag::{indexer, model::IndexStatus, store};
use crate::repo_context::store as repo_store;

#[tauri::command]
pub async fn index_repo(app: AppHandle, db: State<'_, Database>, repo_id: String) -> Result<(), String> {
    {
        let conn = db.conn().map_err(|e| e.to_string())?;
        let _ = repo_store::get(&conn, &repo_id).map_err(|e| e.to_string())?;
    }

    let app_clone = app.clone();
    let repo_id_clone = repo_id.clone();
    tokio::spawn(async move {
        if let Err(e) = indexer::run(&app_clone, &repo_id_clone).await {
            tracing::error!("Indexing failed for {repo_id_clone}: {e}");
        }
    });

    Ok(())
}

#[tauri::command]
pub fn get_index_status(db: State<Database>, repo_id: String) -> Result<IndexStatus, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    let chunk_count = store::count_by_repo(&conn, &repo_id).map_err(|e| e.to_string())?;
    Ok(IndexStatus {
        indexed: chunk_count > 0,
        chunk_count,
    })
}

#[tauri::command]
pub async fn reindex_repo(
    app: AppHandle,
    db: State<'_, Database>,
    repo_id: String,
) -> Result<(), String> {
    {
        let conn = db.conn().map_err(|e| e.to_string())?;
        let _ = repo_store::get(&conn, &repo_id).map_err(|e| e.to_string())?;
        store::delete_by_repo(&conn, &repo_id).map_err(|e| e.to_string())?;
    }

    let app_clone = app.clone();
    let repo_id_clone = repo_id.clone();
    tokio::spawn(async move {
        if let Err(e) = indexer::run(&app_clone, &repo_id_clone).await {
            tracing::error!("Re-indexing failed for {repo_id_clone}: {e}");
        }
    });

    Ok(())
}

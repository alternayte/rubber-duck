use std::path::Path;

use tauri::{AppHandle, Emitter, Manager, State};

use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::rag::{chunker, embedder::Embedder, model::IndexProgress, store};
use crate::repo_context::store as repo_store;

pub async fn run(app: &AppHandle, repo_id: &str) -> AppResult<()> {
    let (repo_name, local_path) = {
        let db: State<Database> = app.state::<Database>();
        let conn = db.conn()?;
        let repo = repo_store::get(&conn, repo_id)?;
        (repo.name, repo.local_path)
    };

    let repo_path = Path::new(&local_path);
    let files = collect_indexable_files(repo_path)?;
    let total = files.len();

    for (i, file_path) in files.iter().enumerate() {
        let bytes = match std::fs::read(file_path) {
            Ok(b) => b,
            Err(_) => {
                emit_progress(app, repo_id, &repo_name, i + 1, total);
                continue;
            }
        };
        if bytes.contains(&0) {
            emit_progress(app, repo_id, &repo_name, i + 1, total);
            continue;
        }
        let content = String::from_utf8_lossy(&bytes);

        let rel_path = file_path
            .strip_prefix(repo_path)
            .unwrap_or(file_path)
            .to_string_lossy();
        let language = chunker::detect_language(&rel_path).unwrap_or("text");
        let chunks = if language == "text" {
            chunker::chunk_lines(&content, &rel_path, 50, 10)
        } else {
            chunker::chunk_file(&content, &rel_path, language, 1600)
        };

        if !chunks.is_empty() {
            let texts: Vec<String> = chunks.iter().map(|c| c.content.clone()).collect();

            let embedder: State<Embedder> = app.state::<Embedder>();
            let embeddings = embedder.embed(&texts)?;

            let db: State<Database> = app.state::<Database>();
            let conn = db.conn()?;
            for (chunk, embedding) in chunks.iter().zip(embeddings.iter()) {
                store::insert_chunk(
                    &conn,
                    repo_id,
                    &chunk.file_path,
                    chunk.start_line,
                    chunk.end_line,
                    &chunk.content,
                    &chunk.language,
                    embedding,
                )?;
            }
        }

        emit_progress(app, repo_id, &repo_name, i + 1, total);
    }

    let _ = app.emit("index:done", serde_json::json!({ "repo_id": repo_id }));
    Ok(())
}

fn emit_progress(app: &AppHandle, repo_id: &str, repo_name: &str, done: usize, total: usize) {
    let _ = app.emit(
        "index:progress",
        IndexProgress {
            repo_id: repo_id.to_string(),
            repo_name: repo_name.to_string(),
            files_done: done,
            files_total: total,
        },
    );
}

fn collect_indexable_files(repo_path: &Path) -> AppResult<Vec<std::path::PathBuf>> {
    let walker = ignore::WalkBuilder::new(repo_path)
        .hidden(false)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .filter_entry(|e| e.file_name() != ".git")
        .build();

    let mut files = Vec::new();
    for result in walker {
        let entry = result.map_err(|e| AppError::Other(e.to_string()))?;
        let is_file = entry.file_type().map(|ft| ft.is_file()).unwrap_or(false);
        if !is_file {
            continue;
        }
        if entry.metadata().ok().map(|m| m.len() > 100_000).unwrap_or(false) {
            continue;
        }
        files.push(entry.path().to_path_buf());
    }

    Ok(files)
}

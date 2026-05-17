mod db;
mod error;
mod llm;
mod session;
mod settings;
mod jira;
mod repo_context;
mod ticket;

use tauri::Manager;

use db::Database;
use session::commands::*;
use settings::commands::*;
use ticket::commands::*;
use jira::commands::*;
use repo_context::commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .register_uri_scheme_protocol("rdimg", |ctx, request| {
            let make_response = |status: u16, body: Vec<u8>, content_type: &str| {
                tauri::http::Response::builder()
                    .status(status)
                    .header("Content-Type", content_type)
                    .body(body)
                    .unwrap_or_else(|_| tauri::http::Response::new(Vec::new()))
            };

            let uri = request.uri();
            let decoded = percent_encoding::percent_decode_str(uri.path())
                .decode_utf8_lossy()
                .to_string();
            let fs_path = if decoded.starts_with('/') {
                &decoded[1..]
            } else {
                &decoded
            };

            let images_root = match ctx.app_handle().path().app_data_dir() {
                Ok(dir) => dir.join("images"),
                Err(_) => return make_response(500, Vec::new(), "text/plain"),
            };

            let canonical = match std::fs::canonicalize(fs_path) {
                Ok(p) => p,
                Err(_) => return make_response(404, Vec::new(), "text/plain"),
            };

            let canonical_root = match std::fs::canonicalize(&images_root) {
                Ok(p) => p,
                Err(_) => return make_response(404, Vec::new(), "text/plain"),
            };

            if !canonical.starts_with(&canonical_root) {
                return make_response(403, Vec::new(), "text/plain");
            }

            let content_type = match canonical.extension().and_then(|e| e.to_str()) {
                Some("png") => "image/png",
                Some("jpg" | "jpeg") => "image/jpeg",
                Some("gif") => "image/gif",
                Some("webp") => "image/webp",
                _ => "application/octet-stream",
            };

            match std::fs::read(&canonical) {
                Ok(bytes) => make_response(200, bytes, content_type),
                Err(_) => make_response(404, Vec::new(), "text/plain"),
            }
        })
        .invoke_handler(tauri::generate_handler![
            create_session,
            get_session,
            list_sessions,
            update_session,
            archive_session,
            get_or_create_note,
            update_note,
            get_conversation,
            delete_conversation_from,
            get_setting,
            set_setting,
            set_api_key,
            has_api_key,
            get_available_models,
            llm::streaming::send_message,
            create_ticket,
            get_ticket,
            list_tickets,
            update_ticket,
            delete_ticket,
            reorder_ticket,
            set_ticket_parent,
            save_pasted_image,
            get_jira_config,
            set_jira_config,
            set_jira_api_token,
            has_jira_config,
            test_jira_connection,
            push_ticket_to_jira,
            get_jira_projects,
            fetch_jira_issues,
            attach_repo,
            detach_repo,
            list_repos,
            get_repo_tree,
            read_repo_file,
            search_repo_files,
        ])
        .setup(|app| {
            let app_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_dir)?;
            let db_path = app_dir.join("rubber-duck.db");
            let db = Database::open(db_path.to_str().expect("invalid db path"))?;
            app.manage(db);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

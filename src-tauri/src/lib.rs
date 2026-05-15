mod db;
mod error;
mod llm;
mod session;
mod settings;
mod ticket;

use tauri::Manager;

use db::Database;
use session::commands::*;
use settings::commands::*;
use ticket::commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .register_uri_scheme_protocol("rdimg", |_ctx, request| {
            let uri = request.uri();
            let decoded = percent_encoding::percent_decode_str(uri.path())
                .decode_utf8_lossy()
                .to_string();
            // URI path starts with '/' on all platforms; strip it to get the absolute fs path
            let fs_path = if decoded.starts_with('/') {
                decoded[1..].to_string()
            } else {
                decoded
            };
            match std::fs::read(&fs_path) {
                Ok(bytes) => tauri::http::Response::builder()
                    .header("Content-Type", "image/png")
                    .body(bytes)
                    .unwrap(),
                Err(_) => tauri::http::Response::builder()
                    .status(404)
                    .body(Vec::new())
                    .unwrap(),
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

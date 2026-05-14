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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            create_session,
            get_session,
            list_sessions,
            update_session,
            archive_session,
            get_or_create_note,
            update_note,
            get_setting,
            set_setting,
            set_api_key,
            has_api_key,
            get_available_models,
            llm::streaming::send_message,
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

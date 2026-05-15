use std::fs;
use std::path::PathBuf;

use crate::error::{AppError, AppResult};

pub fn save_image(app_data_dir: &str, session_id: &str, base64_data: &str) -> AppResult<String> {
    let images_dir = PathBuf::from(app_data_dir).join("images").join(session_id);
    fs::create_dir_all(&images_dir)?;

    let id = uuid::Uuid::new_v4().to_string();
    let file_name = format!("{id}.png");
    let file_path = images_dir.join(&file_name);

    // Strip data URL prefix if present (e.g., "data:image/png;base64,")
    let raw_base64 = if let Some(pos) = base64_data.find(',') {
        &base64_data[pos + 1..]
    } else {
        base64_data
    };

    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(raw_base64)
        .map_err(|e| AppError::Other(format!("Invalid base64: {e}")))?;

    fs::write(&file_path, &bytes)?;

    Ok(file_path.to_string_lossy().to_string())
}

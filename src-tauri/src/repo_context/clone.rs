use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{AppError, AppResult};

pub fn extract_repo_name(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    let last_segment = trimmed.rsplit('/').next().unwrap_or(trimmed);
    last_segment.trim_end_matches(".git").to_string()
}

pub fn is_git_url(source: &str) -> bool {
    source.starts_with("http://")
        || source.starts_with("https://")
        || source.starts_with("git@")
        || source.starts_with("ssh://")
}

pub fn clone_repo(url: &str, repos_dir: &Path) -> AppResult<PathBuf> {
    let name = extract_repo_name(url);
    let target = repos_dir.join(&name);

    if target.exists() {
        let output = Command::new("git")
            .args(["pull", "--ff-only"])
            .current_dir(&target)
            .output()
            .map_err(|e| AppError::Other(format!("Failed to run git: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("git pull failed for {name}: {stderr}");
        }
    } else {
        std::fs::create_dir_all(repos_dir)?;

        let output = Command::new("git")
            .args(["clone", "--depth", "1", url, target.to_str().unwrap_or_default()])
            .output()
            .map_err(|e| AppError::Other(format!("Failed to run git: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AppError::Other(format!("git clone failed: {stderr}")));
        }
    }

    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_repo_name_from_https() {
        assert_eq!(extract_repo_name("https://github.com/user/my-repo.git"), "my-repo");
    }

    #[test]
    fn extract_repo_name_from_ssh() {
        assert_eq!(extract_repo_name("git@github.com:user/my-repo.git"), "my-repo");
    }

    #[test]
    fn extract_repo_name_no_git_suffix() {
        assert_eq!(extract_repo_name("https://github.com/user/my-repo"), "my-repo");
    }

    #[test]
    fn extract_repo_name_trailing_slash() {
        assert_eq!(extract_repo_name("https://github.com/user/my-repo/"), "my-repo");
    }

    #[test]
    fn is_git_url_detects_urls() {
        assert!(is_git_url("https://github.com/user/repo.git"));
        assert!(is_git_url("git@github.com:user/repo.git"));
        assert!(is_git_url("ssh://git@github.com/user/repo.git"));
        assert!(is_git_url("http://github.com/user/repo.git"));
        assert!(!is_git_url("/Users/nate/code/my-repo"));
        assert!(!is_git_url("~/code/my-repo"));
    }
}

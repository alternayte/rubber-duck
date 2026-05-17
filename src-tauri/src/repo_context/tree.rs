use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::{AppError, AppResult};
use super::model::{FileNode, FileSearchResult};

/// Walk the directory tree at `repo_path`, respecting .gitignore.
/// Caps at depth 6 and 5 000 entries. Directories sort before files,
/// alphabetically within each group.
pub fn walk_tree(repo_path: &Path) -> AppResult<Vec<FileNode>> {
    let walker = ignore::WalkBuilder::new(repo_path)
        .hidden(false)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .max_depth(Some(6))
        .filter_entry(|e| e.file_name() != ".git")
        .build();

    // Collect (path, is_dir) pairs, skip the root itself.
    let mut entries: Vec<(PathBuf, bool)> = Vec::new();
    let mut count = 0usize;
    for result in walker {
        let entry = result.map_err(|e| AppError::Other(e.to_string()))?;
        // Skip the root directory entry itself.
        if entry.path() == repo_path {
            continue;
        }
        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        entries.push((entry.path().to_path_buf(), is_dir));
        count += 1;
        if count >= 5000 {
            break;
        }
    }

    build_tree(repo_path, &entries)
}

/// Build a nested FileNode tree from a flat list of paths.
fn build_tree(repo_root: &Path, entries: &[(PathBuf, bool)]) -> AppResult<Vec<FileNode>> {
    // We need to build the tree bottom-up by assembling children maps.
    // Strategy: collect all unique parent dirs and their immediate children.

    // Map from parent_relative_path -> list of (child_name, child_path, is_dir)
    let mut children_map: HashMap<PathBuf, Vec<(String, PathBuf, bool)>> = HashMap::new();

    for (abs_path, is_dir) in entries {
        let rel = abs_path
            .strip_prefix(repo_root)
            .map_err(|e| AppError::Other(e.to_string()))?;
        let parent = rel.parent().unwrap_or(Path::new("")).to_path_buf();
        let name = abs_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let entry = (name, rel.to_path_buf(), *is_dir);
        children_map.entry(parent).or_default().push(entry);
    }

    // Sort each level: dirs first, then alphabetical within each group.
    for children in children_map.values_mut() {
        children.sort_by(|a, b| {
            // a.2 / b.2 == is_dir
            match (a.2, b.2) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.0.to_lowercase().cmp(&b.0.to_lowercase()),
            }
        });
    }

    build_children(Path::new(""), &children_map)
}

fn build_children(
    parent_rel: &Path,
    map: &HashMap<PathBuf, Vec<(String, PathBuf, bool)>>,
) -> AppResult<Vec<FileNode>> {
    let Some(items) = map.get(parent_rel) else {
        return Ok(Vec::new());
    };

    let mut nodes = Vec::new();
    for (name, rel_path, is_dir) in items {
        let children = if *is_dir {
            build_children(rel_path, map)?
        } else {
            Vec::new()
        };
        nodes.push(FileNode {
            name: name.clone(),
            path: rel_path.to_string_lossy().into_owned(),
            is_dir: *is_dir,
            children,
        });
    }
    Ok(nodes)
}

/// Read a file as UTF-8. Rejects binary files and path traversal attempts.
pub fn read_file(repo_path: &Path, relative_path: &str) -> AppResult<String> {
    // Canonicalize the repo root first.
    let canonical_root = repo_path
        .canonicalize()
        .map_err(|e| AppError::Other(format!("Cannot canonicalize repo root: {e}")))?;

    // Build the candidate path and canonicalize it.
    let candidate = canonical_root.join(relative_path);
    let canonical_file = candidate.canonicalize().map_err(|e| {
        AppError::Other(format!("Cannot resolve path '{relative_path}': {e}"))
    })?;

    // Security: ensure the resolved path is inside the repo root.
    if !canonical_file.starts_with(&canonical_root) {
        return Err(AppError::Other(format!(
            "Path traversal detected: '{relative_path}' resolves outside repo root"
        )));
    }

    // Read raw bytes; check first 512 bytes for null bytes (binary detection).
    let bytes = std::fs::read(&canonical_file)?;
    let probe = &bytes[..bytes.len().min(512)];
    if probe.contains(&0u8) {
        return Err(AppError::Other(format!(
            "File '{relative_path}' appears to be binary"
        )));
    }

    String::from_utf8(bytes)
        .map_err(|_| AppError::Other(format!("File '{relative_path}' is not valid UTF-8")))
}

/// Walk the top 2 levels, count files by extension, identify well-known dirs.
/// Returns a human-readable summary string.
pub fn generate_summary(repo_path: &Path) -> AppResult<String> {
    let walker = ignore::WalkBuilder::new(repo_path)
        .hidden(false)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .max_depth(Some(2))
        .filter_entry(|e| e.file_name() != ".git")
        .build();

    let well_known = [
        "src", "lib", "test", "tests", "docs", "cmd", "internal", "api", "pkg", "app",
        "components", "features",
    ];

    let mut ext_counts: HashMap<String, usize> = HashMap::new();
    let mut found_dirs: Vec<String> = Vec::new();
    let mut file_count = 0usize;

    for result in walker {
        let entry = result.map_err(|e| AppError::Other(e.to_string()))?;
        if entry.path() == repo_path {
            continue;
        }
        let ft = entry.file_type().unwrap_or_else(|| {
            // Should not happen for real entries, but guard anyway.
            // We'll treat unknown as a file.
            unreachable!("entry without file type")
        });

        if ft.is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                if well_known.contains(&name) {
                    found_dirs.push(format!("{name}/"));
                }
            }
        } else if ft.is_file() {
            file_count += 1;
            if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                *ext_counts.entry(format!(".{ext}")).or_insert(0) += 1;
            }
        }
    }

    // Sort extensions by count descending, take top 5.
    let mut ext_list: Vec<(String, usize)> = ext_counts.into_iter().collect();
    ext_list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let top_types: Vec<String> = ext_list
        .iter()
        .take(5)
        .map(|(ext, cnt)| format!("{ext}({cnt})"))
        .collect();

    found_dirs.sort();

    let mut parts = vec![format!("{file_count} files.")];
    if !top_types.is_empty() {
        parts.push(format!("Top types: {}.", top_types.join(", ")));
    }
    if !found_dirs.is_empty() {
        parts.push(format!("Key dirs: {}.", found_dirs.join(", ")));
    }

    Ok(parts.join(" "))
}

/// Fuzzy-match `query` against relative file paths in the repo.
/// Scoring: exact filename match = 100, filename contains query = 50,
/// path contains query = 10. Returns top 20 by score, case-insensitive.
pub fn search_files(
    repo_path: &Path,
    query: &str,
    repo_id: &str,
    repo_name: &str,
) -> AppResult<Vec<FileSearchResult>> {
    let query_lower = query.to_lowercase();

    let walker = ignore::WalkBuilder::new(repo_path)
        .hidden(false)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .max_depth(Some(6))
        .filter_entry(|e| e.file_name() != ".git")
        .build();

    let mut scored: Vec<(u32, FileSearchResult)> = Vec::new();

    for result in walker {
        let entry = result.map_err(|e| AppError::Other(e.to_string()))?;
        if entry.path() == repo_path {
            continue;
        }
        let ft = entry.file_type().map(|f| f.is_file()).unwrap_or(false);
        if !ft {
            continue;
        }

        let rel = entry
            .path()
            .strip_prefix(repo_path)
            .map_err(|e| AppError::Other(e.to_string()))?;
        let rel_str = rel.to_string_lossy().into_owned();
        let rel_lower = rel_str.to_lowercase();

        let filename_lower = entry
            .file_name()
            .to_str()
            .unwrap_or("")
            .to_lowercase();

        let score: u32 = if filename_lower == query_lower {
            100
        } else if filename_lower.contains(&query_lower) {
            50
        } else if rel_lower.contains(&query_lower) {
            10
        } else {
            continue; // no match
        };

        scored.push((
            score,
            FileSearchResult {
                repo_id: repo_id.to_string(),
                repo_name: repo_name.to_string(),
                relative_path: rel_str.clone(),
                display: format!("{repo_name}: {rel_str}"),
            },
        ));
    }

    // Sort by score descending, stable.
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.truncate(20);

    Ok(scored.into_iter().map(|(_, r)| r).collect())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    /// Create a small test repo:
    ///   .git/            (bare init so ignore respects .gitignore)
    ///   src/main.ts
    ///   src/features/auth.ts
    ///   README.md
    ///   .gitignore  (contains "node_modules/")
    ///   node_modules/dep/index.js
    fn make_test_repo() -> tempfile::TempDir {
        let dir = tempdir().expect("tempdir");
        let root = dir.path();

        // Initialise a real git repo so the `ignore` crate honours .gitignore.
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(root)
            .status()
            .expect("git init");

        fs::create_dir_all(root.join("src/features")).unwrap();
        fs::create_dir_all(root.join("node_modules/dep")).unwrap();

        fs::write(root.join("src/main.ts"), "const x = 1;").unwrap();
        fs::write(root.join("src/features/auth.ts"), "export function auth() {}").unwrap();
        fs::write(root.join("README.md"), "# Test repo").unwrap();
        fs::write(root.join(".gitignore"), "node_modules/\n").unwrap();
        fs::write(root.join("node_modules/dep/index.js"), "module.exports = {};").unwrap();

        dir
    }

    #[test]
    fn walk_tree_respects_gitignore() {
        let repo = make_test_repo();
        let tree = walk_tree(repo.path()).expect("walk_tree");

        let all_paths: Vec<String> = collect_all_paths(&tree);
        assert!(
            all_paths.iter().all(|p| !p.contains("node_modules")),
            "node_modules should be excluded by .gitignore; found: {all_paths:?}"
        );
    }

    #[test]
    fn walk_tree_nests_children() {
        let repo = make_test_repo();
        let tree = walk_tree(repo.path()).expect("walk_tree");

        // Find the `src` node.
        let src_node = tree.iter().find(|n| n.name == "src");
        assert!(src_node.is_some(), "src dir should be in tree root");
        let src = src_node.unwrap();
        assert!(src.is_dir);
        assert!(
            !src.children.is_empty(),
            "src should have children (main.ts, features/)"
        );
    }

    #[test]
    fn read_file_returns_content() {
        let repo = make_test_repo();
        let content = read_file(repo.path(), "README.md").expect("read_file");
        assert_eq!(content.trim(), "# Test repo");
    }

    #[test]
    fn read_file_rejects_traversal() {
        let repo = make_test_repo();
        let result = read_file(repo.path(), "../../../etc/passwd");
        assert!(
            result.is_err(),
            "Path traversal should return an error"
        );
    }

    #[test]
    fn generate_summary_counts_files() {
        let repo = make_test_repo();
        let summary = generate_summary(repo.path()).expect("generate_summary");
        assert!(
            summary.contains("files"),
            "Summary should mention 'files'; got: {summary}"
        );
        assert!(
            summary.contains(".ts"),
            "Summary should mention .ts extension; got: {summary}"
        );
    }

    #[test]
    fn search_files_finds_by_name() {
        let repo = make_test_repo();
        let results =
            search_files(repo.path(), "auth", "repo-1", "MyRepo").expect("search_files");
        assert!(
            !results.is_empty(),
            "Searching 'auth' should find auth.ts"
        );
        assert!(
            results.iter().any(|r| r.relative_path.contains("auth")),
            "Result should include auth.ts; got: {results:?}"
        );
    }

    #[test]
    fn search_files_returns_empty_for_no_match() {
        let repo = make_test_repo();
        let results =
            search_files(repo.path(), "zzzznotfound", "repo-1", "MyRepo")
                .expect("search_files");
        assert!(
            results.is_empty(),
            "Searching 'zzzznotfound' should return no results"
        );
    }

    // Helper: flatten all paths from a FileNode tree into a Vec<String>.
    fn collect_all_paths(nodes: &[FileNode]) -> Vec<String> {
        let mut acc = Vec::new();
        for node in nodes {
            acc.push(node.path.clone());
            acc.extend(collect_all_paths(&node.children));
        }
        acc
    }
}

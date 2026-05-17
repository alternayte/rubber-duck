# Repo Context Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Attach local folders or git repos to a session, browse file trees, `@` mention files for LLM context injection, with fuzzy file search autocomplete in chat and editor.

**Architecture:** New `repo_context/` feature module with store, tree walker (using `ignore` crate), git clone helper, and fuzzy search. Modified `assemble_context` and `send_message` to extract `@` mentions and inject file contents. Frontend: `RepoPanel` component in sidebar, `@` mention autocomplete in chat input and CodeMirror editor, `@` mention rendering in markdown.

**Tech Stack:** Rust (ignore, regex), React 19, CodeMirror 6 (autocompletion), Jotai, Tailwind, Tauri dialog + fs plugins

---

## File Map

| Action | File | Responsibility |
|--------|------|----------------|
| Create | `src-tauri/migrations/004_add_repos.sql` | New repos table |
| Modify | `src-tauri/src/db.rs` | Register migration |
| Create | `src-tauri/src/repo_context/mod.rs` | Module declarations |
| Create | `src-tauri/src/repo_context/model.rs` | RepoContext, FileNode, FileSearchResult, RepoFileContext |
| Create | `src-tauri/src/repo_context/store.rs` | CRUD for repos table + tests |
| Create | `src-tauri/src/repo_context/tree.rs` | .gitignore-aware tree walk + file read + summary + search + tests |
| Create | `src-tauri/src/repo_context/clone.rs` | Git clone/pull via Command + tests |
| Create | `src-tauri/src/repo_context/commands.rs` | Tauri commands |
| Modify | `src-tauri/src/lib.rs` | Register module + commands |
| Modify | `src-tauri/Cargo.toml` | Add `ignore` crate, `tauri-plugin-dialog`, `tauri-plugin-fs` |
| Modify | `src-tauri/capabilities/default.json` | Add dialog + fs permissions |
| Modify | `src-tauri/src/llm/context.rs` | Add repo_summaries + mentioned_files params |
| Modify | `src-tauri/src/llm/streaming.rs` | Extract @mentions, fetch files, pass to context |
| Create | `src/features/repo/RepoPanel.tsx` | Sidebar repo list + tree + attach dialog |
| Create | `src/features/repo/repo.atoms.ts` | Jotai atoms for repos state |
| Create | `src/features/repo/repo.types.ts` | TypeScript types |
| Create | `src/features/repo/useRepoActions.ts` | Hook for repo CRUD |
| Create | `src/features/repo/FileTree.tsx` | Expandable tree view component |
| Create | `src/components/AtMentionInput.tsx` | Chat input with @mention autocomplete |
| Modify | `src/components/MarkdownEditor.tsx` | Add @mention CodeMirror autocomplete |
| Create | `src/components/MentionText.tsx` | Render @mentions as styled pills in markdown |
| Modify | `src/features/chat/ChatPanel.tsx` | Use AtMentionInput, render mentions |
| Modify | `src/features/session/DumpView.tsx` | Render @mentions in preview |
| Modify | `src/App.tsx` | Replace Context placeholder with RepoPanel |

---

### Task 1: Database migration + model + store

**Files:**
- Create: `src-tauri/migrations/004_add_repos.sql`
- Modify: `src-tauri/src/db.rs`
- Create: `src-tauri/src/repo_context/mod.rs`
- Create: `src-tauri/src/repo_context/model.rs`
- Create: `src-tauri/src/repo_context/store.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create migration file**

Create `src-tauri/migrations/004_add_repos.sql`:

```sql
CREATE TABLE repos (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    source TEXT NOT NULL,
    local_path TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

- [ ] **Step 2: Register migration**

In `src-tauri/src/db.rs`, add to the `MIGRATIONS` array:

```rust
    Migration {
        name: "004_add_repos",
        sql: include_str!("../migrations/004_add_repos.sql"),
    },
```

- [ ] **Step 3: Create module files**

Create `src-tauri/src/repo_context/mod.rs`:

```rust
pub mod model;
pub mod store;
pub mod tree;
pub mod clone;
pub mod commands;
```

Create `src-tauri/src/repo_context/model.rs`:

```rust
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
```

- [ ] **Step 4: Write store tests**

Create `src-tauri/src/repo_context/store.rs`:

```rust
use rusqlite::{params, Connection};

use crate::error::AppResult;
use super::model::RepoContext;

fn row_to_repo(row: &rusqlite::Row) -> rusqlite::Result<RepoContext> {
    Ok(RepoContext {
        id: row.get(0)?,
        session_id: row.get(1)?,
        name: row.get(2)?,
        source: row.get(3)?,
        local_path: row.get(4)?,
        created_at: row.get(5)?,
    })
}

pub fn attach(
    conn: &Connection,
    session_id: &str,
    name: &str,
    source: &str,
    local_path: &str,
) -> AppResult<RepoContext> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO repos (id, session_id, name, source, local_path) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, session_id, name, source, local_path],
    )?;
    get(conn, &id)
}

pub fn detach(conn: &Connection, repo_id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM repos WHERE id = ?1", params![repo_id])?;
    Ok(())
}

pub fn list_by_session(conn: &Connection, session_id: &str) -> AppResult<Vec<RepoContext>> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, name, source, local_path, created_at FROM repos WHERE session_id = ?1 ORDER BY created_at ASC",
    )?;
    let repos = stmt.query_map(params![session_id], row_to_repo)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(repos)
}

pub fn get(conn: &Connection, repo_id: &str) -> AppResult<RepoContext> {
    let repo = conn.query_row(
        "SELECT id, session_id, name, source, local_path, created_at FROM repos WHERE id = ?1",
        params![repo_id],
        row_to_repo,
    )?;
    Ok(repo)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn attach_and_get() {
        let db = test_db();
        let conn = db.conn().unwrap();
        conn.execute("INSERT INTO sessions (id, title) VALUES (?1, ?2)", params!["s1", "Test"]).unwrap();

        let repo = attach(&conn, "s1", "my-repo", "/local/path", "/local/path").unwrap();
        assert_eq!(repo.name, "my-repo");
        assert_eq!(repo.source, "/local/path");
        assert_eq!(repo.local_path, "/local/path");

        let fetched = get(&conn, &repo.id).unwrap();
        assert_eq!(fetched.name, "my-repo");
    }

    #[test]
    fn list_by_session_returns_attached_repos() {
        let db = test_db();
        let conn = db.conn().unwrap();
        conn.execute("INSERT INTO sessions (id, title) VALUES (?1, ?2)", params!["s1", "Test"]).unwrap();
        conn.execute("INSERT INTO sessions (id, title) VALUES (?1, ?2)", params!["s2", "Other"]).unwrap();

        attach(&conn, "s1", "repo-a", "/a", "/a").unwrap();
        attach(&conn, "s1", "repo-b", "/b", "/b").unwrap();
        attach(&conn, "s2", "repo-c", "/c", "/c").unwrap();

        let repos = list_by_session(&conn, "s1").unwrap();
        assert_eq!(repos.len(), 2);
        assert_eq!(repos[0].name, "repo-a");
        assert_eq!(repos[1].name, "repo-b");
    }

    #[test]
    fn detach_removes_repo() {
        let db = test_db();
        let conn = db.conn().unwrap();
        conn.execute("INSERT INTO sessions (id, title) VALUES (?1, ?2)", params!["s1", "Test"]).unwrap();

        let repo = attach(&conn, "s1", "my-repo", "/path", "/path").unwrap();
        detach(&conn, &repo.id).unwrap();

        let repos = list_by_session(&conn, "s1").unwrap();
        assert_eq!(repos.len(), 0);
    }

    #[test]
    fn cascade_delete_on_session() {
        let db = test_db();
        let conn = db.conn().unwrap();
        conn.execute("INSERT INTO sessions (id, title) VALUES (?1, ?2)", params!["s1", "Test"]).unwrap();

        attach(&conn, "s1", "my-repo", "/path", "/path").unwrap();
        conn.execute("DELETE FROM sessions WHERE id = ?1", params!["s1"]).unwrap();

        let count: i64 = conn.query_row("SELECT COUNT(*) FROM repos", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 0);
    }
}
```

- [ ] **Step 5: Register module in lib.rs**

In `src-tauri/src/lib.rs`, add:

```rust
mod repo_context;
```

after the existing `mod jira;` line.

- [ ] **Step 6: Run tests**

Run: `cd src-tauri && cargo test repo_context::store --lib 2>&1`
Expected: 4 tests pass.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/migrations/004_add_repos.sql src-tauri/src/db.rs src-tauri/src/repo_context/ src-tauri/src/lib.rs
git commit -m "feat: add repos table, model, and store with CRUD"
```

---

### Task 2: Tree walking + file read + summary + search

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/repo_context/tree.rs`

- [ ] **Step 1: Add `ignore` crate**

In `src-tauri/Cargo.toml`, add to `[dependencies]`:

```toml
ignore = "0.4"
```

- [ ] **Step 2: Implement tree.rs**

Create `src-tauri/src/repo_context/tree.rs`:

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

use crate::error::{AppError, AppResult};
use super::model::{FileNode, FileSearchResult};

const MAX_DEPTH: usize = 6;
const MAX_ENTRIES: usize = 5000;

pub fn walk_tree(repo_path: &Path) -> AppResult<Vec<FileNode>> {
    let mut root_children: Vec<FileNode> = Vec::new();
    let mut count = 0;

    let walker = WalkBuilder::new(repo_path)
        .hidden(false)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .filter_entry(|e| e.file_name() != ".git")
        .max_depth(Some(MAX_DEPTH))
        .build();

    let mut dirs: HashMap<PathBuf, Vec<FileNode>> = HashMap::new();
    let mut all_paths: Vec<(PathBuf, bool)> = Vec::new();

    for entry in walker.flatten() {
        if count >= MAX_ENTRIES {
            break;
        }
        let path = entry.path();
        if path == repo_path {
            continue;
        }
        let relative = path.strip_prefix(repo_path).unwrap_or(path);
        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        all_paths.push((relative.to_path_buf(), is_dir));
        count += 1;
    }

    all_paths.sort_by(|a, b| a.0.cmp(&b.0));

    for (relative, is_dir) in &all_paths {
        let name = relative
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let node = FileNode {
            name,
            path: relative.to_string_lossy().to_string(),
            is_dir: *is_dir,
            children: Vec::new(),
        };

        if let Some(parent) = relative.parent() {
            if parent == Path::new("") {
                root_children.push(node);
            } else {
                dirs.entry(parent.to_path_buf()).or_default().push(node);
            }
        }
    }

    fn attach_children(node: &mut FileNode, dirs: &mut HashMap<PathBuf, Vec<FileNode>>) {
        if node.is_dir {
            let path = PathBuf::from(&node.path);
            if let Some(children) = dirs.remove(&path) {
                node.children = children;
                for child in &mut node.children {
                    attach_children(child, dirs);
                }
            }
        }
    }

    for node in &mut root_children {
        attach_children(node, &mut dirs);
    }

    root_children.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name))
    });

    Ok(root_children)
}

pub fn read_file(repo_path: &Path, relative_path: &str) -> AppResult<String> {
    let full_path = repo_path.join(relative_path);
    let canonical = full_path.canonicalize().map_err(|_| {
        AppError::Other(format!("File not found: {relative_path}"))
    })?;
    let canonical_root = repo_path.canonicalize().map_err(|_| {
        AppError::Other("Repo path not found".to_string())
    })?;

    if !canonical.starts_with(&canonical_root) {
        return Err(AppError::Other("Path traversal detected".to_string()));
    }

    let bytes = std::fs::read(&canonical)?;
    if bytes.len() >= 512 && bytes[..512].contains(&0) {
        return Err(AppError::Other("Binary file".to_string()));
    }

    String::from_utf8(bytes).map_err(|_| AppError::Other("Not valid UTF-8".to_string()))
}

pub fn generate_summary(repo_path: &Path) -> AppResult<String> {
    let mut ext_counts: HashMap<String, usize> = HashMap::new();
    let mut total_files = 0;
    let mut key_dirs: Vec<String> = Vec::new();

    let well_known = ["src", "lib", "test", "tests", "docs", "cmd", "internal", "api", "pkg", "app", "components", "features"];

    let walker = WalkBuilder::new(repo_path)
        .hidden(false)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .filter_entry(|e| e.file_name() != ".git")
        .max_depth(Some(2))
        .build();

    for entry in walker.flatten() {
        let path = entry.path();
        if path == repo_path {
            continue;
        }
        let relative = path.strip_prefix(repo_path).unwrap_or(path);
        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);

        if is_dir && relative.parent() == Some(Path::new("")) {
            let name = relative.to_string_lossy().to_string();
            if well_known.contains(&name.as_str()) {
                key_dirs.push(format!("{name}/"));
            }
        }

        if !is_dir {
            total_files += 1;
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                *ext_counts.entry(ext.to_string()).or_default() += 1;
            }
        }
    }

    let mut exts: Vec<(String, usize)> = ext_counts.into_iter().collect();
    exts.sort_by(|a, b| b.1.cmp(&a.1));
    let top_exts: String = exts.iter().take(5).map(|(ext, count)| format!(".{ext}({count})")).collect::<Vec<_>>().join(", ");

    let dirs_str = if key_dirs.is_empty() {
        String::new()
    } else {
        format!(". Key dirs: {}", key_dirs.join(", "))
    };

    Ok(format!("{total_files} files. Top types: {top_exts}{dirs_str}"))
}

pub fn search_files(
    repo_path: &Path,
    query: &str,
    repo_id: &str,
    repo_name: &str,
) -> AppResult<Vec<FileSearchResult>> {
    let query_lower = query.to_lowercase();

    let walker = WalkBuilder::new(repo_path)
        .hidden(false)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .filter_entry(|e| e.file_name() != ".git")
        .max_depth(Some(MAX_DEPTH))
        .build();

    let mut scored: Vec<(i32, FileSearchResult)> = Vec::new();

    for entry in walker.flatten() {
        if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(true) {
            continue;
        }
        let path = entry.path();
        let relative = path.strip_prefix(repo_path).unwrap_or(path);
        let relative_str = relative.to_string_lossy().to_string();
        let relative_lower = relative_str.to_lowercase();

        let filename = relative
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let filename_lower = filename.to_lowercase();

        let score = if filename_lower == query_lower {
            100
        } else if filename_lower.contains(&query_lower) {
            50
        } else if relative_lower.contains(&query_lower) {
            10
        } else {
            continue;
        };

        scored.push((score, FileSearchResult {
            repo_id: repo_id.to_string(),
            repo_name: repo_name.to_string(),
            relative_path: relative_str.clone(),
            display: format!("{repo_name}/{relative_str}"),
        }));
    }

    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.truncate(20);

    Ok(scored.into_iter().map(|(_, r)| r).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_test_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        fs::create_dir_all(root.join("src/features")).unwrap();
        fs::write(root.join("src/main.ts"), "console.log('hello')").unwrap();
        fs::write(root.join("src/features/auth.ts"), "export function login() {}").unwrap();
        fs::write(root.join("README.md"), "# Test Repo").unwrap();
        fs::write(root.join(".gitignore"), "node_modules/\n").unwrap();
        fs::create_dir_all(root.join("node_modules/dep")).unwrap();
        fs::write(root.join("node_modules/dep/index.js"), "ignored").unwrap();

        dir
    }

    #[test]
    fn walk_tree_respects_gitignore() {
        let repo = create_test_repo();
        let tree = walk_tree(repo.path()).unwrap();

        let all_paths: Vec<String> = collect_paths(&tree);
        assert!(all_paths.contains(&"src".to_string()));
        assert!(all_paths.contains(&"README.md".to_string()));
        assert!(!all_paths.iter().any(|p| p.contains("node_modules")));
    }

    #[test]
    fn walk_tree_nests_children() {
        let repo = create_test_repo();
        let tree = walk_tree(repo.path()).unwrap();

        let src = tree.iter().find(|n| n.name == "src").unwrap();
        assert!(src.is_dir);
        assert!(!src.children.is_empty());
    }

    #[test]
    fn read_file_returns_content() {
        let repo = create_test_repo();
        let content = read_file(repo.path(), "README.md").unwrap();
        assert_eq!(content, "# Test Repo");
    }

    #[test]
    fn read_file_rejects_traversal() {
        let repo = create_test_repo();
        let result = read_file(repo.path(), "../../../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn generate_summary_counts_files() {
        let repo = create_test_repo();
        let summary = generate_summary(repo.path()).unwrap();
        assert!(summary.contains("files"));
        assert!(summary.contains(".ts"));
    }

    #[test]
    fn search_files_finds_by_name() {
        let repo = create_test_repo();
        let results = search_files(repo.path(), "auth", "r1", "test-repo").unwrap();
        assert!(!results.is_empty());
        assert!(results[0].display.contains("auth.ts"));
    }

    #[test]
    fn search_files_returns_empty_for_no_match() {
        let repo = create_test_repo();
        let results = search_files(repo.path(), "zzzznotfound", "r1", "test-repo").unwrap();
        assert!(results.is_empty());
    }

    fn collect_paths(nodes: &[FileNode]) -> Vec<String> {
        let mut paths = Vec::new();
        for node in nodes {
            paths.push(node.name.clone());
            paths.extend(collect_paths(&node.children));
        }
        paths
    }
}
```

- [ ] **Step 3: Add tempfile dev dependency**

In `src-tauri/Cargo.toml`, add:

```toml
[dev-dependencies]
tempfile = "3"
```

(If `[dev-dependencies]` doesn't exist, create the section. If mockito is already there under dev-dependencies, just add tempfile next to it.)

- [ ] **Step 4: Run tests**

Run: `cd src-tauri && cargo test repo_context::tree --lib 2>&1`
Expected: 7 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/repo_context/tree.rs
git commit -m "feat: add gitignore-aware tree walk, file read, summary, and search"
```

---

### Task 3: Git clone helper

**Files:**
- Create: `src-tauri/src/repo_context/clone.rs`

- [ ] **Step 1: Implement clone.rs**

Create `src-tauri/src/repo_context/clone.rs`:

```rust
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
```

- [ ] **Step 2: Run tests**

Run: `cd src-tauri && cargo test repo_context::clone --lib 2>&1`
Expected: 5 tests pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/repo_context/clone.rs
git commit -m "feat: add git clone helper with URL detection"
```

---

### Task 4: Tauri commands + plugin setup

**Files:**
- Create: `src-tauri/src/repo_context/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/capabilities/default.json`

- [ ] **Step 1: Add Tauri plugin dependencies**

In `src-tauri/Cargo.toml`, add to `[dependencies]`:

```toml
tauri-plugin-dialog = "2"
```

- [ ] **Step 2: Add plugin permissions**

In `src-tauri/capabilities/default.json`, update the `permissions` array:

```json
"permissions": [
    "core:default",
    "opener:default",
    "dialog:default"
]
```

- [ ] **Step 3: Create commands.rs**

Create `src-tauri/src/repo_context/commands.rs`:

```rust
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
```

- [ ] **Step 4: Register commands and plugin in lib.rs**

In `src-tauri/src/lib.rs`, add the import:

```rust
use repo_context::commands::*;
```

Register the dialog plugin (add after `.plugin(tauri_plugin_opener::init())`):

```rust
.plugin(tauri_plugin_dialog::init())
```

Add the commands to `invoke_handler`:

```rust
attach_repo,
detach_repo,
list_repos,
get_repo_tree,
read_repo_file,
search_repo_files,
```

- [ ] **Step 5: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1`
Expected: Compiles.

- [ ] **Step 6: Run all tests**

Run: `cd src-tauri && cargo test 2>&1 | grep "test result"`
Expected: All tests pass.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/repo_context/commands.rs src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/capabilities/default.json
git commit -m "feat: add repo context Tauri commands with dialog plugin"
```

---

### Task 5: Modify context assembly + send_message for @mentions

**Files:**
- Modify: `src-tauri/src/llm/context.rs`
- Modify: `src-tauri/src/llm/streaming.rs`

- [ ] **Step 1: Add @mention extraction function**

In `src-tauri/src/llm/context.rs`, add at the top (after existing imports):

```rust
use regex::Regex;
use crate::repo_context::model::RepoFileContext;
```

Add this function before `assemble_context`:

```rust
pub fn extract_at_mentions(text: &str) -> Vec<String> {
    let re = Regex::new(r"@([\w.\-]+/[\w.\-/]+)").unwrap();
    let mut seen = std::collections::HashSet::new();
    let mut mentions = Vec::new();
    for cap in re.captures_iter(text) {
        let mention = cap[1].to_string();
        if seen.insert(mention.clone()) {
            mentions.push(mention);
        }
    }
    mentions
}
```

- [ ] **Step 2: Update `assemble_context` signature**

Change the signature to add two new parameters after `jira_issues`:

```rust
pub fn assemble_context(
    mode: &ChatMode,
    session_context: &str,
    note_content: &str,
    tickets: &[(String, String, String, String)],
    jira_issues: &[JiraIssueContext],
    repo_summaries: &[String],
    mentioned_files: &[RepoFileContext],
    conversation: &[(String, String)],
) -> Vec<ChatMessage> {
```

- [ ] **Step 3: Add repo sections to system prompt**

After the `jira_issues` block, add:

```rust
    if !repo_summaries.is_empty() {
        let mut repo_text = String::from("## Attached Repositories\n");
        for summary in repo_summaries {
            repo_text.push_str(&format!("- {summary}\n"));
        }
        system_parts.push(repo_text);
    }

    if !mentioned_files.is_empty() {
        let mut files_text = String::from("## Referenced Files\n");
        for file in mentioned_files {
            files_text.push_str(&format!("### {}\n{}\n\n", file.display, file.content));
        }
        system_parts.push(files_text);
    }
```

- [ ] **Step 4: Fix ALL existing test calls**

Every existing test call to `assemble_context` needs two new `&[]` arguments inserted after `jira_issues` and before `conversation`. For example:

```rust
assemble_context(&ChatMode::Assist, "", "", &[], &[], &[], &[])
```
becomes:
```rust
assemble_context(&ChatMode::Assist, "", "", &[], &[], &[], &[], &[])
```

And calls with conversation:
```rust
assemble_context(&ChatMode::Assist, "", "", &[], &[], &conversation)
```
become:
```rust
assemble_context(&ChatMode::Assist, "", "", &[], &[], &[], &[], &conversation)
```

Update all existing test calls (there should be around 10 calls to fix).

- [ ] **Step 5: Add new tests**

```rust
#[test]
fn includes_repo_summaries() {
    let summaries = vec!["245 files. Top types: .ts(120), .tsx(80). Key dirs: src/, tests/".to_string()];
    let messages = assemble_context(&ChatMode::Assist, "", "", &[], &[], &summaries, &[], &[]);
    assert!(messages[0].content.contains("Attached Repositories"));
    assert!(messages[0].content.contains("245 files"));
}

#[test]
fn includes_mentioned_files() {
    let files = vec![RepoFileContext {
        display: "my-repo/src/main.ts".to_string(),
        content: "console.log('hello')".to_string(),
    }];
    let messages = assemble_context(&ChatMode::Assist, "", "", &[], &[], &[], &files, &[]);
    assert!(messages[0].content.contains("Referenced Files"));
    assert!(messages[0].content.contains("my-repo/src/main.ts"));
    assert!(messages[0].content.contains("console.log"));
}

#[test]
fn extract_at_mentions_finds_patterns() {
    let text = "Check @my-repo/src/auth.ts and also @other/lib/utils.ts please";
    let mentions = extract_at_mentions(text);
    assert_eq!(mentions, vec!["my-repo/src/auth.ts", "other/lib/utils.ts"]);
}

#[test]
fn extract_at_mentions_deduplicates() {
    let text = "@repo/file.ts and again @repo/file.ts";
    let mentions = extract_at_mentions(text);
    assert_eq!(mentions, vec!["repo/file.ts"]);
}
```

- [ ] **Step 6: Update streaming.rs**

In `src-tauri/src/llm/streaming.rs`, add imports:

```rust
use crate::repo_context::{store as repo_store, tree as repo_tree, model::RepoFileContext};
use super::context::extract_at_mentions;
```

In the `send_message` function, after the Jira fetch block and before the `let (messages, model) = {` block, add:

```rust
    let (repo_summaries, mentioned_files) = {
        let conn = db.conn().map_err(|e| e.to_string())?;
        let repos = repo_store::list_by_session(&conn, &session_id)
            .map_err(|e| e.to_string())?;

        let summaries: Vec<String> = repos
            .iter()
            .filter_map(|r| {
                repo_tree::generate_summary(std::path::Path::new(&r.local_path))
                    .ok()
                    .map(|s| format!("{} ({})", r.name, s))
            })
            .collect();

        let mut all_text = note_content.clone();
        all_text.push(' ');
        all_text.push_str(&content);
        let mentions = extract_at_mentions(&all_text);

        let mut files: Vec<RepoFileContext> = Vec::new();
        for mention in &mentions {
            if let Some(slash_pos) = mention.find('/') {
                let repo_name = &mention[..slash_pos];
                let file_path = &mention[slash_pos + 1..];
                if let Some(repo) = repos.iter().find(|r| r.name == repo_name) {
                    match repo_tree::read_file(std::path::Path::new(&repo.local_path), file_path) {
                        Ok(content) => {
                            let truncated = if content.chars().count() > 3000 {
                                let t: String = content.chars().take(3000).collect();
                                format!("{t}...")
                            } else {
                                content
                            };
                            files.push(RepoFileContext {
                                display: mention.clone(),
                                content: truncated,
                            });
                        }
                        Err(e) => tracing::warn!("Failed to read @{mention}: {e}"),
                    }
                }
            }
        }

        (summaries, files)
    };
```

Then update the `assemble_context` call to include the new parameters:

```rust
        let messages = context::assemble_context(
            &chat_mode,
            &session_context,
            &note_content,
            &tickets,
            &jira_issues,
            &repo_summaries,
            &mentioned_files,
            &conversation,
        );
```

- [ ] **Step 7: Run all tests**

Run: `cd src-tauri && cargo test 2>&1 | grep "test result"`
Expected: All tests pass.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/llm/context.rs src-tauri/src/llm/streaming.rs
git commit -m "feat: inject repo summaries and @mentioned files into LLM context"
```

---

### Task 6: Frontend types + atoms + useRepoActions

**Files:**
- Create: `src/features/repo/repo.types.ts`
- Create: `src/features/repo/repo.atoms.ts`
- Create: `src/features/repo/useRepoActions.ts`

- [ ] **Step 1: Create types**

Create `src/features/repo/repo.types.ts`:

```ts
export interface RepoContext {
  id: string;
  session_id: string;
  name: string;
  source: string;
  local_path: string;
  created_at: string;
}

export interface FileNode {
  name: string;
  path: string;
  is_dir: boolean;
  children: FileNode[];
}

export interface FileSearchResult {
  repo_id: string;
  repo_name: string;
  relative_path: string;
  display: string;
}
```

- [ ] **Step 2: Create atoms**

Create `src/features/repo/repo.atoms.ts`:

```ts
import { atom } from "jotai";
import type { RepoContext } from "./repo.types";

export const reposAtom = atom<RepoContext[]>([]);
```

- [ ] **Step 3: Create hook**

Create `src/features/repo/useRepoActions.ts`:

```ts
import { useSetAtom } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import { reposAtom } from "./repo.atoms";
import type { RepoContext } from "./repo.types";

export function useRepoActions() {
  const setRepos = useSetAtom(reposAtom);

  async function loadRepos(sessionId: string) {
    const repos = await invoke<RepoContext[]>("list_repos", { sessionId });
    setRepos(repos);
  }

  async function attachRepo(sessionId: string, source: string) {
    const repo = await invoke<RepoContext>("attach_repo", { sessionId, source });
    await loadRepos(sessionId);
    return repo;
  }

  async function detachRepo(repoId: string, sessionId: string) {
    await invoke<void>("detach_repo", { repoId });
    await loadRepos(sessionId);
  }

  return { loadRepos, attachRepo, detachRepo };
}
```

- [ ] **Step 4: Verify build**

Run: `bun run build 2>&1 | tail -5`
Expected: Builds (new files aren't imported yet, so no errors).

- [ ] **Step 5: Commit**

```bash
git add src/features/repo/
git commit -m "feat: add repo frontend types, atoms, and actions hook"
```

---

### Task 7: RepoPanel + FileTree components

**Files:**
- Create: `src/features/repo/FileTree.tsx`
- Create: `src/features/repo/RepoPanel.tsx`
- Modify: `src/App.tsx`

- [ ] **Step 1: Create FileTree component**

Create `src/features/repo/FileTree.tsx`:

```tsx
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ChevronRight, ChevronDown, File, Folder } from "lucide-react";
import type { FileNode } from "./repo.types";

interface FileTreeProps {
  repoId: string;
  repoName: string;
}

export function FileTree({ repoId, repoName }: FileTreeProps) {
  const [tree, setTree] = useState<FileNode[] | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [loading, setLoading] = useState(false);

  async function handleToggle() {
    if (!expanded && tree === null) {
      setLoading(true);
      const nodes = await invoke<FileNode[]>("get_repo_tree", { repoId });
      setTree(nodes);
      setLoading(false);
    }
    setExpanded(!expanded);
  }

  return (
    <div className="text-xs">
      <button
        onClick={handleToggle}
        className="flex items-center gap-1 w-full text-left py-0.5 text-muted-foreground hover:text-foreground"
      >
        {expanded ? <ChevronDown className="size-3" /> : <ChevronRight className="size-3" />}
        <Folder className="size-3" />
        <span>{repoName}/</span>
      </button>
      {loading && <p className="pl-6 text-muted-foreground/60 animate-pulse">Loading...</p>}
      {expanded && tree && (
        <div className="pl-3">
          {tree.map((node) => (
            <TreeNode key={node.path} node={node} />
          ))}
        </div>
      )}
    </div>
  );
}

function TreeNode({ node }: { node: FileNode }) {
  const [expanded, setExpanded] = useState(false);

  if (node.is_dir) {
    return (
      <div>
        <button
          onClick={() => setExpanded(!expanded)}
          className="flex items-center gap-1 w-full text-left py-0.5 text-muted-foreground hover:text-foreground"
        >
          {expanded ? <ChevronDown className="size-3" /> : <ChevronRight className="size-3" />}
          <Folder className="size-3 text-blue-400/70" />
          <span>{node.name}/</span>
        </button>
        {expanded && node.children.length > 0 && (
          <div className="pl-3">
            {node.children.map((child) => (
              <TreeNode key={child.path} node={child} />
            ))}
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="flex items-center gap-1 py-0.5 pl-4 text-muted-foreground/80">
      <File className="size-3" />
      <span>{node.name}</span>
    </div>
  );
}
```

- [ ] **Step 2: Create RepoPanel component**

Create `src/features/repo/RepoPanel.tsx`:

```tsx
import { useEffect, useState } from "react";
import { useAtom } from "jotai";
import { open } from "@tauri-apps/plugin-dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { activeSessionAtom } from "@/features/session/session.atoms";
import { reposAtom } from "./repo.atoms";
import { useRepoActions } from "./useRepoActions";
import { FileTree } from "./FileTree";
import { Plus, FolderOpen, GitBranch, X, Loader2 } from "lucide-react";

export function RepoPanel() {
  const [activeSession] = useAtom(activeSessionAtom);
  const [repos] = useAtom(reposAtom);
  const { loadRepos, attachRepo, detachRepo } = useRepoActions();
  const [showAttach, setShowAttach] = useState(false);
  const [gitUrl, setGitUrl] = useState("");
  const [attaching, setAttaching] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (activeSession) {
      loadRepos(activeSession.id);
    }
  }, [activeSession?.id]);

  async function handleAttachLocal() {
    if (!activeSession) return;
    const selected = await open({ directory: true, multiple: false });
    if (!selected) return;

    setAttaching(true);
    setError(null);
    try {
      await attachRepo(activeSession.id, selected as string);
      setShowAttach(false);
    } catch (err) {
      setError(String(err));
    } finally {
      setAttaching(false);
    }
  }

  async function handleAttachGit() {
    if (!activeSession || !gitUrl.trim()) return;

    setAttaching(true);
    setError(null);
    try {
      await attachRepo(activeSession.id, gitUrl.trim());
      setGitUrl("");
      setShowAttach(false);
    } catch (err) {
      setError(String(err));
    } finally {
      setAttaching(false);
    }
  }

  if (!activeSession) {
    return (
      <div className="border-b border-border p-4">
        <h2 className="text-sm font-medium text-muted-foreground">Context</h2>
        <p className="mt-2 text-xs text-muted-foreground/60">Select a session first</p>
      </div>
    );
  }

  return (
    <div className="border-b border-border p-4 space-y-2">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-medium text-muted-foreground">Context</h2>
        <Button
          variant="ghost"
          size="xs"
          onClick={() => setShowAttach(!showAttach)}
          className="text-muted-foreground"
        >
          <Plus className="size-3" />
        </Button>
      </div>

      {showAttach && (
        <div className="space-y-2 rounded-md border border-border p-2">
          <div className="flex gap-2">
            <Button
              variant="outline"
              size="xs"
              onClick={handleAttachLocal}
              disabled={attaching}
              className="flex-1"
            >
              <FolderOpen className="size-3 mr-1" />
              Local Folder
            </Button>
          </div>
          <div className="flex gap-2">
            <Input
              value={gitUrl}
              onChange={(e) => setGitUrl(e.target.value)}
              placeholder="https://github.com/..."
              className="h-7 text-xs flex-1"
              onKeyDown={(e) => {
                if (e.key === "Enter") handleAttachGit();
              }}
            />
            <Button
              variant="outline"
              size="xs"
              onClick={handleAttachGit}
              disabled={attaching || !gitUrl.trim()}
            >
              {attaching ? <Loader2 className="size-3 animate-spin" /> : <GitBranch className="size-3" />}
            </Button>
          </div>
          {error && <p className="text-[10px] text-red-400">{error}</p>}
        </div>
      )}

      {repos.length === 0 && !showAttach && (
        <p className="text-xs text-muted-foreground/60">No repos attached</p>
      )}

      {repos.map((repo) => (
        <div key={repo.id} className="space-y-1">
          <div className="flex items-center gap-1 text-xs">
            {repo.source.startsWith("http") || repo.source.startsWith("git@") || repo.source.startsWith("ssh://") ? (
              <GitBranch className="size-3 text-muted-foreground" />
            ) : (
              <FolderOpen className="size-3 text-muted-foreground" />
            )}
            <span className="text-muted-foreground flex-1 truncate">{repo.name}</span>
            <button
              onClick={() => detachRepo(repo.id, activeSession.id)}
              className="text-muted-foreground/50 hover:text-red-400"
            >
              <X className="size-3" />
            </button>
          </div>
          <FileTree repoId={repo.id} repoName={repo.name} />
        </div>
      ))}
    </div>
  );
}
```

- [ ] **Step 3: Install dialog frontend plugin**

Run: `bun add @tauri-apps/plugin-dialog`

- [ ] **Step 4: Wire RepoPanel into App.tsx**

In `src/App.tsx`, replace the existing Context section placeholder:

```tsx
          {/* Context section */}
          <div className="border-b border-border p-4">
            <h2 className="text-sm font-medium text-muted-foreground">
              Context
            </h2>
            <p className="mt-2 text-xs text-muted-foreground/60">
              No repos or files attached
            </p>
          </div>
```

with:

```tsx
          <RepoPanel />
```

Add the import at the top:

```tsx
import { RepoPanel } from "@/features/repo/RepoPanel";
```

- [ ] **Step 5: Verify build**

Run: `bun run build 2>&1 | tail -10`
Expected: No errors.

- [ ] **Step 6: Commit**

```bash
git add src/features/repo/ src/App.tsx package.json bun.lock
git commit -m "feat: add RepoPanel with attach/detach, file tree, and folder picker"
```

---

### Task 8: @mention autocomplete in chat input

**Files:**
- Create: `src/components/AtMentionInput.tsx`
- Modify: `src/features/chat/ChatPanel.tsx`

- [ ] **Step 1: Create AtMentionInput**

Create `src/components/AtMentionInput.tsx`:

```tsx
import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Input } from "@/components/ui/input";
import type { FileSearchResult } from "@/features/repo/repo.types";

interface AtMentionInputProps {
  value: string;
  onChange: (value: string) => void;
  onSubmit: () => void;
  sessionId: string;
  placeholder?: string;
  disabled?: boolean;
}

export function AtMentionInput({
  value,
  onChange,
  onSubmit,
  sessionId,
  placeholder,
  disabled,
}: AtMentionInputProps) {
  const [showDropdown, setShowDropdown] = useState(false);
  const [results, setResults] = useState<FileSearchResult[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [mentionQuery, setMentionQuery] = useState("");
  const [mentionStart, setMentionStart] = useState(-1);
  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>();

  const search = useCallback(
    (query: string) => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
      debounceRef.current = setTimeout(async () => {
        if (query.length < 1) {
          setResults([]);
          return;
        }
        const res = await invoke<FileSearchResult[]>("search_repo_files", {
          sessionId,
          query,
        });
        setResults(res);
        setSelectedIndex(0);
      }, 200);
    },
    [sessionId],
  );

  function handleChange(e: React.ChangeEvent<HTMLInputElement>) {
    const newValue = e.target.value;
    onChange(newValue);

    const cursorPos = e.target.selectionStart ?? newValue.length;
    const textBeforeCursor = newValue.slice(0, cursorPos);
    const atIndex = textBeforeCursor.lastIndexOf("@");

    if (atIndex >= 0 && (atIndex === 0 || textBeforeCursor[atIndex - 1] === " ")) {
      const query = textBeforeCursor.slice(atIndex + 1);
      if (!query.includes(" ")) {
        setMentionStart(atIndex);
        setMentionQuery(query);
        setShowDropdown(true);
        search(query);
        return;
      }
    }

    setShowDropdown(false);
  }

  function selectResult(result: FileSearchResult) {
    const before = value.slice(0, mentionStart);
    const after = value.slice(mentionStart + 1 + mentionQuery.length);
    onChange(`${before}@${result.display}${after} `);
    setShowDropdown(false);
    inputRef.current?.focus();
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (showDropdown && results.length > 0) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((i) => Math.min(i + 1, results.length - 1));
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex((i) => Math.max(i - 1, 0));
        return;
      }
      if (e.key === "Enter") {
        e.preventDefault();
        selectResult(results[selectedIndex]);
        return;
      }
      if (e.key === "Escape") {
        setShowDropdown(false);
        return;
      }
    }

    if (e.key === "Enter" && !showDropdown) {
      e.preventDefault();
      onSubmit();
    }
  }

  useEffect(() => {
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, []);

  return (
    <div className="relative flex-1">
      <Input
        ref={inputRef}
        value={value}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        disabled={disabled}
        className="text-sm"
      />
      {showDropdown && results.length > 0 && (
        <div className="absolute bottom-full left-0 right-0 mb-1 max-h-48 overflow-y-auto rounded-md border border-border bg-popover shadow-md z-50">
          {results.map((result, i) => (
            <button
              key={result.display}
              onClick={() => selectResult(result)}
              className={`w-full text-left px-3 py-1.5 text-xs truncate ${
                i === selectedIndex
                  ? "bg-accent text-accent-foreground"
                  : "text-muted-foreground hover:bg-accent/50"
              }`}
            >
              {result.display}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Wire into ChatPanel**

In `src/features/chat/ChatPanel.tsx`:

Add import:
```tsx
import { AtMentionInput } from "@/components/AtMentionInput";
```

Replace the `<Input>` in the form at the bottom of the component:

Find:
```tsx
          <Input
            value={inputValue}
            onChange={(e) => setInputValue(e.target.value)}
            placeholder={
              chatMode === "grill"
                ? "Ask the duck to grill your plan..."
                : "Ask the duck..."
            }
            disabled={isStreaming || editingMessageId != null}
            className="flex-1 text-sm"
          />
```

Replace with:
```tsx
          <AtMentionInput
            value={inputValue}
            onChange={setInputValue}
            onSubmit={handleSend}
            sessionId={activeSession.id}
            placeholder={
              chatMode === "grill"
                ? "Ask the duck to grill your plan..."
                : "Ask the duck..."
            }
            disabled={isStreaming || editingMessageId != null}
          />
```

Also remove the `<form>` wrapper's `onSubmit` since `AtMentionInput` now handles Enter:

Find:
```tsx
        <form
          onSubmit={(e) => {
            e.preventDefault();
            handleSend();
          }}
          className="flex gap-2"
        >
```

Replace with:
```tsx
        <div className="flex gap-2">
```

And the closing `</form>` becomes `</div>`.

- [ ] **Step 3: Verify build**

Run: `bun run build 2>&1 | tail -5`
Expected: No errors.

- [ ] **Step 4: Commit**

```bash
git add src/components/AtMentionInput.tsx src/features/chat/ChatPanel.tsx
git commit -m "feat: add @mention autocomplete to chat input"
```

---

### Task 9: @mention autocomplete in CodeMirror editor

**Files:**
- Modify: `src/components/MarkdownEditor.tsx`

- [ ] **Step 1: Add autocompletion extension**

In `src/components/MarkdownEditor.tsx`, add imports:

```tsx
import { autocompletion, CompletionContext } from "@codemirror/autocomplete";
import { invoke } from "@tauri-apps/api/core";
import type { FileSearchResult } from "@/features/repo/repo.types";
```

- [ ] **Step 2: Add sessionId prop**

Update the interface:

```tsx
interface MarkdownEditorProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  onImagePaste?: (base64: string) => Promise<string | null>;
  sessionId?: string;
}
```

Update the destructuring:

```tsx
export function MarkdownEditor({
  value,
  onChange,
  placeholder = "Start typing...",
  onImagePaste,
  sessionId,
}: MarkdownEditorProps) {
```

Store sessionId in a ref:
```tsx
  const sessionIdRef = useRef(sessionId);
  sessionIdRef.current = sessionId;
```

- [ ] **Step 3: Add completion source**

Before the `useEffect` that creates the editor, add:

```tsx
  const mentionCompletion = useCallback(async (context: CompletionContext) => {
    const before = context.matchBefore(/@[\w.\-/]*/);
    if (!before || before.from === before.to - 1) return null;
    if (!sessionIdRef.current) return null;

    const query = before.text.slice(1);
    if (query.length < 1) return null;

    const results = await invoke<FileSearchResult[]>("search_repo_files", {
      sessionId: sessionIdRef.current,
      query,
    });

    return {
      from: before.from,
      options: results.map((r) => ({
        label: `@${r.display}`,
        apply: `@${r.display}`,
      })),
    };
  }, []);
```

- [ ] **Step 4: Add autocompletion to extensions**

In the `EditorState.create` extensions array, add after `bracketMatching()`:

```tsx
        autocompletion({
          override: [mentionCompletion],
          activateOnTyping: true,
        }),
```

- [ ] **Step 5: Pass sessionId from DumpView**

In `src/features/session/DumpView.tsx`, find the `<MarkdownEditor>` usage and add the `sessionId` prop:

```tsx
          <MarkdownEditor
            value={content}
            onChange={handleChange}
            onImagePaste={handleImagePaste}
            placeholder="Brain dump here... markdown supported"
            sessionId={sessionId}
          />
```

- [ ] **Step 6: Verify build**

Run: `bun run build 2>&1 | tail -5`
Expected: No errors.

- [ ] **Step 7: Commit**

```bash
git add src/components/MarkdownEditor.tsx src/features/session/DumpView.tsx
git commit -m "feat: add @mention autocomplete to CodeMirror editor"
```

---

### Task 10: @mention rendering in markdown preview

**Files:**
- Create: `src/components/MentionText.tsx`
- Modify: `src/features/session/DumpView.tsx`
- Modify: `src/features/chat/ChatPanel.tsx`

- [ ] **Step 1: Create MentionText component**

Create `src/components/MentionText.tsx`:

```tsx
import { File } from "lucide-react";

const MENTION_SPLIT = /(@[\w.\-]+\/[\w.\-/]+)/g;
const MENTION_TEST = /^@[\w.\-]+\/[\w.\-/]+$/;

interface MentionTextProps {
  children: string;
}

export function MentionText({ children }: MentionTextProps) {
  const parts = children.split(MENTION_SPLIT);
  if (parts.length === 1) {
    return <>{children}</>;
  }

  return (
    <>
      {parts.map((part, i) =>
        MENTION_TEST.test(part) ? (
          <span
            key={i}
            className="inline-flex items-center gap-0.5 rounded bg-accent/50 px-1 py-0.5 text-xs font-mono text-accent-foreground"
          >
            <File className="size-3" />
            {part.slice(1)}
          </span>
        ) : (
          <span key={i}>{part}</span>
        ),
      )}
    </>
  );
}
```

- [ ] **Step 2: Update processChildren in DumpView and ChatPanel**

In both `src/features/session/DumpView.tsx` and `src/features/chat/ChatPanel.tsx`, update the `processChildren` function to also handle `@` mentions.

Add import in both files:
```tsx
import { MentionText } from "@/components/MentionText";
```

Update the `processChildren` function in both files:

```tsx
function processChildren(children: React.ReactNode): React.ReactNode {
  return Array.isArray(children)
    ? children.map((child, i) =>
        typeof child === "string" ? (
          <JiraLinkedText key={`j${i}`}>
            {child}
          </JiraLinkedText>
        ) : child,
      )
    : typeof children === "string"
      ? <JiraLinkedText>{children}</JiraLinkedText>
      : children;
}
```

Actually, we need both JiraLinkedText AND MentionText to process the same text. The simplest approach: update `processChildren` to handle both, or compose them. Since they process different patterns and both operate on strings, the cleanest approach is to have JiraLinkedText handle its pattern and MentionText handle its pattern in sequence.

Update `processChildren` in both files:

```tsx
function processChildren(children: React.ReactNode): React.ReactNode {
  if (Array.isArray(children)) {
    return children.map((child, i) =>
      typeof child === "string" ? <LinkedText key={i}>{child}</LinkedText> : child,
    );
  }
  return typeof children === "string" ? <LinkedText>{children}</LinkedText> : children;
}

function LinkedText({ children }: { children: string }) {
  const MENTION_SPLIT = /(@[\w.\-]+\/[\w.\-/]+)/g;
  const MENTION_TEST = /^@[\w.\-]+\/[\w.\-/]+$/;

  const parts = children.split(MENTION_SPLIT);
  return (
    <>
      {parts.map((part, i) =>
        MENTION_TEST.test(part) ? (
          <MentionText key={i}>{part}</MentionText>
        ) : (
          <JiraLinkedText key={i}>{part}</JiraLinkedText>
        ),
      )}
    </>
  );
}
```

- [ ] **Step 3: Verify build**

Run: `bun run build 2>&1 | tail -5`
Expected: No errors.

- [ ] **Step 4: Commit**

```bash
git add src/components/MentionText.tsx src/features/session/DumpView.tsx src/features/chat/ChatPanel.tsx
git commit -m "feat: render @mentions as styled pills in markdown preview"
```

---

### Task 11: Integration test

**Files:** None (testing only)

- [ ] **Step 1: Run all Rust tests**

Run: `cd src-tauri && cargo test 2>&1 | grep "test result"`
Expected: All tests pass.

- [ ] **Step 2: Run frontend build check**

Run: `bun run build 2>&1 | tail -5`
Expected: Builds with no errors.

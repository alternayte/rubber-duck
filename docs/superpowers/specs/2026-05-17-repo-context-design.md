# Repo Context — Design Spec

## Overview

Attach local folders or git repos to a session. The duck gains awareness of your codebase: directory tree, summaries, and file contents via `@` mentions. Multiple repos per session. Git URLs are cloned automatically.

## Backend

### New module: `repo_context/`

Package-by-feature: `src-tauri/src/repo_context/` with `mod.rs`, `model.rs`, `store.rs`, `commands.rs`, `tree.rs`, `clone.rs`.

### Data model

```rust
pub struct RepoContext {
    pub id: String,
    pub session_id: String,
    pub name: String,           // display name (repo dir name)
    pub source: String,         // original input: local path or git URL
    pub local_path: String,     // resolved absolute path on disk
    pub created_at: String,
}
```

### Database

New `repos` table (added via migration):

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

### Store functions

- `attach(conn, session_id, name, source, local_path) -> AppResult<RepoContext>`
- `detach(conn, repo_id) -> AppResult<()>`
- `list_by_session(conn, session_id) -> AppResult<Vec<RepoContext>>`
- `get(conn, repo_id) -> AppResult<RepoContext>`

### Git clone

In `clone.rs`:

```rust
pub fn clone_repo(url: &str, target_dir: &Path) -> AppResult<PathBuf>
```

- Runs `git clone --depth 1 <url> <target_dir>/<repo-name>` via `std::process::Command`
- Extracts repo name from URL (last path segment, strip `.git` suffix)
- Clone target: `{app_data}/repos/{name}` (persistent, not tmp)
- If directory already exists, runs `git pull --ff-only` instead of re-cloning
- Returns the resolved local path
- Errors propagated with clear messages (git not found, clone failed, auth required)

### Tree walking

In `tree.rs`:

```rust
pub struct FileNode {
    pub name: String,
    pub path: String,           // relative to repo root
    pub is_dir: bool,
    pub children: Vec<FileNode>,
}

pub fn walk_tree(repo_path: &Path) -> AppResult<Vec<FileNode>>
```

- Respects `.gitignore` using the `ignore` crate (same crate ripgrep uses)
- Skips `.git/` directory
- Returns nested tree structure
- Caps depth at 6 levels, caps total entries at 5000 (prevents blowup on huge repos)

### File reading

```rust
pub fn read_file(repo_path: &Path, relative_path: &str) -> AppResult<String>
```

- Validates the path is within the repo root (path traversal protection via `canonicalize`)
- Reads file content as UTF-8 (returns error for binary files — check for null bytes in first 512 bytes)
- No size limit on read (context assembly handles truncation)

### Fuzzy file search

```rust
pub fn search_files(repo_path: &Path, query: &str) -> AppResult<Vec<String>>
```

- Walks the file tree (respecting .gitignore via `ignore` crate)
- Filters to files only (not directories)
- Fuzzy matches query against relative file paths
- Uses a simple substring + path-segment matching approach (no external fuzzy library needed):
  - Score by: exact filename match > filename contains query > path contains query
  - Return top 20 results, sorted by score
- Returns relative paths

### Repo summary

```rust
pub fn generate_summary(repo_path: &Path) -> AppResult<String>
```

- Walks tree, counts files by extension
- Identifies key directories (src, lib, test, docs, etc.)
- Generates a brief text summary: "TypeScript/React project. 245 files. Key dirs: src/, tests/, docs/"
- Output is a single string suitable for LLM context injection

### Tauri commands

| Command | Params | Returns | Notes |
|---------|--------|---------|-------|
| `attach_repo` | `session_id, source` | `RepoContext` | Detects local path vs git URL. Clones if URL. |
| `detach_repo` | `repo_id` | `()` | Removes from DB. Does NOT delete cloned files. |
| `list_repos` | `session_id` | `Vec<RepoContext>` | |
| `get_repo_tree` | `repo_id` | `Vec<FileNode>` | Nested tree structure |
| `read_repo_file` | `repo_id, path` | `String` | File content |
| `search_repo_files` | `session_id, query` | `Vec<FileSearchResult>` | Searches all attached repos |

`FileSearchResult`:
```rust
pub struct FileSearchResult {
    pub repo_id: String,
    pub repo_name: String,
    pub relative_path: String,
    pub display: String,  // "repo-name/path/to/file.ts"
}
```

`attach_repo` detection logic:
- If source starts with `http://`, `https://`, `git@`, or `ssh://` → treat as git URL, clone
- Otherwise → treat as local path, validate it exists and is a directory

### Context assembly changes

Modified `assemble_context` gets a new parameter:

```rust
pub struct RepoFileContext {
    pub display: String,       // "repo-name/path/to/file.ts"
    pub content: String,       // file content, truncated to 3000 chars
}
```

New parameters on `assemble_context`:
- `repo_summaries: &[String]` — one summary string per attached repo
- `mentioned_files: &[RepoFileContext]` — files referenced via `@` mentions

New sections in system prompt:
```
## Attached Repositories
TypeScript/React project (rubber-duck). 245 files. Key dirs: src/, tests/, docs/
Go microservice (auth-service). 42 files. Key dirs: cmd/, internal/, api/

## Referenced Files
### rubber-duck/src/features/chat/ChatPanel.tsx
[file content here, truncated at 3000 chars]

### auth-service/internal/handler/login.go
[file content here]
```

### Modified `send_message`

Before assembling context:
1. Load attached repos for the session
2. Generate summaries for each repo
3. Scan notes + user message for `@repo-name/path` patterns
4. Read referenced files, truncate to 3000 chars each
5. Pass summaries + file contexts to `assemble_context`

`@` mention extraction regex: `@([\w.-]+\/[\w./-]+)` — matches `@repo-name/path/to/file.ext`

## Frontend

### Context section (sidebar)

The existing placeholder in `App.tsx` becomes a full `RepoPanel` component:

```
┌─ Context ──────────────────────────────────┐
│  [+ Attach Repo]                            │
│                                             │
│  📁 rubber-duck (local)            [✕]     │
│  📦 auth-service (git)             [✕]     │
│                                             │
│  ▸ rubber-duck/                             │
│    ▸ src/                                   │
│      ▸ features/                            │
│        ▸ chat/                              │
│          ChatPanel.tsx                       │
│          chat.atoms.ts                       │
│    ▸ components/                            │
│  ▸ auth-service/                            │
└─────────────────────────────────────────────┘
```

- "Attach Repo" button opens a small dialog with two options:
  - "Local Folder" → native OS folder picker (Tauri dialog plugin)
  - "Git URL" → text input + "Clone" button
- Cloning shows a spinner + "Cloning..." status
- Each repo shows name + source type icon + detach button
- Expandable tree view (click to expand dirs)
- Tree is lazy-loaded per repo on expand (not all at once)

### `@` mention autocomplete — Chat input

- When user types `@` in the chat input:
  - A floating dropdown appears above the input
  - As they type after `@`, calls `search_repo_files(sessionId, query)` (debounced 200ms)
  - Shows results as `repo-name/path/to/file.ext`
  - Arrow keys to navigate, Enter to select, Escape to dismiss
- Selecting a file inserts `@repo-name/path/to/file.ext` as text in the input
- Visual: the `@mention` is styled inline with a subtle background color (like a pill) using a regex-based decoration on the input display
- On send: the backend extracts `@` mentions, reads files, injects into context

### `@` mention autocomplete — Dump editor (CodeMirror)

- Same trigger: typing `@` opens a CodeMirror autocomplete dropdown
- Uses CodeMirror's built-in `autocompletion` extension
- Completion source: calls `search_repo_files` via Tauri invoke
- Selecting inserts `@repo-name/path/to/file.ext` as plain text
- Visual: CodeMirror `Decoration` marks `@mentions` with a styled class

### `@` mention rendering in markdown preview

- Same approach as `JiraLinkedText`: a component that detects `@repo/path` patterns
- Renders as a styled pill with a file icon
- Not clickable (no action on click — files are local)

### Tauri plugin permissions

Need to add to capabilities:
- `dialog:default` — for native folder picker
- `fs:default` — for reading files outside app data dir (repo files)

### New dependencies (Rust)

- `ignore` crate — for `.gitignore`-aware directory walking (same crate ripgrep uses)

## Out of Scope

- File editing from within rubber-duck
- Real-time file watching (re-index on change)
- Deep semantic indexing / embeddings
- Diff view or git history
- Branch switching on cloned repos
- Repo settings in the settings dialog (clone directory is always `{app_data}/repos/`)

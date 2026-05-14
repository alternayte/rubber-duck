# rubber-duck

A local-first, LLM-powered planning and brainstorming tool that helps you think through problems, structure your thoughts into actionable tickets, and push them to Jira or Linear.

## Project Overview

rubber-duck is a Tauri v2 desktop app with a Rust backend and React/TypeScript frontend. It provides a freeform thinking space where you brain-dump ideas, collaborate with an LLM to refine them, and produce structured outputs — tickets for issue trackers, or SDLC documents like PRDs, SDDs, and test plans.

**Core loop:** Brain dump → LLM structures → Refine iteratively → Push to Jira/Linear OR Generate docs

## Tech Stack

- **Runtime:** Tauri v2
- **Backend:** Rust (2021 edition, latest stable)
- **Frontend:** React 19 + TypeScript + Tailwind CSS v4
- **Storage:** SQLite via `rusqlite` with FTS5 for full-text search
- **LLM:** Anthropic API (primary), OpenRouter (configurable), Ollama (offline fallback)
- **Build:** Cargo workspace (if/when crates are extracted), bun for frontend

## Architecture Principles

- **Package-by-feature / colocated:** Each feature owns its models, Tauri commands, storage queries, and tests. No horizontal layers like `models/`, `services/`, `repositories/`.
- **Idiomatic Rust:** Use the type system. Enums over stringly-typed fields. `Result<T, E>` everywhere. Derive macros for serialization. No unnecessary traits or abstractions — add them when the second implementation arrives.
- **Pragmatic DDD:** Domain types are the source of truth. No anemic models. Business logic lives on the types, not in service layers.
- **YAGNI:** Don't build abstractions for future needs. The code should be simple enough that refactoring later is cheap.
- **Local-first:** All data lives in a local SQLite database. Nothing leaves the machine unless the user explicitly pushes.

## Project Structure

```
rubber-duck/
├── CLAUDE.md
├── docs/
│   ├── PRD.md
│   └── PLAN.md
├── templates/                # Built-in doc templates (PRD, SDD, etc.)
│   ├── prd.md
│   ├── sdd.md
│   ├── test-plan.md
│   └── adr.md
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   └── src/
│       ├── main.rs           # Tauri bootstrap
│       ├── lib.rs            # Module declarations, app state
│       ├── error.rs          # Unified error types
│       ├── db.rs             # SQLite connection, migrations
│       │
│       ├── session/          # Planning sessions
│       │   ├── mod.rs
│       │   ├── model.rs      # Session, Note, Attachment types
│       │   ├── commands.rs   # Tauri commands (#[tauri::command])
│       │   └── store.rs      # SQLite queries
│       │
│       ├── ticket/           # Structured tickets
│       │   ├── mod.rs
│       │   ├── model.rs      # Ticket, Epic, Priority, etc.
│       │   ├── commands.rs
│       │   └── store.rs
│       │
│       ├── llm/              # LLM integration
│       │   ├── mod.rs
│       │   ├── provider.rs   # Provider trait + Anthropic/OpenRouter/Ollama impls
│       │   ├── streaming.rs  # SSE → Tauri event bridge
│       │   └── context.rs    # Context window assembly (notes + tickets + history)
│       │
│       ├── sync/             # Issue tracker sync
│       │   ├── mod.rs
│       │   ├── platform.rs   # TicketPlatform trait
│       │   ├── jira.rs       # Jira Cloud REST API impl
│       │   └── linear.rs     # Linear GraphQL API impl
│       │
│       ├── docs/             # SDLC document generation
│       │   ├── mod.rs
│       │   ├── template.rs   # Template loading + rendering
│       │   └── generator.rs  # LLM-powered doc fill
│       │
│       └── memory/           # Session memory + search
│           ├── mod.rs
│           ├── summary.rs    # Auto-summarize sessions, extract decisions
│           ├── fts.rs        # FTS5 full-text search
│           └── rag.rs        # Vector embeddings (later phase)
│
├── src/                      # React frontend
│   ├── main.tsx
│   ├── App.tsx
│   ├── components/
│   ├── features/
│   │   ├── session/          # Mirrors backend features
│   │   ├── ticket/
│   │   ├── chat/
│   │   ├── board/
│   │   └── docs/
│   ├── hooks/
│   ├── lib/
│   │   └── tauri.ts          # Typed Tauri invoke/listen wrappers
│   └── styles/
│
├── package.json
├── tsconfig.json
└── vite.config.ts
```

## Coding Conventions

### Rust

- Use `thiserror` for error enums, `anyhow` only at the top-level app boundary.
- Tauri commands return `Result<T, String>` (Tauri's requirement). Map errors at the command boundary: `store::get_session(id).map_err(|e| e.to_string())`.
- Use `serde::{Serialize, Deserialize}` on all types that cross the Tauri bridge.
- Use `rusqlite::params![]` for queries. No raw string interpolation in SQL.
- Prefer `&str` over `String` in function signatures where ownership isn't needed.
- Tests live next to the code: `#[cfg(test)] mod tests { ... }` at the bottom of each file.
- No `unwrap()` in non-test code. Use `?` or explicit error handling.

### TypeScript/React

- Functional components with hooks. No class components.
- Use `invoke` and `listen` from `@tauri-apps/api` for backend communication.
- Tailwind for styling. No CSS modules or styled-components.
- Colocate component files: `SessionView.tsx`, `useSession.ts`, `session.types.ts` in the same feature folder.

### General

- Commit messages: conventional commits (`feat:`, `fix:`, `refactor:`, `docs:`).
- No premature optimization. Profile first, then optimize.
- When in doubt, write less code.

## Key Dependencies (Rust)

```toml
[dependencies]
tauri = { version = "2", features = [] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rusqlite = { version = "0.39", features = ["bundled"] }
reqwest = { version = "0.13", features = ["json", "stream"] }
tokio = { version = "1", features = ["full"] }
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
thiserror = "2"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
```

## LLM Context Strategy

When assembling the LLM context for a session:
1. System prompt with role definition and output format instructions
2. Session context (user-provided background, repo summaries)
3. Current notes (the brain dump, trimmed if too long)
4. Current tickets (structured, always included in full)
5. Recent conversation history (last N messages, summarize older ones)
6. Relevant memories from past sessions (via FTS/RAG search)

Keep total context under the model's limit. Prioritize: tickets > notes > conversation > memories.

## Development Workflow

This project uses Claude Code with Superpowers. Follow the standard workflow:
1. **Brainstorm** before building — clarify requirements and approach
2. **Write a plan** before implementing — break work into small, testable tasks
3. **TDD** — write failing tests first, then implement, then refactor
4. **Commit often** — each task gets its own commit with a clear message

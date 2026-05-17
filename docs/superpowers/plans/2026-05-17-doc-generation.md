# Document Generation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Generate SDLC documents (PRDs, SDDs, Test Plans, ADRs) from session context using LLM-powered templates. Built-in templates ship with the app; users can create custom templates. Documents are persistent, per-section editable and regeneratable, and exportable as markdown.

**Architecture:** New `docs/` feature module (Rust) + `src/features/docs/` (React). Backend: SQLite migration for `documents`, `document_sections`, `document_section_versions`, `templates` tables; template parsing from embedded markdown files via `include_str!`; per-section LLM streaming via `doc:chunk/doc:done/doc:error` events (separate namespace from chat). Frontend: rename "Refine" tab to "Docs", `DocsView` (list + generate), `DocumentCard` (sections with expand/edit/regenerate/history), `TemplateManager`, `VersionHistory`.

**Tech Stack:** Rust (rusqlite, tokio, reqwest, serde), React 19, Jotai, Tailwind, shadcn/ui (Button, Popover, DropdownMenu), Lucide icons, react-markdown + remark-gfm

---

## File Map

| Action | File | Responsibility |
|--------|------|----------------|
| Create | `src-tauri/migrations/005_add_docs.sql` | New tables: documents, sections, versions, templates |
| Modify | `src-tauri/src/db.rs` | Register migration |
| Create | `src-tauri/src/docs/mod.rs` | Module declarations |
| Create | `src-tauri/src/docs/model.rs` | Document, DocumentSection, SectionVersion, Template, BuiltinTemplate, TemplateSection |
| Create | `src-tauri/templates/prd.md` | Built-in PRD template |
| Create | `src-tauri/templates/sdd.md` | Built-in SDD template |
| Create | `src-tauri/templates/test-plan.md` | Built-in Test Plan template |
| Create | `src-tauri/templates/adr.md` | Built-in ADR template |
| Create | `src-tauri/src/docs/templates.rs` | parse_template(), get_builtin_templates() |
| Create | `src-tauri/src/docs/store.rs` | Document + section CRUD + version functions |
| Create | `src-tauri/src/docs/template_store.rs` | Custom template CRUD |
| Create | `src-tauri/src/docs/generator.rs` | generate_doc_section LLM streaming |
| Create | `src-tauri/src/docs/commands.rs` | All Tauri commands |
| Modify | `src-tauri/src/lib.rs` | Register mod docs + commands |
| Create | `src/features/docs/docs.types.ts` | TypeScript types |
| Create | `src/features/docs/docs.atoms.ts` | Jotai atoms |
| Create | `src/features/docs/useDocActions.ts` | Hook for document CRUD + generation |
| Modify | `src/App.tsx` | Rename Refine→Docs, render DocsView |
| Create | `src/features/docs/DocsView.tsx` | Document list + generate button + template picker |
| Create | `src/features/docs/DocumentCard.tsx` | Per-document card with sections |
| Create | `src/features/docs/TemplateManager.tsx` | Custom template list + editor |
| Create | `src/features/docs/VersionHistory.tsx` | Version history popover |

---

### Task 1: Database migration + models

**Files:**
- Create: `src-tauri/migrations/005_add_docs.sql`
- Modify: `src-tauri/src/db.rs`
- Create: `src-tauri/src/docs/mod.rs`
- Create: `src-tauri/src/docs/model.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create migration file**

Create `src-tauri/migrations/005_add_docs.sql`:

```sql
CREATE TABLE documents (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    template_name TEXT NOT NULL,
    title TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE document_sections (
    id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    directive TEXT NOT NULL,
    content TEXT NOT NULL DEFAULT '',
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE document_section_versions (
    id TEXT PRIMARY KEY,
    section_id TEXT NOT NULL REFERENCES document_sections(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE templates (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_documents_session ON documents(session_id);
CREATE INDEX idx_document_sections_document ON document_sections(document_id);
CREATE INDEX idx_document_section_versions_section ON document_section_versions(section_id);
```

- [ ] **Step 2: Register migration in db.rs**

In `src-tauri/src/db.rs`, add to the `MIGRATIONS` array after `004_add_repos`:

```rust
    Migration {
        name: "005_add_docs",
        sql: include_str!("../migrations/005_add_docs.sql"),
    },
```

Update the `migrations_apply_on_fresh_db` test assertion from `assert_eq!(count, 4)` to `assert_eq!(count, 5)`.

- [ ] **Step 3: Create docs module files**

Create `src-tauri/src/docs/mod.rs`:

```rust
pub mod commands;
pub mod generator;
pub mod model;
pub mod store;
pub mod template_store;
pub mod templates;
```

Create `src-tauri/src/docs/model.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub session_id: String,
    pub template_name: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSection {
    pub id: String,
    pub document_id: String,
    pub name: String,
    pub directive: String,
    pub content: String,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionVersion {
    pub id: String,
    pub section_id: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub id: String,
    pub name: String,
    pub content: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuiltinTemplate {
    pub name: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSection {
    pub name: String,
    pub directive: String,
}
```

- [ ] **Step 4: Register mod in lib.rs**

In `src-tauri/src/lib.rs`, add `mod docs;` alongside the other module declarations:

```rust
mod docs;
```

- [ ] **Step 5: Verify migration compiles**

```bash
cd src-tauri && cargo check
```

Expected: no errors. The `_migrations` count test will fail until Step 2 is complete — confirm the test now asserts `5`.

---

### Task 2: Built-in templates + parser

**Files:**
- Create: `src-tauri/templates/prd.md`
- Create: `src-tauri/templates/sdd.md`
- Create: `src-tauri/templates/test-plan.md`
- Create: `src-tauri/templates/adr.md`
- Create: `src-tauri/src/docs/templates.rs`

- [ ] **Step 1: Create built-in template files**

Create `src-tauri/templates/prd.md`:

```markdown
# Product Requirements Document
<!-- section: Overview -->
<!-- directive: Write a 2-3 paragraph overview of this product or feature based on the session notes and tickets. Cover: (1) the problem being solved, (2) the target users and their pain points, and (3) why this solution is the right approach. Be concrete — name specific user roles, describe actual frustrations, avoid generic language. -->

<!-- section: User Stories -->
<!-- directive: Extract user stories from the session notes and tickets. Format each as: "As a [specific role], I want [concrete goal] so that [measurable benefit]." Write at least 5 stories covering the main use cases. Derive roles and goals from the actual session context — don't invent scenarios not present in the notes. -->

<!-- section: Functional Requirements -->
<!-- directive: List the functional requirements derived from the notes, tickets, and user stories. Number each requirement (FR-1, FR-2, ...). Each requirement must be specific and testable — avoid vague language like "the system should be fast" or "handle errors gracefully." Include edge cases and boundary conditions mentioned in the notes. -->

<!-- section: Non-Functional Requirements -->
<!-- directive: List non-functional requirements relevant to this feature: performance targets, security constraints, scalability expectations, accessibility standards, browser/OS compatibility. Only include requirements with evidence in the session context — do not add boilerplate. If no NFRs are apparent, write "No non-functional requirements identified in this session." -->

<!-- section: Success Criteria -->
<!-- directive: Define measurable success criteria for this feature. Each criterion must be falsifiable: specify a metric, a target value, and how it will be measured. Examples: "P95 latency < 200ms under 1000 concurrent users (measured via load test)", "Zero P0 bugs in first 30 days post-launch (tracked in Jira)". Base these on the session context, not generic best practices. -->
```

Create `src-tauri/templates/sdd.md`:

```markdown
# Software Design Document
<!-- section: Overview -->
<!-- directive: Write a technical overview of this design based on the session notes and tickets. Cover: (1) what is being built, (2) the key technical decisions and why they were made, and (3) what existing systems this integrates with or replaces. Assume the reader is a senior engineer who will review this design. Be precise about technology choices. -->

<!-- section: Architecture -->
<!-- directive: Describe the high-level architecture. Identify the major components and how they interact. Describe data flow for the primary use cases. Call out any significant architectural patterns used (event-driven, CQRS, microservices split, etc.) and why they apply here. If the session context mentions specific architectural constraints, reflect them accurately. -->

<!-- section: Components -->
<!-- directive: Enumerate the major components, modules, or services that make up this system. For each component, describe: its responsibility, its inputs and outputs, and its key interfaces or APIs. Use a consistent format for each. Base this on the actual work items and notes in the session — don't invent components not discussed. -->

<!-- section: Data Model -->
<!-- directive: Describe the data model. List the key entities, their fields (with types), and the relationships between them. If there are database schema changes, show the table definitions. Highlight any indexing strategy, denormalization decisions, or soft-delete patterns. Base this on the session context. -->

<!-- section: API Design -->
<!-- directive: Document the API surface. For each endpoint or command, specify: method/name, inputs, outputs, error cases, and authentication requirements. Use a consistent format (e.g., REST-style or RPC-style depending on what the session context implies). Include at least the happy path and the 2-3 most important error paths. -->

<!-- section: Error Handling -->
<!-- directive: Describe the error handling strategy for this system. Cover: how errors propagate through layers, what errors are recoverable vs. fatal, what the user experience is for each error type, and how errors are logged or alerted. Reference specific failure modes mentioned in the session notes or tickets. -->
```

Create `src-tauri/templates/test-plan.md`:

```markdown
# Test Plan
<!-- section: Overview -->
<!-- directive: Write a brief overview of this test plan. Describe what is being tested (the feature or system from the session context), the testing goals, and the scope — what is in scope and what is explicitly out of scope. Mention the key risks this test plan is designed to mitigate. -->

<!-- section: Test Strategy -->
<!-- directive: Describe the testing strategy. Cover the test pyramid: what will be covered by unit tests, integration tests, and end-to-end tests. Mention any manual testing requirements. Describe the test environment and test data strategy. If the session context mentions CI/CD or specific testing tools, incorporate them. -->

<!-- section: Test Cases -->
<!-- directive: Write the test cases for the primary functionality. For each test case, specify: a unique ID (TC-1, TC-2, ...), a descriptive name, preconditions, steps to execute, and expected result. Cover the happy path for each major feature, and include at least 2-3 negative test cases (invalid input, unauthorized access, resource not found). Base these on the actual tickets and requirements in the session. -->

<!-- section: Edge Cases -->
<!-- directive: Identify and document edge cases that must be tested. These are the boundary conditions, unusual inputs, and failure scenarios that are easy to overlook. For each edge case, describe the scenario and the expected behavior. Draw from the session notes, tickets, and any explicit edge cases mentioned. If none were mentioned, reason about what could go wrong in this system. -->

<!-- section: Acceptance Criteria -->
<!-- directive: Translate the functional requirements and user stories from the session into acceptance criteria for this release. These are the conditions that must be true for the feature to be considered "done." Format as a checklist. Each criterion must be objectively verifiable — a QA engineer should be able to check it off with a clear pass/fail. -->
```

Create `src-tauri/templates/adr.md`:

```markdown
# Architecture Decision Record
<!-- section: Context -->
<!-- directive: Describe the situation that forced this decision. What problem was being faced? What constraints existed (technical, organizational, timeline, budget)? What triggered the need to make a decision now? Draw directly from the session notes and tickets — be specific about the forces at play, not generic. This section should read like a brief problem statement. -->

<!-- section: Decision -->
<!-- directive: State the decision that was made. Be direct: "We will use X." Then explain the reasoning: why this option over the alternatives? What were the key factors that tipped the decision? Reference the constraints from the Context section. If the session notes include a clear decision or preference, reflect it accurately rather than inventing one. -->

<!-- section: Consequences -->
<!-- directive: Describe the consequences of this decision — both positive and negative. What does this decision make easier? What does it make harder? What technical debt does it introduce? Are there any future decisions that will be constrained by this one? Be honest about tradeoffs. Include any operational implications (deployment complexity, monitoring requirements, on-call burden). -->

<!-- section: Alternatives Considered -->
<!-- directive: List the alternatives that were considered and rejected. For each alternative: (1) briefly describe the approach, (2) explain why it was rejected (cost, complexity, risk, missing capability). If the session notes mention specific alternatives, use those. If not, reason about the 2-3 most likely alternatives for this type of decision. This section justifies the decision to future readers who may question it. -->
```

- [ ] **Step 2: Create the template parser**

Create `src-tauri/src/docs/templates.rs`:

```rust
use super::model::{BuiltinTemplate, TemplateSection};

const PRD_TEMPLATE: &str = include_str!("../../templates/prd.md");
const SDD_TEMPLATE: &str = include_str!("../../templates/sdd.md");
const TEST_PLAN_TEMPLATE: &str = include_str!("../../templates/test-plan.md");
const ADR_TEMPLATE: &str = include_str!("../../templates/adr.md");

pub fn get_builtin_templates() -> Vec<BuiltinTemplate> {
    vec![
        BuiltinTemplate {
            name: "PRD".to_string(),
            content: PRD_TEMPLATE.to_string(),
        },
        BuiltinTemplate {
            name: "SDD".to_string(),
            content: SDD_TEMPLATE.to_string(),
        },
        BuiltinTemplate {
            name: "Test Plan".to_string(),
            content: TEST_PLAN_TEMPLATE.to_string(),
        },
        BuiltinTemplate {
            name: "ADR".to_string(),
            content: ADR_TEMPLATE.to_string(),
        },
    ]
}

/// Parse a template's markdown content into sections.
///
/// Looks for paired `<!-- section: X -->` and `<!-- directive: Y -->` comments.
/// Sections without a corresponding directive are skipped.
/// The directive may span multiple lines — everything from `<!-- directive:` to
/// the closing `-->` is captured as the directive text.
pub fn parse_template(content: &str) -> Vec<TemplateSection> {
    let mut sections: Vec<TemplateSection> = Vec::new();
    let mut current_section: Option<String> = None;
    let mut current_directive: Option<String> = None;
    let mut in_directive = false;
    let mut directive_buf = String::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Detect opening of a directive block (may close on same line)
        if trimmed.starts_with("<!-- directive:") {
            if let Some(close) = trimmed.find("-->") {
                // Single-line directive: <!-- directive: ... -->
                let inner = trimmed["<!-- directive:".len()..close].trim();
                current_directive = Some(inner.to_string());
                in_directive = false;
                directive_buf.clear();
            } else {
                // Multi-line directive: <!-- directive:\n...\n-->
                in_directive = true;
                directive_buf.clear();
                let inner = trimmed["<!-- directive:".len()..].trim();
                if !inner.is_empty() {
                    directive_buf.push_str(inner);
                    directive_buf.push(' ');
                }
            }
            continue;
        }

        if in_directive {
            if trimmed.ends_with("-->") {
                let inner = trimmed.trim_end_matches("-->").trim();
                if !inner.is_empty() {
                    directive_buf.push_str(inner);
                    directive_buf.push(' ');
                }
                current_directive = Some(directive_buf.trim().to_string());
                directive_buf.clear();
                in_directive = false;
            } else {
                directive_buf.push_str(trimmed);
                directive_buf.push(' ');
            }
            continue;
        }

        // Detect section marker: <!-- section: Name -->
        if trimmed.starts_with("<!-- section:") {
            if let Some(close) = trimmed.find("-->") {
                let name = trimmed["<!-- section:".len()..close].trim().to_string();

                // If we have a pending section + directive pair, save it
                if let (Some(sec), Some(dir)) = (current_section.take(), current_directive.take()) {
                    sections.push(TemplateSection {
                        name: sec,
                        directive: dir,
                    });
                }

                current_section = Some(name);
                current_directive = None;
            }
        }
    }

    // Flush final section
    if let (Some(sec), Some(dir)) = (current_section, current_directive) {
        sections.push(TemplateSection {
            name: sec,
            directive: dir,
        });
    }

    sections
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_section() {
        let content = r#"
<!-- section: Overview -->
<!-- directive: Write a brief overview. -->
"#;
        let sections = parse_template(content);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].name, "Overview");
        assert_eq!(sections[0].directive, "Write a brief overview.");
    }

    #[test]
    fn parse_multiple_sections() {
        let content = r#"
# PRD
<!-- section: Overview -->
<!-- directive: Write overview. -->

<!-- section: Requirements -->
<!-- directive: List requirements. -->
"#;
        let sections = parse_template(content);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].name, "Overview");
        assert_eq!(sections[1].name, "Requirements");
    }

    #[test]
    fn section_without_directive_is_skipped() {
        let content = r#"
<!-- section: Orphan -->

<!-- section: HasDirective -->
<!-- directive: Write something. -->
"#;
        let sections = parse_template(content);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].name, "HasDirective");
    }

    #[test]
    fn directive_without_section_is_ignored() {
        let content = r#"
<!-- directive: This has no section. -->

<!-- section: Valid -->
<!-- directive: Write something. -->
"#;
        let sections = parse_template(content);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].name, "Valid");
    }

    #[test]
    fn parses_all_builtin_templates() {
        for tmpl in get_builtin_templates() {
            let sections = parse_template(&tmpl.content);
            assert!(
                !sections.is_empty(),
                "Template '{}' parsed to 0 sections",
                tmpl.name
            );
            for section in &sections {
                assert!(!section.name.is_empty(), "Empty section name in '{}'", tmpl.name);
                assert!(!section.directive.is_empty(), "Empty directive in '{}' section '{}'", tmpl.name, section.name);
            }
        }
    }

    #[test]
    fn prd_has_five_sections() {
        let sections = parse_template(PRD_TEMPLATE);
        assert_eq!(sections.len(), 5);
        assert_eq!(sections[0].name, "Overview");
        assert_eq!(sections[4].name, "Success Criteria");
    }

    #[test]
    fn sdd_has_six_sections() {
        let sections = parse_template(SDD_TEMPLATE);
        assert_eq!(sections.len(), 6);
        assert_eq!(sections[0].name, "Overview");
        assert_eq!(sections[5].name, "Error Handling");
    }

    #[test]
    fn test_plan_has_five_sections() {
        let sections = parse_template(TEST_PLAN_TEMPLATE);
        assert_eq!(sections.len(), 5);
        assert_eq!(sections[0].name, "Overview");
        assert_eq!(sections[4].name, "Acceptance Criteria");
    }

    #[test]
    fn adr_has_four_sections() {
        let sections = parse_template(ADR_TEMPLATE);
        assert_eq!(sections.len(), 4);
        assert_eq!(sections[0].name, "Context");
        assert_eq!(sections[3].name, "Alternatives Considered");
    }
}
```

- [ ] **Step 3: Run parser tests**

```bash
cd src-tauri && cargo test docs::templates
```

Expected output: all 8 tests pass.

---

### Task 3: Document store + template store

**Files:**
- Create: `src-tauri/src/docs/store.rs`
- Create: `src-tauri/src/docs/template_store.rs`

- [ ] **Step 1: Create document + section + version store**

Create `src-tauri/src/docs/store.rs`:

```rust
use rusqlite::{params, Connection};

use crate::error::AppResult;

use super::model::{Document, DocumentSection, SectionVersion};

// ── row mappers ────────────────────────────────────────────────────────────────

fn row_to_document(row: &rusqlite::Row) -> rusqlite::Result<Document> {
    Ok(Document {
        id: row.get(0)?,
        session_id: row.get(1)?,
        template_name: row.get(2)?,
        title: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn row_to_section(row: &rusqlite::Row) -> rusqlite::Result<DocumentSection> {
    Ok(DocumentSection {
        id: row.get(0)?,
        document_id: row.get(1)?,
        name: row.get(2)?,
        directive: row.get(3)?,
        content: row.get(4)?,
        sort_order: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn row_to_version(row: &rusqlite::Row) -> rusqlite::Result<SectionVersion> {
    Ok(SectionVersion {
        id: row.get(0)?,
        section_id: row.get(1)?,
        content: row.get(2)?,
        created_at: row.get(3)?,
    })
}

// ── documents ─────────────────────────────────────────────────────────────────

pub fn create_document(
    conn: &Connection,
    session_id: &str,
    template_name: &str,
    title: &str,
) -> AppResult<Document> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO documents (id, session_id, template_name, title)
         VALUES (?1, ?2, ?3, ?4)",
        params![id, session_id, template_name, title],
    )?;
    get_document(conn, &id)
}

pub fn get_document(conn: &Connection, id: &str) -> AppResult<Document> {
    let doc = conn.query_row(
        "SELECT id, session_id, template_name, title, created_at, updated_at
         FROM documents WHERE id = ?1",
        params![id],
        row_to_document,
    )?;
    Ok(doc)
}

pub fn list_documents(conn: &Connection, session_id: &str) -> AppResult<Vec<Document>> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, template_name, title, created_at, updated_at
         FROM documents WHERE session_id = ?1 ORDER BY created_at ASC",
    )?;
    let docs = stmt
        .query_map(params![session_id], row_to_document)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(docs)
}

pub fn delete_document(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM documents WHERE id = ?1", params![id])?;
    Ok(())
}

// ── sections ──────────────────────────────────────────────────────────────────

pub fn create_section(
    conn: &Connection,
    document_id: &str,
    name: &str,
    directive: &str,
    sort_order: i32,
) -> AppResult<DocumentSection> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO document_sections (id, document_id, name, directive, sort_order)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, document_id, name, directive, sort_order],
    )?;
    get_section(conn, &id)
}

pub fn get_section(conn: &Connection, id: &str) -> AppResult<DocumentSection> {
    let section = conn.query_row(
        "SELECT id, document_id, name, directive, content, sort_order, created_at, updated_at
         FROM document_sections WHERE id = ?1",
        params![id],
        row_to_section,
    )?;
    Ok(section)
}

pub fn list_sections(conn: &Connection, document_id: &str) -> AppResult<Vec<DocumentSection>> {
    let mut stmt = conn.prepare(
        "SELECT id, document_id, name, directive, content, sort_order, created_at, updated_at
         FROM document_sections WHERE document_id = ?1 ORDER BY sort_order ASC",
    )?;
    let sections = stmt
        .query_map(params![document_id], row_to_section)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(sections)
}

pub fn update_section_content(
    conn: &Connection,
    section_id: &str,
    content: &str,
) -> AppResult<DocumentSection> {
    conn.execute(
        "UPDATE document_sections SET content = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![content, section_id],
    )?;
    get_section(conn, section_id)
}

// ── versions ──────────────────────────────────────────────────────────────────

/// Save the current content of a section as a version snapshot.
/// Empty content is not saved (initial state before first generation).
pub fn save_version(conn: &Connection, section_id: &str, content: &str) -> AppResult<()> {
    if content.is_empty() {
        return Ok(());
    }
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO document_section_versions (id, section_id, content) VALUES (?1, ?2, ?3)",
        params![id, section_id, content],
    )?;
    Ok(())
}

pub fn list_versions(conn: &Connection, section_id: &str) -> AppResult<Vec<SectionVersion>> {
    let mut stmt = conn.prepare(
        "SELECT id, section_id, content, created_at
         FROM document_section_versions WHERE section_id = ?1
         ORDER BY created_at DESC",
    )?;
    let versions = stmt
        .query_map(params![section_id], row_to_version)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(versions)
}

pub fn get_version(conn: &Connection, version_id: &str) -> AppResult<SectionVersion> {
    let version = conn.query_row(
        "SELECT id, section_id, content, created_at FROM document_section_versions WHERE id = ?1",
        params![version_id],
        row_to_version,
    )?;
    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::session::store as session_store;

    fn test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn make_session(conn: &Connection) -> String {
        session_store::create(conn, "Test Session").unwrap().id
    }

    fn make_doc(conn: &Connection, session_id: &str) -> Document {
        create_document(conn, session_id, "PRD", "My PRD").unwrap()
    }

    #[test]
    fn create_and_get_document() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let session_id = make_session(&conn);

        let doc = create_document(&conn, &session_id, "PRD", "Auth Flow PRD").unwrap();
        assert_eq!(doc.session_id, session_id);
        assert_eq!(doc.template_name, "PRD");
        assert_eq!(doc.title, "Auth Flow PRD");

        let fetched = get_document(&conn, &doc.id).unwrap();
        assert_eq!(fetched.id, doc.id);
    }

    #[test]
    fn list_documents_ordered_by_created_at() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let session_id = make_session(&conn);

        create_document(&conn, &session_id, "PRD", "First").unwrap();
        create_document(&conn, &session_id, "SDD", "Second").unwrap();

        let docs = list_documents(&conn, &session_id).unwrap();
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].title, "First");
        assert_eq!(docs[1].title, "Second");
    }

    #[test]
    fn delete_document_cascades_sections() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let session_id = make_session(&conn);
        let doc = make_doc(&conn, &session_id);

        create_section(&conn, &doc.id, "Overview", "Write overview", 0).unwrap();
        delete_document(&conn, &doc.id).unwrap();

        let sections = list_sections(&conn, &doc.id).unwrap();
        assert!(sections.is_empty());
    }

    #[test]
    fn create_and_list_sections() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let session_id = make_session(&conn);
        let doc = make_doc(&conn, &session_id);

        create_section(&conn, &doc.id, "Overview", "Write overview", 0).unwrap();
        create_section(&conn, &doc.id, "Requirements", "List requirements", 1).unwrap();

        let sections = list_sections(&conn, &doc.id).unwrap();
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].name, "Overview");
        assert_eq!(sections[0].sort_order, 0);
        assert_eq!(sections[0].content, "");
        assert_eq!(sections[1].name, "Requirements");
    }

    #[test]
    fn update_section_content_changes_content() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let session_id = make_session(&conn);
        let doc = make_doc(&conn, &session_id);
        let section = create_section(&conn, &doc.id, "Overview", "Write overview", 0).unwrap();

        let updated = update_section_content(&conn, &section.id, "This is the overview.").unwrap();
        assert_eq!(updated.content, "This is the overview.");
    }

    #[test]
    fn save_version_skips_empty_content() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let session_id = make_session(&conn);
        let doc = make_doc(&conn, &session_id);
        let section = create_section(&conn, &doc.id, "Overview", "Write overview", 0).unwrap();

        save_version(&conn, &section.id, "").unwrap();
        let versions = list_versions(&conn, &section.id).unwrap();
        assert!(versions.is_empty());
    }

    #[test]
    fn save_and_list_versions() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let session_id = make_session(&conn);
        let doc = make_doc(&conn, &session_id);
        let section = create_section(&conn, &doc.id, "Overview", "Write overview", 0).unwrap();

        save_version(&conn, &section.id, "Version 1 content").unwrap();
        save_version(&conn, &section.id, "Version 2 content").unwrap();

        let versions = list_versions(&conn, &section.id).unwrap();
        assert_eq!(versions.len(), 2);
        // Ordered newest first
        assert_eq!(versions[0].content, "Version 2 content");
        assert_eq!(versions[1].content, "Version 1 content");
    }

    #[test]
    fn get_version() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let session_id = make_session(&conn);
        let doc = make_doc(&conn, &session_id);
        let section = create_section(&conn, &doc.id, "Overview", "Write overview", 0).unwrap();

        save_version(&conn, &section.id, "Saved content").unwrap();
        let versions = list_versions(&conn, &section.id).unwrap();

        let fetched = get_version(&conn, &versions[0].id).unwrap();
        assert_eq!(fetched.content, "Saved content");
        assert_eq!(fetched.section_id, section.id);
    }

    #[test]
    fn cascade_delete_with_session() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let session_id = make_session(&conn);
        let doc = make_doc(&conn, &session_id);
        let section = create_section(&conn, &doc.id, "Overview", "Write", 0).unwrap();
        save_version(&conn, &section.id, "Some content").unwrap();

        crate::session::store::delete(&conn, &session_id).unwrap();

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }
}
```

- [ ] **Step 2: Create template store**

Create `src-tauri/src/docs/template_store.rs`:

```rust
use rusqlite::{params, Connection};

use crate::error::AppResult;

use super::model::Template;

fn row_to_template(row: &rusqlite::Row) -> rusqlite::Result<Template> {
    Ok(Template {
        id: row.get(0)?,
        name: row.get(1)?,
        content: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
    })
}

pub fn create_template(conn: &Connection, name: &str, content: &str) -> AppResult<Template> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO templates (id, name, content) VALUES (?1, ?2, ?3)",
        params![id, name, content],
    )?;
    get_template(conn, &id)
}

pub fn get_template(conn: &Connection, id: &str) -> AppResult<Template> {
    let template = conn.query_row(
        "SELECT id, name, content, created_at, updated_at FROM templates WHERE id = ?1",
        params![id],
        row_to_template,
    )?;
    Ok(template)
}

pub fn list_templates(conn: &Connection) -> AppResult<Vec<Template>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, content, created_at, updated_at FROM templates ORDER BY created_at ASC",
    )?;
    let templates = stmt
        .query_map([], row_to_template)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(templates)
}

pub fn update_template(
    conn: &Connection,
    id: &str,
    name: &str,
    content: &str,
) -> AppResult<Template> {
    conn.execute(
        "UPDATE templates SET name = ?1, content = ?2, updated_at = datetime('now') WHERE id = ?3",
        params![name, content, id],
    )?;
    get_template(conn, id)
}

pub fn delete_template(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM templates WHERE id = ?1", params![id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    #[test]
    fn create_and_get_template() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let tmpl = create_template(&conn, "My Template", "<!-- section: Overview -->\n<!-- directive: Write overview. -->").unwrap();
        assert_eq!(tmpl.name, "My Template");
        assert!(!tmpl.id.is_empty());

        let fetched = get_template(&conn, &tmpl.id).unwrap();
        assert_eq!(fetched.id, tmpl.id);
        assert_eq!(fetched.name, "My Template");
    }

    #[test]
    fn list_templates_ordered_by_created_at() {
        let db = test_db();
        let conn = db.conn().unwrap();

        create_template(&conn, "Alpha", "content").unwrap();
        create_template(&conn, "Beta", "content").unwrap();

        let templates = list_templates(&conn).unwrap();
        assert_eq!(templates.len(), 2);
        assert_eq!(templates[0].name, "Alpha");
        assert_eq!(templates[1].name, "Beta");
    }

    #[test]
    fn update_template_changes_name_and_content() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let tmpl = create_template(&conn, "Old Name", "old content").unwrap();
        let updated = update_template(&conn, &tmpl.id, "New Name", "new content").unwrap();
        assert_eq!(updated.name, "New Name");
        assert_eq!(updated.content, "new content");
    }

    #[test]
    fn delete_template() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let tmpl = create_template(&conn, "To Delete", "content").unwrap();
        delete_template(&conn, &tmpl.id).unwrap();

        let result = get_template(&conn, &tmpl.id);
        assert!(result.is_err());
    }

    #[test]
    fn list_empty_returns_empty_vec() {
        let db = test_db();
        let conn = db.conn().unwrap();

        let templates = list_templates(&conn).unwrap();
        assert!(templates.is_empty());
    }
}
```

- [ ] **Step 3: Run store tests**

```bash
cd src-tauri && cargo test docs::store docs::template_store
```

Expected: all tests pass.

---

### Task 4: Doc section generator

**Files:**
- Create: `src-tauri/src/docs/generator.rs`

- [ ] **Step 1: Create generator**

Create `src-tauri/src/docs/generator.rs`. This follows the same structure as `send_message` in `llm/streaming.rs` — same imports, same context assembly approach, same channel pattern — but uses `doc:*` event names, assembles context from the document's session, and saves version + content on completion.

```rust
use rusqlite::params;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::mpsc;

use crate::db::Database;
use crate::jira::client::{extract_jira_keys, JiraClient};
use crate::jira::model::JiraIssueContext;
use crate::llm::client::{self, StreamEvent};
use crate::llm::context::{self, ChatMessage};
use crate::llm::models;
use crate::repo_context::{model::RepoFileContext, store as repo_store, tree as repo_tree};
use crate::session::note_store;
use crate::settings::commands::get_api_key_from_keyring;
use crate::settings::store as settings_store;

use super::store;

#[derive(Clone, Serialize)]
struct DocChunkPayload {
    section_id: String,
    content: String,
}

#[derive(Clone, Serialize)]
struct DocDonePayload {
    section_id: String,
    full_content: String,
}

#[derive(Clone, Serialize)]
struct DocErrorPayload {
    section_id: String,
    message: String,
}

const DOC_SYSTEM_PROMPT: &str = "You are generating a section of a technical document. \
Write in clear, professional prose. Use markdown formatting where appropriate (headings, \
lists, code blocks). Be specific and grounded in the session context provided — do not \
pad with generic content. Write only the content for this section; do not include the \
section heading or any preamble.";

pub async fn generate_section(
    app: AppHandle,
    db: State<'_, Database>,
    document_id: String,
    section_id: String,
) -> Result<(), String> {
    let api_key = get_api_key_from_keyring().map_err(|e| e.to_string())?;

    // Load section + document + session context
    let (session_id, directive, existing_content) = {
        let conn = db.conn().map_err(|e| e.to_string())?;
        let section = store::get_section(&conn, &section_id).map_err(|e| e.to_string())?;
        let document = store::get_document(&conn, &section.document_id)
            .map_err(|e| e.to_string())?;
        (document.session_id, section.directive, section.content)
    };

    let (session_context, note_content, tickets, jira_keys) = {
        let conn = db.conn().map_err(|e| e.to_string())?;

        let session = crate::session::store::get(&conn, &session_id)
            .map_err(|e| e.to_string())?;

        let note_content = note_store::get_or_create(&conn, &session_id)
            .map(|n| n.content)
            .unwrap_or_default();

        let mut ticket_stmt = conn
            .prepare(
                "SELECT title, ticket_type, priority, description FROM tickets WHERE session_id = ?1",
            )
            .map_err(|e| e.to_string())?;
        let tickets: Vec<(String, String, String, String)> = ticket_stmt
            .query_map(params![session_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        let mut all_text = note_content.clone();
        all_text.push(' ');
        all_text.push_str(&directive);
        let jira_keys = extract_jira_keys(&all_text);

        (session.context, note_content, tickets, jira_keys)
    };

    let jira_issues: Vec<JiraIssueContext> = if !jira_keys.is_empty() {
        match crate::jira::commands::get_jira_credentials(&db) {
            Ok((base_url, auth)) => match JiraClient::new(&base_url, auth) {
                Ok(client) => {
                    let mut issues = Vec::new();
                    for key in &jira_keys {
                        match client.get_issue(key).await {
                            Ok(issue) => issues.push(issue),
                            Err(e) => tracing::warn!("Failed to fetch Jira issue {key}: {e}"),
                        }
                    }
                    issues
                }
                Err(_) => vec![],
            },
            Err(_) => vec![],
        }
    } else {
        vec![]
    };

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

        // No @mentions in directives — mentioned_files is always empty for doc generation
        let files: Vec<RepoFileContext> = Vec::new();

        (summaries, files)
    };

    let (messages, model) = {
        let conn = db.conn().map_err(|e| e.to_string())?;

        // Assemble session context exactly like send_message, but with:
        // - doc-specific system prompt (no conversation mode)
        // - directive as the sole user message (no conversation history)
        let mut system_parts = vec![DOC_SYSTEM_PROMPT.to_string()];

        if !session_context.is_empty() {
            system_parts.push(format!("## Session Context\n{session_context}"));
        }
        if !note_content.is_empty() {
            system_parts.push(format!("## Brain Dump Notes\n{note_content}"));
        }
        if !tickets.is_empty() {
            let mut ticket_text = String::from("## Current Tickets\n");
            for (title, ticket_type, priority, description) in &tickets {
                let desc_preview = if description.len() > 100 {
                    let truncated: String = description.chars().take(100).collect();
                    format!("{truncated}...")
                } else {
                    description.clone()
                };
                ticket_text.push_str(&format!(
                    "- {title} ({ticket_type}, {priority}) — {desc_preview}\n"
                ));
            }
            system_parts.push(ticket_text);
        }
        if !jira_issues.is_empty() {
            let mut jira_text = String::from("## Referenced Jira Tickets\n");
            for issue in &jira_issues {
                jira_text.push_str(&format!(
                    "- {} ({}, {}, {}): {}\n",
                    issue.key, issue.issue_type, issue.status, issue.priority, issue.summary
                ));
            }
            system_parts.push(jira_text);
        }
        if !repo_summaries.is_empty() {
            let mut repo_text = String::from("## Attached Repositories\n");
            for summary in &repo_summaries {
                repo_text.push_str(&format!("- {summary}\n"));
            }
            system_parts.push(repo_text);
        }

        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_parts.join("\n\n"),
            },
            ChatMessage {
                role: "user".to_string(),
                content: directive.clone(),
            },
        ];

        let model = settings_store::get(&conn, "llm.model")
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| models::DEFAULT_MODEL.to_string());

        (messages, model)
    };

    let (tx, mut rx) = mpsc::channel::<StreamEvent>(100);

    let app_clone = app.clone();
    let section_id_clone = section_id.clone();

    tokio::spawn(async move {
        client::stream_completion(&api_key, &model, messages, tx).await;
    });

    tokio::spawn(async move {
        let mut full_content = String::new();
        while let Some(event) = rx.recv().await {
            match event {
                StreamEvent::Chunk(text) => {
                    let _ = app_clone.emit(
                        "doc:chunk",
                        DocChunkPayload {
                            section_id: section_id_clone.clone(),
                            content: text,
                        },
                    );
                }
                StreamEvent::Done(text) => {
                    full_content = text;
                    let _ = app_clone.emit(
                        "doc:done",
                        DocDonePayload {
                            section_id: section_id_clone.clone(),
                            full_content: full_content.clone(),
                        },
                    );
                    break;
                }
                StreamEvent::Error(msg) => {
                    let _ = app_clone.emit(
                        "doc:error",
                        DocErrorPayload {
                            section_id: section_id_clone.clone(),
                            message: msg,
                        },
                    );
                    return;
                }
            }
        }

        if !full_content.is_empty() {
            let db: State<Database> = app_clone.state::<Database>();
            if let Ok(conn) = db.conn() {
                // Save previous content as a version before overwriting
                if let Err(e) = store::save_version(&conn, &section_id_clone, &existing_content) {
                    tracing::warn!("Failed to save section version: {e}");
                }
                if let Err(e) =
                    store::update_section_content(&conn, &section_id_clone, &full_content)
                {
                    tracing::error!("Failed to save generated section content: {e}");
                    let _ = app_clone.emit(
                        "doc:error",
                        DocErrorPayload {
                            section_id: section_id_clone,
                            message: "Content generated but failed to save — please try again"
                                .to_string(),
                        },
                    );
                }
            }
        }
    });

    Ok(())
}
```

- [ ] **Step 2: Verify compilation**

```bash
cd src-tauri && cargo check
```

Expected: no errors.

---

### Task 5: Tauri commands

**Files:**
- Create: `src-tauri/src/docs/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create commands**

Create `src-tauri/src/docs/commands.rs`:

```rust
use tauri::{AppHandle, State};

use crate::db::Database;

use super::model::{BuiltinTemplate, Document, DocumentSection, SectionVersion, Template};
use super::store;
use super::template_store;
use super::templates;

// ── built-in templates ────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_builtin_templates() -> Vec<BuiltinTemplate> {
    templates::get_builtin_templates()
}

// ── custom templates ──────────────────────────────────────────────────────────

#[tauri::command]
pub fn create_custom_template(
    db: State<Database>,
    name: String,
    content: String,
) -> Result<Template, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    template_store::create_template(&conn, &name, &content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_custom_templates(db: State<Database>) -> Result<Vec<Template>, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    template_store::list_templates(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_custom_template(db: State<Database>, template_id: String) -> Result<Template, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    template_store::get_template(&conn, &template_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_custom_template(
    db: State<Database>,
    template_id: String,
    name: String,
    content: String,
) -> Result<Template, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    template_store::update_template(&conn, &template_id, &name, &content)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_custom_template(db: State<Database>, template_id: String) -> Result<(), String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    template_store::delete_template(&conn, &template_id).map_err(|e| e.to_string())
}

// ── documents ─────────────────────────────────────────────────────────────────

/// Create a document and all its sections in one command.
/// `sections` is a list of `[name, directive, sort_order]` triples serialized
/// from the parsed template on the frontend.
#[tauri::command]
pub fn create_doc(
    db: State<Database>,
    session_id: String,
    template_name: String,
    title: String,
    sections: Vec<serde_json::Value>,
) -> Result<(Document, Vec<DocumentSection>), String> {
    let conn = db.conn().map_err(|e| e.to_string())?;

    let doc = store::create_document(&conn, &session_id, &template_name, &title)
        .map_err(|e| e.to_string())?;

    let mut created_sections = Vec::new();
    for (i, section_val) in sections.iter().enumerate() {
        let name = section_val["name"]
            .as_str()
            .ok_or("section missing name")?
            .to_string();
        let directive = section_val["directive"]
            .as_str()
            .ok_or("section missing directive")?
            .to_string();
        let sort_order = section_val["sort_order"]
            .as_i64()
            .unwrap_or(i as i64) as i32;

        let section = store::create_section(&conn, &doc.id, &name, &directive, sort_order)
            .map_err(|e| e.to_string())?;
        created_sections.push(section);
    }

    Ok((doc, created_sections))
}

#[tauri::command]
pub fn get_doc(db: State<Database>, document_id: String) -> Result<Document, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::get_document(&conn, &document_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_docs(db: State<Database>, session_id: String) -> Result<Vec<Document>, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::list_documents(&conn, &session_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_doc(db: State<Database>, document_id: String) -> Result<(), String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::delete_document(&conn, &document_id).map_err(|e| e.to_string())
}

// ── sections ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_document_sections(
    db: State<Database>,
    document_id: String,
) -> Result<Vec<DocumentSection>, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::list_sections(&conn, &document_id).map_err(|e| e.to_string())
}

/// Update section content manually (from the edit textarea).
/// Saves the current content as a version before overwriting.
#[tauri::command]
pub fn update_document_section(
    db: State<Database>,
    section_id: String,
    content: String,
) -> Result<DocumentSection, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;

    let existing = store::get_section(&conn, &section_id).map_err(|e| e.to_string())?;
    store::save_version(&conn, &section_id, &existing.content).map_err(|e| e.to_string())?;
    store::update_section_content(&conn, &section_id, &content).map_err(|e| e.to_string())
}

// ── generation ────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn generate_doc_section(
    app: AppHandle,
    db: State<'_, Database>,
    document_id: String,
    section_id: String,
) -> Result<(), String> {
    super::generator::generate_section(app, db, document_id, section_id).await
}

// ── versions ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn list_section_versions(
    db: State<Database>,
    section_id: String,
) -> Result<Vec<SectionVersion>, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;
    store::list_versions(&conn, &section_id).map_err(|e| e.to_string())
}

/// Restore a section to a previous version.
/// Saves the current content as a new version first, then sets content to the restored version.
#[tauri::command]
pub fn restore_section_version(
    db: State<Database>,
    section_id: String,
    version_id: String,
) -> Result<DocumentSection, String> {
    let conn = db.conn().map_err(|e| e.to_string())?;

    let current = store::get_section(&conn, &section_id).map_err(|e| e.to_string())?;
    store::save_version(&conn, &section_id, &current.content).map_err(|e| e.to_string())?;

    let version = store::get_version(&conn, &version_id).map_err(|e| e.to_string())?;
    store::update_section_content(&conn, &section_id, &version.content)
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Register in lib.rs**

In `src-tauri/src/lib.rs`:

Add to the imports:
```rust
use docs::commands::*;
```

Add to the `invoke_handler` list:
```rust
list_builtin_templates,
create_custom_template,
list_custom_templates,
get_custom_template,
update_custom_template,
delete_custom_template,
create_doc,
get_doc,
list_docs,
delete_doc,
list_document_sections,
update_document_section,
generate_doc_section,
list_section_versions,
restore_section_version,
```

- [ ] **Step 3: Full build + test**

```bash
cd src-tauri && cargo test
```

Expected: all existing tests pass, new docs tests pass. No compilation errors.

---

### Task 6: Frontend types + atoms + hook

**Files:**
- Create: `src/features/docs/docs.types.ts`
- Create: `src/features/docs/docs.atoms.ts`
- Create: `src/features/docs/useDocActions.ts`

- [ ] **Step 1: Types**

Create `src/features/docs/docs.types.ts`:

```typescript
export interface Document {
  id: string;
  session_id: string;
  template_name: string;
  title: string;
  created_at: string;
  updated_at: string;
}

export interface DocumentSection {
  id: string;
  document_id: string;
  name: string;
  directive: string;
  content: string;
  sort_order: number;
  created_at: string;
  updated_at: string;
}

export interface SectionVersion {
  id: string;
  section_id: string;
  content: string;
  created_at: string;
}

export interface Template {
  id: string;
  name: string;
  content: string;
  created_at: string;
  updated_at: string;
}

export interface BuiltinTemplate {
  name: string;
  content: string;
}

export interface TemplateSection {
  name: string;
  directive: string;
}

export type SectionGenerationState =
  | { status: "idle" }
  | { status: "generating"; accumulated: string }
  | { status: "error"; message: string };
```

- [ ] **Step 2: Atoms**

Create `src/features/docs/docs.atoms.ts`:

```typescript
import { atom } from "jotai";
import type { Document, DocumentSection, SectionGenerationState } from "./docs.types";

// All documents for the active session
export const documentsAtom = atom<Document[]>([]);

// Sections keyed by document_id
export const sectionsByDocAtom = atom<Record<string, DocumentSection[]>>({});

// Per-section generation state: sectionId → state
export const sectionGenerationAtom = atom<Record<string, SectionGenerationState>>({});

// Whether the template picker dropdown is open
export const templatePickerOpenAtom = atom(false);

// Whether the template manager modal is open
export const templateManagerOpenAtom = atom(false);
```

- [ ] **Step 3: Hook**

Create `src/features/docs/useDocActions.ts`:

```typescript
import { useSetAtom, useAtom } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { documentsAtom, sectionsByDocAtom, sectionGenerationAtom } from "./docs.atoms";
import type {
  Document,
  DocumentSection,
  BuiltinTemplate,
  Template,
  SectionVersion,
} from "./docs.types";

export function useDocActions(sessionId: string | undefined) {
  const setDocuments = useSetAtom(documentsAtom);
  const setSectionsByDoc = useSetAtom(sectionsByDocAtom);
  const [sectionGeneration, setSectionGeneration] = useAtom(sectionGenerationAtom);

  // Subscribe to doc streaming events
  useEffect(() => {
    const unlisteners: (() => void)[] = [];

    listen<{ section_id: string; content: string }>("doc:chunk", (event) => {
      const { section_id, content } = event.payload;
      setSectionGeneration((prev) => {
        const current = prev[section_id];
        const accumulated =
          current?.status === "generating" ? current.accumulated + content : content;
        return { ...prev, [section_id]: { status: "generating", accumulated } };
      });
    }).then((u) => unlisteners.push(u));

    listen<{ section_id: string; full_content: string }>("doc:done", (event) => {
      const { section_id, full_content } = event.payload;
      // Mark generation done and reload the section from the store
      setSectionGeneration((prev) => {
        const next = { ...prev };
        delete next[section_id];
        return next;
      });
      // Reload all sections for the document containing this section
      setSectionsByDoc((prev) => {
        const next = { ...prev };
        for (const [docId, sections] of Object.entries(next)) {
          if (sections.some((s) => s.id === section_id)) {
            invoke<DocumentSection[]>("list_document_sections", { documentId: docId }).then(
              (updated) => {
                setSectionsByDoc((p) => ({ ...p, [docId]: updated }));
              }
            );
            break;
          }
        }
        return next;
      });
    }).then((u) => unlisteners.push(u));

    listen<{ section_id: string; message: string }>("doc:error", (event) => {
      const { section_id, message } = event.payload;
      setSectionGeneration((prev) => ({
        ...prev,
        [section_id]: { status: "error", message },
      }));
    }).then((u) => unlisteners.push(u));

    return () => unlisteners.forEach((fn) => fn());
  }, []);

  async function loadDocuments() {
    if (!sessionId) return;
    const docs = await invoke<Document[]>("list_docs", { sessionId });
    setDocuments(docs);
  }

  async function loadSections(documentId: string) {
    const sections = await invoke<DocumentSection[]>("list_document_sections", { documentId });
    setSectionsByDoc((prev) => ({ ...prev, [documentId]: sections }));
  }

  async function createDocument(
    templateName: string,
    title: string,
    sections: Array<{ name: string; directive: string; sort_order: number }>
  ) {
    if (!sessionId) return;
    const [doc, secs] = await invoke<[Document, DocumentSection[]]>("create_doc", {
      sessionId,
      templateName,
      title,
      sections,
    });
    await loadDocuments();
    setSectionsByDoc((prev) => ({ ...prev, [doc.id]: secs }));
    return { doc, sections: secs };
  }

  async function deleteDocument(documentId: string) {
    await invoke("delete_doc", { documentId });
    setSectionsByDoc((prev) => {
      const next = { ...prev };
      delete next[documentId];
      return next;
    });
    await loadDocuments();
  }

  async function generateSection(documentId: string, sectionId: string) {
    setSectionGeneration((prev) => ({
      ...prev,
      [sectionId]: { status: "generating", accumulated: "" },
    }));
    try {
      await invoke("generate_doc_section", { documentId, sectionId });
    } catch (err) {
      setSectionGeneration((prev) => ({
        ...prev,
        [sectionId]: { status: "error", message: String(err) },
      }));
    }
  }

  async function updateSection(sectionId: string, content: string, documentId: string) {
    await invoke<DocumentSection>("update_document_section", { sectionId, content });
    await loadSections(documentId);
  }

  async function listVersions(sectionId: string) {
    return invoke<SectionVersion[]>("list_section_versions", { sectionId });
  }

  async function restoreVersion(sectionId: string, versionId: string, documentId: string) {
    await invoke<DocumentSection>("restore_section_version", { sectionId, versionId });
    await loadSections(documentId);
  }

  async function listBuiltinTemplates() {
    return invoke<BuiltinTemplate[]>("list_builtin_templates");
  }

  async function listCustomTemplates() {
    return invoke<Template[]>("list_custom_templates");
  }

  async function createCustomTemplate(name: string, content: string) {
    return invoke<Template>("create_custom_template", { name, content });
  }

  async function updateCustomTemplate(templateId: string, name: string, content: string) {
    return invoke<Template>("update_custom_template", { templateId, name, content });
  }

  async function deleteCustomTemplate(templateId: string) {
    await invoke("delete_custom_template", { templateId });
  }

  return {
    loadDocuments,
    loadSections,
    createDocument,
    deleteDocument,
    generateSection,
    updateSection,
    listVersions,
    restoreVersion,
    listBuiltinTemplates,
    listCustomTemplates,
    createCustomTemplate,
    updateCustomTemplate,
    deleteCustomTemplate,
    sectionGeneration,
  };
}
```

---

### Task 7: Docs tab + DocsView + DocumentCard

**Files:**
- Modify: `src/App.tsx`
- Create: `src/features/docs/DocsView.tsx`
- Create: `src/features/docs/DocumentCard.tsx`

- [ ] **Step 1: Rename Refine → Docs in App.tsx**

In `src/App.tsx`:

Change the `Tab` type union and the `TABS` array. Replace:
```typescript
type Tab = "dump" | "refine" | "board";

const TABS: { id: Tab; label: string }[] = [
  { id: "dump", label: "Dump" },
  { id: "refine", label: "Refine" },
  { id: "board", label: "Board" },
];
```
With:
```typescript
type Tab = "dump" | "docs" | "board";

const TABS: { id: Tab; label: string }[] = [
  { id: "dump", label: "Dump" },
  { id: "docs", label: "Docs" },
  { id: "board", label: "Board" },
];
```

Change the tab content render from:
```typescript
{activeTab === "refine" && (
  <p className="text-sm text-muted-foreground">
    Split pane refine view will go here
  </p>
)}
```
To:
```typescript
{activeTab === "docs" && (
  <DocsView sessionId={activeSession.id} />
)}
```

Add the import at the top:
```typescript
import { DocsView } from "@/features/docs/DocsView";
```

- [ ] **Step 2: Create DocsView**

Create `src/features/docs/DocsView.tsx`:

```typescript
import { useEffect, useState } from "react";
import { useAtomValue } from "jotai";
import { FileText, Plus, Settings } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { documentsAtom } from "./docs.atoms";
import { DocumentCard } from "./DocumentCard";
import { TemplateManager } from "./TemplateManager";
import { useDocActions } from "./useDocActions";
import type { BuiltinTemplate, Template, TemplateSection } from "./docs.types";

// Helper: parse section markers from template content
function parseTemplateSections(content: string): TemplateSection[] {
  const sections: TemplateSection[] = [];
  const lines = content.split("\n");
  let currentSection: string | null = null;

  for (const line of lines) {
    const trimmed = line.trim();
    const sectionMatch = trimmed.match(/^<!--\s*section:\s*(.+?)\s*-->$/);
    if (sectionMatch) {
      currentSection = sectionMatch[1];
      continue;
    }
    const directiveMatch = trimmed.match(/^<!--\s*directive:\s*([\s\S]+?)\s*-->$/);
    if (directiveMatch && currentSection) {
      sections.push({ name: currentSection, directive: directiveMatch[1] });
      currentSection = null;
    }
  }
  return sections;
}

interface DocsViewProps {
  sessionId: string;
}

export function DocsView({ sessionId }: DocsViewProps) {
  const documents = useAtomValue(documentsAtom);
  const [templateManagerOpen, setTemplateManagerOpen] = useState(false);
  const [builtinTemplates, setBuiltinTemplates] = useState<BuiltinTemplate[]>([]);
  const [customTemplates, setCustomTemplates] = useState<Template[]>([]);
  const [isCreating, setIsCreating] = useState(false);

  const actions = useDocActions(sessionId);

  useEffect(() => {
    actions.loadDocuments();
    actions.listBuiltinTemplates().then(setBuiltinTemplates);
    actions.listCustomTemplates().then(setCustomTemplates);
  }, [sessionId]);

  async function handleSelectTemplate(templateContent: string, templateName: string) {
    setIsCreating(true);
    try {
      const sections = parseTemplateSections(templateContent);
      const title = `${templateName} — ${new Date().toLocaleDateString("en-US", { month: "short", day: "numeric" })}`;
      const result = await actions.createDocument(
        templateName,
        title,
        sections.map((s, i) => ({ ...s, sort_order: i }))
      );
      if (!result) return;

      // Kick off sequential generation: one section at a time, top to bottom
      for (const section of result.sections) {
        await actions.generateSection(result.doc.id, section.id);
        // Brief pause between sections to avoid overwhelming the UI
        await new Promise((r) => setTimeout(r, 200));
      }
    } finally {
      setIsCreating(false);
    }
  }

  function refreshCustomTemplates() {
    actions.listCustomTemplates().then(setCustomTemplates);
  }

  if (documents.length === 0 && !isCreating) {
    return (
      <div className="flex h-full flex-col">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-sm font-medium text-muted-foreground">Documents</h2>
          <GenerateButton
            builtinTemplates={builtinTemplates}
            customTemplates={customTemplates}
            onSelect={handleSelectTemplate}
            onManageTemplates={() => setTemplateManagerOpen(true)}
            disabled={isCreating}
          />
        </div>
        <div className="flex flex-1 flex-col items-center justify-center gap-3 text-center">
          <FileText className="size-8 text-muted-foreground/40" />
          <p className="text-sm text-muted-foreground">No documents yet</p>
          <p className="text-xs text-muted-foreground/60">
            Generate a PRD, SDD, Test Plan, or ADR from your session context
          </p>
        </div>
        {templateManagerOpen && (
          <TemplateManager
            onClose={() => setTemplateManagerOpen(false)}
            onTemplatesChanged={refreshCustomTemplates}
            builtinTemplates={builtinTemplates}
          />
        )}
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col gap-4 overflow-y-auto">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-medium text-muted-foreground">
          Documents ({documents.length})
        </h2>
        <GenerateButton
          builtinTemplates={builtinTemplates}
          customTemplates={customTemplates}
          onSelect={handleSelectTemplate}
          onManageTemplates={() => setTemplateManagerOpen(true)}
          disabled={isCreating}
        />
      </div>

      {documents.map((doc) => (
        <DocumentCard
          key={doc.id}
          document={doc}
          sessionId={sessionId}
          onDeleted={actions.loadDocuments}
        />
      ))}

      {templateManagerOpen && (
        <TemplateManager
          onClose={() => setTemplateManagerOpen(false)}
          onTemplatesChanged={refreshCustomTemplates}
          builtinTemplates={builtinTemplates}
        />
      )}
    </div>
  );
}

interface GenerateButtonProps {
  builtinTemplates: BuiltinTemplate[];
  customTemplates: Template[];
  onSelect: (content: string, name: string) => void;
  onManageTemplates: () => void;
  disabled: boolean;
}

function GenerateButton({
  builtinTemplates,
  customTemplates,
  onSelect,
  onManageTemplates,
  disabled,
}: GenerateButtonProps) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button size="sm" disabled={disabled}>
          <Plus className="mr-1 size-3.5" />
          Generate Document
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-52">
        {builtinTemplates.map((t) => (
          <DropdownMenuItem key={t.name} onSelect={() => onSelect(t.content, t.name)}>
            {t.name}
          </DropdownMenuItem>
        ))}
        {customTemplates.length > 0 && (
          <>
            <DropdownMenuSeparator />
            {customTemplates.map((t) => (
              <DropdownMenuItem key={t.id} onSelect={() => onSelect(t.content, t.name)}>
                {t.name}
              </DropdownMenuItem>
            ))}
          </>
        )}
        <DropdownMenuSeparator />
        <DropdownMenuItem onSelect={onManageTemplates} className="text-muted-foreground">
          <Settings className="mr-2 size-3.5" />
          Manage Templates
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
```

- [ ] **Step 3: Create DocumentCard**

Create `src/features/docs/DocumentCard.tsx`:

```typescript
import { useEffect, useState } from "react";
import { useAtomValue } from "jotai";
import {
  ChevronDown,
  ChevronRight,
  Copy,
  Download,
  History,
  Pencil,
  RefreshCw,
  Trash2,
} from "lucide-react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import { Button } from "@/components/ui/button";
import { sectionsByDocAtom, sectionGenerationAtom } from "./docs.atoms";
import { VersionHistory } from "./VersionHistory";
import { useDocActions } from "./useDocActions";
import type { Document, DocumentSection } from "./docs.types";

interface DocumentCardProps {
  document: Document;
  sessionId: string;
  onDeleted: () => void;
}

export function DocumentCard({ document, sessionId, onDeleted }: DocumentCardProps) {
  const [expanded, setExpanded] = useState(true);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const sectionsByDoc = useAtomValue(sectionsByDocAtom);
  const sectionGeneration = useAtomValue(sectionGenerationAtom);
  const sections = sectionsByDoc[document.id] ?? [];

  const actions = useDocActions(sessionId);

  useEffect(() => {
    actions.loadSections(document.id);
  }, [document.id]);

  async function handleDelete() {
    if (!confirmDelete) {
      setConfirmDelete(true);
      return;
    }
    await actions.deleteDocument(document.id);
    onDeleted();
  }

  function buildMarkdown(): string {
    const parts = [`# ${document.title}\n`];
    for (const section of sections) {
      parts.push(`## ${section.name}\n\n${section.content}`);
    }
    return parts.join("\n\n");
  }

  async function handleExport() {
    const content = buildMarkdown();
    const filePath = await save({
      defaultPath: `${document.title.replace(/[^a-z0-9]/gi, "-").toLowerCase()}.md`,
      filters: [{ name: "Markdown", extensions: ["md"] }],
    });
    if (filePath) {
      await writeTextFile(filePath, content);
    }
  }

  async function handleCopyAll() {
    await navigator.clipboard.writeText(buildMarkdown());
  }

  return (
    <div className="rounded-lg border border-border bg-card">
      {/* Card header */}
      <div className="flex items-center gap-2 px-4 py-3">
        <button
          onClick={() => setExpanded(!expanded)}
          className="flex min-w-0 flex-1 items-center gap-2 text-left"
        >
          {expanded ? (
            <ChevronDown className="size-4 shrink-0 text-muted-foreground" />
          ) : (
            <ChevronRight className="size-4 shrink-0 text-muted-foreground" />
          )}
          <div className="min-w-0">
            <p className="truncate text-sm font-medium">{document.title}</p>
            <p className="text-xs text-muted-foreground">{document.template_name}</p>
          </div>
        </button>

        {confirmDelete ? (
          <div className="flex gap-1">
            <Button size="xs" variant="destructive" onClick={handleDelete}>
              Confirm
            </Button>
            <Button size="xs" variant="ghost" onClick={() => setConfirmDelete(false)}>
              Cancel
            </Button>
          </div>
        ) : (
          <Button
            size="xs"
            variant="ghost"
            onClick={handleDelete}
            className="text-muted-foreground hover:text-destructive"
          >
            <Trash2 className="size-3.5" />
          </Button>
        )}
      </div>

      {/* Sections */}
      {expanded && (
        <div className="border-t border-border">
          {sections.map((section) => (
            <SectionRow
              key={section.id}
              section={section}
              document={document}
              generation={sectionGeneration[section.id]}
              onRegenerate={() => actions.generateSection(document.id, section.id)}
              onUpdate={(content) => actions.updateSection(section.id, content, document.id)}
              onRestoreVersion={(versionId) =>
                actions.restoreVersion(section.id, versionId, document.id)
              }
              listVersions={() => actions.listVersions(section.id)}
            />
          ))}

          {/* Footer actions */}
          <div className="flex gap-2 border-t border-border px-4 py-2">
            <Button
              size="xs"
              variant="ghost"
              onClick={handleExport}
              className="text-muted-foreground"
            >
              <Download className="mr-1 size-3" />
              Export .md
            </Button>
            <Button
              size="xs"
              variant="ghost"
              onClick={handleCopyAll}
              className="text-muted-foreground"
            >
              <Copy className="mr-1 size-3" />
              Copy All
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}

interface SectionRowProps {
  section: DocumentSection;
  document: Document;
  generation: { status: "generating"; accumulated: string } | { status: "error"; message: string } | undefined;
  onRegenerate: () => void;
  onUpdate: (content: string) => void;
  onRestoreVersion: (versionId: string) => void;
  listVersions: () => Promise<import("./docs.types").SectionVersion[]>;
}

function SectionRow({
  section,
  generation,
  onRegenerate,
  onUpdate,
  onRestoreVersion,
  listVersions,
}: SectionRowProps) {
  const [sectionExpanded, setSectionExpanded] = useState(true);
  const [editing, setEditing] = useState(false);
  const [editValue, setEditValue] = useState(section.content);
  const [historyOpen, setHistoryOpen] = useState(false);

  const isGenerating = generation?.status === "generating";
  const hasError = generation?.status === "error";
  const displayContent = isGenerating ? generation.accumulated : section.content;

  function handleEditSave() {
    onUpdate(editValue);
    setEditing(false);
  }

  function handleEditCancel() {
    setEditValue(section.content);
    setEditing(false);
  }

  return (
    <div className="border-b border-border/50 last:border-0">
      {/* Section header */}
      <div className="flex items-center gap-2 px-4 py-2">
        <button
          onClick={() => setSectionExpanded(!sectionExpanded)}
          className="flex min-w-0 flex-1 items-center gap-1.5 text-left"
        >
          {sectionExpanded ? (
            <ChevronDown className="size-3.5 shrink-0 text-muted-foreground" />
          ) : (
            <ChevronRight className="size-3.5 shrink-0 text-muted-foreground" />
          )}
          <span className="truncate text-xs font-medium">{section.name}</span>
          {isGenerating && (
            <span className="text-xs text-muted-foreground animate-pulse">Generating...</span>
          )}
        </button>

        <div className="flex gap-0.5">
          <Button
            size="xs"
            variant="ghost"
            onClick={() => setHistoryOpen(true)}
            className="size-6 p-0 text-muted-foreground"
            title="Version history"
          >
            <History className="size-3" />
          </Button>
          <Button
            size="xs"
            variant="ghost"
            onClick={onRegenerate}
            disabled={isGenerating}
            className="size-6 p-0 text-muted-foreground"
            title="Regenerate"
          >
            <RefreshCw className={`size-3 ${isGenerating ? "animate-spin" : ""}`} />
          </Button>
          <Button
            size="xs"
            variant="ghost"
            onClick={() => {
              setEditValue(section.content);
              setEditing(true);
              setSectionExpanded(true);
            }}
            disabled={isGenerating || editing}
            className="size-6 p-0 text-muted-foreground"
            title="Edit"
          >
            <Pencil className="size-3" />
          </Button>
        </div>
      </div>

      {/* Section content */}
      {sectionExpanded && (
        <div className="px-4 pb-3">
          {hasError && (
            <div className="mb-2 flex items-center gap-2 rounded border border-destructive/30 bg-destructive/10 px-3 py-1.5 text-xs text-destructive-foreground">
              <span className="flex-1">{generation.message}</span>
              <Button size="xs" variant="ghost" onClick={onRegenerate} className="h-5 px-2">
                Retry
              </Button>
            </div>
          )}

          {editing ? (
            <div className="space-y-2">
              <textarea
                autoFocus
                value={editValue}
                onChange={(e) => setEditValue(e.target.value)}
                rows={8}
                className="w-full rounded border border-input bg-background px-2 py-1.5 font-mono text-xs focus:outline-none focus:ring-1 focus:ring-ring resize-y"
              />
              <div className="flex justify-end gap-1.5">
                <Button size="xs" variant="ghost" onClick={handleEditCancel}>
                  Cancel
                </Button>
                <Button size="xs" onClick={handleEditSave}>
                  Save
                </Button>
              </div>
            </div>
          ) : displayContent ? (
            <div className="prose prose-invert prose-sm max-w-none">
              <Markdown remarkPlugins={[remarkGfm]}>{displayContent}</Markdown>
            </div>
          ) : !isGenerating ? (
            <p className="text-xs text-muted-foreground/60 italic">
              Empty — click regenerate to generate this section
            </p>
          ) : null}
        </div>
      )}

      {historyOpen && (
        <VersionHistory
          sectionName={section.name}
          listVersions={listVersions}
          onRestore={(versionId) => {
            onRestoreVersion(versionId);
            setHistoryOpen(false);
          }}
          onClose={() => setHistoryOpen(false)}
        />
      )}
    </div>
  );
}
```

- [ ] **Step 4: Verify the app builds**

```bash
cd /Users/nathananderson-tennant/Development/rubber-duck && bun run build
```

Expected: no TypeScript errors, no Vite errors. You may see missing `@tauri-apps/plugin-dialog` and `@tauri-apps/plugin-fs` import errors — these will be addressed in the export/import steps. If those plugins are not yet installed, stub the export with:

```typescript
// Temporary stub if plugins not installed:
const save = async (_opts: unknown) => null;
const writeTextFile = async (_path: string, _content: string) => {};
```

Then install in Task 7 Step 5.

- [ ] **Step 5: Install Tauri dialog + fs plugins if not already present**

Check `src-tauri/Cargo.toml` and `package.json` — the repo context plan already added `tauri-plugin-dialog`. If `tauri-plugin-fs` is not present:

```bash
cd /Users/nathananderson-tennant/Development/rubber-duck
bun add @tauri-apps/plugin-fs
cd src-tauri && cargo add tauri-plugin-fs
```

In `src-tauri/src/lib.rs`, add `.plugin(tauri_plugin_fs::init())` to `tauri::Builder::default()` alongside the other plugins.

In `src-tauri/capabilities/default.json`, add the required fs permissions for `writeTextFile`:
```json
"fs:allow-write-text-file",
"fs:allow-write-text-file-script"
```

---

### Task 8: Template management UI

**Files:**
- Create: `src/features/docs/TemplateManager.tsx`

- [ ] **Step 1: Create TemplateManager**

Create `src/features/docs/TemplateManager.tsx`:

```typescript
import { useEffect, useState } from "react";
import { Copy, Pencil, Plus, Trash2, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useDocActions } from "./useDocActions";
import type { BuiltinTemplate, Template } from "./docs.types";

interface TemplateManagerProps {
  onClose: () => void;
  onTemplatesChanged: () => void;
  builtinTemplates: BuiltinTemplate[];
}

type EditorMode =
  | { mode: "new" }
  | { mode: "edit"; template: Template }
  | null;

export function TemplateManager({
  onClose,
  onTemplatesChanged,
  builtinTemplates,
}: TemplateManagerProps) {
  const [customTemplates, setCustomTemplates] = useState<Template[]>([]);
  const [editorState, setEditorState] = useState<EditorMode>(null);
  const [editorName, setEditorName] = useState("");
  const [editorContent, setEditorContent] = useState("");
  const [saving, setSaving] = useState(false);

  const actions = useDocActions(undefined);

  useEffect(() => {
    actions.listCustomTemplates().then(setCustomTemplates);
  }, []);

  async function handleSave() {
    if (!editorName.trim() || !editorContent.trim()) return;
    setSaving(true);
    try {
      if (editorState?.mode === "edit") {
        await actions.updateCustomTemplate(
          editorState.template.id,
          editorName.trim(),
          editorContent.trim()
        );
      } else {
        await actions.createCustomTemplate(editorName.trim(), editorContent.trim());
      }
      const updated = await actions.listCustomTemplates();
      setCustomTemplates(updated);
      onTemplatesChanged();
      setEditorState(null);
    } finally {
      setSaving(false);
    }
  }

  async function handleDelete(id: string) {
    await actions.deleteCustomTemplate(id);
    const updated = await actions.listCustomTemplates();
    setCustomTemplates(updated);
    onTemplatesChanged();
  }

  function handleCloneBuiltin(builtin: BuiltinTemplate) {
    setEditorName(`${builtin.name} (custom)`);
    setEditorContent(builtin.content);
    setEditorState({ mode: "new" });
  }

  function handleEditCustom(template: Template) {
    setEditorName(template.name);
    setEditorContent(template.content);
    setEditorState({ mode: "edit", template });
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="relative flex h-[80vh] w-[700px] flex-col rounded-lg border border-border bg-card shadow-lg">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-border px-4 py-3">
          <h2 className="text-sm font-medium">
            {editorState ? (editorState.mode === "edit" ? "Edit Template" : "New Template") : "Manage Templates"}
          </h2>
          <Button size="xs" variant="ghost" onClick={onClose}>
            <X className="size-4" />
          </Button>
        </div>

        {editorState ? (
          /* Template editor */
          <div className="flex flex-1 flex-col gap-3 overflow-hidden p-4">
            <input
              type="text"
              value={editorName}
              onChange={(e) => setEditorName(e.target.value)}
              placeholder="Template name"
              className="rounded border border-input bg-background px-3 py-1.5 text-sm focus:outline-none focus:ring-1 focus:ring-ring"
            />
            <div className="flex-1 overflow-hidden">
              <textarea
                value={editorContent}
                onChange={(e) => setEditorContent(e.target.value)}
                className="h-full w-full resize-none rounded border border-input bg-background p-3 font-mono text-xs focus:outline-none focus:ring-1 focus:ring-ring"
                placeholder={`# Template Title\n<!-- section: Section Name -->\n<!-- directive: Instructions for the LLM to generate this section. -->`}
              />
            </div>
            <div className="flex justify-between">
              <p className="text-xs text-muted-foreground/60">
                Use <code>{"<!-- section: Name -->"}</code> and <code>{"<!-- directive: ... -->"}</code> markers
              </p>
              <div className="flex gap-2">
                <Button size="sm" variant="ghost" onClick={() => setEditorState(null)}>
                  Cancel
                </Button>
                <Button
                  size="sm"
                  onClick={handleSave}
                  disabled={saving || !editorName.trim() || !editorContent.trim()}
                >
                  {saving ? "Saving..." : "Save Template"}
                </Button>
              </div>
            </div>
          </div>
        ) : (
          /* Template list */
          <div className="flex flex-1 flex-col overflow-y-auto p-4 gap-4">
            {/* Built-in templates */}
            <div>
              <p className="mb-2 text-xs font-medium text-muted-foreground uppercase tracking-wider">
                Built-in Templates
              </p>
              <div className="space-y-1">
                {builtinTemplates.map((t) => (
                  <div
                    key={t.name}
                    className="flex items-center justify-between rounded px-3 py-2 hover:bg-accent/30"
                  >
                    <span className="text-sm">{t.name}</span>
                    <Button
                      size="xs"
                      variant="ghost"
                      onClick={() => handleCloneBuiltin(t)}
                      className="text-muted-foreground"
                    >
                      <Copy className="mr-1 size-3" />
                      Clone
                    </Button>
                  </div>
                ))}
              </div>
            </div>

            {/* Custom templates */}
            <div>
              <div className="mb-2 flex items-center justify-between">
                <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                  Custom Templates
                </p>
                <Button
                  size="xs"
                  variant="ghost"
                  onClick={() => {
                    setEditorName("");
                    setEditorContent("");
                    setEditorState({ mode: "new" });
                  }}
                >
                  <Plus className="mr-1 size-3" />
                  New
                </Button>
              </div>
              {customTemplates.length === 0 ? (
                <p className="text-xs text-muted-foreground/60 px-3">
                  No custom templates yet. Clone a built-in or create from scratch.
                </p>
              ) : (
                <div className="space-y-1">
                  {customTemplates.map((t) => (
                    <div
                      key={t.id}
                      className="flex items-center justify-between rounded px-3 py-2 hover:bg-accent/30"
                    >
                      <span className="text-sm">{t.name}</span>
                      <div className="flex gap-1">
                        <Button
                          size="xs"
                          variant="ghost"
                          onClick={() => handleEditCustom(t)}
                          className="text-muted-foreground"
                        >
                          <Pencil className="size-3" />
                        </Button>
                        <Button
                          size="xs"
                          variant="ghost"
                          onClick={() => handleDelete(t.id)}
                          className="text-muted-foreground hover:text-destructive"
                        >
                          <Trash2 className="size-3" />
                        </Button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
```

---

### Task 9: Version history UI

**Files:**
- Create: `src/features/docs/VersionHistory.tsx`

- [ ] **Step 1: Create VersionHistory**

Create `src/features/docs/VersionHistory.tsx`:

```typescript
import { useEffect, useState } from "react";
import { X } from "lucide-react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Button } from "@/components/ui/button";
import type { SectionVersion } from "./docs.types";

interface VersionHistoryProps {
  sectionName: string;
  listVersions: () => Promise<SectionVersion[]>;
  onRestore: (versionId: string) => void;
  onClose: () => void;
}

export function VersionHistory({
  sectionName,
  listVersions,
  onRestore,
  onClose,
}: VersionHistoryProps) {
  const [versions, setVersions] = useState<SectionVersion[]>([]);
  const [loading, setLoading] = useState(true);
  const [previewVersion, setPreviewVersion] = useState<SectionVersion | null>(null);

  useEffect(() => {
    listVersions()
      .then((v) => {
        setVersions(v);
        if (v.length > 0) setPreviewVersion(v[0]);
      })
      .finally(() => setLoading(false));
  }, []);

  function formatDate(isoString: string): string {
    const date = new Date(isoString);
    return date.toLocaleString("en-US", {
      month: "short",
      day: "numeric",
      hour: "numeric",
      minute: "2-digit",
      hour12: true,
    });
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="relative flex h-[70vh] w-[680px] flex-col rounded-lg border border-border bg-card shadow-lg">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-border px-4 py-3">
          <h2 className="text-sm font-medium">
            Version History — {sectionName}
          </h2>
          <Button size="xs" variant="ghost" onClick={onClose}>
            <X className="size-4" />
          </Button>
        </div>

        {loading ? (
          <div className="flex flex-1 items-center justify-center">
            <p className="text-sm text-muted-foreground animate-pulse">Loading...</p>
          </div>
        ) : versions.length === 0 ? (
          <div className="flex flex-1 items-center justify-center">
            <p className="text-sm text-muted-foreground">No saved versions yet</p>
          </div>
        ) : (
          <div className="flex min-h-0 flex-1">
            {/* Version list */}
            <div className="w-56 shrink-0 overflow-y-auto border-r border-border">
              {versions.map((v) => (
                <button
                  key={v.id}
                  onClick={() => setPreviewVersion(v)}
                  className={`w-full px-4 py-3 text-left text-xs border-b border-border/50 hover:bg-accent/30 transition-colors ${
                    previewVersion?.id === v.id ? "bg-accent/50" : ""
                  }`}
                >
                  {formatDate(v.created_at)}
                </button>
              ))}
            </div>

            {/* Preview */}
            <div className="flex flex-1 flex-col overflow-hidden">
              <div className="flex-1 overflow-y-auto p-4">
                {previewVersion && (
                  <div className="prose prose-invert prose-sm max-w-none">
                    <Markdown remarkPlugins={[remarkGfm]}>
                      {previewVersion.content}
                    </Markdown>
                  </div>
                )}
              </div>
              {previewVersion && (
                <div className="border-t border-border px-4 py-3 flex justify-end">
                  <Button size="sm" onClick={() => onRestore(previewVersion.id)}>
                    Restore This Version
                  </Button>
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
```

---

### Task 10: Integration test

- [ ] **Step 1: Full Rust test suite**

```bash
cd src-tauri && cargo test 2>&1 | tail -20
```

Expected output ends with:
```
test result: ok. N passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

- [ ] **Step 2: Full frontend build**

```bash
cd /Users/nathananderson-tennant/Development/rubber-duck && bun run build
```

Expected: exits 0. No type errors.

- [ ] **Step 3: Dev run**

```bash
bun tauri dev
```

Manually verify:
1. Docs tab appears (was Refine)
2. "Generate Document" dropdown shows PRD, SDD, Test Plan, ADR
3. Select PRD → creates card with 5 sections, each shows "Generating..."
4. Sections stream in one at a time
5. After completion, content is rendered as markdown
6. [↻] regenerate works per-section
7. [✏] edit → textarea → Save persists
8. [⏱] history → shows previous versions → Restore works
9. Export .md opens save dialog
10. Copy All puts markdown on clipboard
11. Delete document (with confirmation)
12. "Manage Templates" → create / edit / delete custom templates

- [ ] **Step 4: Commit**

```bash
git add src-tauri/migrations/005_add_docs.sql \
        src-tauri/templates/ \
        src-tauri/src/docs/ \
        src-tauri/src/db.rs \
        src-tauri/src/lib.rs \
        src/features/docs/ \
        src/App.tsx
git commit -m "feat: add document generation with LLM-powered per-section streaming"
```

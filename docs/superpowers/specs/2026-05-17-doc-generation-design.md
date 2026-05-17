# Document Generation — Design Spec

## Overview

Generate SDLC documents (PRDs, SDDs, Test Plans, ADRs) from session context using LLM-powered templates. Built-in templates ship with the app; users can create custom templates. Documents are persistent, per-section editable and regeneratable, and exportable as markdown.

## Templates

### Format

Templates are markdown files with section delimiters and LLM directives:

```markdown
# Product Requirements Document
<!-- section: Overview -->
<!-- directive: Write a 2-3 paragraph overview of the product/feature based on the session notes and tickets. Include the problem being solved and the target users. -->

<!-- section: User Stories -->
<!-- directive: Extract user stories from the notes and tickets. Format as "As a [role], I want [goal] so that [benefit]." Include at least 5 stories covering the main functionality. -->

<!-- section: Functional Requirements -->
<!-- directive: List the functional requirements derived from the notes, tickets, and user stories. Number each requirement. Be specific and testable. -->

<!-- section: Non-Functional Requirements -->
<!-- directive: List non-functional requirements: performance, security, scalability, accessibility, compatibility. Only include requirements relevant to the context. -->

<!-- section: Success Criteria -->
<!-- directive: Define measurable success criteria for this feature/product. How will you know it shipped successfully? -->
```

### Built-in Templates

Four templates bundled via `include_str!`:
- **PRD** — Overview, User Stories, Functional Requirements, Non-Functional Requirements, Success Criteria
- **SDD** — Overview, Architecture, Components, Data Model, API Design, Error Handling
- **Test Plan** — Overview, Test Strategy, Test Cases, Edge Cases, Acceptance Criteria
- **ADR** — Context, Decision, Consequences, Alternatives Considered

### Custom Templates

Stored in `templates` table in SQLite:

```sql
CREATE TABLE templates (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

- Users create custom templates via a template editor (markdown textarea)
- Templates use the same `<!-- section: Name -->` / `<!-- directive: ... -->` format
- Built-in templates can be cloned as a starting point for custom ones
- Custom templates can be edited and deleted; built-in ones are read-only

### Template Parsing

```rust
pub struct TemplateSection {
    pub name: String,
    pub directive: String,
}

pub fn parse_template(content: &str) -> Vec<TemplateSection>
```

Parses `<!-- section: X -->` and `<!-- directive: Y -->` pairs. Each section must have both markers. Sections without directives are skipped.

## Documents

### Data Model

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
```

### Store Functions

- `create_document(conn, session_id, template_name, title) -> Document`
- `get_document(conn, document_id) -> Document`
- `list_documents(conn, session_id) -> Vec<Document>`
- `delete_document(conn, document_id)`
- `create_section(conn, document_id, name, directive, sort_order) -> DocumentSection`
- `update_section_content(conn, section_id, content) -> DocumentSection`
- `list_sections(conn, document_id) -> Vec<DocumentSection>`

### Custom Template Store

- `create_template(conn, name, content) -> Template`
- `get_template(conn, template_id) -> Template`
- `list_templates(conn) -> Vec<Template>`
- `update_template(conn, template_id, name, content) -> Template`
- `delete_template(conn, template_id)`

## Generation

### Per-Section LLM Call

Each section is generated independently via its own LLM call. This allows:
- Streaming per section (user sees progress)
- Per-section regeneration without affecting others
- Parallel generation if desired (future optimization)

### `generate_doc_section` Tauri Command

```rust
pub async fn generate_doc_section(
    app: AppHandle,
    db: State<'_, Database>,
    document_id: String,
    section_id: String,
) -> Result<(), String>
```

Flow:
1. Load the section (gets directive)
2. Load the document (gets session_id)
3. Load session context: notes, tickets, repo summaries, Jira issues (same as `send_message`)
4. Build LLM messages:
   - System prompt: "You are generating a section of a document. Write in clear, professional prose. Use markdown formatting."
   - Session context (notes, tickets, repos, Jira — same assembly as chat)
   - User message: the section directive
5. Stream response via `doc:chunk`, `doc:done`, `doc:error` events (separate event namespace from chat)
6. On completion, save content to `document_sections` via `update_section_content`

### Context Assembly

Reuses the existing context assembly pipeline. The `assemble_context` function already handles notes, tickets, Jira issues, repo summaries, and @mentioned files. For doc generation, we use the same function but with:
- A document-generation-specific system prompt
- The section directive as the user message
- No conversation history (each section is independent)

## Tauri Commands

| Command | Params | Returns |
|---------|--------|---------|
| `list_builtin_templates` | none | `Vec<BuiltinTemplate>` |
| `create_custom_template` | `name, content` | `Template` |
| `list_custom_templates` | none | `Vec<Template>` |
| `get_custom_template` | `template_id` | `Template` |
| `update_custom_template` | `template_id, name, content` | `Template` |
| `delete_custom_template` | `template_id` | `()` |
| `create_document` | `session_id, template_name, title, sections` | `Document` |
| `get_document` | `document_id` | `Document` |
| `list_documents` | `session_id` | `Vec<Document>` |
| `delete_document` | `document_id` | `()` |
| `list_document_sections` | `document_id` | `Vec<DocumentSection>` |
| `update_document_section` | `section_id, content` | `DocumentSection` |
| `generate_doc_section` | `document_id, section_id` | `()` (streams events) |

`BuiltinTemplate`:
```rust
pub struct BuiltinTemplate {
    pub name: String,
    pub content: String,
}
```

## Frontend

### Docs Tab

Rename the existing "Refine" tab to "Docs". It shows:

1. **Document list** — cards for each generated document in the session
2. **"Generate Document" button** — opens a template picker (dropdown showing built-in + custom templates, plus "New Template" and "Manage Templates" options)
3. **Per-document card** — title, template name, list of sections as expandable rows

### Document Card

```
┌─ PRD: Auth Flow Redesign ─────────────── [🗑] ─┐
│                                                  │
│  📋 Overview                          [↻] [✏]   │
│  ┌──────────────────────────────────────────┐   │
│  │ This document outlines the requirements   │   │
│  │ for redesigning the authentication flow...│   │
│  └──────────────────────────────────────────┘   │
│                                                  │
│  📋 User Stories                      [↻] [✏]   │
│  📋 Functional Requirements           [↻] [✏]   │
│  📋 Non-Functional Requirements       [↻] [✏]   │
│  📋 Success Criteria                  [↻] [✏]   │
│                                                  │
│  [Export .md] [Copy All]                         │
└──────────────────────────────────────────────────┘
```

- Click section name → toggle expand/collapse, shows rendered markdown
- [↻] → regenerate section (calls `generate_doc_section`, shows spinner, streams new content)
- [✏] → edit mode (textarea, save on blur or Enter)
- [🗑] → delete document (with confirmation)
- Export → concatenates `# Title\n\n## Section Name\n\nContent\n\n...` → native save dialog
- Copy All → same concatenation → clipboard

### Generation Flow UI

1. Click "Generate Document" → template picker dropdown
2. Select template → creates Document record with empty sections
3. Sections appear with "Generating..." spinners
4. Each section streams in sequence (one at a time, top to bottom)
5. As each section completes, it switches from spinner to rendered content
6. Errors per-section are shown inline (red text, retry button)

### Template Management

Accessible via "Manage Templates" in the template picker dropdown. Shows:
- List of custom templates with edit/delete
- "New Template" button → opens template editor
- Template editor: name input + markdown textarea with section/directive syntax
- "Clone Built-in" option on each built-in template → creates a custom copy

### Streaming Events

Separate namespace from chat events:
- `doc:chunk` — `{ section_id: String, content: String }`
- `doc:done` — `{ section_id: String, full_content: String }`
- `doc:error` — `{ section_id: String, message: String }`

Section ID in each event allows the frontend to route content to the correct section.

## New Module Structure

```
src-tauri/src/docs/
├── mod.rs
├── model.rs          # Document, DocumentSection, Template, BuiltinTemplate, TemplateSection
├── store.rs          # Document + section CRUD
├── template_store.rs # Custom template CRUD
├── templates.rs      # Built-in template loading + parsing
├── generator.rs      # generate_doc_section LLM call
└── commands.rs       # Tauri commands
```

## Built-in Template Files

```
src-tauri/templates/
├── prd.md
├── sdd.md
├── test-plan.md
└── adr.md
```

Loaded via `include_str!` — bundled into the binary.

## Out of Scope

- Version history (showing previous generations of a section)
- Collaborative editing
- PDF export
- Template marketplace / sharing
- Auto-regeneration when session context changes
- Parallel section generation (sequential is fine for v1)

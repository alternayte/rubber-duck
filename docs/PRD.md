# rubber-duck — Product Requirements Document

## 1. Problem Statement

Planning software work is messy. Ideas start as scattered thoughts, half-formed sentences, and mental models that don't map cleanly to issue tracker fields. The current workflow forces a premature jump from fuzzy thinking to structured tickets in Jira/Linear, losing nuance and skipping the critical refinement step. There's no local, private tool that bridges the gap between "thinking about a problem" and "tickets ready to implement."

## 2. Solution

rubber-duck is a local-first desktop app that provides a dedicated space for technical planning and brainstorming, powered by an LLM assistant. It lets you:

- **Dump** messy thoughts, code snippets, and repo context into a freeform markdown editor
- **Refine** with LLM assistance — structuring thoughts into tickets, finding gaps, estimating complexity
- **Get grilled** by the LLM acting as a critical reviewer that challenges assumptions and finds missing pieces
- **Generate** structured outputs: issue tracker tickets or SDLC documents (PRD, SDD, test plan, ADR)
- **Push** refined tickets to Jira or Linear when ready

## 3. Target User

A solo developer or IC who plans their own work. Someone who thinks before coding and wants a tool that supports that thinking process without forcing premature structure.

Primary user: the developer building this tool (dogfooding at Frontiers). The data model is single-user — no team assignment, capacity tracking, or workload features. Team features may come later but are not in scope.

## 3.1 Architecture Constraint: Local Storage, Cloud Intelligence

All data (sessions, notes, tickets, conversations) is stored locally in SQLite. Nothing leaves the machine unless the user explicitly pushes to Jira/Linear. However, the LLM features (chat, grill, ticket extraction, doc generation) require an API connection to OpenRouter. Without internet, the app functions as a markdown editor with structured note storage. Offline LLM via Ollama is a stretch goal, not a v1 requirement.

## 4. Core Concepts

### Session
A planning session is the top-level container. It represents one coherent planning effort (e.g., "CDC Pipeline Rework", "Auth Service Migration"). A session contains notes, tickets, conversations, attachments, and generated documents.

### Notes (Brain Dump)
Freeform markdown content. The raw, unstructured thinking. Supports inline images and file attachments.

### Tickets
Structured work items extracted from the brainstorm. Each has a title, description, acceptance criteria, estimate, priority, labels, type, and optional parent (epic). Tickets are drafts until pushed to an external platform.

### Duck Chat
The LLM conversation. Always available in a side panel. Operates in two modes:
- **Assist mode (default):** You ask, it helps. "Break this into tickets." "What am I missing?" "Write acceptance criteria for this."
- **Grill mode:** The LLM takes initiative. It reads your session context and asks probing questions, challenges vague requirements, identifies gaps, and pushes for specificity. Inspired by the Superpowers brainstorming approach.

### Repo Context
Git repositories attached to a session for LLM awareness. The tool indexes the repo structure and key files so the LLM can reference actual code paths, existing patterns, and architecture when generating tickets or docs.

### Generated Documents
SDLC artifacts produced from session content using templates. The LLM fills each section based on the brainstorm notes, tickets, and conversation history. Exportable as Markdown, PDF, or DOCX.

### Memory
Cross-session knowledge. After archiving a session, a summary and key decisions are stored. Future sessions can draw on this context via full-text search (and later, vector search/RAG).

### Settings
Application configuration stored in SQLite (key-value). Includes: selected LLM model, Jira/Linear connection details, and UI preferences. The API key is stored in the OS keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service) — never in the database or config files.

## 5. User Flows

### 5.1 — Core Planning Flow
1. Create a new session with a title and optional context
2. Attach repos (local path or git URL) for code awareness
3. Brain-dump in the markdown editor
4. Open duck chat → ask to structure notes into tickets
5. Review generated tickets in the Refine view (side-by-side with notes)
6. Edit tickets directly, ask LLM to improve specific ones
7. Switch to Grill mode → LLM challenges your plan, you fill gaps
8. Move to Board view → reorder, set priorities, group into epics
9. Push selected tickets to Jira or Linear
10. Optionally generate a PRD or other doc from the session

### 5.2 — Document Generation Flow
1. From any session, open Generate menu
2. Select document type (PRD, SDD, Test Plan, ADR, RFC, Runbook)
3. Select template (built-in or custom)
4. LLM generates document using session context
5. Review and edit in-app
6. Export as Markdown (PDF via print-to-PDF is a stretch goal)
7. Regenerate specific sections as needed

### 5.3 — Cross-Session Memory
1. Archive a completed session
2. System auto-generates summary + extracts decisions
3. In a new session, LLM automatically retrieves relevant past context
4. User can also search across all sessions via cmd+K

## 6. UI Layout

```
┌─────────────────────────────────────────┬──────────────────────┐
│           MAIN AREA                     │    SIDE PANEL        │
│                                         │                      │
│  [Dump]  [Refine]  [Board]   ← tabs    │  Context:            │
│                                         │  • attached repos    │
│  Dump:   Freeform markdown editor       │  • images/files      │
│  Refine: Split (notes left,             │  • session settings  │
│          tickets right, editable)       │                      │
│  Board:  Kanban of tickets              │  Duck Chat:          │
│          (drag, reorder, group)         │  [Assist] [Grill]    │
│                                         │  (always available)  │
│                                         │                      │
└─────────────────────────────────────────┴──────────────────────┘
```

The side panel is collapsible. The duck chat is the primary interaction point with the LLM. Context panel shows what the LLM "knows" about this session.

## 7. Data Model

### Session
| Field | Type | Notes |
|-------|------|-------|
| id | UUID | Primary key |
| title | String | User-provided |
| context | Text | Persistent background for LLM |
| status | Enum | Draft, Active, Archived |
| created_at | DateTime | |
| updated_at | DateTime | |

### Note
| Field | Type | Notes |
|-------|------|-------|
| id | UUID | |
| session_id | UUID | FK → Session |
| content | Text | Markdown |
| sort_order | Integer | |
| created_at | DateTime | |

### Ticket
| Field | Type | Notes |
|-------|------|-------|
| id | UUID | |
| session_id | UUID | FK → Session |
| title | String | |
| description | Text | Markdown |
| acceptance_criteria | Text | Markdown |
| estimate | Enum | XS, S, M, L, XL (nullable) |
| priority | Enum | Low, Medium, High, Critical |
| ticket_type | Enum | Task, Bug, Story, Epic |
| labels | JSON | Array of strings |
| parent_id | UUID | Nullable, FK → Ticket (epic grouping) |
| dependencies | JSON | Array of ticket UUIDs |
| status | Enum | Draft, Refined, Pushed |
| external_ref | JSON | Nullable: { platform, id, url } |
| sort_order | Integer | |
| created_at | DateTime | |

### Conversation
| Field | Type | Notes |
|-------|------|-------|
| id | UUID | |
| session_id | UUID | FK → Session |
| role | Enum | User, Assistant, System |
| content | Text | |
| referenced_ticket_ids | JSON | Array of UUIDs |
| created_at | DateTime | |

### Attachment
| Field | Type | Notes |
|-------|------|-------|
| id | UUID | |
| session_id | UUID | FK → Session |
| kind | Enum | Image, File, CodeSnippet, Excalidraw |
| name | String | |
| data_path | String | Path to local file |
| thumbnail_path | String | Nullable |
| created_at | DateTime | |

### RepoContext
| Field | Type | Notes |
|-------|------|-------|
| id | UUID | |
| session_id | UUID | FK → Session |
| name | String | e.g., "editorial-workflow" |
| source_type | Enum | LocalPath, GitUrl |
| source_value | String | Path or URL |
| summary | Text | Cached repo summary |
| included_paths | JSON | Optional path filters |
| last_indexed | DateTime | |

### DocTemplate
| Field | Type | Notes |
|-------|------|-------|
| id | UUID | |
| name | String | e.g., "Frontiers PRD Template" |
| doc_type | Enum | PRD, SDD, TestPlan, ADR, RFC, Runbook |
| template_content | Text | Markdown with LLM instruction comments |
| is_builtin | Boolean | |
| created_at | DateTime | |

### GeneratedDoc
| Field | Type | Notes |
|-------|------|-------|
| id | UUID | |
| session_id | UUID | FK → Session |
| doc_type | Enum | |
| template_id | UUID | FK → DocTemplate |
| content | Text | Generated markdown |
| version | Integer | |
| created_at | DateTime | |

### SessionMemory
| Field | Type | Notes |
|-------|------|-------|
| id | UUID | |
| session_id | UUID | FK → Session |
| summary | Text | LLM-generated summary |
| decisions_json | JSON | Array of { what, why, context, tags } |
| key_entities | JSON | Array of strings |
| created_at | DateTime | |

### Settings
| Field | Type | Notes |
|-------|------|-------|
| key | String | Primary key (e.g., "llm.model") |
| value | String | Setting value |
| category | String | Grouping (e.g., "llm", "jira") |
| updated_at | DateTime | |

## 8. Integration Abstraction

```rust
trait TicketPlatform {
    async fn test_connection(&self) -> Result<()>;
    async fn list_projects(&self) -> Result<Vec<Project>>;
    async fn list_labels(&self, project_id: &str) -> Result<Vec<Label>>;
    async fn list_issue_types(&self, project_id: &str) -> Result<Vec<IssueType>>;
    async fn push_ticket(&self, ticket: &Ticket, config: &PushConfig) -> Result<ExternalRef>;
    async fn push_batch(&self, tickets: &[Ticket], config: &PushConfig) -> Result<Vec<ExternalRef>>;
}
```

### Jira (v1 priority)
- Jira Cloud REST API v3
- Auth: API token (email + token) or OAuth 2.0
- Push: POST /rest/api/3/issue
- Fields: summary, description (ADF), priority, labels, issuetype, parent (epic link)

### Linear (v2)
- GraphQL API
- Auth: API key or OAuth
- Push: createIssue mutation
- Fields: title, description (markdown — easier than Jira), priority, labels, parent (project)

## 9. LLM Modes

### Assist Mode (System Prompt Core)
```
You are a technical planning assistant embedded in a local brainstorming tool called rubber-duck. Your job is to help the user think through technical problems and produce well-structured work items.

You have access to:
- The user's freeform notes for this session
- Any structured tickets already created
- Repo context (code structure, key files) if attached
- Summaries and decisions from past sessions

When asked to create tickets, produce structured JSON that the app can parse. When asked to review or improve, be specific and actionable.
```

### Grill Mode (System Prompt Core)
```
You are a critical technical reviewer. Your job is to find gaps, ambiguities, missing edge cases, and unstated assumptions in the user's planning session.

Read the current notes and tickets carefully. Then ask ONE focused question at a time. Be specific — reference actual content from their notes. Don't be generic.

Examples of good questions:
- "You mention migrating the CDC pipeline but there's no ticket for schema migration — is that intentional or missing?"
- "The acceptance criteria for ticket #3 say 'handles errors gracefully' — what does that mean specifically? Which error cases?"
- "I see nothing about rollback strategy. What happens if this deployment fails halfway?"

Do not provide solutions unless asked. Your job is to find the holes.
```

## 10. Document Templates

Templates use markdown with HTML comment directives for the LLM:

```markdown
# {title}

## 1. Overview
<!-- RUBBER_DUCK_LLM: Summarize the session's core problem and proposed solution in 2-3 paragraphs. Use the brain dump notes and conversation history. -->

## 2. Goals & Non-Goals
<!-- RUBBER_DUCK_LLM: Extract explicit goals from the brainstorm. If non-goals were not discussed, ask the user before filling this section. -->

## 3. Technical Approach
<!-- RUBBER_DUCK_LLM: Describe the implementation strategy using repo context and tickets. Reference specific code paths where possible. -->
```

Built-in templates: PRD, SDD (Software Design Document), Test Plan, ADR (Architecture Decision Record), RFC, Runbook.

Users can create custom templates following the same format.

## 11. Non-Goals (v1)

- Bidirectional sync (pulling updates from Jira/Linear back)
- Multi-user / collaboration / team features
- Cloud sync between devices
- Mobile app
- Embedded Excalidraw editor (images only for now)
- Real-time co-editing
- Offline LLM (Ollama — stretch goal, not v1)
- PDF/DOCX export (markdown export only; PDF via print-to-PDF is a stretch goal)

## 12. Success Criteria

Baseline: time yourself doing one full planning session manually (brain dump → tickets in Jira) before using rubber-duck. Record the time and the number of tickets.

- **Daily use:** Nathan uses rubber-duck for at least 3 planning sessions per week at Frontiers for 2+ consecutive weeks
- **Speed:** Time from "idea" to "tickets in Jira" is under 50% of the manual baseline (measured on 3+ sessions)
- **Grill effectiveness:** Grill mode surfaces at least 1 concrete gap per session that results in a new ticket or a revised acceptance criteria (track by counting grill-originated tickets)
- **Doc quality:** Generated documents require fewer than 5 manual edits before sharing (count edits on 3+ documents)

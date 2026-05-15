# Jira Push UI — Design Spec

## Overview

Add frontend UI for configuring Jira and pushing tickets. The backend commands (`get_jira_config`, `set_jira_config`, `set_jira_api_token`, `has_jira_config`, `test_jira_connection`, `push_ticket_to_jira`) already exist. This task adds the settings UI, per-ticket push action, and one new backend command for fetching projects.

## New Backend Command

### `get_jira_projects`

Fetches the list of projects from the configured Jira instance.

- **Endpoint:** `GET /rest/api/2/project`
- **Returns:** `Result<Vec<JiraProject>, String>` where `JiraProject = { key: String, name: String }`
- **Called:** After a successful `test_jira_connection` in the settings dialog
- **Errors:** Same auth/network error handling as `test_jira_connection`

## Settings Dialog — Jira Section

A new "Jira" section is added below the existing model selector in the settings dialog.

### Fields

| Field | Type | Stored via | Condition |
|-------|------|------------|-----------|
| Base URL | Text input | `set_jira_config` | Always |
| Auth Method | Select: "Basic (Cloud)" / "PAT (Server/DC)" | `set_jira_config` | Always |
| Email | Text input | `set_jira_config` | Basic only |
| API Token | Password input with show/hide | `set_jira_api_token` (keyring) | Basic only |
| Personal Access Token | Password input with show/hide | `set_jira_api_token` (keyring) | PAT only |
| Default Project | Searchable combobox | `set_setting("jira.default_project", key, "jira")` | After test |

### Test Connection Flow

1. User fills in base URL, auth method, and credentials
2. User clicks "Test Connection" button
3. Button shows spinner during request
4. **Success:** Green check + "Connected as {display_name}". Then calls `get_jira_projects` and populates the project combobox.
5. **Failure:** Red X + error message:
   - 401/403: "Authentication failed. Check your credentials and try again."
   - Connection refused / DNS: "Could not reach server. Check your base URL."
   - Other: Jira error message if parseable, otherwise generic message
6. Error clears on next "Test Connection" click

### Project Combobox

- Disabled until projects are fetched via successful test
- Searchable — user types to filter by project key or name
- Displays as "KEY - Project Name" (e.g., "FRONT - Frontiers")
- Saved as just the project key string

### Save Behavior

- "Save" button persists all fields
- Config fields: `set_jira_config(base_url, auth_method, email)`
- Token: `set_jira_api_token(token)` (stored in OS keyring)
- Default project: `set_setting("jira.default_project", project_key, "jira")`
- Save does NOT require a successful test — user can save config while offline

### Load Behavior

- On dialog open: `get_jira_config()` populates base URL, auth method, email
- Token field shows placeholder if config exists (we can't read it back from keyring; show "••••••••" if `has_jira_config` returns true)
- Default project: `get_setting("jira.default_project")` sets the combobox value
- Project list is NOT re-fetched on open — only on explicit "Test Connection"

## Ticket List — Push Action

### Push Icon

- Added to the hover action row: `[↑] [↓] [⬆ Jira] [🗑]`
- Only visible if `has_jira_config()` returns true (checked once on ticket list mount, stored in Jotai atom)
- Hidden for tickets that already have `external_ref` set (already pushed)

### Push Flow

1. User hovers ticket, clicks push icon
2. Read default project key from settings atom
3. If no default project set: show inline warning "Jira not configured. Open Settings" (clickable to open settings dialog)
4. Call `push_ticket_to_jira(ticket_id, project_key)`
5. During push: replace push icon with spinner, show "Pushing..." text below title
6. **Success:** Update ticket in `ticketsAtom` with returned ticket (has `external_ref`). Display Jira key as clickable link.
7. **Failure:** Show red inline error below ticket title, dismissible with X button. Push icon returns so user can retry.

### Pushed State Display

- When `external_ref` is set: show "PROJ-42 ↗" below the ticket title
- Clicking the key opens the Jira URL in the default browser via Tauri's `shell.open`
- Push icon is not shown for pushed tickets (no re-push)

## Error Messages

| Scenario | Location | Message |
|----------|----------|---------|
| Auth failure (test) | Settings, below Test button | "Authentication failed. Check your credentials and try again." |
| Network failure (test) | Settings, below Test button | "Could not reach server. Check your base URL." |
| Auth failure (push) | Ticket, inline below title | "Push failed: authentication error" |
| Project not found (push) | Ticket, inline below title | "Push failed: project {KEY} not found" |
| Permission denied (push) | Ticket, inline below title | "Push failed: no permission to create issues in {KEY}" |
| No Jira config (push) | Ticket, inline below title | "Jira not configured. Open Settings" (clickable) |
| Generic error | Context-dependent | Jira error message if parsed, otherwise "Unexpected error" |

All errors are inline (no modal dialogs). Push errors are dismissible. Settings errors clear on next test attempt.

## Frontend State

### New Atoms

| Atom | Type | Purpose |
|------|------|---------|
| `jiraConfiguredAtom` | `boolean` | Whether Jira is fully configured. Controls push icon visibility. |
| `jiraDefaultProjectAtom` | `string \| null` | Default project key for push. |

### Modified Components

| Component | Changes |
|-----------|---------|
| `SettingsDialog.tsx` | Add Jira section with all fields, test connection, project combobox |
| `TicketList.tsx` | Add push icon to hover row, pushed state display, inline errors |

### New Types

```typescript
interface JiraProject {
  key: string;
  name: string;
}

interface ExternalRef {
  platform: string;
  key: string;
  url: string;
}
```

## Out of Scope

- Batch push (push all tickets at once)
- Per-ticket project override
- Re-push / update existing Jira issues
- ADF conversion for rich descriptions
- Issue type mapping to Jira issue types
- Epic linking in Jira
- Fetching Jira issue types per project

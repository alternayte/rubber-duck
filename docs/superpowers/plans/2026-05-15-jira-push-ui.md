# Jira Push UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add frontend UI for configuring Jira connection settings, selecting a default project, and pushing individual tickets to Jira — plus one new backend command for fetching Jira projects.

**Architecture:** One new backend method + Tauri command (`get_jira_projects`), a Jira settings section added to the existing `SettingsDialog.tsx`, and push/status UI added to the existing `TicketList.tsx`. Two new Jotai atoms track Jira config state. A searchable combobox component handles project selection.

**Tech Stack:** Rust (reqwest, mockito), React 19, Jotai, Tailwind CSS v4, shadcn/ui, Lucide icons, cmdk (combobox), @tauri-apps/plugin-opener (open Jira links)

---

## File Map

| Action | File | Responsibility |
|--------|------|----------------|
| Modify | `src-tauri/src/jira/model.rs` | Add `JiraProject` struct |
| Modify | `src-tauri/src/jira/client.rs` | Add `get_projects()` method + tests |
| Modify | `src-tauri/src/jira/commands.rs` | Add `get_jira_projects` Tauri command |
| Modify | `src-tauri/src/lib.rs` | Register `get_jira_projects` command |
| Create | `src/components/ui/combobox.tsx` | Searchable combobox (shadcn Popover + Command) |
| Create | `src/components/ui/popover.tsx` | Popover primitive (shadcn, dependency of combobox) |
| Create | `src/components/ui/command.tsx` | Command palette primitive (shadcn, wraps cmdk) |
| Modify | `src/features/settings/settings.atoms.ts` | Add `jiraConfiguredAtom`, `jiraDefaultProjectAtom` |
| Modify | `src/features/settings/SettingsDialog.tsx` | Add Jira settings section |
| Modify | `src/features/ticket/ticket.types.ts` | Add `JiraProject`, `ExternalRef` interfaces |
| Modify | `src/features/ticket/useTicketActions.ts` | Add `pushToJira` action |
| Modify | `src/features/ticket/TicketList.tsx` | Add push icon, pushed state, inline errors |

---

### Task 1: Add `get_projects()` to JiraClient

**Files:**
- Modify: `src-tauri/src/jira/model.rs`
- Modify: `src-tauri/src/jira/client.rs`

- [ ] **Step 1: Add `JiraProject` model**

In `src-tauri/src/jira/model.rs`, add at the end of the file:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraProject {
    pub key: String,
    pub name: String,
}
```

- [ ] **Step 2: Write failing test for `get_projects`**

In `src-tauri/src/jira/client.rs`, add to the `tests` module:

```rust
#[tokio::test]
async fn get_projects_success() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/rest/api/2/project")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[{"key":"FRONT","name":"Frontiers"},{"key":"INFRA","name":"Infrastructure","extra":"ignored"}]"#)
        .create_async()
        .await;

    let client = JiraClient::new(&server.url(), JiraAuth::Basic {
        email: "test@example.com".to_string(),
        api_token: "token".to_string(),
    }).unwrap();
    let projects = client.get_projects().await.unwrap();

    assert_eq!(projects.len(), 2);
    assert_eq!(projects[0].key, "FRONT");
    assert_eq!(projects[0].name, "Frontiers");
    assert_eq!(projects[1].key, "INFRA");
    mock.assert_async().await;
}

#[tokio::test]
async fn get_projects_auth_failure() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/rest/api/2/project")
        .with_status(401)
        .with_body("")
        .create_async()
        .await;

    let client = JiraClient::new(&server.url(), JiraAuth::Basic {
        email: "bad@example.com".to_string(),
        api_token: "wrong".to_string(),
    }).unwrap();
    let result = client.get_projects().await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Authentication failed"), "Expected auth error, got: {err}");
    mock.assert_async().await;
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd src-tauri && cargo test jira --lib -- get_projects 2>&1`
Expected: Compilation error — `get_projects` method does not exist.

- [ ] **Step 4: Implement `get_projects`**

In `src-tauri/src/jira/client.rs`, add the import for `JiraProject` to the existing use statement at the top:

```rust
use super::model::{
    CreateIssueFields, CreateIssueRequest, IssueTypeRef, ProjectRef,
    JiraAuth, JiraErrorResponse, JiraUser, CreateIssueResponse, JiraProject,
};
```

Then add this method to the `impl JiraClient` block, after `create_issue`:

```rust
pub async fn get_projects(&self) -> AppResult<Vec<JiraProject>> {
    let url = format!("{}/rest/api/2/project", self.base_url);
    let response = self
        .apply_auth(self.client.get(&url))
        .send()
        .await?;

    if response.status().is_success() {
        let projects: Vec<JiraProject> = response.json().await?;
        return Ok(projects);
    }

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let message = parse_jira_error(&body, status.as_u16());
    Err(AppError::Other(message))
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd src-tauri && cargo test jira --lib -- get_projects 2>&1`
Expected: 2 tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/jira/model.rs src-tauri/src/jira/client.rs
git commit -m "feat: add get_projects to JiraClient"
```

---

### Task 2: Add `get_jira_projects` Tauri command

**Files:**
- Modify: `src-tauri/src/jira/commands.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add the Tauri command**

In `src-tauri/src/jira/commands.rs`, add the import for `JiraProject` to the existing use statement:

```rust
use super::model::{JiraAuth, JiraUser, JiraProject};
```

Then add this command after `push_ticket_to_jira`:

```rust
#[tauri::command]
pub async fn get_jira_projects(db: State<'_, Database>) -> Result<Vec<JiraProject>, String> {
    let (base_url, auth) = get_jira_credentials(&db)?;
    let client = JiraClient::new(&base_url, auth).map_err(|e| e.to_string())?;
    client.get_projects().await.map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Register the command in `lib.rs`**

In `src-tauri/src/lib.rs`, add `get_jira_projects` to the `invoke_handler` list, after `push_ticket_to_jira`:

```rust
push_ticket_to_jira,
get_jira_projects,
```

- [ ] **Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check 2>&1`
Expected: Compiles with no errors.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/jira/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add get_jira_projects Tauri command"
```

---

### Task 3: Add shadcn/ui Combobox component

**Files:**
- Create: `src/components/ui/popover.tsx`
- Create: `src/components/ui/command.tsx`
- Create: `src/components/ui/combobox.tsx`

The searchable project picker needs a combobox. shadcn/ui builds this from Popover + Command (which wraps `cmdk`).

- [ ] **Step 1: Install cmdk dependency**

Run: `bun add cmdk@1`

- [ ] **Step 2: Create Popover component**

Create `src/components/ui/popover.tsx`:

```tsx
import * as React from "react";
import * as PopoverPrimitive from "@radix-ui/react-popover";
import { cn } from "@/lib/utils";

const Popover = PopoverPrimitive.Root;
const PopoverTrigger = PopoverPrimitive.Trigger;

const PopoverContent = React.forwardRef<
  React.ComponentRef<typeof PopoverPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof PopoverPrimitive.Content>
>(({ className, align = "start", sideOffset = 4, ...props }, ref) => (
  <PopoverPrimitive.Portal>
    <PopoverPrimitive.Content
      ref={ref}
      align={align}
      sideOffset={sideOffset}
      className={cn(
        "z-50 w-[--radix-popover-trigger-width] rounded-md border bg-popover p-0 text-popover-foreground shadow-md outline-none data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95",
        className,
      )}
      {...props}
    />
  </PopoverPrimitive.Portal>
));
PopoverContent.displayName = PopoverPrimitive.Content.displayName;

export { Popover, PopoverTrigger, PopoverContent };
```

- [ ] **Step 3: Install Radix popover**

Run: `bun add @radix-ui/react-popover`

- [ ] **Step 4: Check for `cn` utility**

The `cn` utility should exist at `src/lib/utils.ts`. If it doesn't, create it:

```ts
import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
```

Run: `bun add clsx tailwind-merge` (only if `cn` doesn't exist yet)

- [ ] **Step 5: Create Command component**

Create `src/components/ui/command.tsx`:

```tsx
import * as React from "react";
import { Command as CommandPrimitive } from "cmdk";
import { cn } from "@/lib/utils";

const Command = React.forwardRef<
  React.ComponentRef<typeof CommandPrimitive>,
  React.ComponentPropsWithoutRef<typeof CommandPrimitive>
>(({ className, ...props }, ref) => (
  <CommandPrimitive
    ref={ref}
    className={cn("flex h-full w-full flex-col overflow-hidden rounded-md bg-popover text-popover-foreground", className)}
    {...props}
  />
));
Command.displayName = CommandPrimitive.displayName;

const CommandInput = React.forwardRef<
  React.ComponentRef<typeof CommandPrimitive.Input>,
  React.ComponentPropsWithoutRef<typeof CommandPrimitive.Input>
>(({ className, ...props }, ref) => (
  <div className="flex items-center border-b px-3">
    <CommandPrimitive.Input
      ref={ref}
      className={cn("flex h-9 w-full rounded-md bg-transparent py-3 text-sm outline-none placeholder:text-muted-foreground disabled:cursor-not-allowed disabled:opacity-50", className)}
      {...props}
    />
  </div>
));
CommandInput.displayName = CommandPrimitive.Input.displayName;

const CommandList = React.forwardRef<
  React.ComponentRef<typeof CommandPrimitive.List>,
  React.ComponentPropsWithoutRef<typeof CommandPrimitive.List>
>(({ className, ...props }, ref) => (
  <CommandPrimitive.List
    ref={ref}
    className={cn("max-h-[200px] overflow-y-auto overflow-x-hidden", className)}
    {...props}
  />
));
CommandList.displayName = CommandPrimitive.List.displayName;

const CommandEmpty = React.forwardRef<
  React.ComponentRef<typeof CommandPrimitive.Empty>,
  React.ComponentPropsWithoutRef<typeof CommandPrimitive.Empty>
>((props, ref) => (
  <CommandPrimitive.Empty ref={ref} className="py-6 text-center text-sm" {...props} />
));
CommandEmpty.displayName = CommandPrimitive.Empty.displayName;

const CommandItem = React.forwardRef<
  React.ComponentRef<typeof CommandPrimitive.Item>,
  React.ComponentPropsWithoutRef<typeof CommandPrimitive.Item>
>(({ className, ...props }, ref) => (
  <CommandPrimitive.Item
    ref={ref}
    className={cn("relative flex cursor-default select-none items-center rounded-sm px-2 py-1.5 text-sm outline-none data-[selected=true]:bg-accent data-[selected=true]:text-accent-foreground data-[disabled=true]:pointer-events-none data-[disabled=true]:opacity-50", className)}
    {...props}
  />
));
CommandItem.displayName = CommandPrimitive.Item.displayName;

export { Command, CommandInput, CommandList, CommandEmpty, CommandItem };
```

- [ ] **Step 6: Create Combobox component**

Create `src/components/ui/combobox.tsx`:

```tsx
import { useState } from "react";
import { Check, ChevronsUpDown } from "lucide-react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Command, CommandInput, CommandList, CommandEmpty, CommandItem } from "@/components/ui/command";

export interface ComboboxOption {
  value: string;
  label: string;
}

interface ComboboxProps {
  options: ComboboxOption[];
  value: string;
  onValueChange: (value: string) => void;
  placeholder?: string;
  searchPlaceholder?: string;
  emptyText?: string;
  disabled?: boolean;
}

export function Combobox({
  options,
  value,
  onValueChange,
  placeholder = "Select...",
  searchPlaceholder = "Search...",
  emptyText = "No results found.",
  disabled = false,
}: ComboboxProps) {
  const [open, setOpen] = useState(false);
  const selectedLabel = options.find((o) => o.value === value)?.label;

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          role="combobox"
          aria-expanded={open}
          disabled={disabled}
          className="w-full justify-between font-normal"
        >
          <span className="truncate">{selectedLabel ?? placeholder}</span>
          <ChevronsUpDown className="ml-2 size-4 shrink-0 opacity-50" />
        </Button>
      </PopoverTrigger>
      <PopoverContent>
        <Command>
          <CommandInput placeholder={searchPlaceholder} />
          <CommandList>
            <CommandEmpty>{emptyText}</CommandEmpty>
            {options.map((option) => (
              <CommandItem
                key={option.value}
                value={option.label}
                onSelect={() => {
                  onValueChange(option.value);
                  setOpen(false);
                }}
              >
                <Check className={cn("mr-2 size-4", value === option.value ? "opacity-100" : "opacity-0")} />
                {option.label}
              </CommandItem>
            ))}
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}
```

- [ ] **Step 7: Verify it compiles**

Run: `cd /Users/nathananderson-tennant/Development/rubber-duck && bun run build 2>&1 | tail -20`
Expected: No TypeScript errors related to the new components.

- [ ] **Step 8: Commit**

```bash
git add src/components/ui/popover.tsx src/components/ui/command.tsx src/components/ui/combobox.tsx package.json bun.lock
# Also add src/lib/utils.ts if it was created
git commit -m "feat: add searchable combobox component (shadcn Popover + Command + cmdk)"
```

---

### Task 4: Add Jira atoms and types

**Files:**
- Modify: `src/features/settings/settings.atoms.ts`
- Modify: `src/features/ticket/ticket.types.ts`

- [ ] **Step 1: Add Jira atoms**

In `src/features/settings/settings.atoms.ts`, add:

```ts
export const jiraConfiguredAtom = atom(false);
export const jiraDefaultProjectAtom = atom<string | null>(null);
```

- [ ] **Step 2: Add frontend types**

In `src/features/ticket/ticket.types.ts`, add at the end:

```ts
export interface JiraProject {
  key: string;
  name: string;
}

export interface ExternalRef {
  platform: string;
  key: string;
  url: string;
}
```

- [ ] **Step 3: Commit**

```bash
git add src/features/settings/settings.atoms.ts src/features/ticket/ticket.types.ts
git commit -m "feat: add Jira atoms and frontend types"
```

---

### Task 5: Add Jira section to SettingsDialog

**Files:**
- Modify: `src/features/settings/SettingsDialog.tsx`

This is the largest frontend change. The Jira section goes below the existing model selector, inside the same `<div className="space-y-6 py-4">` container.

- [ ] **Step 1: Add imports**

At the top of `SettingsDialog.tsx`, add:

```tsx
import { Combobox } from "@/components/ui/combobox";
import type { ComboboxOption } from "@/components/ui/combobox";
import {
  jiraConfiguredAtom,
  jiraDefaultProjectAtom,
} from "./settings.atoms";
import type { JiraProject } from "@/features/ticket/ticket.types";
```

Update the existing `useAtom` import to also import `useSetAtom` if not already imported:

```tsx
import { useAtom, useSetAtom } from "jotai";
```

- [ ] **Step 2: Add Jira state variables**

Inside the `SettingsDialog` function, after the existing state declarations (after `const [saving, setSaving] = useState(false);`), add:

```tsx
const setJiraConfigured = useSetAtom(jiraConfiguredAtom);
const [jiraDefaultProject, setJiraDefaultProject] = useAtom(jiraDefaultProjectAtom);

const [jiraBaseUrl, setJiraBaseUrl] = useState("");
const [jiraAuthMethod, setJiraAuthMethod] = useState("basic");
const [jiraEmail, setJiraEmail] = useState("");
const [jiraToken, setJiraToken] = useState("");
const [showJiraToken, setShowJiraToken] = useState(false);
const [jiraHasToken, setJiraHasToken] = useState(false);

const [testingConnection, setTestingConnection] = useState(false);
const [connectionResult, setConnectionResult] = useState<{ ok: boolean; message: string } | null>(null);
const [jiraProjects, setJiraProjects] = useState<ComboboxOption[]>([]);
const [savingJira, setSavingJira] = useState(false);
```

- [ ] **Step 3: Load Jira config on dialog open**

In the existing `useEffect` that fires when `open` changes, add after the existing `invoke` calls:

```tsx
invoke<{ base_url: string; auth_method: string; email: string | null } | null>("get_jira_config").then((config) => {
  if (config) {
    setJiraBaseUrl(config.base_url);
    setJiraAuthMethod(config.auth_method);
    setJiraEmail(config.email ?? "");
  }
});
invoke<boolean>("has_jira_config").then((has) => {
  setJiraHasToken(has);
  setJiraConfigured(has);
});
invoke<string | null>("get_setting", { key: "jira.default_project" }).then((val) => {
  if (val) setJiraDefaultProject(val);
});
setJiraToken("");
setShowJiraToken(false);
setConnectionResult(null);
setJiraProjects([]);
```

- [ ] **Step 4: Add Test Connection handler**

After the existing `handleModelChange` function, add:

```tsx
async function handleTestConnection() {
  setTestingConnection(true);
  setConnectionResult(null);

  // Save config first so backend can read it
  await invoke("set_jira_config", {
    baseUrl: jiraBaseUrl.trim(),
    authMethod: jiraAuthMethod,
    email: jiraAuthMethod === "basic" ? jiraEmail.trim() : null,
  });
  if (jiraToken) {
    await invoke("set_jira_api_token", { key: jiraToken });
  }

  try {
    const user = await invoke<{ display_name: string }>("test_jira_connection");
    setConnectionResult({ ok: true, message: `Connected as ${user.display_name}` });
    setJiraHasToken(true);
    setJiraConfigured(true);

    const projects = await invoke<JiraProject[]>("get_jira_projects");
    setJiraProjects(projects.map((p) => ({ value: p.key, label: `${p.key} - ${p.name}` })));
  } catch (err) {
    setConnectionResult({ ok: false, message: String(err) });
  } finally {
    setTestingConnection(false);
  }
}
```

- [ ] **Step 5: Add Save Jira handler**

After `handleTestConnection`, add:

```tsx
async function handleSaveJira() {
  setSavingJira(true);
  await invoke("set_jira_config", {
    baseUrl: jiraBaseUrl.trim(),
    authMethod: jiraAuthMethod,
    email: jiraAuthMethod === "basic" ? jiraEmail.trim() : null,
  });
  if (jiraToken) {
    await invoke("set_jira_api_token", { key: jiraToken });
    setJiraHasToken(true);
  }
  if (jiraDefaultProject) {
    await invoke("set_setting", {
      key: "jira.default_project",
      value: jiraDefaultProject,
      category: "jira",
    });
  }
  const has = await invoke<boolean>("has_jira_config");
  setJiraConfigured(has);
  setSavingJira(false);
}
```

- [ ] **Step 6: Add Jira section JSX**

In the return JSX, after the closing `</div>` of the Model section (the `<div className="space-y-2">` containing the `<Select>`), add:

```tsx
{/* Jira */}
<div className="border-t border-border pt-4 space-y-3">
  <Label className="text-sm font-medium">Jira</Label>

  <div className="space-y-1">
    <Label className="text-xs text-muted-foreground">Base URL</Label>
    <Input
      value={jiraBaseUrl}
      onChange={(e) => setJiraBaseUrl(e.target.value)}
      placeholder="https://jira.company.com"
      className="h-8 text-sm"
    />
  </div>

  <div className="space-y-1">
    <Label className="text-xs text-muted-foreground">Auth Method</Label>
    <Select value={jiraAuthMethod} onValueChange={setJiraAuthMethod}>
      <SelectTrigger className="h-8 text-sm">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem value="basic">Basic (Cloud)</SelectItem>
        <SelectItem value="pat">PAT (Server/DC)</SelectItem>
      </SelectContent>
    </Select>
  </div>

  {jiraAuthMethod === "basic" && (
    <div className="space-y-1">
      <Label className="text-xs text-muted-foreground">Email</Label>
      <Input
        value={jiraEmail}
        onChange={(e) => setJiraEmail(e.target.value)}
        placeholder="you@company.com"
        className="h-8 text-sm"
      />
    </div>
  )}

  <div className="space-y-1">
    <Label className="text-xs text-muted-foreground">
      {jiraAuthMethod === "pat" ? "Personal Access Token" : "API Token"}
    </Label>
    <div className="flex gap-2">
      <Input
        type={showJiraToken ? "text" : "password"}
        value={jiraToken}
        onChange={(e) => setJiraToken(e.target.value)}
        placeholder={jiraHasToken ? "••••••••" : "Enter token"}
        className="h-8 text-sm flex-1"
      />
      <Button
        variant="ghost"
        size="sm"
        onClick={() => setShowJiraToken(!showJiraToken)}
        className="h-8"
      >
        {showJiraToken ? "Hide" : "Show"}
      </Button>
    </div>
  </div>

  <div className="space-y-1">
    <Button
      size="sm"
      variant="outline"
      onClick={handleTestConnection}
      disabled={testingConnection || !jiraBaseUrl.trim()}
      className="h-8"
    >
      {testingConnection ? "Testing..." : "Test Connection"}
    </Button>
    {connectionResult && (
      <p className={`text-xs ${connectionResult.ok ? "text-green-500" : "text-red-400"}`}>
        {connectionResult.ok ? "✓" : "✗"} {connectionResult.message}
      </p>
    )}
  </div>

  <div className="space-y-1">
    <Label className="text-xs text-muted-foreground">Default Project</Label>
    <Combobox
      options={jiraProjects}
      value={jiraDefaultProject ?? ""}
      onValueChange={setJiraDefaultProject}
      placeholder="Test connection to load projects"
      searchPlaceholder="Search projects..."
      emptyText="No projects found."
      disabled={jiraProjects.length === 0}
    />
  </div>

  <Button
    size="sm"
    onClick={handleSaveJira}
    disabled={savingJira || !jiraBaseUrl.trim()}
    className="h-8"
  >
    {savingJira ? "Saving..." : "Save"}
  </Button>
</div>
```

- [ ] **Step 7: Widen the dialog**

Change the `DialogContent` className from `"sm:max-w-md"` to `"sm:max-w-lg"` to give the Jira section more room:

```tsx
<DialogContent className="sm:max-w-lg max-h-[80vh] overflow-y-auto">
```

- [ ] **Step 8: Verify it compiles and renders**

Run: `bun tauri dev`
Open Settings dialog. Verify the Jira section appears below the model selector with all fields. Auth method toggle should show/hide the email field.

- [ ] **Step 9: Commit**

```bash
git add src/features/settings/SettingsDialog.tsx
git commit -m "feat: add Jira settings section with test connection and project picker"
```

---

### Task 6: Add `pushToJira` action

**Files:**
- Modify: `src/features/ticket/useTicketActions.ts`

- [ ] **Step 1: Add `pushToJira` function**

In `useTicketActions.ts`, add this function inside the `useTicketActions` hook, after `reorderTicket`:

```ts
async function pushToJira(ticketId: string, sessionId: string, projectKey: string) {
  const ticket = await invoke<Ticket>("push_ticket_to_jira", { ticketId, projectKey });
  await loadTickets(sessionId);
  return ticket;
}
```

- [ ] **Step 2: Add to return value**

Update the return statement:

```ts
return { loadTickets, createTicket, updateTicket, deleteTicket, reorderTicket, pushToJira };
```

- [ ] **Step 3: Commit**

```bash
git add src/features/ticket/useTicketActions.ts
git commit -m "feat: add pushToJira action to useTicketActions"
```

---

### Task 7: Add push UI to TicketList

**Files:**
- Modify: `src/features/ticket/TicketList.tsx`

- [ ] **Step 1: Add imports**

Add to the existing imports in `TicketList.tsx`:

```tsx
import { useAtom, useAtomValue } from "jotai";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Upload, Loader2, X, ExternalLink } from "lucide-react";
import { jiraConfiguredAtom, jiraDefaultProjectAtom, settingsOpenAtom } from "@/features/settings/settings.atoms";
import type { ExternalRef } from "./ticket.types";
```

Remove the existing standalone `import { useAtomValue } from "jotai"` line since it's now covered above. Keep the existing `invoke` import if already present (don't duplicate).

- [ ] **Step 2: Add Jira state inside the component**

Inside `TicketList`, after the existing state declarations, add:

```tsx
const [jiraConfigured, setJiraConfigured] = useAtom(jiraConfiguredAtom);
const jiraDefaultProject = useAtomValue(jiraDefaultProjectAtom);
const [, setSettingsOpen] = useAtom(settingsOpenAtom);
const [pushingId, setPushingId] = useState<string | null>(null);
const [pushError, setPushError] = useState<{ id: string; message: string } | null>(null);
```

Also destructure `pushToJira` from `useTicketActions`:

```tsx
const { loadTickets, createTicket, updateTicket, deleteTicket, reorderTicket, pushToJira } = useTicketActions();
```

- [ ] **Step 3: Check Jira config on mount**

The settings dialog sets `jiraConfiguredAtom` when it opens, but for the initial app load (before settings dialog has been opened), we need to check once. Add this `useEffect` after the existing one that loads tickets:

```tsx
useEffect(() => {
  invoke<boolean>("has_jira_config").then(setJiraConfigured);
}, []);
```

- [ ] **Step 4: Add push handler**

After the existing `handleMoveDown` function, add:

```tsx
async function handlePush(ticket: Ticket) {
  if (!jiraDefaultProject) {
    setPushError({ id: ticket.id, message: "Jira not configured." });
    return;
  }
  setPushingId(ticket.id);
  setPushError(null);
  try {
    await pushToJira(ticket.id, sessionId, jiraDefaultProject);
  } catch (err) {
    setPushError({ id: ticket.id, message: String(err) });
  } finally {
    setPushingId(null);
  }
}

function parseExternalRef(ref: string | null): ExternalRef | null {
  if (!ref) return null;
  try {
    return JSON.parse(ref);
  } catch {
    return null;
  }
}
```

- [ ] **Step 5: Add pushed state display and push icon to ticket JSX**

In the ticket map, find the `{/* Actions (visible on hover) */}` comment. We need to add:

1. A Jira key link below the title (when pushed)
2. A push icon in the hover actions (when not pushed)
3. A pushing spinner state
4. Inline error display

Replace the entire ticket card `<div key={ticket.id} ...>` content. After the top row div (the one with title + badges + actions), add this block before the expandable description section:

```tsx
{/* Jira status / push error */}
{(() => {
  const extRef = parseExternalRef(ticket.external_ref);
  if (pushingId === ticket.id) {
    return (
      <p className="text-[10px] text-muted-foreground flex items-center gap-1 mt-0.5">
        <Loader2 className="size-3 animate-spin" /> Pushing...
      </p>
    );
  }
  if (extRef) {
    return (
      <button
        onClick={() => openUrl(extRef.url)}
        className="text-[10px] text-blue-400 hover:text-blue-300 flex items-center gap-1 mt-0.5"
      >
        {extRef.key} <ExternalLink className="size-2.5" />
      </button>
    );
  }
  if (pushError?.id === ticket.id) {
    return (
      <div className="flex items-center gap-1 mt-0.5">
        {pushError.message === "Jira not configured." ? (
          <button
            onClick={() => setSettingsOpen(true)}
            className="text-[10px] text-yellow-400 hover:text-yellow-300"
          >
            ⚠ Jira not configured. Open Settings
          </button>
        ) : (
          <span className="text-[10px] text-red-400">✗ {pushError.message}</span>
        )}
        <button onClick={() => setPushError(null)} className="text-muted-foreground hover:text-foreground">
          <X className="size-3" />
        </button>
      </div>
    );
  }
  return null;
})()}
```

- [ ] **Step 6: Add push icon to hover actions**

In the hover actions div (`<div className="hidden group-hover:flex items-center gap-0.5">`), add the push button between the ArrowDown and Trash2 buttons:

```tsx
{jiraConfigured && !parseExternalRef(ticket.external_ref) && pushingId !== ticket.id && (
  <button
    onClick={() => handlePush(ticket)}
    className="p-0.5 text-muted-foreground hover:text-blue-400"
    title="Push to Jira"
  >
    <Upload className="size-3" />
  </button>
)}
```

- [ ] **Step 7: Verify it compiles and renders**

Run: `bun tauri dev`
Open a session with tickets. Verify:
- If Jira is not configured: no push icon appears
- Configure Jira in settings, return to tickets: push icon appears on hover
- Clicking push shows spinner, then Jira key on success or error on failure

- [ ] **Step 8: Commit**

```bash
git add src/features/ticket/TicketList.tsx
git commit -m "feat: add push to Jira button and status display on tickets"
```

---

### Task 8: Manual integration test

**Files:** None (testing only)

- [ ] **Step 1: Run all Rust tests**

Run: `cd src-tauri && cargo test 2>&1`
Expected: All tests pass (53 total — 51 existing + 2 new get_projects tests).

- [ ] **Step 2: Run frontend build check**

Run: `bun run build 2>&1 | tail -20`
Expected: Builds with no errors.

- [ ] **Step 3: Full flow test with `bun tauri dev`**

Test the following flow:
1. Open Settings → Jira section visible
2. Enter base URL, select auth method, enter credentials
3. Click "Test Connection" → see spinner, then success message with username
4. Project combobox populates → search and select a project
5. Click Save
6. Go to a session with tickets
7. Hover a ticket → push icon visible
8. Click push icon → spinner → Jira key appears as link
9. Click Jira key → opens in browser
10. Push icon no longer shows for pushed ticket

- [ ] **Step 4: Test error flows**

1. Enter wrong credentials → Test Connection → auth error message
2. Enter wrong URL → Test Connection → network error message
3. Push to invalid project → inline error on ticket, dismissible
4. No Jira config → click push → "Jira not configured" with settings link

- [ ] **Step 5: Update PLAN.md**

Mark Task 2.2 items as complete in `docs/PLAN.md`.

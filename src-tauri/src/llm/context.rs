use crate::jira::model::JiraIssueContext;
use crate::rag::model::RetrievedChunk;
use crate::repo_context::model::RepoFileContext;
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub enum ChatMode {
    Assist,
    Grill,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

const SYSTEM_PROMPT: &str = "You are a technical planning assistant embedded in a brainstorming tool called rubber-duck. You help the user think through technical problems and produce well-structured kanban tickets and specifications.

Key context: the user works in an agentic development environment where AI coding agents (Claude Code, Cursor, Copilot, etc.) implement tickets. This means:
- Tickets should be written so an agent can pick them up and start coding without asking clarifying questions.
- Acceptance criteria should distinguish what can be verified automatically ([auto]: tests pass, types check, lint clean) vs. what needs human judgment ([human]: UX feels right, business logic is correct).
- Specs should be precise about file paths, APIs, and interfaces — agents work better with concrete references than vague descriptions.
- Don't over-specify implementation steps. Describe WHAT and WHY, let the agent figure out HOW.

When asked to create tickets, produce structured JSON that the app can parse. When asked to review or improve, be specific and actionable. Reference actual content from the session notes.";

const GRILL_PROMPT: &str = "You are a critical technical reviewer for an agentic development workflow. Your job is to find gaps, ambiguities, missing edge cases, and unstated assumptions in the user's planning session — especially things that would cause an AI coding agent to make wrong decisions.

Read the current notes and tickets carefully. Then ask ONE focused question at a time. Be specific — reference actual content from their notes. Don't be generic.

Focus areas:
- Are tickets specific enough for an agent to implement without asking questions?
- Are acceptance criteria verifiable (automated or human-checked)?
- Is there missing context that would lead an agent astray (e.g. undocumented constraints, existing code patterns to follow)?
- Are there security, performance, or data integrity risks the notes don't address?
- Is scope clearly bounded? What's in vs. out?

Examples of good questions:
- \"You mention migrating the CDC pipeline but there's no ticket for schema migration — an agent would miss this dependency.\"
- \"The acceptance criteria for ticket #3 say 'handles errors gracefully' — what does that mean specifically? An agent needs concrete error cases to implement and test.\"
- \"I see nothing about rollback strategy. What happens if this deployment fails halfway?\"
- \"This ticket says 'update the auth flow' but doesn't reference which files or APIs. An agent would need to search the entire codebase — can you @mention the relevant files?\"

Do not provide solutions unless asked. Your job is to find the holes.";

const MAX_CONVERSATION_MESSAGES: usize = 40;

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

pub fn assemble_context(
    mode: &ChatMode,
    session_context: &str,
    note_content: &str,
    tickets: &[(String, String, String, String)], // (title, type, priority, description)
    jira_issues: &[JiraIssueContext],
    repo_summaries: &[String],
    mentioned_files: &[RepoFileContext],
    retrieved_chunks: &[RetrievedChunk],
    conversation: &[(String, String)], // (role, content)
) -> Vec<ChatMessage> {
    let base_prompt = match mode {
        ChatMode::Assist => SYSTEM_PROMPT,
        ChatMode::Grill => GRILL_PROMPT,
    };
    let mut system_parts = vec![base_prompt.to_string()];

    if !session_context.is_empty() {
        system_parts.push(format!("## Session Context\n{session_context}"));
    }

    if !note_content.is_empty() {
        system_parts.push(format!("## Brain Dump Notes\n{note_content}"));
    }

    if !tickets.is_empty() {
        let mut ticket_text = String::from("## Current Tickets\n");
        for (title, ticket_type, priority, description) in tickets {
            let desc_preview = if description.len() > 100 {
                let truncated: String = description.chars().take(100).collect();
                format!("{truncated}...")
            } else {
                description.clone()
            };
            ticket_text.push_str(&format!("- {title} ({ticket_type}, {priority}) — {desc_preview}\n"));
        }
        system_parts.push(ticket_text);
    }

    if !jira_issues.is_empty() {
        let mut jira_text = String::from("## Referenced Jira Tickets\n");
        for issue in jira_issues {
            let desc_preview = if issue.description.is_empty() {
                String::new()
            } else {
                format!(" — {}", issue.description)
            };
            jira_text.push_str(&format!(
                "- {} ({}, {}, {}): {}{}\n",
                issue.key, issue.issue_type, issue.status, issue.priority, issue.summary, desc_preview
            ));
        }
        system_parts.push(jira_text);
    }

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

    if !retrieved_chunks.is_empty() {
        let mut rag_text = String::from(
            "## Retrieved Code Context\nThe following code snippets were automatically retrieved from attached repositories as potentially relevant to this conversation. Use them to give accurate, grounded answers.\n\n",
        );
        for chunk in retrieved_chunks {
            let lang = if chunk.file_path.ends_with(".rs") {
                "rust"
            } else if chunk.file_path.ends_with(".ts") || chunk.file_path.ends_with(".tsx") {
                "typescript"
            } else if chunk.file_path.ends_with(".py") {
                "python"
            } else if chunk.file_path.ends_with(".go") {
                "go"
            } else if chunk.file_path.ends_with(".java") {
                "java"
            } else if chunk.file_path.ends_with(".cs") {
                "csharp"
            } else if chunk.file_path.ends_with(".js") || chunk.file_path.ends_with(".jsx") {
                "javascript"
            } else {
                ""
            };
            rag_text.push_str(&format!(
                "### {}/{} (lines {}-{})\n```{lang}\n{}\n```\n\n",
                chunk.repo_name, chunk.file_path, chunk.start_line, chunk.end_line, chunk.content,
            ));
        }
        system_parts.push(rag_text);
    }

    let mut messages = vec![ChatMessage {
        role: "system".to_string(),
        content: system_parts.join("\n\n"),
    }];

    let conv_slice = if conversation.len() > MAX_CONVERSATION_MESSAGES {
        &conversation[conversation.len() - MAX_CONVERSATION_MESSAGES..]
    } else {
        conversation
    };

    for (role, content) in conv_slice {
        messages.push(ChatMessage {
            role: match role.as_str() {
                "User" => "user",
                "Assistant" => "assistant",
                _ => "system",
            }
            .to_string(),
            content: content.clone(),
        });
    }

    messages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_message_includes_prompt() {
        let messages = assemble_context(&ChatMode::Assist, "", "", &[], &[], &[], &[], &[], &[]);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "system");
        assert!(messages[0].content.contains("rubber-duck"));
    }

    #[test]
    fn includes_session_context() {
        let messages = assemble_context(&ChatMode::Assist, "We are migrating the auth service", "", &[], &[], &[], &[], &[], &[]);
        assert!(messages[0].content.contains("migrating the auth service"));
    }

    #[test]
    fn includes_note_content() {
        let messages = assemble_context(&ChatMode::Assist, "", "# My brainstorm\nSome ideas here", &[], &[], &[], &[], &[], &[]);
        assert!(messages[0].content.contains("My brainstorm"));
    }

    #[test]
    fn includes_tickets() {
        let tickets = vec![
            ("Fix login".to_string(), "Task".to_string(), "High".to_string(), "Auth is broken".to_string()),
        ];
        let messages = assemble_context(&ChatMode::Assist, "", "", &tickets, &[], &[], &[], &[], &[]);
        assert!(messages[0].content.contains("Fix login"));
        assert!(messages[0].content.contains("Task"));
    }

    #[test]
    fn includes_conversation_as_separate_messages() {
        let conversation = vec![
            ("User".to_string(), "Break this into tickets".to_string()),
            ("Assistant".to_string(), "Here are 3 tickets...".to_string()),
        ];
        let messages = assemble_context(&ChatMode::Assist, "", "", &[], &[], &[], &[], &[], &conversation);
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[1].role, "user");
        assert_eq!(messages[2].role, "assistant");
    }

    #[test]
    fn caps_conversation_at_40_messages() {
        let conversation: Vec<(String, String)> = (0..60)
            .map(|i| {
                let role = if i % 2 == 0 { "User" } else { "Assistant" };
                (role.to_string(), format!("Message {i}"))
            })
            .collect();
        let messages = assemble_context(&ChatMode::Assist, "", "", &[], &[], &[], &[], &[], &conversation);
        assert_eq!(messages.len(), 41); // system + 40 conversation
        assert!(messages[1].content.contains("Message 20")); // keeps LAST 40
    }

    #[test]
    fn grill_mode_uses_different_prompt() {
        let messages = assemble_context(&ChatMode::Grill, "", "", &[], &[], &[], &[], &[], &[]);
        assert_eq!(messages.len(), 1);
        assert!(messages[0].content.contains("critical technical reviewer"));
        assert!(!messages[0].content.contains("rubber-duck"));
    }

    #[test]
    fn truncates_long_descriptions_without_utf8_panic() {
        // '€' is 3 bytes (E2 82 AC); 40 repetitions = 120 bytes total
        // byte index 100 falls inside a '€' char — byte-slicing [..100] would panic
        let description = "\u{20ac}".repeat(40);
        let tickets = vec![(
            "Test".to_string(),
            "Task".to_string(),
            "High".to_string(),
            description,
        )];
        let messages = assemble_context(&ChatMode::Assist, "", "", &tickets, &[], &[], &[], &[], &[]);
        assert!(messages[0].content.contains("Test"));
        assert!(messages[0].content.contains("..."));
    }

    #[test]
    fn includes_jira_issues_in_context() {
        let jira_issues = vec![
            JiraIssueContext {
                key: "FRONT-42".to_string(),
                summary: "Fix login timeout".to_string(),
                status: "In Progress".to_string(),
                issue_type: "Bug".to_string(),
                priority: "High".to_string(),
                description: "SSO times out after 30s".to_string(),
            },
        ];
        let messages = assemble_context(&ChatMode::Assist, "", "", &[], &jira_issues, &[], &[], &[], &[]);
        assert!(messages[0].content.contains("Referenced Jira Tickets"));
        assert!(messages[0].content.contains("FRONT-42"));
        assert!(messages[0].content.contains("Fix login timeout"));
        assert!(messages[0].content.contains("SSO times out"));
    }

    #[test]
    fn empty_jira_issues_omits_section() {
        let messages = assemble_context(&ChatMode::Assist, "", "", &[], &[], &[], &[], &[], &[]);
        assert!(!messages[0].content.contains("Referenced Jira Tickets"));
    }

    #[test]
    fn includes_repo_summaries() {
        let summaries = vec!["my-repo (245 files. Top types: .ts(120), .tsx(80). Key dirs: src/, tests/)".to_string()];
        let messages = assemble_context(&ChatMode::Assist, "", "", &[], &[], &summaries, &[], &[], &[]);
        assert!(messages[0].content.contains("Attached Repositories"));
        assert!(messages[0].content.contains("245 files"));
    }

    #[test]
    fn includes_mentioned_files() {
        let files = vec![RepoFileContext {
            display: "my-repo/src/main.ts".to_string(),
            content: "console.log('hello')".to_string(),
        }];
        let messages = assemble_context(&ChatMode::Assist, "", "", &[], &[], &[], &files, &[], &[]);
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

    #[test]
    fn includes_retrieved_code_chunks() {
        use crate::rag::model::RetrievedChunk;
        let chunks = vec![
            RetrievedChunk {
                file_path: "src/auth.rs".to_string(),
                repo_name: "my-repo".to_string(),
                start_line: 42,
                end_line: 78,
                content: "fn authenticate() { verify(); }".to_string(),
                score: 0.5,
            },
        ];
        let messages = assemble_context(
            &ChatMode::Assist, "", "", &[], &[], &[], &[], &chunks, &[],
        );
        assert!(messages[0].content.contains("Retrieved Code Context"));
        assert!(messages[0].content.contains("my-repo/src/auth.rs"));
        assert!(messages[0].content.contains("authenticate"));
    }

    #[test]
    fn empty_retrieved_chunks_omits_section() {
        let messages = assemble_context(
            &ChatMode::Assist, "", "", &[], &[], &[], &[], &[], &[],
        );
        assert!(!messages[0].content.contains("Retrieved Code Context"));
    }
}

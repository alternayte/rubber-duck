use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

const SYSTEM_PROMPT: &str = "You are a technical planning assistant embedded in a local brainstorming tool called rubber-duck. Your job is to help the user think through technical problems and produce well-structured work items.

When asked to create tickets, produce structured JSON that the app can parse. When asked to review or improve, be specific and actionable.";

const MAX_CONVERSATION_MESSAGES: usize = 40;

pub fn assemble_context(
    session_context: &str,
    note_content: &str,
    tickets: &[(String, String, String, String)], // (title, type, priority, description)
    conversation: &[(String, String)], // (role, content)
) -> Vec<ChatMessage> {
    let mut system_parts = vec![SYSTEM_PROMPT.to_string()];

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
                format!("{}...", &description[..100])
            } else {
                description.clone()
            };
            ticket_text.push_str(&format!("- {title} ({ticket_type}, {priority}) — {desc_preview}\n"));
        }
        system_parts.push(ticket_text);
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
        let messages = assemble_context("", "", &[], &[]);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "system");
        assert!(messages[0].content.contains("rubber-duck"));
    }

    #[test]
    fn includes_session_context() {
        let messages = assemble_context("We are migrating the auth service", "", &[], &[]);
        assert!(messages[0].content.contains("migrating the auth service"));
    }

    #[test]
    fn includes_note_content() {
        let messages = assemble_context("", "# My brainstorm\nSome ideas here", &[], &[]);
        assert!(messages[0].content.contains("My brainstorm"));
    }

    #[test]
    fn includes_tickets() {
        let tickets = vec![
            ("Fix login".to_string(), "Task".to_string(), "High".to_string(), "Auth is broken".to_string()),
        ];
        let messages = assemble_context("", "", &tickets, &[]);
        assert!(messages[0].content.contains("Fix login"));
        assert!(messages[0].content.contains("Task"));
    }

    #[test]
    fn includes_conversation_as_separate_messages() {
        let conversation = vec![
            ("User".to_string(), "Break this into tickets".to_string()),
            ("Assistant".to_string(), "Here are 3 tickets...".to_string()),
        ];
        let messages = assemble_context("", "", &[], &conversation);
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
        let messages = assemble_context("", "", &[], &conversation);
        assert_eq!(messages.len(), 41); // system + 40 conversation
        assert!(messages[1].content.contains("Message 20")); // keeps LAST 40
    }
}

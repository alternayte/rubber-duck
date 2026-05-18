use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};

use super::context::ChatMessage;

const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

#[derive(Debug, Serialize)]
struct CompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct StreamResponse {
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: Delta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Delta {
    content: Option<String>,
}

#[derive(Debug)]
pub enum StreamEvent {
    Chunk(String),
    Done(String),
    Error(String),
}

pub async fn stream_completion(
    api_key: &str,
    model: &str,
    messages: Vec<ChatMessage>,
    tx: mpsc::Sender<StreamEvent>,
    cancel: CancellationToken,
) {
    let result = stream_inner(api_key, model, messages, &tx, &cancel).await;
    if let Err(e) = result {
        let _ = tx.send(StreamEvent::Error(e.to_string())).await;
    }
}

async fn stream_inner(
    api_key: &str,
    model: &str,
    messages: Vec<ChatMessage>,
    tx: &mpsc::Sender<StreamEvent>,
    cancel: &CancellationToken,
) -> AppResult<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| AppError::Other(format!("Failed to create HTTP client: {e}")))?;
    let response = client
        .post(OPENROUTER_URL)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("HTTP-Referer", "rubber-duck")
        .header("X-Title", "rubber-duck")
        .json(&CompletionRequest {
            model: model.to_string(),
            messages,
            stream: true,
        })
        .send()
        .await
        .map_err(|e| AppError::Other(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "unknown error".to_string());
        return Err(AppError::Other(format!("OpenRouter {status}: {body}")));
    }

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut full_content = String::new();

    loop {
        let chunk = tokio::select! {
            _ = cancel.cancelled() => {
                let _ = tx.send(StreamEvent::Done(full_content)).await;
                return Ok(());
            }
            chunk = stream.next() => chunk,
        };

        let Some(chunk) = chunk else { break; };
        let chunk = chunk.map_err(|e| AppError::Other(e.to_string()))?;
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim().to_string();
            buffer = buffer[pos + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };

            if data == "[DONE]" {
                let _ = tx.send(StreamEvent::Done(full_content.clone())).await;
                return Ok(());
            }

            match serde_json::from_str::<StreamResponse>(data) {
                Ok(parsed) => {
                    if let Some(choice) = parsed.choices.first() {
                        if let Some(content) = &choice.delta.content {
                            full_content.push_str(content);
                            let _ = tx.send(StreamEvent::Chunk(content.clone())).await;
                        }
                        if choice.finish_reason.is_some() {
                            let _ = tx.send(StreamEvent::Done(full_content.clone())).await;
                            return Ok(());
                        }
                    }
                }
                Err(_) => continue,
            }
        }
    }

    let _ = tx.send(StreamEvent::Done(full_content)).await;
    Ok(())
}

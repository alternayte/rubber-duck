use reqwest::Client;
use std::time::Duration;

use crate::error::{AppError, AppResult};
use super::model::{JiraUser, JiraErrorResponse};

pub struct JiraClient {
    client: Client,
    base_url: String,
    email: String,
    api_token: String,
}

impl JiraClient {
    pub fn new(base_url: &str, email: &str, api_token: &str) -> AppResult<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            email: email.to_string(),
            api_token: api_token.to_string(),
        })
    }

    pub async fn test_connection(&self) -> AppResult<JiraUser> {
        let url = format!("{}/rest/api/2/myself", self.base_url);
        let response = self
            .client
            .get(&url)
            .basic_auth(&self.email, Some(&self.api_token))
            .send()
            .await?;

        if response.status().is_success() {
            let user: JiraUser = response.json().await?;
            return Ok(user);
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let message = parse_jira_error(&body, status.as_u16());
        Err(AppError::Other(message))
    }
}

fn parse_jira_error(body: &str, status: u16) -> String {
    if let Ok(err) = serde_json::from_str::<JiraErrorResponse>(body) {
        let messages: Vec<&str> = err
            .error_messages
            .iter()
            .map(|s| s.as_str())
            .chain(err.errors.values().map(|s| s.as_str()))
            .collect();
        if !messages.is_empty() {
            return messages.join("; ");
        }
    }

    match status {
        401 => "Authentication failed — check your email and API token".to_string(),
        403 => "Permission denied — check your Jira permissions".to_string(),
        404 => "Jira site not found — check your base URL".to_string(),
        _ => format!("Jira API error (HTTP {status})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connection_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/rest/api/2/myself")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"accountId":"abc123","displayName":"Test User"}"#)
            .create_async()
            .await;

        let client = JiraClient::new(&server.url(), "test@example.com", "token").unwrap();
        let user = client.test_connection().await.unwrap();

        assert_eq!(user.display_name, "Test User");
        assert_eq!(user.account_id, "abc123");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_connection_auth_failure() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/rest/api/2/myself")
            .with_status(401)
            .with_header("content-type", "application/json")
            .with_body(r#"{"errorMessages":["You do not have the permission"],"errors":{}}"#)
            .create_async()
            .await;

        let client = JiraClient::new(&server.url(), "bad@example.com", "wrong").unwrap();
        let result = client.test_connection().await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("permission"), "Expected permission error, got: {err}");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_connection_fallback_error_message() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/rest/api/2/myself")
            .with_status(401)
            .with_body("not json")
            .create_async()
            .await;

        let client = JiraClient::new(&server.url(), "bad@example.com", "wrong").unwrap();
        let result = client.test_connection().await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Authentication failed"),
            "Expected auth fallback message, got: {err}"
        );
        mock.assert_async().await;
    }
}

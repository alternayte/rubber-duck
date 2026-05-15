use reqwest::Client;
use std::time::Duration;

use crate::error::{AppError, AppResult};
use super::model::{
    CreateIssueFields, CreateIssueRequest, IssueTypeRef, ProjectRef,
    JiraAuth, JiraErrorResponse, JiraUser, CreateIssueResponse, JiraProject,
};
use crate::ticket::model::ExternalRef;

pub struct JiraClient {
    client: Client,
    base_url: String,
    auth: JiraAuth,
}

impl JiraClient {
    pub fn new(base_url: &str, auth: JiraAuth) -> AppResult<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            auth,
        })
    }

    fn apply_auth(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.auth {
            JiraAuth::Basic { email, api_token } => builder.basic_auth(email, Some(api_token)),
            JiraAuth::Pat(token) => builder.bearer_auth(token),
        }
    }

    pub async fn test_connection(&self) -> AppResult<JiraUser> {
        let url = format!("{}/rest/api/2/myself", self.base_url);
        let response = self
            .apply_auth(self.client.get(&url))
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

    pub async fn create_issue(
        &self,
        project_key: &str,
        summary: &str,
        description: &str,
        issue_type: &str,
    ) -> AppResult<ExternalRef> {
        let url = format!("{}/rest/api/2/issue", self.base_url);
        let body = CreateIssueRequest {
            fields: CreateIssueFields {
                project: ProjectRef {
                    key: project_key.to_string(),
                },
                summary: summary.to_string(),
                description: description.to_string(),
                issuetype: IssueTypeRef {
                    name: issue_type.to_string(),
                },
            },
        };

        let response = self
            .apply_auth(self.client.post(&url))
            .json(&body)
            .send()
            .await?;

        if response.status().is_success() {
            let created: CreateIssueResponse = response.json().await?;
            let browse_url = format!("{}/browse/{}", self.base_url, created.key);
            return Ok(ExternalRef {
                platform: "jira".to_string(),
                key: created.key,
                url: browse_url,
            });
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        let message = parse_jira_error(&body, status.as_u16());
        Err(AppError::Other(message))
    }

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
        401 => "Authentication failed — check your credentials".to_string(),
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

        let client = JiraClient::new(&server.url(), JiraAuth::Basic {
            email: "test@example.com".to_string(),
            api_token: "token".to_string(),
        }).unwrap();
        let user = client.test_connection().await.unwrap();

        assert_eq!(user.display_name, "Test User");
        assert_eq!(user.account_id, Some("abc123".to_string()));
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

        let client = JiraClient::new(&server.url(), JiraAuth::Basic {
            email: "bad@example.com".to_string(),
            api_token: "wrong".to_string(),
        }).unwrap();
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

        let client = JiraClient::new(&server.url(), JiraAuth::Basic {
            email: "bad@example.com".to_string(),
            api_token: "wrong".to_string(),
        }).unwrap();
        let result = client.test_connection().await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("check your credentials"),
            "Expected auth fallback message, got: {err}"
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn create_issue_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/rest/api/2/issue")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(r#"{"id":"10001","key":"PROJ-42","self":"https://site.atlassian.net/rest/api/2/issue/10001"}"#)
            .create_async()
            .await;

        let client = JiraClient::new(&server.url(), JiraAuth::Basic {
            email: "test@example.com".to_string(),
            api_token: "token".to_string(),
        }).unwrap();
        let ext_ref = client
            .create_issue("PROJ", "Fix the bug", "It's broken", "Bug")
            .await
            .unwrap();

        assert_eq!(ext_ref.platform, "jira");
        assert_eq!(ext_ref.key, "PROJ-42");
        assert!(ext_ref.url.contains("/browse/PROJ-42"));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn create_issue_validation_error() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/rest/api/2/issue")
            .with_status(400)
            .with_header("content-type", "application/json")
            .with_body(r#"{"errorMessages":[],"errors":{"project":"project is required"}}"#)
            .create_async()
            .await;

        let client = JiraClient::new(&server.url(), JiraAuth::Basic {
            email: "test@example.com".to_string(),
            api_token: "token".to_string(),
        }).unwrap();
        let result = client.create_issue("BAD", "Title", "Desc", "Task").await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("project is required"),
            "Expected field error, got: {err}"
        );
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn create_issue_auth_failure() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/rest/api/2/issue")
            .with_status(401)
            .with_body("")
            .create_async()
            .await;

        let client = JiraClient::new(&server.url(), JiraAuth::Basic {
            email: "bad@example.com".to_string(),
            api_token: "wrong".to_string(),
        }).unwrap();
        let result = client.create_issue("PROJ", "Title", "Desc", "Task").await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Authentication failed"),
            "Expected auth error, got: {err}"
        );
        mock.assert_async().await;
    }

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

    #[tokio::test]
    async fn test_connection_with_pat() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/rest/api/2/myself")
            .match_header("authorization", "Bearer my-pat-token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"displayName":"Server User","name":"jdoe"}"#)
            .create_async()
            .await;

        let client = JiraClient::new(
            &server.url(),
            JiraAuth::Pat("my-pat-token".to_string()),
        )
        .unwrap();
        let user = client.test_connection().await.unwrap();

        assert_eq!(user.display_name, "Server User");
        assert_eq!(user.name, Some("jdoe".to_string()));
        assert_eq!(user.account_id, None);
        mock.assert_async().await;
    }
}

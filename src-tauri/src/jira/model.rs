use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum JiraAuth {
    Basic { email: String, api_token: String },
    Pat(String),
}

#[derive(Debug, Serialize)]
pub struct CreateIssueRequest {
    pub fields: CreateIssueFields,
}

#[derive(Debug, Serialize)]
pub struct CreateIssueFields {
    pub project: ProjectRef,
    pub summary: String,
    pub description: String,
    pub issuetype: IssueTypeRef,
}

#[derive(Debug, Serialize)]
pub struct ProjectRef {
    pub key: String,
}

#[derive(Debug, Serialize)]
pub struct IssueTypeRef {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JiraUser {
    #[serde(default)]
    pub account_id: Option<String>,
    pub display_name: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateIssueResponse {
    pub id: String,
    pub key: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JiraErrorResponse {
    #[serde(default)]
    pub error_messages: Vec<String>,
    #[serde(default)]
    pub errors: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraProject {
    pub key: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JiraIssueContext {
    pub key: String,
    pub summary: String,
    pub status: String,
    pub issue_type: String,
    pub priority: String,
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct JiraIssueResponse {
    pub key: String,
    pub fields: JiraIssueFields,
}

#[derive(Debug, Deserialize)]
pub struct JiraIssueFields {
    pub summary: String,
    pub status: JiraNameField,
    pub issuetype: JiraNameField,
    pub priority: Option<JiraNameField>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct JiraNameField {
    pub name: String,
}

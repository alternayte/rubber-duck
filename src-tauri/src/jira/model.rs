use serde::{Deserialize, Serialize};

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
    pub account_id: String,
    pub display_name: String,
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

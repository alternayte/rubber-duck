use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticket {
    pub id: String,
    pub session_id: String,
    pub title: String,
    pub description: String,
    pub acceptance_criteria: String,
    pub estimate: Option<String>,
    pub priority: String,
    pub ticket_type: String,
    pub labels: Vec<String>,
    pub parent_id: Option<String>,
    pub dependencies: Vec<String>,
    pub status: String,
    pub external_ref: Option<String>,
    pub sort_order: i32,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTicketParams {
    pub session_id: String,
    pub title: String,
    pub description: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub estimate: Option<String>,
    pub priority: Option<String>,
    pub ticket_type: Option<String>,
    pub labels: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTicketParams {
    pub title: Option<String>,
    pub description: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub estimate: Option<String>,
    pub priority: Option<String>,
    pub ticket_type: Option<String>,
    pub labels: Option<Vec<String>>,
    pub status: Option<String>,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalRef {
    pub platform: String,
    pub key: String,
    pub url: String,
}

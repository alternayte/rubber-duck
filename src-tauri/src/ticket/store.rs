use rusqlite::{params, Connection};

use crate::error::AppResult;

use super::model::{CreateTicketParams, Ticket, UpdateTicketParams};

fn row_to_ticket(row: &rusqlite::Row) -> rusqlite::Result<Ticket> {
    let labels_json: String = row.get(8)?;
    let deps_json: String = row.get(10)?;
    Ok(Ticket {
        id: row.get(0)?,
        session_id: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        acceptance_criteria: row.get(4)?,
        estimate: row.get(5)?,
        priority: row.get(6)?,
        ticket_type: row.get(7)?,
        labels: serde_json::from_str(&labels_json).unwrap_or_default(),
        parent_id: row.get(9)?,
        dependencies: serde_json::from_str(&deps_json).unwrap_or_default(),
        status: row.get(11)?,
        external_ref: row.get(12)?,
        sort_order: row.get(13)?,
        created_at: row.get(14)?,
    })
}

const SELECT_COLS: &str =
    "id, session_id, title, description, acceptance_criteria, estimate, priority, ticket_type,
     labels, parent_id, dependencies, status, external_ref, sort_order, created_at";

pub fn create(conn: &Connection, params: &CreateTicketParams) -> AppResult<Ticket> {
    let id = uuid::Uuid::new_v4().to_string();
    let description = params.description.as_deref().unwrap_or("");
    let acceptance_criteria = params.acceptance_criteria.as_deref().unwrap_or("");
    let priority = params.priority.as_deref().unwrap_or("Medium");
    let ticket_type = params.ticket_type.as_deref().unwrap_or("Task");
    let labels_json = serde_json::to_string(
        &params.labels.as_deref().unwrap_or(&[]),
    )
    .unwrap_or_else(|_| "[]".to_string());

    conn.execute(
        "INSERT INTO tickets
            (id, session_id, title, description, acceptance_criteria, estimate,
             priority, ticket_type, labels)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            id,
            params.session_id,
            params.title,
            description,
            acceptance_criteria,
            params.estimate,
            priority,
            ticket_type,
            labels_json,
        ],
    )?;
    get(conn, &id)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Ticket> {
    let ticket = conn.query_row(
        &format!("SELECT {} FROM tickets WHERE id = ?1", SELECT_COLS),
        params![id],
        row_to_ticket,
    )?;
    Ok(ticket)
}

pub fn list_by_session(conn: &Connection, session_id: &str) -> AppResult<Vec<Ticket>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {} FROM tickets WHERE session_id = ?1 ORDER BY sort_order ASC",
        SELECT_COLS
    ))?;
    let tickets = stmt
        .query_map(params![session_id], row_to_ticket)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(tickets)
}

pub fn update(conn: &Connection, id: &str, params: &UpdateTicketParams) -> AppResult<Ticket> {
    let mut sets: Vec<&'static str> = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

    if let Some(title) = &params.title {
        sets.push("title = ?");
        values.push(Box::new(title.clone()));
    }
    if let Some(description) = &params.description {
        sets.push("description = ?");
        values.push(Box::new(description.clone()));
    }
    if let Some(acceptance_criteria) = &params.acceptance_criteria {
        sets.push("acceptance_criteria = ?");
        values.push(Box::new(acceptance_criteria.clone()));
    }
    if let Some(estimate) = &params.estimate {
        sets.push("estimate = ?");
        values.push(Box::new(estimate.clone()));
    }
    if let Some(priority) = &params.priority {
        sets.push("priority = ?");
        values.push(Box::new(priority.clone()));
    }
    if let Some(ticket_type) = &params.ticket_type {
        sets.push("ticket_type = ?");
        values.push(Box::new(ticket_type.clone()));
    }
    if let Some(labels) = &params.labels {
        sets.push("labels = ?");
        values.push(Box::new(
            serde_json::to_string(labels).unwrap_or_else(|_| "[]".to_string()),
        ));
    }
    if let Some(status) = &params.status {
        sets.push("status = ?");
        values.push(Box::new(status.clone()));
    }
    if let Some(parent_id) = &params.parent_id {
        sets.push("parent_id = ?");
        values.push(Box::new(parent_id.clone()));
    }

    if sets.is_empty() {
        return get(conn, id);
    }

    values.push(Box::new(id.to_string()));
    let sql = format!("UPDATE tickets SET {} WHERE id = ?", sets.join(", "));
    conn.execute(&sql, rusqlite::params_from_iter(values))?;
    get(conn, id)
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM tickets WHERE id = ?1", params![id])?;
    Ok(())
}

pub fn reorder(conn: &Connection, id: &str, new_sort_order: i32) -> AppResult<()> {
    conn.execute(
        "UPDATE tickets SET sort_order = ?1 WHERE id = ?2",
        params![new_sort_order, id],
    )?;
    Ok(())
}

pub fn set_parent(conn: &Connection, id: &str, parent_id: Option<&str>) -> AppResult<Ticket> {
    conn.execute(
        "UPDATE tickets SET parent_id = ?1 WHERE id = ?2",
        params![parent_id, id],
    )?;
    get(conn, id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::session::store as session_store;

    fn test_db() -> Database {
        Database::open_in_memory().unwrap()
    }

    fn make_session(conn: &Connection) -> String {
        let session = session_store::create(conn, "Test Session").unwrap();
        session.id
    }

    fn minimal_params(session_id: &str, title: &str) -> CreateTicketParams {
        CreateTicketParams {
            session_id: session_id.to_string(),
            title: title.to_string(),
            description: None,
            acceptance_criteria: None,
            estimate: None,
            priority: None,
            ticket_type: None,
            labels: None,
        }
    }

    #[test]
    fn create_and_get() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let session_id = make_session(&conn);

        let params = CreateTicketParams {
            session_id: session_id.clone(),
            title: "Fix login bug".to_string(),
            description: Some("Auth flow is broken".to_string()),
            acceptance_criteria: Some("Login works".to_string()),
            estimate: Some("2h".to_string()),
            priority: Some("High".to_string()),
            ticket_type: Some("Bug".to_string()),
            labels: Some(vec!["auth".to_string(), "urgent".to_string()]),
        };

        let ticket = create(&conn, &params).unwrap();

        assert_eq!(ticket.session_id, session_id);
        assert_eq!(ticket.title, "Fix login bug");
        assert_eq!(ticket.description, "Auth flow is broken");
        assert_eq!(ticket.acceptance_criteria, "Login works");
        assert_eq!(ticket.estimate, Some("2h".to_string()));
        assert_eq!(ticket.priority, "High");
        assert_eq!(ticket.ticket_type, "Bug");
        assert_eq!(ticket.labels, vec!["auth", "urgent"]);
        assert_eq!(ticket.status, "Draft");
        assert!(ticket.parent_id.is_none());

        let fetched = get(&conn, &ticket.id).unwrap();
        assert_eq!(fetched.id, ticket.id);
        assert_eq!(fetched.title, "Fix login bug");
    }

    #[test]
    fn list_by_session_ordered_by_sort_order() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let session_id = make_session(&conn);

        let p1 = minimal_params(&session_id, "Ticket B");
        let p2 = minimal_params(&session_id, "Ticket A");

        // Create first with sort_order 0 (default), then reorder
        let t1 = create(&conn, &p1).unwrap();
        let t2 = create(&conn, &p2).unwrap();

        // Give t1 a higher sort_order so t2 comes first
        reorder(&conn, &t1.id, 10).unwrap();
        reorder(&conn, &t2.id, 5).unwrap();

        let tickets = list_by_session(&conn, &session_id).unwrap();
        assert_eq!(tickets.len(), 2);
        assert_eq!(tickets[0].id, t2.id);
        assert_eq!(tickets[1].id, t1.id);
    }

    #[test]
    fn update_ticket() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let session_id = make_session(&conn);

        let ticket = create(&conn, &minimal_params(&session_id, "Original")).unwrap();

        let update_params = UpdateTicketParams {
            title: Some("Renamed".to_string()),
            description: None,
            acceptance_criteria: None,
            estimate: None,
            priority: Some("High".to_string()),
            ticket_type: None,
            labels: None,
            status: None,
            parent_id: None,
        };

        let updated = update(&conn, &ticket.id, &update_params).unwrap();
        assert_eq!(updated.title, "Renamed");
        assert_eq!(updated.priority, "High");
        // Unchanged fields stay the same
        assert_eq!(updated.status, "Draft");
    }

    #[test]
    fn delete_ticket() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let session_id = make_session(&conn);

        let ticket = create(&conn, &minimal_params(&session_id, "To Delete")).unwrap();
        delete(&conn, &ticket.id).unwrap();

        let result = get(&conn, &ticket.id);
        assert!(result.is_err());
    }

    #[test]
    fn set_parent_epic_grouping() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let session_id = make_session(&conn);

        let epic = create(&conn, &minimal_params(&session_id, "Epic")).unwrap();
        let child = create(&conn, &minimal_params(&session_id, "Sub-task")).unwrap();

        let updated = super::set_parent(&conn, &child.id, Some(&epic.id)).unwrap();
        assert_eq!(updated.parent_id, Some(epic.id.clone()));

        // Clear the parent
        let cleared = super::set_parent(&conn, &child.id, None).unwrap();
        assert!(cleared.parent_id.is_none());
    }

    #[test]
    fn cascade_delete_with_session() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let session_id = make_session(&conn);

        create(&conn, &minimal_params(&session_id, "T1")).unwrap();
        create(&conn, &minimal_params(&session_id, "T2")).unwrap();

        let count_before: i64 = conn
            .query_row("SELECT COUNT(*) FROM tickets", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count_before, 2);

        crate::session::store::delete(&conn, &session_id).unwrap();

        let count_after: i64 = conn
            .query_row("SELECT COUNT(*) FROM tickets", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count_after, 0);
    }

    #[test]
    fn default_values() {
        let db = test_db();
        let conn = db.conn().unwrap();
        let session_id = make_session(&conn);

        let ticket = create(&conn, &minimal_params(&session_id, "Minimal")).unwrap();

        assert_eq!(ticket.priority, "Medium");
        assert_eq!(ticket.ticket_type, "Task");
        assert_eq!(ticket.status, "Draft");
        assert_eq!(ticket.description, "");
        assert_eq!(ticket.acceptance_criteria, "");
        assert!(ticket.labels.is_empty());
        assert!(ticket.dependencies.is_empty());
        assert!(ticket.estimate.is_none());
        assert!(ticket.parent_id.is_none());
        assert!(ticket.external_ref.is_none());
    }
}

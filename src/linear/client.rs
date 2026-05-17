use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::{MyceliumError, Result};

pub struct LinearClient {
    api_key: String,
    http: reqwest::blocking::Client,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearTeam {
    pub id: String,
    pub name: String,
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinearUser {
    pub id: String,
    pub name: String,
    pub email: String,
    #[serde(default)]
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinearWorkflowState {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub state_type: String, // "triage", "backlog", "unstarted", "started", "completed", "cancelled"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinearIssue {
    pub id: String,
    pub identifier: String, // e.g. "TEAM-123"
    pub title: String,
    pub description: Option<String>,
    pub priority: u8,
    pub state: LinearWorkflowState,
    pub assignee: Option<LinearUser>,
    pub due_date: Option<String>, // ISO date string
    pub labels: LinearLabelConnection,
    pub updated_at: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearLabelConnection {
    pub nodes: Vec<LinearLabel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearLabel {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearProject {
    pub id: String,
    pub name: String,
}

impl LinearClient {
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            http: reqwest::blocking::Client::new(),
        }
    }

    fn graphql(&self, query: &str, variables: Option<Value>) -> Result<Value> {
        let mut body = json!({ "query": query });
        if let Some(vars) = variables {
            body["variables"] = vars;
        }

        let resp = self
            .http
            .post("https://api.linear.app/graphql")
            .header("Authorization", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .map_err(|e| MyceliumError::Http(e.to_string()))?;

        let status = resp.status();
        let text = resp
            .text()
            .map_err(|e| MyceliumError::Http(e.to_string()))?;

        if !status.is_success() {
            return Err(MyceliumError::LinearApi(format!(
                "HTTP {}: {}",
                status, text
            )));
        }

        let json: Value =
            serde_json::from_str(&text).map_err(|e| MyceliumError::LinearApi(e.to_string()))?;

        if let Some(errors) = json.get("errors") {
            return Err(MyceliumError::LinearApi(format!("{}", errors)));
        }

        json.get("data")
            .cloned()
            .ok_or_else(|| MyceliumError::LinearApi("No data in response".to_string()))
    }

    pub fn fetch_teams(&self) -> Result<Vec<LinearTeam>> {
        let query = r#"
            query {
                teams {
                    nodes {
                        id
                        name
                        key
                    }
                }
            }
        "#;
        let data = self.graphql(query, None)?;
        let nodes = &data["teams"]["nodes"];
        serde_json::from_value(nodes.clone()).map_err(|e| MyceliumError::LinearApi(e.to_string()))
    }

    pub fn fetch_team_members(&self, team_id: &str) -> Result<Vec<LinearUser>> {
        let query = r#"
            query($teamId: String!) {
                team(id: $teamId) {
                    members {
                        nodes {
                            id
                            name
                            email
                            displayName
                        }
                    }
                }
            }
        "#;
        let data = self.graphql(query, Some(json!({ "teamId": team_id })))?;
        let nodes = &data["team"]["members"]["nodes"];
        serde_json::from_value(nodes.clone()).map_err(|e| MyceliumError::LinearApi(e.to_string()))
    }

    pub fn fetch_workflow_states(&self, team_id: &str) -> Result<Vec<LinearWorkflowState>> {
        let query = r#"
            query($teamId: String!) {
                team(id: $teamId) {
                    states {
                        nodes {
                            id
                            name
                            type
                        }
                    }
                }
            }
        "#;
        let data = self.graphql(query, Some(json!({ "teamId": team_id })))?;
        let nodes = &data["team"]["states"]["nodes"];
        serde_json::from_value(nodes.clone()).map_err(|e| MyceliumError::LinearApi(e.to_string()))
    }

    /// Fetch issues with optional label filtering and active-only state filtering.
    /// `filter_labels`: if non-empty, only issues with ALL these labels are returned.
    /// `active_only`: if true, skip issues in completed/cancelled states.
    pub fn fetch_issues_filtered(
        &self,
        team_id: &str,
        filter_labels: &[String],
        active_only: bool,
        cursor: Option<&str>,
    ) -> Result<(Vec<LinearIssue>, Option<String>)> {
        let query = r#"
            query($teamId: String!, $after: String, $filter: IssueFilter) {
                team(id: $teamId) {
                    issues(first: 50, after: $after, filter: $filter) {
                        nodes {
                            id
                            identifier
                            title
                            description
                            priority
                            state {
                                id
                                name
                                type
                            }
                            assignee {
                                id
                                name
                                email
                                displayName
                            }
                            dueDate
                            labels {
                                nodes {
                                    id
                                    name
                                }
                            }
                            updatedAt
                            createdAt
                        }
                        pageInfo {
                            hasNextPage
                            endCursor
                        }
                    }
                }
            }
        "#;

        let mut vars = json!({ "teamId": team_id });
        if let Some(c) = cursor {
            vars["after"] = json!(c);
        }

        // Build filter
        let mut filter = json!({});
        let mut and_conditions: Vec<Value> = Vec::new();

        // Label filter: each label must be present (AND logic)
        if !filter_labels.is_empty() {
            for label_name in filter_labels {
                and_conditions.push(json!({
                    "labels": { "some": { "name": { "eq": label_name } } }
                }));
            }
        }

        // Active-only: exclude completed and cancelled states
        if active_only {
            and_conditions.push(json!({
                "state": {
                    "type": { "nin": ["completed", "cancelled"] }
                }
            }));
        }

        if !and_conditions.is_empty() {
            filter["and"] = json!(and_conditions);
            vars["filter"] = filter;
        }

        let data = self.graphql(query, Some(vars))?;
        let issues_data = &data["team"]["issues"];
        let nodes: Vec<LinearIssue> = serde_json::from_value(issues_data["nodes"].clone())
            .map_err(|e| MyceliumError::LinearApi(e.to_string()))?;

        let next_cursor = if issues_data["pageInfo"]["hasNextPage"]
            .as_bool()
            .unwrap_or(false)
        {
            issues_data["pageInfo"]["endCursor"]
                .as_str()
                .map(|s| s.to_string())
        } else {
            None
        };

        Ok((nodes, next_cursor))
    }

    pub fn fetch_all_issues_filtered(
        &self,
        team_id: &str,
        filter_labels: &[String],
        active_only: bool,
    ) -> Result<Vec<LinearIssue>> {
        let mut all = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let (issues, next) =
                self.fetch_issues_filtered(team_id, filter_labels, active_only, cursor.as_deref())?;
            all.extend(issues);
            if next.is_none() {
                break;
            }
            cursor = next;
        }
        Ok(all)
    }

    pub fn create_issue(
        &self,
        team_id: &str,
        title: &str,
        description: Option<&str>,
        priority: u8,
        state_id: &str,
        assignee_id: Option<&str>,
        due_date: Option<&str>,
        label_ids: &[String],
    ) -> Result<LinearIssue> {
        let query = r#"
            mutation($input: IssueCreateInput!) {
                issueCreate(input: $input) {
                    success
                    issue {
                        id
                        identifier
                        title
                        description
                        priority
                        state {
                            id
                            name
                            type
                        }
                        assignee {
                            id
                            name
                            email
                            displayName
                        }
                        dueDate
                        labels {
                            nodes {
                                id
                                name
                            }
                        }
                        updatedAt
                        createdAt
                    }
                }
            }
        "#;
        let mut input = json!({
            "teamId": team_id,
            "title": title,
            "priority": priority,
            "stateId": state_id,
        });
        if let Some(desc) = description {
            input["description"] = json!(desc);
        }
        if let Some(aid) = assignee_id {
            input["assigneeId"] = json!(aid);
        }
        if let Some(due) = due_date {
            input["dueDate"] = json!(due);
        }
        if !label_ids.is_empty() {
            input["labelIds"] = json!(label_ids);
        }

        let data = self.graphql(query, Some(json!({ "input": input })))?;
        let issue = &data["issueCreate"]["issue"];
        serde_json::from_value(issue.clone()).map_err(|e| MyceliumError::LinearApi(e.to_string()))
    }

    pub fn update_issue(
        &self,
        issue_id: &str,
        title: Option<&str>,
        description: Option<&str>,
        priority: Option<u8>,
        state_id: Option<&str>,
        assignee_id: Option<&str>,
        due_date: Option<&str>,
        label_ids: Option<&[String]>,
    ) -> Result<LinearIssue> {
        let query = r#"
            mutation($issueId: String!, $input: IssueUpdateInput!) {
                issueUpdate(id: $issueId, input: $input) {
                    success
                    issue {
                        id
                        identifier
                        title
                        description
                        priority
                        state {
                            id
                            name
                            type
                        }
                        assignee {
                            id
                            name
                            email
                            displayName
                        }
                        dueDate
                        labels {
                            nodes {
                                id
                                name
                            }
                        }
                        updatedAt
                        createdAt
                    }
                }
            }
        "#;
        let mut input = json!({});
        if let Some(t) = title {
            input["title"] = json!(t);
        }
        if let Some(d) = description {
            input["description"] = json!(d);
        }
        if let Some(p) = priority {
            input["priority"] = json!(p);
        }
        if let Some(s) = state_id {
            input["stateId"] = json!(s);
        }
        if let Some(a) = assignee_id {
            input["assigneeId"] = json!(a);
        }
        if let Some(d) = due_date {
            input["dueDate"] = json!(d);
        }
        if let Some(l) = label_ids {
            input["labelIds"] = json!(l);
        }

        let data = self.graphql(query, Some(json!({ "issueId": issue_id, "input": input })))?;
        let issue = &data["issueUpdate"]["issue"];
        serde_json::from_value(issue.clone()).map_err(|e| MyceliumError::LinearApi(e.to_string()))
    }

    pub fn fetch_labels(&self, team_id: &str) -> Result<Vec<LinearLabel>> {
        let query = r#"
            query($teamId: String!) {
                team(id: $teamId) {
                    labels {
                        nodes {
                            id
                            name
                        }
                    }
                }
            }
        "#;
        let data = self.graphql(query, Some(json!({ "teamId": team_id })))?;
        let nodes = &data["team"]["labels"]["nodes"];
        serde_json::from_value(nodes.clone()).map_err(|e| MyceliumError::LinearApi(e.to_string()))
    }

    pub fn create_label(&self, team_id: &str, name: &str) -> Result<LinearLabel> {
        let query = r#"
            mutation($input: IssueLabelCreateInput!) {
                issueLabelCreate(input: $input) {
                    success
                    issueLabel {
                        id
                        name
                    }
                }
            }
        "#;
        let data = self.graphql(
            query,
            Some(json!({ "input": { "teamId": team_id, "name": name } })),
        )?;
        let label = &data["issueLabelCreate"]["issueLabel"];
        serde_json::from_value(label.clone()).map_err(|e| MyceliumError::LinearApi(e.to_string()))
    }

    pub fn fetch_projects(&self) -> Result<Vec<LinearProject>> {
        let query = r#"
            query {
                projects(first: 100) {
                    nodes {
                        id
                        name
                    }
                }
            }
        "#;
        let data = self.graphql(query, None)?;
        let nodes = &data["projects"]["nodes"];
        serde_json::from_value(nodes.clone()).map_err(|e| MyceliumError::LinearApi(e.to_string()))
    }

    pub fn create_project(&self, team_id: &str, name: &str) -> Result<LinearProject> {
        let query = r#"
            mutation($input: ProjectCreateInput!) {
                projectCreate(input: $input) {
                    success
                    project {
                        id
                        name
                    }
                }
            }
        "#;
        let data = self.graphql(
            query,
            Some(json!({
                "input": {
                    "name": name,
                    "teamIds": [team_id]
                }
            })),
        )?;
        let project = &data["projectCreate"]["project"];
        serde_json::from_value(project.clone()).map_err(|e| MyceliumError::LinearApi(e.to_string()))
    }
}

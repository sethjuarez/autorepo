use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Operation {
    Label {
        id: String,
        marker: String,
        name: String,
        color: Option<String>,
        description: Option<String>,
    },
    Milestone {
        id: String,
        marker: String,
        title: String,
        description: Option<String>,
    },
    File {
        id: String,
        marker: String,
        path: String,
        content: String,
    },
    Branch {
        id: String,
        marker: String,
        name: String,
    },
    Issue {
        id: String,
        marker: String,
        title: String,
        body: String,
        labels: Vec<String>,
        milestone: Option<String>,
    },
    PullRequest {
        id: String,
        marker: String,
        title: String,
        body: String,
        branch: String,
        labels: Vec<String>,
    },
    WorkflowDispatch {
        id: String,
        marker: String,
        workflow: String,
        inputs: serde_json::Value,
    },
    WarmupNote {
        id: String,
        marker: String,
        title: String,
    },
    WarmupChecklist {
        id: String,
        marker: String,
        title: String,
    },
    WarmupAppSession {
        id: String,
        marker: String,
        title: String,
    },
    CopilotTask {
        id: String,
        title: String,
    },
}

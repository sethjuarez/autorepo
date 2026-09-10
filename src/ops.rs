use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Operation {
    Label {
        id: String,
        name: String,
        color: Option<String>,
        description: Option<String>,
    },
    Milestone {
        id: String,
        title: String,
        description: Option<String>,
    },
    File {
        id: String,
        path: String,
    },
    Branch {
        id: String,
        name: String,
    },
    Issue {
        id: String,
        title: String,
    },
    PullRequest {
        id: String,
        title: String,
    },
    WorkflowDispatch {
        id: String,
        workflow: String,
        inputs: serde_json::Value,
    },
    WarmupNote {
        id: String,
        title: String,
    },
    WarmupChecklist {
        id: String,
        title: String,
    },
    CopilotTask {
        id: String,
        title: String,
    },
}

use comfy_table::{Cell, Table, presets::UTF8_FULL};

use crate::planner::Plan;

pub fn print_plan_table(plan: &Plan) {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["#", "Operation", "ID", "Target"]);

    for (index, operation) in plan.operations.iter().enumerate() {
        let (kind, id, target) = operation_summary(operation);
        table.add_row(vec![
            Cell::new(index + 1),
            Cell::new(kind),
            Cell::new(id),
            Cell::new(target),
        ]);
    }

    println!("Plan for {} using pack '{}':", plan.repo, plan.pack_id);
    println!("{table}");
}

fn operation_summary(operation: &crate::ops::Operation) -> (&'static str, &str, &str) {
    match operation {
        crate::ops::Operation::Label { id, name, .. } => ("label", id, name),
        crate::ops::Operation::Milestone { id, title, .. } => ("milestone", id, title),
        crate::ops::Operation::File { id, path, .. } => ("file", id, path),
        crate::ops::Operation::Branch { id, name, .. } => ("branch", id, name),
        crate::ops::Operation::Issue { id, title, .. } => ("issue", id, title),
        crate::ops::Operation::PullRequest { id, title, .. } => ("pull_request", id, title),
        crate::ops::Operation::WorkflowDispatch { id, workflow, .. } => {
            ("workflow_dispatch", id, workflow)
        }
        crate::ops::Operation::WarmupNote { id, title, .. } => ("warmup_note", id, title),
        crate::ops::Operation::WarmupChecklist { id, title, .. } => ("warmup_checklist", id, title),
        crate::ops::Operation::WarmupAppSession { id, title, .. } => {
            ("warmup_app_session", id, title)
        }
        crate::ops::Operation::CopilotTask { id, title } => ("copilot_task", id, title),
    }
}

use std::collections::HashMap;

use anyhow::Result;

use crate::{
    doctor,
    github::{LiveGitHubClient, RepoRef},
    ops::Operation,
    planner::Plan,
};

#[derive(Debug, Default)]
pub struct Executor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionProgressStatus {
    Started,
    Completed,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionProgress {
    pub index: usize,
    pub total: usize,
    pub id: String,
    pub kind: &'static str,
    pub target: String,
    pub status: ExecutionProgressStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepositoryHydrationMode {
    PreserveExisting,
    ExactReset,
}

impl Executor {
    pub async fn execute_serial(&self, plan: &Plan) -> Result<()> {
        self.execute_serial_with_progress(plan, |_| {}).await
    }

    pub async fn execute_serial_with_progress<F>(&self, plan: &Plan, progress: F) -> Result<()>
    where
        F: FnMut(ExecutionProgress),
    {
        self.execute_serial_with_repository_mode(
            plan,
            RepositoryHydrationMode::PreserveExisting,
            progress,
        )
        .await
    }

    pub async fn execute_serial_with_repository_mode<F>(
        &self,
        plan: &Plan,
        repository_mode: RepositoryHydrationMode,
        mut progress: F,
    ) -> Result<()>
    where
        F: FnMut(ExecutionProgress),
    {
        let repo = RepoRef::parse(&plan.repo)?;
        let github = LiveGitHubClient::from_env()?;
        match repository_mode {
            RepositoryHydrationMode::PreserveExisting => github.ensure_repository(&repo).await?,
            RepositoryHydrationMode::ExactReset => github.recreate_repository(&repo).await?,
        }
        doctor::ensure_empty_repo(github.repo_is_empty(&repo).await?, plan.allow_non_empty)?;

        let mut milestones = HashMap::new();
        let total = plan.operations.len();

        for (offset, operation) in plan.operations.iter().enumerate() {
            let index = offset + 1;
            progress(operation_progress(
                index,
                total,
                operation,
                ExecutionProgressStatus::Started,
            ));
            let completion_status = operation_completion_status(operation);
            let result = async {
                match operation {
                Operation::Label {
                    name,
                    color,
                    description,
                    ..
                } => {
                    github
                        .ensure_label(&repo, name, color.as_deref(), description.as_deref())
                        .await?;
                }
                Operation::Milestone {
                    id,
                    title,
                    description,
                    ..
                } => {
                    let number = github
                        .ensure_milestone(&repo, title, description.as_deref())
                        .await?;
                    milestones.insert(id.clone(), number);
                }
                Operation::File {
                    path,
                    content,
                    marker,
                    ..
                } => {
                    github
                        .ensure_file(&repo, path, content, marker, None)
                        .await?;
                }
                Operation::Branch {
                    id, name, marker, ..
                } => {
                    github.ensure_branch(&repo, id, name, marker).await?;
                }
                Operation::Issue {
                    title,
                    body,
                    marker,
                    labels,
                    milestone,
                    ..
                } => {
                    let milestone = milestone
                        .as_ref()
                        .map(|id| {
                            milestones
                                .get(id)
                                .copied()
                                .ok_or_else(|| anyhow::anyhow!("milestone '{id}' was not created"))
                        })
                        .transpose()?;
                    github
                        .ensure_issue(&repo, title, body, marker, labels, milestone)
                        .await?;
                }
                Operation::PullRequest {
                    title,
                    body,
                    marker,
                    branch,
                    labels,
                    ..
                } => {
                    github
                        .ensure_pull_request(&repo, title, body, marker, branch, labels)
                        .await?;
                }
                Operation::WorkflowDispatch { workflow, .. } => {
                    crate::github::output_line(format!(
                        "skip workflow_dispatch {workflow}: idempotent dispatch markers are not available yet",
                    ));
                }
                Operation::WarmupNote { title, .. }
                | Operation::WarmupChecklist { title, .. }
                | Operation::WarmupAppSession { title, .. }
                | Operation::WarmupAppLink { title, .. }
                | Operation::WarmupAutomationDraft { title, .. } => {
                    crate::github::output_line(format!("skip warmup item {title}: use autorepo warm"));
                }
            }
                Ok(())
            }
            .await;

            match result {
                Ok(()) => progress(operation_progress(
                    index,
                    total,
                    operation,
                    completion_status,
                )),
                Err(error) => {
                    progress(operation_progress(
                        index,
                        total,
                        operation,
                        ExecutionProgressStatus::Failed,
                    ));
                    return Err(error);
                }
            }
        }

        Ok(())
    }
}

fn operation_progress(
    index: usize,
    total: usize,
    operation: &Operation,
    status: ExecutionProgressStatus,
) -> ExecutionProgress {
    let (id, kind, target) = match operation {
        Operation::Label { id, name, .. } => (id, "Label", name),
        Operation::Milestone { id, title, .. } => (id, "Milestone", title),
        Operation::File { id, path, .. } => (id, "File", path),
        Operation::Branch { id, name, .. } => (id, "Branch", name),
        Operation::Issue { id, title, .. } => (id, "Issue", title),
        Operation::PullRequest { id, title, .. } => (id, "Pull request", title),
        Operation::WorkflowDispatch { id, workflow, .. } => (id, "Workflow dispatch", workflow),
        Operation::WarmupNote { id, title, .. } => (id, "Warmup note", title),
        Operation::WarmupChecklist { id, title, .. } => (id, "Warmup checklist", title),
        Operation::WarmupAppSession { id, title, .. } => (id, "Warmup app session", title),
        Operation::WarmupAppLink { id, title, .. } => (id, "Warmup app link", title),
        Operation::WarmupAutomationDraft { id, title, .. } => {
            (id, "Warmup automation draft", title)
        }
    };

    ExecutionProgress {
        index,
        total,
        id: id.clone(),
        kind,
        target: target.clone(),
        status,
    }
}

fn operation_completion_status(operation: &Operation) -> ExecutionProgressStatus {
    match operation {
        Operation::WorkflowDispatch { .. }
        | Operation::WarmupNote { .. }
        | Operation::WarmupChecklist { .. }
        | Operation::WarmupAppSession { .. }
        | Operation::WarmupAppLink { .. }
        | Operation::WarmupAutomationDraft { .. } => ExecutionProgressStatus::Skipped,
        _ => ExecutionProgressStatus::Completed,
    }
}

use std::collections::HashMap;

use anyhow::{Result, bail};

use crate::{
    doctor,
    github::{LiveGitHubClient, RepoRef},
    ops::Operation,
    planner::Plan,
};

#[derive(Debug, Default)]
pub struct Executor;

impl Executor {
    pub async fn execute_serial(&self, plan: &Plan) -> Result<()> {
        let repo = RepoRef::parse(&plan.repo)?;
        let github = LiveGitHubClient::from_env()?;
        doctor::ensure_empty_repo(github.repo_is_empty(&repo).await?, plan.allow_non_empty)?;

        let mut milestones = HashMap::new();

        for operation in &plan.operations {
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
                    println!(
                        "skip workflow_dispatch {workflow}: idempotent dispatch markers are not available yet"
                    );
                }
                Operation::WarmupNote { title, .. }
                | Operation::WarmupChecklist { title, .. }
                | Operation::WarmupAppSession { title, .. } => {
                    println!("skip warmup item {title}: use autorepo warm");
                }
                Operation::CopilotTask { .. } => {
                    bail!("copilot_task operations are allowed only in autorepo warm");
                }
            }
        }

        Ok(())
    }
}

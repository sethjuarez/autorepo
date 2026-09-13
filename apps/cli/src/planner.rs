use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    doctor,
    github::{GitHubClient, RepoRef},
    markers,
    ops::Operation,
    pack::{Pack, WarmupKind},
};

#[derive(Debug, Clone)]
pub struct PlanContext {
    pub repo: String,
    pub allow_non_empty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    pub repo: String,
    pub pack_id: String,
    pub pack_name: String,
    pub pack_description: Option<String>,
    pub allow_non_empty: bool,
    pub operations: Vec<Operation>,
}

#[derive(Debug, Default)]
pub struct Planner {
    github: GitHubClient,
}

impl Planner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn plan(&self, pack: &Pack, context: PlanContext) -> Result<Plan> {
        let repo = RepoRef::parse(&context.repo)?;
        let empty = self.github.repo_is_empty_fixture(&repo);
        doctor::ensure_empty_repo(empty, context.allow_non_empty)?;
        let manifest = pack.manifest();
        let label_names = manifest
            .labels
            .iter()
            .map(|label| (label.id.as_str(), label.name.as_str()))
            .collect::<HashMap<_, _>>();
        let mut operations = Vec::new();

        operations.extend(
            manifest
                .labels
                .iter()
                .map(|label| -> Result<Operation> {
                    Ok(Operation::Label {
                        id: label.id.clone(),
                        marker: markers::format_marker_token(
                            &manifest.id,
                            &format!("label.{}", label.id),
                        )?,
                        name: label.name.clone(),
                        color: label.color.clone(),
                        description: label.description.clone(),
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        );
        operations.extend(
            manifest
                .milestones
                .iter()
                .map(|milestone| -> Result<Operation> {
                    Ok(Operation::Milestone {
                        id: milestone.id.clone(),
                        marker: markers::format_marker_token(
                            &manifest.id,
                            &format!("milestone.{}", milestone.id),
                        )?,
                        title: milestone.title.clone(),
                        description: milestone.description.clone(),
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        );
        operations.extend(
            manifest
                .files
                .iter()
                .map(|file| -> Result<Operation> {
                    Ok(Operation::File {
                        id: file.id.clone(),
                        marker: markers::format_marker_token(
                            &manifest.id,
                            &format!("file.{}", file.id),
                        )?,
                        path: file.path.clone(),
                        content: pack.template_text(&file.template)?,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        );
        operations.extend(
            manifest
                .branches
                .iter()
                .map(|branch| -> Result<Operation> {
                    Ok(Operation::Branch {
                        id: branch.id.clone(),
                        marker: markers::format_marker_token(
                            &manifest.id,
                            &format!("branch.{}", branch.id),
                        )?,
                        name: branch.name.clone(),
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        );
        operations.extend(
            manifest
                .issues
                .iter()
                .map(|issue| -> Result<Operation> {
                    Ok(Operation::Issue {
                        id: issue.id.clone(),
                        marker: markers::format_marker_token(
                            &manifest.id,
                            &format!("issue.{}", issue.id),
                        )?,
                        title: issue.title.clone(),
                        body: pack.template_text(&issue.template)?,
                        labels: issue
                            .labels
                            .iter()
                            .map(|id| label_names[id.as_str()].to_string())
                            .collect(),
                        milestone: issue.milestone.clone(),
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        );
        operations.extend(
            manifest
                .pull_requests
                .iter()
                .map(|pr| -> Result<Operation> {
                    let branch_name = manifest
                        .branches
                        .iter()
                        .find(|branch| branch.id == pr.branch)
                        .map(|branch| branch.name.clone())
                        .unwrap_or_else(|| pr.branch.clone());
                    Ok(Operation::PullRequest {
                        id: pr.id.clone(),
                        marker: markers::format_marker_token(
                            &manifest.id,
                            &format!("pull_request.{}", pr.id),
                        )?,
                        title: pr.title.clone(),
                        body: pack.template_text(&pr.template)?,
                        branch: branch_name,
                        labels: pr
                            .labels
                            .iter()
                            .map(|id| label_names[id.as_str()].to_string())
                            .collect(),
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        );
        operations.extend(
            manifest
                .workflow_dispatches
                .iter()
                .map(|dispatch| -> Result<Operation> {
                    Ok(Operation::WorkflowDispatch {
                        id: dispatch.id.clone(),
                        marker: markers::format_marker_token(
                            &manifest.id,
                            &format!("workflow_dispatch.{}", dispatch.id),
                        )?,
                        workflow: dispatch.workflow.clone(),
                        inputs: dispatch.inputs.clone(),
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        );
        operations.extend(
            manifest
                .warmup
                .iter()
                .map(|warmup| -> Result<Operation> {
                    Ok(match warmup.kind {
                        WarmupKind::Note => Operation::WarmupNote {
                            id: warmup.id.clone(),
                            marker: markers::format_marker_token(
                                &manifest.id,
                                &format!("warmup.{}", warmup.id),
                            )?,
                            title: warmup.title.clone(),
                        },
                        WarmupKind::Checklist => Operation::WarmupChecklist {
                            id: warmup.id.clone(),
                            marker: markers::format_marker_token(
                                &manifest.id,
                                &format!("warmup.{}", warmup.id),
                            )?,
                            title: warmup.title.clone(),
                        },
                        WarmupKind::AppSession => Operation::WarmupAppSession {
                            id: warmup.id.clone(),
                            marker: markers::format_marker_token(
                                &manifest.id,
                                &format!("warmup.{}", warmup.id),
                            )?,
                            title: warmup.title.clone(),
                        },
                        WarmupKind::AppLink => Operation::WarmupAppLink {
                            id: warmup.id.clone(),
                            marker: markers::format_marker_token(
                                &manifest.id,
                                &format!("warmup.{}", warmup.id),
                            )?,
                            title: warmup.title.clone(),
                        },
                        WarmupKind::AutomationDraft => Operation::WarmupAutomationDraft {
                            id: warmup.id.clone(),
                            marker: markers::format_marker_token(
                                &manifest.id,
                                &format!("warmup.{}", warmup.id),
                            )?,
                            title: warmup.title.clone(),
                        },
                    })
                })
                .collect::<Result<Vec<_>>>()?,
        );

        let _ = &self.github;

        Ok(Plan {
            repo: format!("{}/{}", repo.owner, repo.name),
            pack_id: manifest.id.clone(),
            pack_name: manifest.name.clone(),
            pack_description: manifest.description.clone(),
            allow_non_empty: context.allow_non_empty,
            operations,
        })
    }
}

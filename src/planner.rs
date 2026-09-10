use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{
    doctor,
    github::{GitHubClient, RepoRef},
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
        let mut operations = Vec::new();

        operations.extend(manifest.labels.iter().map(|label| Operation::Label {
            id: label.id.clone(),
            name: label.name.clone(),
            color: label.color.clone(),
            description: label.description.clone(),
        }));
        operations.extend(
            manifest
                .milestones
                .iter()
                .map(|milestone| Operation::Milestone {
                    id: milestone.id.clone(),
                    title: milestone.title.clone(),
                    description: milestone.description.clone(),
                }),
        );
        operations.extend(manifest.files.iter().map(|file| Operation::File {
            id: file.id.clone(),
            path: file.path.clone(),
        }));
        operations.extend(manifest.branches.iter().map(|branch| Operation::Branch {
            id: branch.id.clone(),
            name: branch.name.clone(),
        }));
        operations.extend(manifest.issues.iter().map(|issue| Operation::Issue {
            id: issue.id.clone(),
            title: issue.title.clone(),
        }));
        operations.extend(
            manifest
                .pull_requests
                .iter()
                .map(|pr| Operation::PullRequest {
                    id: pr.id.clone(),
                    title: pr.title.clone(),
                }),
        );
        operations.extend(manifest.workflow_dispatches.iter().map(|dispatch| {
            Operation::WorkflowDispatch {
                id: dispatch.id.clone(),
                workflow: dispatch.workflow.clone(),
                inputs: dispatch.inputs.clone(),
            }
        }));
        operations.extend(manifest.warmup.iter().map(|warmup| match warmup.kind {
            WarmupKind::Note => Operation::WarmupNote {
                id: warmup.id.clone(),
                title: warmup.title.clone(),
            },
            WarmupKind::Checklist => Operation::WarmupChecklist {
                id: warmup.id.clone(),
                title: warmup.title.clone(),
            },
        }));

        let _ = &self.github;

        Ok(Plan {
            repo: format!("{}/{}", repo.owner, repo.name),
            pack_id: manifest.id.clone(),
            pack_name: manifest.name.clone(),
            pack_description: manifest.description.clone(),
            operations,
        })
    }
}

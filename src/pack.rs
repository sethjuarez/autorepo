use std::{
    collections::HashSet,
    fs,
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::{markers, ops::Operation};

#[derive(Debug, Clone)]
pub struct Pack {
    root: PathBuf,
    manifest: PackManifest,
}

impl Pack {
    pub fn load(root: PathBuf) -> Result<Self> {
        let manifest_path = root.join("pack.yml");
        let manifest_text = fs::read_to_string(&manifest_path)
            .with_context(|| format!("failed to read {}", manifest_path.display()))?;
        let manifest: PackManifest = serde_yaml::from_str(&manifest_text)
            .with_context(|| format!("failed to parse {}", manifest_path.display()))?;

        Ok(Self { root, manifest })
    }

    pub fn manifest(&self) -> &PackManifest {
        &self.manifest
    }

    pub fn validate(&self) -> Result<()> {
        self.manifest.validate(&self.root)
    }

    pub fn template_text(&self, path: &str) -> Result<String> {
        let template_path = self.root.join(path);
        fs::read_to_string(&template_path)
            .with_context(|| format!("failed to read template {}", template_path.display()))
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackManifest {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub safety: Safety,
    #[serde(default)]
    pub labels: Vec<Label>,
    #[serde(default)]
    pub milestones: Vec<Milestone>,
    #[serde(default)]
    pub files: Vec<FileResource>,
    #[serde(default)]
    pub branches: Vec<Branch>,
    #[serde(default)]
    pub issues: Vec<Issue>,
    #[serde(default)]
    pub pull_requests: Vec<PullRequest>,
    #[serde(default)]
    pub workflow_dispatches: Vec<WorkflowDispatch>,
    #[serde(default)]
    pub warmup: Vec<WarmupItem>,
}

impl PackManifest {
    fn validate(&self, root: &Path) -> Result<()> {
        if self.schema != 1 {
            bail!("unsupported pack schema {}; expected 1", self.schema);
        }

        validate_stable_id("pack id", &self.id)?;
        if self.name.trim().is_empty() {
            bail!("pack name is required");
        }

        let mut ids = HashSet::new();
        let mut label_ids = HashSet::new();
        let mut milestone_ids = HashSet::new();
        let mut branch_ids = HashSet::new();
        let mut write_count = 0_u32;

        for label in &self.labels {
            validate_resource_id("label", &label.id, &mut ids)?;
            validate_marker(&self.id, "label", &label.id)?;
            require_non_empty("label name", &label.name)?;
            label_ids.insert(label.id.as_str());
            write_count += 1;
        }

        for milestone in &self.milestones {
            validate_resource_id("milestone", &milestone.id, &mut ids)?;
            validate_marker(&self.id, "milestone", &milestone.id)?;
            require_non_empty("milestone title", &milestone.title)?;
            milestone_ids.insert(milestone.id.as_str());
            write_count += 1;
        }

        for file in &self.files {
            validate_resource_id("file", &file.id, &mut ids)?;
            validate_marker(&self.id, "file", &file.id)?;
            validate_safe_path("file path", &file.path)?;
            validate_safe_path("file template", &file.template)?;
            require_template(root, &file.template)?;
            write_count += 1;
        }

        for branch in &self.branches {
            validate_resource_id("branch", &branch.id, &mut ids)?;
            validate_marker(&self.id, "branch", &branch.id)?;
            require_non_empty("branch name", &branch.name)?;
            if !(branch.name.starts_with("demo/") || branch.name.starts_with("autorepo/")) {
                bail!(
                    "branch '{}' must be namespaced under demo/ or autorepo/",
                    branch.id
                );
            }
            branch_ids.insert(branch.id.as_str());
            write_count += 1;
        }

        for issue in &self.issues {
            validate_resource_id("issue", &issue.id, &mut ids)?;
            validate_marker(&self.id, "issue", &issue.id)?;
            require_non_empty("issue title", &issue.title)?;
            validate_safe_path("issue template", &issue.template)?;
            require_template(root, &issue.template)?;
            validate_labels(&issue.id, &issue.labels, &label_ids)?;
            validate_optional_ref("milestone", &issue.milestone, &milestone_ids)?;
            write_count += 1;
        }

        for pr in &self.pull_requests {
            validate_resource_id("pull_request", &pr.id, &mut ids)?;
            validate_marker(&self.id, "pull_request", &pr.id)?;
            require_non_empty("pull request title", &pr.title)?;
            validate_safe_path("pull request template", &pr.template)?;
            require_template(root, &pr.template)?;
            validate_labels(&pr.id, &pr.labels, &label_ids)?;
            validate_required_ref("pull_request branch", &pr.branch, &branch_ids)?;
            write_count += 1;
        }

        for dispatch in &self.workflow_dispatches {
            validate_resource_id("workflow_dispatch", &dispatch.id, &mut ids)?;
            validate_marker(&self.id, "workflow_dispatch", &dispatch.id)?;
            require_non_empty("workflow", &dispatch.workflow)?;
            write_count += 1;
        }

        for warmup in &self.warmup {
            validate_resource_id("warmup", &warmup.id, &mut ids)?;
            validate_marker(&self.id, "warmup", &warmup.id)?;
            require_non_empty("warmup title", &warmup.title)?;
            if let Some(template) = &warmup.prompt_template {
                validate_safe_path("warmup prompt template", template)?;
                require_template(root, template)?;
            }
            if matches!(warmup.kind, WarmupKind::AppSession)
                && warmup.prompt.is_none()
                && warmup.prompt_template.is_none()
            {
                bail!(
                    "warmup app_session '{}' requires prompt or prompt_template",
                    warmup.id
                );
            }
        }

        if write_count > self.safety.max_writes {
            bail!(
                "pack declares {write_count} writes, exceeding safety.max_writes {}",
                self.safety.max_writes
            );
        }

        Ok(())
    }

    pub fn copilot_tasks(&self) -> Vec<Operation> {
        self.warmup
            .iter()
            .filter(|item| item.start_agent_task)
            .map(|item| Operation::CopilotTask {
                id: item.id.clone(),
                title: item.title.clone(),
            })
            .collect()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Safety {
    pub max_writes: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Label {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Milestone {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileResource {
    pub id: String,
    pub path: String,
    pub template: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Branch {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Issue {
    pub id: String,
    pub title: String,
    pub template: String,
    #[serde(default)]
    pub labels: Vec<String>,
    pub milestone: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PullRequest {
    pub id: String,
    pub title: String,
    pub template: String,
    pub branch: String,
    #[serde(default)]
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkflowDispatch {
    pub id: String,
    pub workflow: String,
    #[serde(default)]
    pub inputs: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WarmupItem {
    pub id: String,
    pub title: String,
    pub kind: WarmupKind,
    pub body: Option<String>,
    pub prompt: Option<String>,
    pub prompt_template: Option<String>,
    pub mode: Option<WarmupMode>,
    #[serde(default)]
    pub start_agent_task: bool,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarmupKind {
    Note,
    Checklist,
    AppSession,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarmupMode {
    Interactive,
    Plan,
    Autopilot,
}

impl WarmupMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Interactive => "interactive",
            Self::Plan => "plan",
            Self::Autopilot => "autopilot",
        }
    }
}

fn validate_resource_id(kind: &str, id: &str, seen: &mut HashSet<String>) -> Result<()> {
    validate_stable_id(kind, id)?;
    let full_id = format!("{kind}.{id}");
    if !seen.insert(full_id.clone()) {
        bail!("duplicate resource id '{full_id}'");
    }
    Ok(())
}

fn validate_marker(pack_id: &str, kind: &str, id: &str) -> Result<()> {
    markers::format_marker(pack_id, &format!("{kind}.{id}"))?;
    Ok(())
}

fn validate_stable_id(name: &str, id: &str) -> Result<()> {
    if id.is_empty()
        || !id
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '-' | '_'))
    {
        bail!("{name} '{id}' must use lowercase ASCII letters, numbers, '-' or '_'");
    }

    Ok(())
}

fn validate_safe_path(name: &str, value: &str) -> Result<()> {
    let path = Path::new(value);
    if value.trim().is_empty() || path.is_absolute() {
        bail!("{name} '{value}' must be a non-empty relative path");
    }

    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        bail!("{name} '{value}' must not contain parent-directory or root components");
    }

    Ok(())
}

fn require_template(root: &Path, value: &str) -> Result<()> {
    let template = root.join(value);
    if !template.is_file() {
        bail!("template '{}' does not exist", template.display());
    }
    Ok(())
}

fn validate_labels(id: &str, labels: &[String], declared: &HashSet<&str>) -> Result<()> {
    for label in labels {
        if !declared.contains(label.as_str()) {
            bail!("resource '{id}' uses undeclared label '{label}'");
        }
    }
    Ok(())
}

fn validate_optional_ref(
    name: &str,
    value: &Option<String>,
    declared: &HashSet<&str>,
) -> Result<()> {
    if let Some(value) = value {
        validate_required_ref(name, value, declared)?;
    }
    Ok(())
}

fn validate_required_ref(name: &str, value: &str, declared: &HashSet<&str>) -> Result<()> {
    if !declared.contains(value) {
        bail!("{name} references unknown id '{value}'");
    }
    Ok(())
}

fn require_non_empty(name: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("{name} is required");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::Pack;

    #[test]
    fn validates_builtin_pack() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("packs")
            .join("generic-starter");
        let pack = Pack::load(root).unwrap();
        pack.validate().unwrap();
    }

    #[test]
    fn rejects_unknown_fields() {
        let temp = pack_dir(
            r#"
schema: 1
id: bad_pack
name: Bad pack
unexpected: true
safety:
  max_writes: 1
"#,
        );

        assert!(Pack::load(temp.path().to_path_buf()).is_err());
    }

    #[test]
    fn rejects_undeclared_issue_labels() {
        let temp = pack_dir(
            r#"
schema: 1
id: bad_pack
name: Bad pack
safety:
  max_writes: 1
issues:
  - id: first
    title: First issue
    template: templates/issues/first.md
    labels:
      - missing
"#,
        );
        write_template(&temp, &["templates", "issues", "first.md"]);

        let pack = Pack::load(temp.path().to_path_buf()).unwrap();
        assert!(pack.validate().is_err());
    }

    #[test]
    fn rejects_unsafe_paths() {
        let temp = pack_dir(
            r#"
schema: 1
id: bad_pack
name: Bad pack
safety:
  max_writes: 1
files:
  - id: escape
    path: ../README.md
    template: templates/README.md
"#,
        );
        write_template(&temp, &["templates", "README.md"]);

        let pack = Pack::load(temp.path().to_path_buf()).unwrap();
        assert!(pack.validate().is_err());
    }

    #[test]
    fn rejects_app_session_without_prompt() {
        let temp = pack_dir(
            r#"
schema: 1
id: bad_pack
name: Bad pack
safety:
  max_writes: 1
warmup:
  - id: app
    title: App warmup
    kind: app_session
"#,
        );

        let pack = Pack::load(temp.path().to_path_buf()).unwrap();
        assert!(pack.validate().is_err());
    }

    fn pack_dir(manifest: &str) -> TempDir {
        let temp = TempDir::new().unwrap();
        fs::write(temp.path().join("pack.yml"), manifest.trim_start()).unwrap();
        temp
    }

    fn write_template(temp: &TempDir, path: &[&str]) {
        let full = path
            .iter()
            .fold(temp.path().to_path_buf(), |path, part| path.join(part));
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, "template").unwrap();
    }
}

use std::{
    collections::HashSet,
    fs,
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::markers;

#[derive(Debug, Clone)]
pub struct Pack {
    root: PathBuf,
    manifest: PackManifest,
}

impl Pack {
    pub fn load(root: PathBuf) -> Result<Self> {
        let root = root
            .canonicalize()
            .with_context(|| format!("pack path '{}' does not exist", root.display()))?;
        let manifest_path = manifest_path(&root)?;
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
        let template_path = canonical_pack_file(&self.root, path, "template")?;
        fs::read_to_string(&template_path)
            .with_context(|| format!("failed to read template {}", template_path.display()))
    }
}

fn manifest_path(root: &Path) -> Result<PathBuf> {
    if let Some(pack_yml) = existing_canonical_pack_file(root, "pack.yml")? {
        return Ok(pack_yml);
    }

    if let Some(pack_yaml) = existing_canonical_pack_file(root, "pack.yaml")? {
        return Ok(pack_yaml);
    }

    bail!(
        "pack manifest not found in {}; expected pack.yml or pack.yaml",
        root.display()
    );
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
            match warmup.kind {
                WarmupKind::Note | WarmupKind::Checklist => {
                    reject_fields(
                        warmup,
                        &[
                            (warmup.prompt.is_some(), "prompt"),
                            (warmup.prompt_template.is_some(), "prompt_template"),
                            (warmup.mode.is_some(), "mode"),
                            (warmup.target.is_some(), "target"),
                            (warmup.trigger.is_some(), "trigger"),
                            (warmup.time.is_some(), "time"),
                            (warmup.day.is_some(), "day"),
                        ],
                    )?;
                }
                WarmupKind::AppSession => {
                    require_warmup_prompt(warmup)?;
                    reject_fields(
                        warmup,
                        &[
                            (warmup.body.is_some(), "body"),
                            (warmup.target.is_some(), "target"),
                            (warmup.trigger.is_some(), "trigger"),
                            (warmup.time.is_some(), "time"),
                            (warmup.day.is_some(), "day"),
                            (warmup.start_agent_task, "start_agent_task"),
                        ],
                    )?;
                }
                WarmupKind::AppLink => {
                    if warmup.target.is_none() {
                        bail!("warmup app_link '{}' requires target", warmup.id);
                    }
                    reject_fields(
                        warmup,
                        &[
                            (warmup.body.is_some(), "body"),
                            (warmup.prompt.is_some(), "prompt"),
                            (warmup.prompt_template.is_some(), "prompt_template"),
                            (warmup.mode.is_some(), "mode"),
                            (warmup.trigger.is_some(), "trigger"),
                            (warmup.time.is_some(), "time"),
                            (warmup.day.is_some(), "day"),
                            (warmup.start_agent_task, "start_agent_task"),
                        ],
                    )?;
                }
                WarmupKind::AutomationDraft => {
                    require_warmup_prompt(warmup)?;
                    if warmup.trigger.is_none() {
                        bail!("warmup automation_draft '{}' requires trigger", warmup.id);
                    }
                    validate_automation_schedule(warmup)?;
                    reject_fields(
                        warmup,
                        &[
                            (warmup.body.is_some(), "body"),
                            (warmup.mode.is_some(), "mode"),
                            (warmup.target.is_some(), "target"),
                            (warmup.start_agent_task, "start_agent_task"),
                        ],
                    )?;
                }
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
    pub target: Option<WarmupAppTarget>,
    pub trigger: Option<WarmupAutomationTrigger>,
    pub time: Option<String>,
    pub day: Option<WarmupAutomationDay>,
    #[serde(default)]
    pub start_agent_task: bool,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarmupKind {
    Note,
    Checklist,
    AppSession,
    AppLink,
    AutomationDraft,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarmupMode {
    Interactive,
    Plan,
    Autopilot,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarmupAppTarget {
    Home,
    MyWork,
    Repo,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarmupAutomationTrigger {
    Manual,
    Hourly,
    Daily,
    Weekly,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarmupAutomationDay {
    Sunday,
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
}

impl WarmupAutomationDay {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sunday => "sunday",
            Self::Monday => "monday",
            Self::Tuesday => "tuesday",
            Self::Wednesday => "wednesday",
            Self::Thursday => "thursday",
            Self::Friday => "friday",
            Self::Saturday => "saturday",
        }
    }
}

impl WarmupAutomationTrigger {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Hourly => "hourly",
            Self::Daily => "daily",
            Self::Weekly => "weekly",
        }
    }
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
    canonical_pack_file(root, value, "template")?;
    Ok(())
}

fn canonical_pack_file(root: &Path, relative_path: &str, kind: &str) -> Result<PathBuf> {
    validate_safe_path(kind, relative_path)?;
    let path = root.join(relative_path);
    let canonical = path
        .canonicalize()
        .with_context(|| format!("{kind} '{}' does not exist", path.display()))?;
    if !canonical.starts_with(root) {
        bail!(
            "{kind} '{}' resolves outside pack root '{}'",
            path.display(),
            root.display()
        );
    }
    if !canonical.is_file() {
        bail!("{kind} '{}' is not a file", path.display());
    }
    Ok(canonical)
}

fn existing_canonical_pack_file(root: &Path, relative_path: &str) -> Result<Option<PathBuf>> {
    let path = root.join(relative_path);
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(canonical_pack_file(
        root,
        relative_path,
        "pack manifest",
    )?))
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

fn require_warmup_prompt(warmup: &WarmupItem) -> Result<()> {
    if warmup.prompt.is_none() && warmup.prompt_template.is_none() {
        bail!(
            "warmup {} '{}' requires prompt or prompt_template",
            warmup_kind_name(warmup.kind),
            warmup.id
        );
    }

    Ok(())
}

fn reject_fields(warmup: &WarmupItem, fields: &[(bool, &str)]) -> Result<()> {
    for (present, field) in fields {
        if *present {
            bail!(
                "warmup {} '{}' does not support field '{}'",
                warmup_kind_name(warmup.kind),
                warmup.id,
                field
            );
        }
    }

    Ok(())
}

fn validate_automation_schedule(warmup: &WarmupItem) -> Result<()> {
    match warmup.trigger.expect("validated caller requires trigger") {
        WarmupAutomationTrigger::Manual | WarmupAutomationTrigger::Hourly => {
            if warmup.time.is_some() || warmup.day.is_some() {
                bail!(
                    "warmup automation_draft '{}' with manual/hourly trigger must not set time or day",
                    warmup.id
                );
            }
        }
        WarmupAutomationTrigger::Daily => {
            require_time(warmup)?;
            if warmup.day.is_some() {
                bail!(
                    "warmup automation_draft '{}' with daily trigger must not set day",
                    warmup.id
                );
            }
        }
        WarmupAutomationTrigger::Weekly => {
            require_time(warmup)?;
            if warmup.day.is_none() {
                bail!(
                    "warmup automation_draft '{}' with weekly trigger requires day",
                    warmup.id
                );
            }
        }
    }

    Ok(())
}

fn require_time(warmup: &WarmupItem) -> Result<()> {
    let Some(time) = &warmup.time else {
        bail!(
            "warmup automation_draft '{}' requires time for daily/weekly trigger",
            warmup.id
        );
    };

    let Some((hour, minute)) = time.split_once(':') else {
        bail!(
            "warmup automation_draft '{}' time must use HH:MM",
            warmup.id
        );
    };

    let valid = hour.len() == 2
        && minute.len() == 2
        && hour.chars().all(|ch| ch.is_ascii_digit())
        && minute.chars().all(|ch| ch.is_ascii_digit())
        && hour.parse::<u8>().is_ok_and(|hour| hour < 24)
        && minute.parse::<u8>().is_ok_and(|minute| minute < 60);

    if !valid {
        bail!(
            "warmup automation_draft '{}' time must use HH:MM",
            warmup.id
        );
    }

    Ok(())
}

fn warmup_kind_name(kind: WarmupKind) -> &'static str {
    match kind {
        WarmupKind::Note => "note",
        WarmupKind::Checklist => "checklist",
        WarmupKind::AppSession => "app_session",
        WarmupKind::AppLink => "app_link",
        WarmupKind::AutomationDraft => "automation_draft",
    }
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
    fn accepts_pack_yaml_manifest() {
        let temp = TempDir::new().unwrap();
        fs::write(
            temp.path().join("pack.yaml"),
            r#"
schema: 1
id: yaml_pack
name: YAML pack
safety:
  max_writes: 1
"#
            .trim_start(),
        )
        .unwrap();

        let pack = Pack::load(temp.path().to_path_buf()).unwrap();
        pack.validate().unwrap();
        assert_eq!(pack.manifest().id, "yaml_pack");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_template_symlink_escape() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let outside = temp.path().join("outside.md");
        fs::write(&outside, "secret").unwrap();

        let pack_root = temp.path().join("pack");
        fs::create_dir_all(pack_root.join("templates")).unwrap();
        symlink(&outside, pack_root.join("templates").join("readme.md")).unwrap();
        fs::write(
            pack_root.join("pack.yml"),
            r#"
schema: 1
id: bad_pack
name: Bad pack
safety:
  max_writes: 1
files:
  - id: readme
    path: README.md
    template: templates/readme.md
"#
            .trim_start(),
        )
        .unwrap();

        let pack = Pack::load(pack_root).unwrap();
        assert!(pack.validate().is_err());
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
    fn rejects_prompted_warmup_without_prompt() {
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

    #[test]
    fn rejects_app_link_without_target() {
        let temp = pack_dir(
            r#"
schema: 1
id: bad_pack
name: Bad pack
safety:
  max_writes: 1
warmup:
  - id: link
    title: App link
    kind: app_link
"#,
        );

        let pack = Pack::load(temp.path().to_path_buf()).unwrap();
        assert!(pack.validate().is_err());
    }

    #[test]
    fn rejects_automation_draft_with_bad_time() {
        let temp = pack_dir(
            r#"
schema: 1
id: bad_pack
name: Bad pack
safety:
  max_writes: 1
warmup:
  - id: automation
    title: Automation
    kind: automation_draft
    trigger: daily
    time: "9am"
    prompt_template: templates/warmup/automation.md
"#,
        );
        write_template(&temp, &["templates", "warmup", "automation.md"]);

        let pack = Pack::load(temp.path().to_path_buf()).unwrap();
        assert!(pack.validate().is_err());
    }

    #[test]
    fn rejects_irrelevant_warmup_fields() {
        let temp = pack_dir(
            r#"
schema: 1
id: bad_pack
name: Bad pack
safety:
  max_writes: 1
warmup:
  - id: link
    title: Link
    kind: app_link
    target: repo
    prompt: should not be here
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

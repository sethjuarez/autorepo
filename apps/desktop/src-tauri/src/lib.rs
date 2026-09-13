use autorepo::{
    executor::{ExecutionProgress, Executor, RepositoryHydrationMode},
    github::RepoRef,
    ops::Operation,
    pack::Pack,
    planner::{Plan, PlanContext, Planner},
};
use autorepo_observation::ALL_EVENT_KINDS;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, Manager};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    product_name: &'static str,
    cli_package: &'static str,
    version: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GithubAuthStatus {
    installed: bool,
    authenticated: bool,
    login: Option<String>,
    avatar_url: Option<String>,
    error_message: Option<&'static str>,
    api_authenticated: bool,
    api_login: Option<String>,
    api_avatar_url: Option<String>,
    api_error_message: Option<String>,
    api_token_source: Option<&'static str>,
}

#[derive(Debug, Deserialize)]
struct GithubUserResponse {
    login: String,
    avatar_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitHubRepositorySuggestion {
    full_name: String,
    description: Option<String>,
    private: bool,
    default_branch: String,
    url: String,
    pack_count: usize,
    pack_paths: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitHubRepositoryListItem {
    name: String,
    full_name: String,
    description: Option<String>,
    private: bool,
    default_branch: String,
    url: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitHubRepositoryOwner {
    login: String,
    kind: &'static str,
    avatar_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GitHubTargetRepositoryStatus {
    owner: String,
    name: String,
    full_name: String,
    exists: bool,
    private: Option<bool>,
    default_branch: Option<String>,
    url: Option<String>,
    can_push: bool,
    can_admin: bool,
    can_exact_hydrate: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct GitHubRepositoryResponse {
    name: String,
    full_name: String,
    description: Option<String>,
    #[serde(rename = "private")]
    is_private: bool,
    default_branch: String,
    html_url: String,
    permissions: Option<GitHubRepositoryPermissions>,
}

#[derive(Clone, Debug, Deserialize)]
struct GitHubRepositoryPermissions {
    push: bool,
    admin: bool,
}

#[derive(Debug, Deserialize)]
struct GitHubOrganizationResponse {
    login: String,
    avatar_url: Option<String>,
}

#[derive(Clone, Debug)]
struct GitHubToken {
    source: &'static str,
    value: String,
}

static GITHUB_TOKEN_CACHE: OnceLock<Mutex<Option<GitHubToken>>> = OnceLock::new();
static GITHUB_LOGIN_CACHE: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static GITHUB_OWNER_CACHE: OnceLock<Mutex<Option<Vec<GitHubRepositoryOwner>>>> = OnceLock::new();
static GITHUB_REPOSITORY_CACHE: OnceLock<Mutex<HashMap<String, Vec<GitHubRepositoryResponse>>>> =
    OnceLock::new();

#[derive(Debug, Deserialize)]
struct GitHubTreeResponse {
    tree: Vec<GitHubTreeItem>,
    truncated: bool,
}

#[derive(Debug, Deserialize)]
struct GitHubTreeItem {
    path: String,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PackValidationSummary {
    id: String,
    name: String,
    description: Option<String>,
    max_writes: u32,
    labels: usize,
    milestones: usize,
    files: usize,
    branches: usize,
    issues: usize,
    pull_requests: usize,
    workflow_dispatches: usize,
    warmups: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PackPlanPreview {
    repo: String,
    pack_id: String,
    pack_name: String,
    pack_description: Option<String>,
    allow_non_empty: bool,
    max_writes: u32,
    total_operations: usize,
    write_operations: usize,
    local_warmups: usize,
    operations: Vec<PlanOperationPreview>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PackDryRunReport {
    repo: String,
    pack_id: String,
    pack_name: String,
    total_operations: usize,
    write_operations: usize,
    local_warmups: usize,
    operations: Vec<PlanOperationPreview>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PackHydrateReport {
    repo: String,
    pack_id: String,
    pack_name: String,
    total_operations: usize,
    write_operations: usize,
    local_warmups: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PackHydrateProgress {
    run_id: String,
    index: usize,
    total: usize,
    id: String,
    kind: &'static str,
    target: String,
    status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PackListItem {
    id: String,
    name: String,
    description: Option<String>,
    source: String,
    location: String,
    write_operations: usize,
    warmups: usize,
    valid: bool,
    status: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlanOperationPreview {
    index: usize,
    kind: &'static str,
    id: String,
    target: String,
    details: Vec<String>,
    writes_to_github: bool,
    content_preview: Option<OperationContentPreview>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OperationContentPreview {
    title: String,
    subtitle: String,
    body: String,
    format: &'static str,
}

#[tauri_plugin_auditaur::auditaur_command(skip_all)]
fn app_info() -> AppInfo {
    AppInfo {
        product_name: "Autorepo",
        cli_package: "autorepo",
        version: env!("CARGO_PKG_VERSION"),
    }
}

#[tauri_plugin_auditaur::auditaur_command(skip_all)]
fn observation_kinds() -> Vec<&'static str> {
    ALL_EVENT_KINDS
        .iter()
        .map(|kind| kind.wire_name())
        .collect()
}

#[tauri_plugin_auditaur::auditaur_command(skip_all, err)]
async fn validate_pack(
    app: AppHandle,
    pack_source: String,
) -> Result<PackValidationSummary, String> {
    tauri::async_runtime::spawn_blocking(move || validate_pack_blocking(&app, &pack_source))
        .await
        .map_err(|_| "Pack validation task failed.".to_owned())?
}

fn validate_pack_blocking(
    app: &AppHandle,
    pack_source: &str,
) -> Result<PackValidationSummary, String> {
    let root = resolve_pack_source(app, pack_source)?;
    let pack = Pack::load(root).map_err(sanitize_pack_error)?;
    pack.validate().map_err(sanitize_pack_error)?;
    let manifest = pack.manifest();

    Ok(PackValidationSummary {
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        description: manifest.description.clone(),
        max_writes: manifest.safety.max_writes,
        labels: manifest.labels.len(),
        milestones: manifest.milestones.len(),
        files: manifest.files.len(),
        branches: manifest.branches.len(),
        issues: manifest.issues.len(),
        pull_requests: manifest.pull_requests.len(),
        workflow_dispatches: manifest.workflow_dispatches.len(),
        warmups: manifest.warmup.len(),
    })
}

#[tauri_plugin_auditaur::auditaur_command(skip_all, err)]
async fn preview_pack_plan(
    app: AppHandle,
    pack_source: String,
    repo_source: String,
    repo: String,
    allow_non_empty: bool,
) -> Result<PackPlanPreview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        preview_pack_plan_blocking(&app, &pack_source, &repo_source, repo, allow_non_empty)
    })
    .await
    .map_err(|_| "Plan preview task failed.".to_owned())?
}

#[tauri_plugin_auditaur::auditaur_command(skip_all, err)]
async fn execute_pack_dry_run(
    app: AppHandle,
    pack_source: String,
    repo_source: String,
    repo: String,
    allow_non_empty: bool,
) -> Result<PackDryRunReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        execute_pack_dry_run_blocking(&app, &pack_source, &repo_source, repo, allow_non_empty)
    })
    .await
    .map_err(|_| "Dry-run task failed.".to_owned())?
}

#[tauri_plugin_auditaur::auditaur_command(skip_all, err)]
async fn execute_pack_hydrate(
    app: AppHandle,
    pack_source: String,
    repo_source: String,
    repo: String,
    allow_non_empty: bool,
    hydrate_mode: String,
    run_id: String,
) -> Result<PackHydrateReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        execute_pack_hydrate_blocking(
            &app,
            &pack_source,
            &repo_source,
            repo,
            allow_non_empty,
            &hydrate_mode,
            &run_id,
        )
    })
    .await
    .map_err(|_| "Hydrate task failed.".to_owned())?
}

fn execute_pack_dry_run_blocking(
    app: &AppHandle,
    pack_source: &str,
    repo_source: &str,
    repo: String,
    allow_non_empty: bool,
) -> Result<PackDryRunReport, String> {
    RepoRef::parse(&repo).map_err(sanitize_pack_error)?;
    let preview = preview_pack_plan_blocking(app, pack_source, repo_source, repo, allow_non_empty)?;
    Ok(PackDryRunReport {
        repo: preview.repo,
        pack_id: preview.pack_id,
        pack_name: preview.pack_name,
        total_operations: preview.total_operations,
        write_operations: preview.write_operations,
        local_warmups: preview.local_warmups,
        operations: preview.operations,
    })
}

fn execute_pack_hydrate_blocking(
    app: &AppHandle,
    pack_source: &str,
    repo_source: &str,
    repo: String,
    allow_non_empty: bool,
    hydrate_mode: &str,
    run_id: &str,
) -> Result<PackHydrateReport, String> {
    RepoRef::parse(&repo).map_err(sanitize_pack_error)?;
    let repository_mode = repository_hydration_mode(hydrate_mode)?;
    let (built, _repo_source_guard) =
        build_pack_plan(app, pack_source, repo_source, repo, allow_non_empty)?;
    let plan = built.plan;

    let runtime = tokio::runtime::Runtime::new()
        .map_err(|_| "Hydrate runtime could not be started.".to_owned())?;
    let progress_app = app.clone();
    let progress_run_id = run_id.to_owned();
    runtime
        .block_on(Executor.execute_serial_with_repository_mode(
            &plan,
            repository_mode,
            move |progress| {
                let _ = progress_app.emit(
                    "pack-hydrate-progress",
                    pack_hydrate_progress(&progress_run_id, progress),
                );
            },
        ))
        .map_err(sanitize_hydrate_error)?;

    Ok(PackHydrateReport {
        repo: plan.repo,
        pack_id: plan.pack_id,
        pack_name: plan.pack_name,
        total_operations: plan.operations.len(),
        write_operations: plan_operation_write_count(&plan.operations),
        local_warmups: plan.operations.len() - plan_operation_write_count(&plan.operations),
    })
}

fn preview_pack_plan_blocking(
    app: &AppHandle,
    pack_source: &str,
    repo_source: &str,
    repo: String,
    allow_non_empty: bool,
) -> Result<PackPlanPreview, String> {
    let (built, _repo_source_guard) =
        build_pack_plan(app, pack_source, repo_source, repo, allow_non_empty)?;
    let max_writes = built.max_writes;
    let plan = built.plan;
    let milestone_titles = milestone_titles(&plan.operations);

    let operations = plan
        .operations
        .iter()
        .enumerate()
        .map(|(index, operation)| operation_preview(index + 1, operation, &milestone_titles))
        .collect::<Vec<_>>();
    let write_operations = operations
        .iter()
        .filter(|operation| operation.writes_to_github)
        .count();
    let local_warmups = operations.len() - write_operations;

    Ok(PackPlanPreview {
        repo: plan.repo,
        pack_id: plan.pack_id,
        pack_name: plan.pack_name,
        pack_description: plan.pack_description,
        allow_non_empty: plan.allow_non_empty,
        max_writes,
        total_operations: operations.len(),
        write_operations,
        local_warmups,
        operations,
    })
}

struct BuiltPackPlan {
    plan: Plan,
    max_writes: u32,
}

fn build_pack_plan(
    app: &AppHandle,
    pack_source: &str,
    repo_source: &str,
    repo: String,
    allow_non_empty: bool,
) -> Result<(BuiltPackPlan, Option<ResolvedRepoSource>), String> {
    let repo_source_guard;
    let root = if let Some(relative_pack) = pack_source.strip_prefix("repo:") {
        let resolved_repo_source = ResolvedRepoSource::resolve(repo_source)?;
        let repo_root = resolved_repo_source.path().to_path_buf();
        let candidate = repo_root.join(relative_pack);
        let resolved = candidate
            .canonicalize()
            .map_err(|_| "Repository pack path does not exist.".to_owned())?;
        if !resolved.starts_with(&repo_root) {
            return Err("Repository pack path must stay inside the selected repo.".to_owned());
        }

        repo_source_guard = Some(resolved_repo_source);
        resolved
    } else {
        repo_source_guard = None;
        resolve_pack_source(app, pack_source)?
    };
    let pack = Pack::load(root).map_err(sanitize_pack_error)?;
    pack.validate().map_err(sanitize_pack_error)?;
    let max_writes = pack.manifest().safety.max_writes;
    let plan = Planner::new()
        .plan(
            &pack,
            PlanContext {
                repo,
                allow_non_empty,
            },
        )
        .map_err(sanitize_pack_error)?;

    Ok((BuiltPackPlan { plan, max_writes }, repo_source_guard))
}

fn plan_operation_write_count(operations: &[Operation]) -> usize {
    operations
        .iter()
        .filter(|operation| {
            matches!(
                operation,
                Operation::Label { .. }
                    | Operation::Milestone { .. }
                    | Operation::File { .. }
                    | Operation::Branch { .. }
                    | Operation::Issue { .. }
                    | Operation::PullRequest { .. }
                    | Operation::WorkflowDispatch { .. }
            )
        })
        .count()
}

fn pack_hydrate_progress(run_id: &str, progress: ExecutionProgress) -> PackHydrateProgress {
    let status = match progress.status {
        autorepo::executor::ExecutionProgressStatus::Started => "started",
        autorepo::executor::ExecutionProgressStatus::Completed => "completed",
        autorepo::executor::ExecutionProgressStatus::Skipped => "skipped",
        autorepo::executor::ExecutionProgressStatus::Failed => "failed",
    };
    PackHydrateProgress {
        run_id: run_id.to_owned(),
        index: progress.index,
        total: progress.total,
        id: progress.id,
        kind: progress.kind,
        target: progress.target,
        status: status.to_owned(),
    }
}

fn repository_hydration_mode(value: &str) -> Result<RepositoryHydrationMode, String> {
    match value {
        "exact" => Ok(RepositoryHydrationMode::ExactReset),
        "existing" => Ok(RepositoryHydrationMode::PreserveExisting),
        _ => Err("Unknown hydrate mode.".to_owned()),
    }
}

#[tauri_plugin_auditaur::auditaur_command(skip_all, err)]
async fn list_repo_packs(app: AppHandle, repo_source: String) -> Result<Vec<PackListItem>, String> {
    tauri::async_runtime::spawn_blocking(move || list_repo_packs_blocking(&app, &repo_source))
        .await
        .map_err(|_| "Pack discovery task failed.".to_owned())?
}

fn list_repo_packs_blocking(
    app: &AppHandle,
    repo_source: &str,
) -> Result<Vec<PackListItem>, String> {
    if matches!(repo_source.trim(), "builtin" | "generic-starter") {
        let builtin_root = resolve_pack_source(app, "builtin")?;
        let pack = Pack::load(builtin_root).map_err(sanitize_pack_error)?;
        return Ok(vec![pack_list_item(
            &pack,
            "builtin".to_owned(),
            "Built-in pack".to_owned(),
        )]);
    }

    let repo_source = ResolvedRepoSource::resolve(repo_source)?;
    let repo_root = repo_source.path();
    let mut packs = Vec::new();

    for pack_root in discover_pack_roots(&repo_root)? {
        let location = pack_root
            .strip_prefix(&repo_root)
            .ok()
            .and_then(|path| path.to_str())
            .filter(|value| !value.is_empty())
            .unwrap_or("Repository root")
            .to_owned();
        let source = if location == "Repository root" {
            "repo:.".to_owned()
        } else {
            format!("repo:{location}")
        };

        match Pack::load(pack_root) {
            Ok(pack) => {
                let mut item = pack_list_item(&pack, source, location);
                item.valid = pack.validate().is_ok();
                item.status = if item.valid { "Ready" } else { "Needs review" };
                packs.push(item);
            }
            Err(_) => packs.push(PackListItem {
                id: "unreadable-pack".to_owned(),
                name: "Unreadable pack".to_owned(),
                description: Some("Pack manifest could not be loaded.".to_owned()),
                source,
                location,
                write_operations: 0,
                warmups: 0,
                valid: false,
                status: "Blocked",
            }),
        }
    }

    packs.sort_by(|left, right| {
        left.location
            .cmp(&right.location)
            .then_with(|| left.name.cmp(&right.name))
    });
    packs.dedup_by(|left, right| left.source == right.source);

    Ok(packs)
}

fn pack_list_item(pack: &Pack, source: String, location: String) -> PackListItem {
    let manifest = pack.manifest();
    let write_operations = manifest.labels.len()
        + manifest.milestones.len()
        + manifest.files.len()
        + manifest.branches.len()
        + manifest.issues.len()
        + manifest.pull_requests.len()
        + manifest.workflow_dispatches.len();

    PackListItem {
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        description: manifest.description.clone(),
        source,
        location,
        write_operations,
        warmups: manifest.warmup.len(),
        valid: true,
        status: "Ready",
    }
}

fn discover_pack_roots(repo_root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut roots = Vec::new();
    let mut stack = vec![(repo_root.to_path_buf(), 0_usize)];

    while let Some((directory, depth)) = stack.pop() {
        if directory.join("pack.yml").is_file() || directory.join("pack.yaml").is_file() {
            roots.push(directory);
            continue;
        }

        if depth >= 6 {
            continue;
        }

        let entries =
            fs::read_dir(&directory).map_err(|_| "Repository source could not be read.")?;
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                continue;
            };

            if matches!(
                name,
                ".git" | ".auditaur" | "target" | "node_modules" | "dist" | ".next"
            ) {
                continue;
            }

            stack.push((path, depth + 1));
        }
    }

    Ok(roots)
}

struct ResolvedRepoSource {
    path: PathBuf,
    temp_root: Option<PathBuf>,
}

impl ResolvedRepoSource {
    fn resolve(value: &str) -> Result<Self, String> {
        let trimmed = value.trim();
        if trimmed.is_empty() || trimmed == "." || trimmed == "current" {
            return Ok(Self {
                path: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("..")
                    .join("..")
                    .join("..")
                    .canonicalize()
                    .map_err(|_| "Current repository source could not be resolved.".to_owned())?,
                temp_root: None,
            });
        }

        if trimmed.starts_with("github:") {
            return Self::clone_github(trimmed);
        }

        if trimmed.starts_with("https://github.com/") {
            return Self::clone_github_url(trimmed);
        }

        if RepoRef::parse(trimmed).is_ok() {
            return Self::clone_github(&format!("github:{trimmed}"));
        }

        Ok(Self {
            path: PathBuf::from(trimmed).canonicalize().map_err(|_| {
                "Repository source must be a local path or GitHub repository.".to_owned()
            })?,
            temp_root: None,
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn clone_github(value: &str) -> Result<Self, String> {
        let spec = value.trim_start_matches("github:");
        let (spec, git_ref) = split_ref_query(spec)?;
        let (repo, source_path) = spec.split_once("//").unwrap_or((spec, ""));
        let repo = RepoRef::parse(repo)
            .map_err(|_| "GitHub source must be in OWNER/REPO form.".to_owned())?;
        Self::clone_github_repo(&repo, source_path, git_ref)
    }

    fn clone_github_url(value: &str) -> Result<Self, String> {
        let without_prefix = value
            .strip_prefix("https://github.com/")
            .ok_or_else(|| "GitHub source URL must start with https://github.com/.".to_owned())?;
        let (path, git_ref) = split_ref_query(without_prefix)?;
        let parts = path.split('/').collect::<Vec<_>>();
        if parts.len() < 2 {
            return Err("GitHub source URL must include OWNER/REPO.".to_owned());
        }

        let repo_name = parts[1].trim_end_matches(".git");
        let repo = RepoRef::parse(&format!("{}/{}", parts[0], repo_name))
            .map_err(|_| "GitHub source URL must include OWNER/REPO.".to_owned())?;
        if parts.get(2) == Some(&"tree") {
            return Err("GitHub tree collection URLs are not supported yet; use github:OWNER/REPO//path?ref=BRANCH.".to_owned());
        }

        let source_path = parts.get(2..).unwrap_or(&[]).join("/");
        Self::clone_github_repo(&repo, &source_path, git_ref)
    }

    fn clone_github_repo(
        repo: &RepoRef,
        source_path: &str,
        git_ref: Option<&str>,
    ) -> Result<Self, String> {
        validate_remote_source_path(source_path)?;
        let temp_root = temp_repo_root()?;
        fs::create_dir_all(&temp_root)
            .map_err(|_| "Temporary clone directory could not be created.".to_owned())?;

        let checkout = temp_root.join("repo");
        let clone_url = format!("https://github.com/{}/{}.git", repo.owner, repo.name);
        let mut command = Command::new("git");
        command
            .args(["clone", "--depth", "1"])
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GCM_INTERACTIVE", "never")
            .env("GIT_ASKPASS", "echo");
        if let Some(git_ref) = git_ref {
            command.args(["--branch", git_ref]);
        }
        let output = command
            .arg(&clone_url)
            .arg(&checkout)
            .output()
            .map_err(|_| "GitHub repository clone could not be started.".to_owned())?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let _ = fs::remove_dir_all(&temp_root);
            return Err(format!("GitHub repository clone failed: {stderr}"));
        }

        let checkout_root = checkout
            .canonicalize()
            .map_err(|_| "GitHub repository checkout could not be resolved.".to_owned())?;
        let source = if source_path.trim().is_empty() {
            checkout_root.clone()
        } else {
            checkout_root.join(source_path)
        }
        .canonicalize()
        .map_err(|_| "GitHub repository source path could not be resolved.".to_owned())?;

        if !source.starts_with(&checkout_root) {
            let _ = fs::remove_dir_all(&temp_root);
            return Err(
                "GitHub repository source path must stay inside the repository.".to_owned(),
            );
        }

        Ok(Self {
            path: source,
            temp_root: Some(temp_root),
        })
    }
}

impl Drop for ResolvedRepoSource {
    fn drop(&mut self) {
        if let Some(temp_root) = &self.temp_root {
            let _ = fs::remove_dir_all(temp_root);
        }
    }
}

fn split_ref_query(value: &str) -> Result<(&str, Option<&str>), String> {
    let Some((path, query)) = value.split_once('?') else {
        return Ok((value, None));
    };

    let Some(git_ref) = query.strip_prefix("ref=") else {
        return Err("GitHub source supports only ?ref=<branch-or-tag>.".to_owned());
    };
    if git_ref.contains('&') || git_ref.trim().is_empty() {
        return Err("GitHub source ?ref must be a single non-empty branch or tag.".to_owned());
    }

    Ok((path, Some(git_ref)))
}

fn validate_remote_source_path(value: &str) -> Result<(), String> {
    let path = Path::new(value);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(
            "GitHub repository source path must be relative and stay inside the repository."
                .to_owned(),
        );
    }

    Ok(())
}

fn temp_repo_root() -> Result<PathBuf, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "System clock is before UNIX_EPOCH.".to_owned())?
        .as_nanos();
    Ok(std::env::temp_dir().join(format!("autorepo-repo-{}-{now}", std::process::id())))
}

fn milestone_titles(operations: &[Operation]) -> HashMap<&str, &str> {
    operations
        .iter()
        .filter_map(|operation| match operation {
            Operation::Milestone { id, title, .. } => Some((id.as_str(), title.as_str())),
            _ => None,
        })
        .collect()
}

fn operation_preview(
    index: usize,
    operation: &Operation,
    milestone_titles: &HashMap<&str, &str>,
) -> PlanOperationPreview {
    match operation {
        Operation::Label {
            id,
            name,
            color,
            description,
            ..
        } => PlanOperationPreview {
            index,
            kind: "Label",
            id: id.clone(),
            target: name.clone(),
            details: optional_details([
                color.as_ref().map(|value| format!("#{value}")),
                description.clone(),
            ]),
            writes_to_github: true,
            content_preview: description.clone().map(|body| OperationContentPreview {
                title: name.clone(),
                subtitle: "GitHub label description".to_owned(),
                body,
                format: "text",
            }),
        },
        Operation::Milestone {
            id,
            title,
            description,
            ..
        } => PlanOperationPreview {
            index,
            kind: "Milestone",
            id: id.clone(),
            target: title.clone(),
            details: optional_details([description.clone(), None]),
            writes_to_github: true,
            content_preview: description.clone().map(|body| OperationContentPreview {
                title: title.clone(),
                subtitle: "GitHub milestone description".to_owned(),
                body,
                format: "markdown",
            }),
        },
        Operation::File {
            id, path, content, ..
        } => PlanOperationPreview {
            index,
            kind: "File",
            id: id.clone(),
            target: path.clone(),
            details: vec!["Create file from pack template".to_owned()],
            writes_to_github: true,
            content_preview: Some(OperationContentPreview {
                title: path.clone(),
                subtitle: "File content preview".to_owned(),
                body: content.clone(),
                format: file_preview_format(path),
            }),
        },
        Operation::Branch { id, name, .. } => PlanOperationPreview {
            index,
            kind: "Branch",
            id: id.clone(),
            target: name.clone(),
            details: vec!["Create branch from default branch".to_owned()],
            writes_to_github: true,
            content_preview: None,
        },
        Operation::Issue {
            id,
            title,
            body,
            labels,
            milestone,
            ..
        } => PlanOperationPreview {
            index,
            kind: "Issue",
            id: id.clone(),
            target: title.clone(),
            details: labeled_details(
                labels,
                milestone
                    .as_deref()
                    .map(|id| ("Milestone", milestone_titles.get(id).copied().unwrap_or(id))),
            ),
            writes_to_github: true,
            content_preview: Some(OperationContentPreview {
                title: title.clone(),
                subtitle: "GitHub issue preview".to_owned(),
                body: body.clone(),
                format: "markdown",
            }),
        },
        Operation::PullRequest {
            id,
            title,
            body,
            branch,
            labels,
            ..
        } => PlanOperationPreview {
            index,
            kind: "Pull request",
            id: id.clone(),
            target: title.clone(),
            details: labeled_details(labels, Some(("Branch", branch))),
            writes_to_github: true,
            content_preview: Some(OperationContentPreview {
                title: title.clone(),
                subtitle: format!("GitHub pull request preview from {branch}"),
                body: body.clone(),
                format: "markdown",
            }),
        },
        Operation::WorkflowDispatch {
            id,
            workflow,
            inputs,
            ..
        } => {
            let input_count = inputs
                .as_object()
                .map(|value| value.len())
                .unwrap_or_default();

            PlanOperationPreview {
                index,
                kind: "Workflow dispatch",
                id: id.clone(),
                target: workflow.clone(),
                details: vec![format!(
                    "{} input{}",
                    input_count,
                    if input_count == 1 { "" } else { "s" }
                )],
                writes_to_github: true,
                content_preview: Some(OperationContentPreview {
                    title: workflow.clone(),
                    subtitle: "Workflow dispatch inputs".to_owned(),
                    body: serde_json::to_string_pretty(inputs).unwrap_or_else(|_| "{}".to_owned()),
                    format: "json",
                }),
            }
        }
        Operation::WarmupNote { id, title, .. } => warmup_preview(index, "Warmup note", id, title),
        Operation::WarmupChecklist { id, title, .. } => {
            warmup_preview(index, "Warmup checklist", id, title)
        }
        Operation::WarmupAppSession { id, title, .. } => {
            warmup_preview(index, "Warmup app session", id, title)
        }
        Operation::WarmupAppLink { id, title, .. } => {
            warmup_preview(index, "Warmup app link", id, title)
        }
        Operation::WarmupAutomationDraft { id, title, .. } => {
            warmup_preview(index, "Warmup automation draft", id, title)
        }
    }
}

fn optional_details<const N: usize>(values: [Option<String>; N]) -> Vec<String> {
    values.into_iter().flatten().collect()
}

fn labeled_details(labels: &[String], secondary: Option<(&str, &str)>) -> Vec<String> {
    let mut details = Vec::new();
    if !labels.is_empty() {
        details.push(format!("Labels: {}", labels.join(", ")));
    }
    if let Some((label, value)) = secondary {
        details.push(format!("{label}: {value}"));
    }
    details
}

fn warmup_preview(index: usize, kind: &'static str, id: &str, title: &str) -> PlanOperationPreview {
    PlanOperationPreview {
        index,
        kind,
        id: id.to_owned(),
        target: title.to_owned(),
        details: vec!["Local app warmup guidance".to_owned()],
        writes_to_github: false,
        content_preview: None,
    }
}

fn file_preview_format(path: &str) -> &'static str {
    if path.ends_with(".md") || path.ends_with(".markdown") {
        "markdown"
    } else if path.ends_with(".json") {
        "json"
    } else {
        "code"
    }
}

fn resolve_pack_source(app: &AppHandle, value: &str) -> Result<PathBuf, String> {
    if value == "builtin" || value == "generic-starter" {
        let resource_pack = app
            .path()
            .resource_dir()
            .ok()
            .map(|resource_dir| resource_dir.join("packs").join("generic-starter"))
            .filter(|path| path.exists());
        if let Some(resource_pack) = resource_pack {
            return Ok(resource_pack);
        }

        return Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("apps")
            .join("cli")
            .join("packs")
            .join("generic-starter"));
    }

    if value.starts_with("github:") || value.starts_with("https://github.com/") {
        return Err(
            "Remote GitHub pack sources are still CLI-only in this desktop preview.".to_owned(),
        );
    }

    PathBuf::from(value)
        .canonicalize()
        .map_err(|_| "Pack path does not exist.".to_owned())
}

#[tauri_plugin_auditaur::auditaur_command(skip_all, err)]
async fn github_auth_status() -> Result<GithubAuthStatus, String> {
    tauri::async_runtime::spawn_blocking(github_auth_status_blocking)
        .await
        .map_err(|_| "GitHub authentication check failed.".to_owned())?
}

#[tauri_plugin_auditaur::auditaur_command(skip_all, err)]
async fn search_github_repositories(
    owner: Option<String>,
    query: String,
) -> Result<Vec<GitHubRepositorySuggestion>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        search_github_repositories_blocking(owner.as_deref(), &query)
    })
    .await
    .map_err(|_| "GitHub repository search failed.".to_owned())?
}

#[tauri_plugin_auditaur::auditaur_command(skip_all, err)]
async fn list_github_repository_owners() -> Result<Vec<GitHubRepositoryOwner>, String> {
    tauri::async_runtime::spawn_blocking(list_github_repository_owners_blocking)
        .await
        .map_err(|_| "GitHub owner list failed.".to_owned())?
}

#[tauri_plugin_auditaur::auditaur_command(skip_all, err)]
async fn list_github_owner_repositories(
    owner: String,
) -> Result<Vec<GitHubRepositoryListItem>, String> {
    tauri::async_runtime::spawn_blocking(move || list_github_owner_repositories_blocking(&owner))
        .await
        .map_err(|_| "GitHub repository list failed.".to_owned())?
}

#[tauri_plugin_auditaur::auditaur_command(skip_all, err)]
async fn check_github_repository_packs(
    owner: String,
    repo: String,
) -> Result<GitHubRepositorySuggestion, String> {
    tauri::async_runtime::spawn_blocking(move || {
        check_github_repository_packs_blocking(&owner, &repo)
    })
    .await
    .map_err(|_| "GitHub repository pack check failed.".to_owned())?
}

#[tauri_plugin_auditaur::auditaur_command(skip_all, err)]
async fn check_github_target_repository(
    owner: String,
    repo: String,
) -> Result<GitHubTargetRepositoryStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        check_github_target_repository_blocking(&owner, &repo)
    })
    .await
    .map_err(|_| "GitHub target repository check failed.".to_owned())?
}

fn list_github_repository_owners_blocking() -> Result<Vec<GitHubRepositoryOwner>, String> {
    if let Some(owners) = GITHUB_OWNER_CACHE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|cache| cache.clone())
    {
        return Ok(owners);
    }

    let Some((_token_source, token)) = github_api_token() else {
        return Err("Connect GitHub before loading organizations.".to_owned());
    };

    let user = ureq::get("https://api.github.com/user")
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .set("User-Agent", "autorepo-desktop")
        .set("Authorization", &format!("Bearer {token}"))
        .call()
        .map_err(github_repo_search_error)?
        .into_json::<GithubUserResponse>()
        .map_err(|_| "GitHub returned an unreadable user response.".to_owned())?;

    let organizations = ureq::get("https://api.github.com/user/orgs")
        .query("per_page", "100")
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .set("User-Agent", "autorepo-desktop")
        .set("Authorization", &format!("Bearer {token}"))
        .call()
        .map_err(github_repo_search_error)?
        .into_json::<Vec<GitHubOrganizationResponse>>()
        .map_err(|_| "GitHub returned an unreadable organization list.".to_owned())?;

    let user_login = user.login;
    if let Ok(mut cache) = GITHUB_LOGIN_CACHE.get_or_init(|| Mutex::new(None)).lock() {
        *cache = Some(user_login.clone());
    }

    let mut owners = vec![GitHubRepositoryOwner {
        login: user_login,
        kind: "User",
        avatar_url: Some(user.avatar_url),
    }];
    owners.extend(organizations.into_iter().map(|org| GitHubRepositoryOwner {
        login: org.login,
        kind: "Organization",
        avatar_url: org.avatar_url,
    }));
    owners.sort_by(|left, right| {
        left.kind
            .cmp(right.kind)
            .then_with(|| left.login.cmp(&right.login))
    });
    if let Ok(mut cache) = GITHUB_OWNER_CACHE.get_or_init(|| Mutex::new(None)).lock() {
        *cache = Some(owners.clone());
    }
    Ok(owners)
}

fn list_github_owner_repositories_blocking(
    owner: &str,
) -> Result<Vec<GitHubRepositoryListItem>, String> {
    let Some((_token_source, token)) = github_api_token() else {
        return Err("Connect GitHub before loading repositories.".to_owned());
    };

    Ok(cached_github_owner_repositories(&token, owner)?
        .into_iter()
        .map(|repo| GitHubRepositoryListItem {
            name: repo.name,
            full_name: repo.full_name,
            description: repo.description,
            private: repo.is_private,
            default_branch: repo.default_branch,
            url: repo.html_url,
        })
        .collect())
}

fn check_github_repository_packs_blocking(
    owner: &str,
    repo: &str,
) -> Result<GitHubRepositorySuggestion, String> {
    let Some((_token_source, token)) = github_api_token() else {
        return Err("Connect GitHub before checking repositories.".to_owned());
    };
    let full_name = if repo.contains('/') {
        repo.to_owned()
    } else {
        format!("{owner}/{repo}")
    };
    let Some(repository) = github_repository(&token, &full_name)? else {
        return Err(format!(
            "Repository {full_name} was not found or is not accessible."
        ));
    };
    let pack_paths = github_pack_paths(&token, &repository.full_name, &repository.default_branch)?;
    if pack_paths.is_empty() {
        return Err(format!(
            "{} does not contain pack.yml or pack.yaml manifests.",
            repository.full_name
        ));
    }

    Ok(GitHubRepositorySuggestion {
        pack_count: pack_paths.len(),
        pack_paths,
        full_name: repository.full_name,
        description: repository.description,
        private: repository.is_private,
        default_branch: repository.default_branch,
        url: repository.html_url,
    })
}

fn check_github_target_repository_blocking(
    owner: &str,
    repo: &str,
) -> Result<GitHubTargetRepositoryStatus, String> {
    let owner = owner.trim();
    let repo = repo.trim();
    if owner.is_empty() || repo.is_empty() {
        return Err("Choose an organization and repository name.".to_owned());
    }
    if repo.contains('/') || repo.contains('\\') {
        return Err("Repository name must not include an owner or path separator.".to_owned());
    }

    let Some((_token_source, token)) = github_api_token() else {
        return Err("Connect GitHub before checking the target repository.".to_owned());
    };

    let full_name = format!("{owner}/{repo}");
    let Some(repository) = github_repository(&token, &full_name)? else {
        return Ok(GitHubTargetRepositoryStatus {
            owner: owner.to_owned(),
            name: repo.to_owned(),
            full_name,
            exists: false,
            private: None,
            default_branch: None,
            url: None,
            can_push: true,
            can_admin: true,
            can_exact_hydrate: true,
        });
    };

    let can_push = repository
        .permissions
        .as_ref()
        .map(|permissions| permissions.push)
        .unwrap_or(false);
    let can_admin = repository
        .permissions
        .as_ref()
        .map(|permissions| permissions.admin)
        .unwrap_or(false);
    let token_can_delete_repo = github_token_has_scope(&token, "delete_repo")?;
    let can_exact_hydrate = can_admin && token_can_delete_repo;

    Ok(GitHubTargetRepositoryStatus {
        owner: owner.to_owned(),
        name: repository.name,
        full_name: repository.full_name,
        exists: true,
        private: Some(repository.is_private),
        default_branch: Some(repository.default_branch),
        url: Some(repository.html_url),
        can_push,
        can_admin,
        can_exact_hydrate,
    })
}

fn search_github_repositories_blocking(
    owner: Option<&str>,
    query: &str,
) -> Result<Vec<GitHubRepositorySuggestion>, String> {
    let Some((_token_source, token)) = github_api_token() else {
        return Err("Connect GitHub before searching repositories.".to_owned());
    };

    let normalized_query = query.trim().to_lowercase();
    let mut repositories = Vec::new();

    for page in 1..=10 {
        let response = ureq::get("https://api.github.com/user/repos")
            .query("affiliation", "owner,collaborator,organization_member")
            .query("visibility", "all")
            .query("sort", "updated")
            .query("per_page", "100")
            .query("page", &page.to_string())
            .set("Accept", "application/vnd.github+json")
            .set("X-GitHub-Api-Version", "2022-11-28")
            .set("User-Agent", "autorepo-desktop")
            .set("Authorization", &format!("Bearer {token}"))
            .call()
            .map_err(github_repo_search_error)?;

        let page_repositories = response
            .into_json::<Vec<GitHubRepositoryResponse>>()
            .map_err(|_| "GitHub returned an unreadable repository list.".to_owned())?;
        let page_len = page_repositories.len();
        repositories.extend(page_repositories);
        if page_len < 100 {
            break;
        }
    }

    if let Some(exact_repo) = exact_owner_repo_candidate(owner, query) {
        if !repositories.iter().any(|repo| repo.full_name == exact_repo) {
            if let Some(repository) = github_repository(&token, &exact_repo)? {
                repositories.insert(0, repository);
            }
        }
    }

    Ok(repositories
        .into_iter()
        .filter(|repo| {
            normalized_query.is_empty()
                || repo.full_name.to_lowercase().contains(&normalized_query)
                || repo
                    .description
                    .as_deref()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(&normalized_query)
        })
        .filter(|repo| owner.is_none_or(|owner| repo.full_name.starts_with(&format!("{owner}/"))))
        .take(20)
        .filter_map(|repo| {
            let pack_paths =
                github_pack_paths(&token, &repo.full_name, &repo.default_branch).ok()?;
            if pack_paths.is_empty() {
                return None;
            }

            Some(GitHubRepositorySuggestion {
                pack_count: pack_paths.len(),
                pack_paths,
                full_name: repo.full_name,
                description: repo.description,
                private: repo.is_private,
                default_branch: repo.default_branch,
                url: repo.html_url,
            })
        })
        .take(12)
        .collect())
}

fn github_owner_repositories(
    token: &str,
    owner: &str,
) -> Result<Vec<GitHubRepositoryResponse>, String> {
    if github_authenticated_login(token)
        .map(|login| login.eq_ignore_ascii_case(owner))
        .unwrap_or(false)
    {
        return github_user_owned_repositories(token);
    }

    if let Some(repositories) = github_organization_repositories(token, owner)? {
        return Ok(repositories);
    }

    github_public_user_repositories(token, owner)
}

fn cached_github_owner_repositories(
    token: &str,
    owner: &str,
) -> Result<Vec<GitHubRepositoryResponse>, String> {
    let cache_key = owner.to_lowercase();
    if let Some(repositories) = GITHUB_REPOSITORY_CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .ok()
        .and_then(|cache| cache.get(&cache_key).cloned())
    {
        return Ok(repositories);
    }

    let repositories = github_owner_repositories(token, owner)?;
    if let Ok(mut cache) = GITHUB_REPOSITORY_CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        cache.insert(cache_key, repositories.clone());
    }
    Ok(repositories)
}

fn github_user_owned_repositories(token: &str) -> Result<Vec<GitHubRepositoryResponse>, String> {
    github_paginated_repositories(
        token,
        "https://api.github.com/user/repos",
        &[("affiliation", "owner"), ("visibility", "all")],
    )
}

fn github_organization_repositories(
    token: &str,
    owner: &str,
) -> Result<Option<Vec<GitHubRepositoryResponse>>, String> {
    let url = format!("https://api.github.com/orgs/{owner}/repos");
    match github_paginated_repositories(token, &url, &[("type", "all")]) {
        Ok(repositories) => Ok(Some(repositories)),
        Err(error) if error.contains("404") || error.contains("not found") => Ok(None),
        Err(error) => Err(error),
    }
}

fn github_public_user_repositories(
    token: &str,
    owner: &str,
) -> Result<Vec<GitHubRepositoryResponse>, String> {
    let url = format!("https://api.github.com/users/{owner}/repos");
    github_paginated_repositories(token, &url, &[("type", "owner")])
}

fn github_paginated_repositories(
    token: &str,
    url: &str,
    query: &[(&str, &str)],
) -> Result<Vec<GitHubRepositoryResponse>, String> {
    let mut repositories = Vec::new();

    for page in 1..=10 {
        let mut request = ureq::get(url)
            .query("sort", "full_name")
            .query("direction", "asc")
            .query("per_page", "100")
            .query("page", &page.to_string())
            .set("Accept", "application/vnd.github+json")
            .set("X-GitHub-Api-Version", "2022-11-28")
            .set("User-Agent", "autorepo-desktop")
            .set("Authorization", &format!("Bearer {token}"));
        for (name, value) in query {
            request = request.query(name, value);
        }

        let response = request.call().map_err(github_repo_search_error)?;
        let page_repositories = response
            .into_json::<Vec<GitHubRepositoryResponse>>()
            .map_err(|_| "GitHub returned an unreadable repository list.".to_owned())?;
        let page_len = page_repositories.len();
        repositories.extend(page_repositories);
        if page_len < 100 {
            break;
        }
    }

    repositories.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(repositories)
}

fn github_authenticated_login(token: &str) -> Result<String, String> {
    if let Some(login) = GITHUB_LOGIN_CACHE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|cache| cache.clone())
    {
        return Ok(login);
    }

    let user = ureq::get("https://api.github.com/user")
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .set("User-Agent", "autorepo-desktop")
        .set("Authorization", &format!("Bearer {token}"))
        .call()
        .map_err(github_repo_search_error)?
        .into_json::<GithubUserResponse>()
        .map_err(|_| "GitHub returned an unreadable user response.".to_owned())?;
    if let Ok(mut cache) = GITHUB_LOGIN_CACHE.get_or_init(|| Mutex::new(None)).lock() {
        *cache = Some(user.login.clone());
    }
    Ok(user.login)
}

fn exact_owner_repo_candidate(owner: Option<&str>, query: &str) -> Option<String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Some(repo) = trimmed.strip_prefix("github:") {
        return Some(repo.to_owned());
    }
    if let Some(repo) = trimmed.strip_prefix("https://github.com/") {
        let repo = repo
            .trim_end_matches('/')
            .split('/')
            .take(2)
            .collect::<Vec<_>>();
        return (repo.len() == 2).then(|| format!("{}/{}", repo[0], repo[1]));
    }
    if trimmed.matches('/').count() == 1 {
        return Some(trimmed.to_owned());
    }

    owner.map(|owner| format!("{owner}/{trimmed}"))
}

fn github_repository(token: &str, repo: &str) -> Result<Option<GitHubRepositoryResponse>, String> {
    let Some((owner, name)) = repo.split_once('/') else {
        return Ok(None);
    };
    let url = format!("https://api.github.com/repos/{owner}/{name}");
    match ureq::get(&url)
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .set("User-Agent", "autorepo-desktop")
        .set("Authorization", &format!("Bearer {token}"))
        .call()
    {
        Ok(response) => response
            .into_json::<GitHubRepositoryResponse>()
            .map(Some)
            .map_err(|_| "GitHub returned an unreadable repository response.".to_owned()),
        Err(ureq::Error::Status(404, _)) => Ok(None),
        Err(error) => Err(github_repo_search_error(error)),
    }
}

fn github_token_has_scope(token: &str, scope: &str) -> Result<bool, String> {
    let response = ureq::get("https://api.github.com/user")
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .set("User-Agent", "autorepo-desktop")
        .set("Authorization", &format!("Bearer {token}"))
        .call()
        .map_err(github_repo_search_error)?;
    let scopes = response.header("x-oauth-scopes").unwrap_or("");
    Ok(scopes
        .split(',')
        .map(str::trim)
        .any(|token_scope| token_scope == scope))
}

fn github_pack_paths(token: &str, repo: &str, default_branch: &str) -> Result<Vec<String>, String> {
    let Some((owner, name)) = repo.split_once('/') else {
        return Ok(Vec::new());
    };
    let url = format!("https://api.github.com/repos/{owner}/{name}/git/trees/{default_branch}");
    let response = ureq::get(&url)
        .query("recursive", "1")
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .set("User-Agent", "autorepo-desktop")
        .set("Authorization", &format!("Bearer {token}"))
        .call()
        .map_err(github_repo_search_error)?;
    let tree = response
        .into_json::<GitHubTreeResponse>()
        .map_err(|_| "GitHub returned an unreadable repository tree.".to_owned())?;
    if tree.truncated {
        return Ok(Vec::new());
    }

    Ok(tree
        .tree
        .into_iter()
        .filter(|item| {
            item.kind == "blob"
                && (item.path.ends_with("pack.yml") || item.path.ends_with("pack.yaml"))
        })
        .map(|item| item.path)
        .collect())
}

fn github_repo_search_error(error: ureq::Error) -> String {
    match error {
        ureq::Error::Status(401, _) => "GitHub rejected the configured token.".to_owned(),
        ureq::Error::Status(403, _) => {
            "GitHub token is valid but lacks repository access or is rate-limited.".to_owned()
        }
        ureq::Error::Status(status, _) => {
            format!("GitHub repository search failed with HTTP {status}.")
        }
        _ => "GitHub repository search could not reach GitHub.".to_owned(),
    }
}

fn github_auth_status_blocking() -> Result<GithubAuthStatus, String> {
    let api_status = github_api_auth_status();
    let status_output = gh_command()
        .args(["auth", "status", "--hostname", "github.com"])
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .output();

    let Ok(status_output) = status_output else {
        return Ok(GithubAuthStatus {
            installed: false,
            authenticated: false,
            login: None,
            avatar_url: None,
            error_message: Some("GitHub CLI is not installed or is not on PATH."),
            api_authenticated: api_status.authenticated,
            api_login: api_status.login,
            api_avatar_url: api_status.avatar_url,
            api_error_message: api_status.error_message,
            api_token_source: api_status.token_source,
        });
    };

    if !status_output.status.success() {
        return Ok(GithubAuthStatus {
            installed: true,
            authenticated: false,
            login: None,
            avatar_url: None,
            error_message: Some(
                "GitHub CLI is installed, but no GitHub.com authentication was found.",
            ),
            api_authenticated: api_status.authenticated,
            api_login: api_status.login,
            api_avatar_url: api_status.avatar_url,
            api_error_message: api_status.error_message,
            api_token_source: api_status.token_source,
        });
    }

    Ok(GithubAuthStatus {
        installed: true,
        authenticated: true,
        login: api_status.login.clone(),
        avatar_url: api_status.avatar_url.clone(),
        error_message: None,
        api_authenticated: api_status.authenticated,
        api_login: api_status.login,
        api_avatar_url: api_status.avatar_url,
        api_error_message: api_status.error_message,
        api_token_source: api_status.token_source,
    })
}

struct GithubApiAuthStatus {
    authenticated: bool,
    login: Option<String>,
    avatar_url: Option<String>,
    error_message: Option<String>,
    token_source: Option<&'static str>,
}

fn github_api_auth_status() -> GithubApiAuthStatus {
    let Some((token_source, token)) = github_api_token() else {
        return GithubApiAuthStatus {
            authenticated: false,
            login: None,
            avatar_url: None,
            error_message: Some(
                "No GitHub API token is available from the environment or GitHub CLI sign-in."
                    .to_owned(),
            ),
            token_source: None,
        };
    };

    let response = ureq::get("https://api.github.com/user")
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .set("User-Agent", "autorepo-desktop")
        .set("Authorization", &format!("Bearer {token}"))
        .call();

    match response {
        Ok(response) => match response.into_json::<GithubUserResponse>() {
            Ok(user) => GithubApiAuthStatus {
                authenticated: true,
                login: Some(user.login),
                avatar_url: Some(user.avatar_url),
                error_message: None,
                token_source: Some(token_source),
            },
            Err(_) => GithubApiAuthStatus {
                authenticated: false,
                login: None,
                avatar_url: None,
                error_message: Some("GitHub API returned an unreadable user response.".to_owned()),
                token_source: Some(token_source),
            },
        },
        Err(ureq::Error::Status(status, _)) => GithubApiAuthStatus {
            authenticated: false,
            login: None,
            avatar_url: None,
            error_message: Some(match status {
                401 => "GitHub API rejected the configured token.".to_owned(),
                403 => "GitHub API token is valid but lacks access or is rate-limited.".to_owned(),
                _ => format!("GitHub API authentication check failed with HTTP {status}."),
            }),
            token_source: Some(token_source),
        },
        Err(_) => GithubApiAuthStatus {
            authenticated: false,
            login: None,
            avatar_url: None,
            error_message: Some(
                "GitHub API authentication check could not reach GitHub.".to_owned(),
            ),
            token_source: Some(token_source),
        },
    }
}

fn github_api_token() -> Option<(&'static str, String)> {
    if let Some(token) = GITHUB_TOKEN_CACHE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|cache| cache.clone())
    {
        return Some((token.source, token.value));
    }

    let env_token = ["AUTOREPO_GITHUB_TOKEN", "GITHUB_TOKEN"]
        .into_iter()
        .find_map(|name| {
            std::env::var(name)
                .ok()
                .map(|token| token.trim().to_owned())
                .filter(|token| !token.is_empty())
                .map(|token| (name, token))
        });

    let token = env_token.or_else(github_cli_token)?;
    if let Ok(mut cache) = GITHUB_TOKEN_CACHE.get_or_init(|| Mutex::new(None)).lock() {
        *cache = Some(GitHubToken {
            source: token.0,
            value: token.1.clone(),
        });
    }
    Some(token)
}

fn github_cli_token() -> Option<(&'static str, String)> {
    gh_command()
        .args(["auth", "token", "--hostname", "github.com"])
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .output()
        .ok()
        .and_then(|output| {
            output
                .status
                .success()
                .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        })
        .filter(|token| !token.is_empty())
        .map(|token| ("GitHub CLI", token))
}

fn gh_command() -> Command {
    let mut command = Command::new("gh");
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

fn sanitize_pack_error(error: anyhow::Error) -> String {
    let message = error.to_string();
    if message.contains(":\\") || message.contains(":/") {
        "Pack could not be loaded or validated. Check the source and manifest.".to_owned()
    } else {
        message
    }
}

fn sanitize_hydrate_error(error: anyhow::Error) -> String {
    let message = error.to_string();
    if message.contains(":\\") {
        "Hydrate failed while reading a local pack path.".to_owned()
    } else {
        message
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(error) = tracing_subscriber::registry()
        .with(tauri_plugin_auditaur::tracing_layer())
        .try_init()
    {
        eprintln!("Auditaur tracing layer was not installed: {error}");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))?;
                window.set_icon(icon)?;
            }

            Ok(())
        })
        .plugin(
            tauri_plugin_auditaur::Builder::new()
                .service_name("autorepo")
                .session_name("autorepo-app")
                .redact_defaults(true)
                .max_session_bytes(256 * 1024 * 1024)
                .allow_release_builds(false)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            app_info,
            observation_kinds,
            github_auth_status,
            search_github_repositories,
            list_github_repository_owners,
            list_github_owner_repositories,
            check_github_repository_packs,
            check_github_target_repository,
            validate_pack,
            list_repo_packs,
            preview_pack_plan,
            execute_pack_dry_run,
            execute_pack_hydrate
        ])
        .run(tauri::generate_context!())
        .expect("error while running autorepo desktop app");
}

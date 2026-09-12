use std::{
    fs,
    path::{Component, Path, PathBuf},
    process::Command as ProcessCommand,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};

use crate::{
    doctor,
    executor::Executor,
    github::RepoRef,
    labs::{self, CaptureSessionArgs, InspectSessionArgs, RehydrateSessionArgs},
    pack::Pack,
    pack_publish::{self, PackPublishOptions, PackUpdateOptions},
    pack_scaffold::{self, FromRepoOptions},
    planner::{PlanContext, Planner},
    render, warm,
};

#[derive(Debug, Parser)]
#[command(name = "autorepo")]
#[command(about = "Prepare GitHub repositories for demos, workshops, and agent workflows.")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Check whether a target repository and local environment are ready.
    Doctor {
        /// GitHub repository in OWNER/REPO form.
        repo: String,
    },
    /// Validate a pack source.
    Validate {
        /// Pack source: directory, "builtin", or github:OWNER/REPO//path?ref=BRANCH.
        pack_source: String,
    },
    /// Plan or apply deterministic repository preparation operations.
    Prepare(PrepareArgs),
    /// Render optional app session links, notes/checklists, and agent-task guidance.
    Warm(WarmArgs),
    /// Scaffold a pack from a local checkout or GitHub repository.
    PackFrom(FromRepoCliArgs),
    /// Create and inspect pack authoring artifacts.
    Pack(PackArgs),
    /// Experimental local Copilot app/session state tools.
    Labs(LabsArgs),
}

#[derive(Debug, Args)]
pub struct PackArgs {
    #[command(subcommand)]
    command: PackCommand,
}

#[derive(Debug, Subcommand)]
enum PackCommand {
    /// Scaffold a pack from a local checkout or GitHub repository.
    FromRepo(FromRepoCliArgs),
    /// Refresh a pack from a local checkout or GitHub repository.
    Update(PackUpdateCliArgs),
    /// Refresh a pack in a target git repository, push a branch, and optionally open a PR.
    Publish(PackPublishCliArgs),
}

#[derive(Debug, Args)]
pub struct LabsArgs {
    #[command(subcommand)]
    command: LabsCommand,
}

#[derive(Debug, Subcommand)]
enum LabsCommand {
    /// Experimental Copilot session snapshot tools.
    Session(LabsSessionArgs),
}

#[derive(Debug, Args)]
pub struct LabsSessionArgs {
    #[command(subcommand)]
    command: LabsSessionCommand,
}

#[derive(Debug, Subcommand)]
enum LabsSessionCommand {
    /// Inspect a local Copilot session and its app indexes without changing state.
    Inspect(InspectSessionCliArgs),
    /// Capture a session fixture from local Copilot state.
    Capture(CaptureSessionCliArgs),
    /// Rehydrate a captured session fixture into a Copilot home.
    Rehydrate(RehydrateSessionCliArgs),
}

#[derive(Debug, Args)]
pub struct PrepareArgs {
    /// GitHub repository in OWNER/REPO form.
    repo: String,
    /// Pack source: directory, pack id, "builtin", or github:OWNER/REPO//path?ref=BRANCH.
    #[arg(long)]
    pack: String,
    /// Render the plan without making writes.
    #[arg(long, conflicts_with = "yes")]
    dry_run: bool,
    /// Confirm serial execution of safe writes.
    #[arg(long, conflicts_with = "dry_run")]
    yes: bool,
    /// Allow preparation of repositories that are not empty.
    #[arg(long)]
    allow_non_empty: bool,
}

#[derive(Debug, Args)]
pub struct WarmArgs {
    /// GitHub repository in OWNER/REPO form.
    repo: String,
    /// Pack source: directory, pack id, "builtin", or github:OWNER/REPO//path?ref=BRANCH.
    #[arg(long)]
    pack: String,
    /// Include explicit Copilot cloud-agent task startup guidance.
    #[arg(long)]
    start_agent_tasks: bool,
    /// Open generated Copilot app session links.
    #[arg(long)]
    open_app: bool,
    /// Limit warmup output/actions to one or more warmup ids.
    #[arg(long, value_delimiter = ',')]
    r#only: Vec<String>,
}

#[derive(Debug, Args)]
struct FromRepoCliArgs {
    /// Local checkout path, OWNER/REPO, github:OWNER/REPO, or GitHub URL to extract templates from.
    source: String,
    /// Remote branch or tag to clone. Only valid for remote sources.
    #[arg(long = "ref")]
    git_ref: Option<String>,
    /// Output pack directory to create.
    #[arg(long)]
    out: PathBuf,
    /// Stable pack id.
    #[arg(long)]
    id: String,
    /// Human-readable pack name.
    #[arg(long)]
    name: String,
    /// Optional pack description.
    #[arg(long)]
    description: Option<String>,
    /// Include glob relative to the source repo. Repeats allowed. Defaults to everything.
    #[arg(long)]
    include: Vec<String>,
    /// Exclude glob relative to the source repo. Repeats allowed.
    #[arg(long)]
    exclude: Vec<String>,
    /// Add a starter issue asking maintainers to review the extracted pack.
    #[arg(long)]
    with_issues: bool,
    /// Add modest Copilot app warmup stubs.
    #[arg(long)]
    with_warmup: bool,
    /// Print the generated manifest and copy summary without writing files.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Args)]
struct PackUpdateCliArgs {
    /// Local checkout path, OWNER/REPO, github:OWNER/REPO, or GitHub URL to extract templates from.
    source: String,
    /// Remote branch or tag to clone. Only valid for remote sources.
    #[arg(long = "ref")]
    git_ref: Option<String>,
    /// Existing or new pack directory to refresh.
    #[arg(long)]
    out: PathBuf,
    /// Stable pack id. Required when creating a new pack.
    #[arg(long)]
    id: Option<String>,
    /// Human-readable pack name. Required when creating a new pack.
    #[arg(long)]
    name: Option<String>,
    /// Optional pack description. Used when creating a new pack or replacing the manifest.
    #[arg(long)]
    description: Option<String>,
    /// Include glob relative to the source repo. Repeats allowed. Defaults to everything.
    #[arg(long)]
    include: Vec<String>,
    /// Exclude glob relative to the source repo. Repeats allowed.
    #[arg(long)]
    exclude: Vec<String>,
    /// Add starter issue stubs when creating a new pack or using --replace.
    #[arg(long)]
    with_issues: bool,
    /// Add modest Copilot app warmup stubs when creating a new pack or using --replace.
    #[arg(long)]
    with_warmup: bool,
    /// Replace the whole pack directory instead of preserving curated metadata/templates.
    #[arg(long)]
    replace: bool,
    /// Print the refresh summary without writing files.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Args)]
struct PackPublishCliArgs {
    /// Local checkout path, OWNER/REPO, github:OWNER/REPO, or GitHub URL to extract templates from.
    source: String,
    /// Remote branch or tag to clone. Only valid for remote sources.
    #[arg(long = "ref")]
    git_ref: Option<String>,
    /// GitHub repository that owns the pack catalog, in OWNER/REPO form.
    #[arg(long)]
    target_repo: String,
    /// Pack path inside the target repository, for example packs/contract-expert.
    #[arg(long)]
    target_path: PathBuf,
    /// Branch to create or update in the target repository.
    #[arg(long)]
    branch: String,
    /// Base branch for the publish branch and PR. Defaults to the target repository default branch.
    #[arg(long)]
    base: Option<String>,
    /// Use this local checkout of the target repository instead of cloning a temporary checkout.
    #[arg(long)]
    target_checkout: Option<PathBuf>,
    /// Stable pack id. Required when creating a new pack.
    #[arg(long)]
    id: Option<String>,
    /// Human-readable pack name. Required when creating a new pack.
    #[arg(long)]
    name: Option<String>,
    /// Optional pack description. Used when creating a new pack or replacing the manifest.
    #[arg(long)]
    description: Option<String>,
    /// Include glob relative to the source repo. Repeats allowed. Defaults to everything.
    #[arg(long)]
    include: Vec<String>,
    /// Exclude glob relative to the source repo. Repeats allowed.
    #[arg(long)]
    exclude: Vec<String>,
    /// Add starter issue stubs when creating a new pack or using --replace.
    #[arg(long)]
    with_issues: bool,
    /// Add modest Copilot app warmup stubs when creating a new pack or using --replace.
    #[arg(long)]
    with_warmup: bool,
    /// Replace the whole pack directory instead of preserving curated metadata/templates.
    #[arg(long)]
    replace: bool,
    /// Open or reuse a pull request for the pushed branch.
    #[arg(long)]
    pr: bool,
    /// Print the publish plan and generated diff without committing or pushing.
    #[arg(long, conflicts_with = "yes")]
    dry_run: bool,
    /// Confirm branch creation, commit, push, and optional PR creation.
    #[arg(long, conflicts_with = "dry_run")]
    yes: bool,
}

#[derive(Debug, Args)]
struct InspectSessionCliArgs {
    /// Copilot session id to inspect.
    session_id: String,
    /// Copilot home directory. Defaults to ~/.copilot.
    #[arg(long)]
    copilot_home: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct CaptureSessionCliArgs {
    /// Copilot session id to capture.
    session_id: String,
    /// Output directory for the snapshot fixture.
    #[arg(long)]
    out: PathBuf,
    /// Copilot home directory. Defaults to ~/.copilot.
    #[arg(long)]
    copilot_home: Option<PathBuf>,
    /// Include user and assistant transcript text in the snapshot fixture.
    #[arg(long)]
    include_transcripts: bool,
}

#[derive(Debug, Args)]
struct RehydrateSessionCliArgs {
    /// Target GitHub repository in OWNER/REPO form.
    repo: String,
    /// Snapshot fixture directory created by capture.
    #[arg(long)]
    snapshot: PathBuf,
    /// Copilot home directory to write. Defaults to ~/.copilot.
    #[arg(long)]
    copilot_home: Option<PathBuf>,
    /// Target workspace path to bind into the rehydrated session.
    #[arg(long)]
    workspace: PathBuf,
    /// Target branch name to show in indexes.
    #[arg(long)]
    branch: Option<String>,
    /// Print planned state changes without writing them.
    #[arg(long, conflicts_with = "yes")]
    dry_run: bool,
    /// Confirm writing local Copilot session/index state.
    #[arg(long, conflicts_with = "dry_run")]
    yes: bool,
    /// Allow writing to the default live ~/.copilot home.
    #[arg(long)]
    allow_live_copilot_home: bool,
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Doctor { repo } => {
            doctor::run(&repo).await?;
        }
        Command::Validate { pack_source } => {
            let source = PackSource::resolve(&pack_source)?;
            let pack = Pack::load(source.path().to_path_buf())?;
            pack.validate()?;
            println!("Pack '{}' is valid.", pack.manifest().id);
        }
        Command::Prepare(args) => {
            if !args.dry_run && !args.yes {
                bail!("prepare requires either --dry-run or --yes");
            }

            let source = PackSource::resolve(&args.pack)?;
            let pack = Pack::load(source.path().to_path_buf())?;
            pack.validate()?;
            let plan = Planner::new().plan(
                &pack,
                PlanContext {
                    repo: args.repo,
                    allow_non_empty: args.allow_non_empty,
                },
            )?;

            if args.dry_run {
                render::print_plan_table(&plan);
                println!("{}", serde_json::to_string_pretty(&plan)?);
            } else {
                Executor.execute_serial(&plan).await?;
            }
        }
        Command::Warm(args) => {
            let source = PackSource::resolve(&args.pack)?;
            let pack = Pack::load(source.path().to_path_buf())?;
            pack.validate()?;
            warm::run(
                &args.repo,
                &pack,
                args.start_agent_tasks,
                args.open_app,
                &args.r#only,
            )?;
        }
        Command::PackFrom(args) => {
            pack_scaffold::from_repo(FromRepoOptions {
                source: args.source,
                git_ref: args.git_ref,
                out: args.out,
                id: args.id,
                name: args.name,
                description: args.description,
                include: args.include,
                exclude: args.exclude,
                with_issues: args.with_issues,
                with_warmup: args.with_warmup,
                dry_run: args.dry_run,
            })?;
        }
        Command::Pack(args) => match args.command {
            PackCommand::FromRepo(args) => {
                pack_scaffold::from_repo(FromRepoOptions {
                    source: args.source,
                    git_ref: args.git_ref,
                    out: args.out,
                    id: args.id,
                    name: args.name,
                    description: args.description,
                    include: args.include,
                    exclude: args.exclude,
                    with_issues: args.with_issues,
                    with_warmup: args.with_warmup,
                    dry_run: args.dry_run,
                })?;
            }
            PackCommand::Update(args) => {
                pack_publish::update(PackUpdateOptions {
                    source: args.source,
                    git_ref: args.git_ref,
                    out: args.out,
                    id: args.id,
                    name: args.name,
                    description: args.description,
                    include: args.include,
                    exclude: args.exclude,
                    with_issues: args.with_issues,
                    with_warmup: args.with_warmup,
                    replace: args.replace,
                    dry_run: args.dry_run,
                })?;
            }
            PackCommand::Publish(args) => {
                if !args.dry_run && !args.yes {
                    bail!("pack publish requires either --dry-run or --yes");
                }
                pack_publish::publish(PackPublishOptions {
                    source: args.source,
                    git_ref: args.git_ref,
                    target_repo: RepoRef::parse(&args.target_repo)?,
                    target_path: args.target_path,
                    branch: args.branch,
                    base: args.base,
                    target_checkout: args.target_checkout,
                    id: args.id,
                    name: args.name,
                    description: args.description,
                    include: args.include,
                    exclude: args.exclude,
                    with_issues: args.with_issues,
                    with_warmup: args.with_warmup,
                    replace: args.replace,
                    pr: args.pr,
                    dry_run: args.dry_run,
                })?;
            }
        },
        Command::Labs(args) => match args.command {
            LabsCommand::Session(args) => match args.command {
                LabsSessionCommand::Inspect(args) => {
                    labs::inspect_session(InspectSessionArgs {
                        copilot_home: args.copilot_home,
                        session_id: args.session_id,
                    })?;
                }
                LabsSessionCommand::Capture(args) => {
                    labs::capture_session(CaptureSessionArgs {
                        copilot_home: args.copilot_home,
                        session_id: args.session_id,
                        out: args.out,
                        include_transcripts: args.include_transcripts,
                    })?;
                }
                LabsSessionCommand::Rehydrate(args) => {
                    if !args.dry_run && !args.yes {
                        bail!("labs session rehydrate requires either --dry-run or --yes");
                    }
                    labs::rehydrate_session(RehydrateSessionArgs {
                        copilot_home: args.copilot_home,
                        repo: args.repo,
                        snapshot: args.snapshot,
                        workspace: args.workspace,
                        branch: args.branch,
                        dry_run: args.dry_run,
                        allow_live_copilot_home: args.allow_live_copilot_home,
                    })?;
                }
            },
        },
    }

    Ok(())
}

#[derive(Debug)]
struct PackSource {
    path: PathBuf,
    temp_root: Option<PathBuf>,
}

impl PackSource {
    fn resolve(value: &str) -> Result<Self> {
        if value.starts_with("github:") {
            return Self::from_github_spec(value);
        }

        if value.starts_with("https://github.com/") {
            return Self::from_github_url(value);
        }

        Ok(Self {
            path: resolve_local_pack(value)?,
            temp_root: None,
        })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn from_github_spec(value: &str) -> Result<Self> {
        let spec = value.trim_start_matches("github:");
        let (spec, git_ref) = split_ref_query(spec)?;
        let (repo, pack_path) = spec.split_once("//").unwrap_or((spec, ""));
        let repo = RepoRef::parse(repo)?;
        Self::clone_github_pack(&repo, pack_path, git_ref)
    }

    fn from_github_url(value: &str) -> Result<Self> {
        let without_prefix = value
            .strip_prefix("https://github.com/")
            .context("GitHub pack URL must start with https://github.com/")?;
        let (path, git_ref) = split_ref_query(without_prefix)?;
        let parts = path.split('/').collect::<Vec<_>>();
        if parts.len() < 2 {
            bail!("GitHub pack URL must include OWNER/REPO");
        }

        let repo_name = parts[1].trim_end_matches(".git");
        let repo = RepoRef::parse(&format!("{}/{}", parts[0], repo_name))?;
        let pack_path = if parts.get(2) == Some(&"tree") {
            if git_ref.is_some() {
                bail!("GitHub tree URLs must not also include ?ref=");
            }
            let tree_tail = parts.get(3..).unwrap_or(&[]).join("/");
            let (git_ref, pack_path) = resolve_tree_url_ref(&repo, &tree_tail)?;
            return Self::clone_github_pack(&repo, &pack_path, Some(&git_ref));
        } else {
            parts.get(2..).unwrap_or(&[]).join("/")
        };

        Self::clone_github_pack(&repo, &pack_path, git_ref)
    }

    fn clone_github_pack(repo: &RepoRef, pack_path: &str, git_ref: Option<&str>) -> Result<Self> {
        validate_remote_pack_path(pack_path)?;
        let temp_root = temp_pack_root()?;
        fs::create_dir_all(&temp_root)
            .with_context(|| format!("failed to create {}", temp_root.display()))?;

        let checkout = temp_root.join("repo");
        let clone_url = format!("https://github.com/{}/{}.git", repo.owner, repo.name);
        let mut command = ProcessCommand::new("git");
        command.args(["clone", "--depth", "1"]);
        command
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GCM_INTERACTIVE", "never")
            .env("GIT_ASKPASS", "echo");
        if let Some(git_ref) = git_ref {
            command.args(["--branch", git_ref]);
        }
        command.arg(&clone_url).arg(&checkout);

        let output = command
            .output()
            .with_context(|| format!("failed to start git clone for {clone_url}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("failed to clone GitHub pack source {clone_url}: {stderr}");
        }

        let path = if pack_path.trim().is_empty() {
            checkout
        } else {
            checkout.join(pack_path)
        };

        Ok(Self {
            path,
            temp_root: Some(temp_root),
        })
    }
}

impl Drop for PackSource {
    fn drop(&mut self) {
        if let Some(temp_root) = &self.temp_root {
            let _ = fs::remove_dir_all(temp_root);
        }
    }
}

fn resolve_local_pack(value: &str) -> Result<PathBuf> {
    if value == "builtin" || value == "generic-starter" {
        return Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("packs")
            .join("generic-starter"));
    }

    PathBuf::from(value)
        .canonicalize()
        .with_context(|| format!("pack path '{}' does not exist", value))
}

fn split_ref_query(value: &str) -> Result<(&str, Option<&str>)> {
    let Some((path, query)) = value.split_once('?') else {
        return Ok((value, None));
    };

    let Some(git_ref) = query.strip_prefix("ref=") else {
        bail!("GitHub pack source supports only ?ref=<branch-or-tag>");
    };
    if git_ref.contains('&') {
        bail!("GitHub pack source supports only ?ref=<branch-or-tag>");
    }
    if git_ref.trim().is_empty() {
        bail!("GitHub pack source ?ref must not be empty");
    }

    Ok((path, Some(git_ref)))
}

fn resolve_tree_url_ref(repo: &RepoRef, tree_tail: &str) -> Result<(String, String)> {
    if tree_tail.trim().is_empty() {
        bail!("GitHub tree URL must include a ref");
    }

    let refs = list_github_refs(repo)?;
    match_tree_tail_to_ref(tree_tail, refs.iter().map(String::as_str))
        .with_context(|| {
            format!(
                "failed to resolve a branch or tag in GitHub tree URL for {}/{}; use github:{}/{}//path?ref=<branch-or-tag> instead",
                repo.owner, repo.name, repo.owner, repo.name
            )
        })
}

fn list_github_refs(repo: &RepoRef) -> Result<Vec<String>> {
    let clone_url = format!("https://github.com/{}/{}.git", repo.owner, repo.name);
    let output = ProcessCommand::new("git")
        .args(["ls-remote", "--heads", "--tags", "--refs", &clone_url])
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .env("GIT_ASKPASS", "echo")
        .output()
        .with_context(|| format!("failed to start git ls-remote for {clone_url}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("failed to list GitHub refs for {clone_url}: {stderr}");
    }

    let refs = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1))
        .filter_map(|name| {
            name.strip_prefix("refs/heads/")
                .or_else(|| name.strip_prefix("refs/tags/"))
        })
        .map(str::to_owned)
        .collect();
    Ok(refs)
}

fn match_tree_tail_to_ref<'a>(
    tree_tail: &str,
    refs: impl IntoIterator<Item = &'a str>,
) -> Result<(String, String)> {
    let mut matches = refs
        .into_iter()
        .filter_map(|git_ref| {
            if tree_tail == git_ref {
                Some((git_ref, ""))
            } else {
                tree_tail
                    .strip_prefix(&format!("{git_ref}/"))
                    .map(|pack_path| (git_ref, pack_path))
            }
        })
        .collect::<Vec<_>>();

    matches.sort_by_key(|(git_ref, _)| std::cmp::Reverse(git_ref.len()));
    let Some((git_ref, pack_path)) = matches.first() else {
        bail!("no matching branch or tag");
    };

    Ok(((*git_ref).to_owned(), (*pack_path).to_owned()))
}

fn validate_remote_pack_path(value: &str) -> Result<()> {
    let path = Path::new(value);
    if path.is_absolute() {
        bail!("remote pack path '{value}' must be relative");
    }

    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        bail!("remote pack path '{value}' must not contain parent-directory or root components");
    }

    Ok(())
}

fn temp_pack_root() -> Result<PathBuf> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before UNIX_EPOCH")?
        .as_nanos();
    Ok(std::env::temp_dir().join(format!("autorepo-pack-{}-{now}", std::process::id())))
}

#[cfg(test)]
mod tests {
    use super::{match_tree_tail_to_ref, split_ref_query, validate_remote_pack_path};

    #[test]
    fn parses_github_ref_query() {
        assert_eq!(
            split_ref_query("owner/repo//packs/demo?ref=main").unwrap(),
            ("owner/repo//packs/demo", Some("main"))
        );
        assert!(split_ref_query("owner/repo?branch=main").is_err());
        assert!(split_ref_query("owner/repo?ref=main&foo=bar").is_err());
    }

    #[test]
    fn resolves_tree_url_refs_with_slashes() {
        assert_eq!(
            match_tree_tail_to_ref(
                "release/v1/packs/demo",
                ["main", "release/v1", "release"].into_iter(),
            )
            .unwrap(),
            ("release/v1".to_owned(), "packs/demo".to_owned())
        );
        assert_eq!(
            match_tree_tail_to_ref("main", ["main"].into_iter()).unwrap(),
            ("main".to_owned(), "".to_owned())
        );
    }

    #[test]
    fn rejects_unsafe_remote_pack_paths() {
        assert!(validate_remote_pack_path("packs/demo").is_ok());
        assert!(validate_remote_pack_path("").is_ok());
        assert!(validate_remote_pack_path("../secret").is_err());
        assert!(validate_remote_pack_path("/secret").is_err());
    }
}

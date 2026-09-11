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
    pack::Pack,
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

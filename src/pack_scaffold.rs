use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::Serialize;

use crate::{github::RepoRef, pack::Pack};

#[derive(Debug)]
pub struct FromRepoOptions {
    pub source: String,
    pub git_ref: Option<String>,
    pub out: PathBuf,
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub with_issues: bool,
    pub with_warmup: bool,
    pub dry_run: bool,
}

#[derive(Debug)]
struct Candidate {
    absolute: PathBuf,
    relative: String,
    template: String,
    id: String,
    content: String,
}

#[derive(Debug)]
struct Skipped {
    relative: String,
    reason: String,
}

#[derive(Debug)]
struct SourceRepo {
    path: PathBuf,
    temp_root: Option<PathBuf>,
}

impl Drop for SourceRepo {
    fn drop(&mut self) {
        if let Some(temp_root) = &self.temp_root {
            let _ = fs::remove_dir_all(temp_root);
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct RemoteSource {
    repo: RepoRef,
    path: String,
    git_ref: Option<String>,
}

#[derive(Debug, Serialize)]
struct Manifest {
    schema: u32,
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    safety: Safety,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    files: Vec<FileEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    issues: Vec<IssueEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    warmup: Vec<WarmupEntry>,
}

#[derive(Debug, Serialize)]
struct Safety {
    max_writes: u32,
}

#[derive(Debug, Serialize)]
struct FileEntry {
    id: String,
    path: String,
    template: String,
}

#[derive(Debug, Serialize)]
struct IssueEntry {
    id: String,
    title: String,
    template: String,
}

#[derive(Debug, Serialize)]
struct WarmupEntry {
    id: String,
    title: String,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_template: Option<String>,
}

pub fn from_repo(options: FromRepoOptions) -> Result<()> {
    let source = SourceRepo::resolve(&options.source, options.git_ref.as_deref())?;
    if !source.path.is_dir() {
        bail!("source '{}' is not a directory", source.path.display());
    }
    if !options.dry_run && options.out.exists() && !is_empty_dir(&options.out)? {
        bail!(
            "output pack directory '{}' already exists and is not empty",
            options.out.display()
        );
    }

    let include = compile_patterns(&options.include, "include")?;
    let exclude = compile_patterns(&options.exclude, "exclude")?;
    let mut skipped = Vec::new();
    let mut candidates = collect_candidates(&source.path, &include, &exclude, &mut skipped)?;
    assign_ids(&mut candidates);
    let manifest = build_manifest(&options, &candidates)?;
    let manifest_text = serde_yaml::to_string(&manifest).context("failed to render pack.yml")?;

    print_summary(
        &options.out,
        &candidates,
        &skipped,
        &manifest_text,
        options.dry_run,
    );
    if options.dry_run {
        return Ok(());
    }

    write_pack(
        &options.out,
        &manifest_text,
        &candidates,
        options.with_issues,
        options.with_warmup,
    )?;
    let pack = Pack::load(options.out.clone())?;
    pack.validate().with_context(|| {
        format!(
            "generated pack '{}' did not validate",
            options.out.display()
        )
    })?;
    println!("Generated pack '{}' is valid.", pack.manifest().id);
    Ok(())
}

impl SourceRepo {
    fn resolve(value: &str, git_ref_override: Option<&str>) -> Result<Self> {
        let local = PathBuf::from(value);
        if local.exists() {
            if git_ref_override.is_some() {
                bail!("--ref is only supported for remote sources");
            }
            let path = local
                .canonicalize()
                .with_context(|| format!("source repo '{}' does not exist", local.display()))?;
            return Ok(Self {
                path,
                temp_root: None,
            });
        }

        let mut remote = parse_remote_source(value)?;
        if let Some(git_ref) = git_ref_override {
            if git_ref.trim().is_empty() {
                bail!("--ref must not be empty");
            }
            if remote.git_ref.is_some() {
                bail!("remote source must not use both embedded ?ref= and --ref");
            }
            remote.git_ref = Some(git_ref.to_owned());
        }
        Self::clone_remote(remote)
    }

    fn clone_remote(remote: RemoteSource) -> Result<Self> {
        let temp_root = temp_source_root()?;
        fs::create_dir_all(&temp_root)
            .with_context(|| format!("failed to create '{}'", temp_root.display()))?;
        let checkout = temp_root.join("repo");
        let clone_url = format!(
            "https://github.com/{}/{}.git",
            remote.repo.owner, remote.repo.name
        );
        let mut command = ProcessCommand::new("git");
        command.args(["clone", "--depth", "1"]);
        command
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GCM_INTERACTIVE", "never")
            .env("GIT_ASKPASS", "echo");
        if let Some(git_ref) = &remote.git_ref {
            command.args(["--branch", git_ref]);
        }
        command.arg(&clone_url).arg(&checkout);

        let output = command
            .output()
            .with_context(|| format!("failed to start git clone for {clone_url}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("failed to clone source repository {clone_url}: {stderr}");
        }

        let path = if remote.path.trim().is_empty() {
            checkout
        } else {
            checkout.join(remote.path)
        }
        .canonicalize()
        .context("remote source path does not exist in cloned repository")?;

        Ok(Self {
            path,
            temp_root: Some(temp_root),
        })
    }
}

fn parse_remote_source(value: &str) -> Result<RemoteSource> {
    if let Some(spec) = value.strip_prefix("github:") {
        return parse_github_spec(spec);
    }

    if value.starts_with("https://github.com/") {
        return parse_github_url(value);
    }

    if looks_like_repo_ref(value) {
        return Ok(RemoteSource {
            repo: RepoRef::parse(value)?,
            path: String::new(),
            git_ref: None,
        });
    }

    bail!(
        "source '{}' is not an existing path, OWNER/REPO, github:OWNER/REPO, or GitHub URL",
        value
    );
}

fn parse_github_spec(spec: &str) -> Result<RemoteSource> {
    let (spec, git_ref) = split_ref_query(spec)?;
    let (repo, path) = spec.split_once("//").unwrap_or((spec, ""));
    Ok(RemoteSource {
        repo: RepoRef::parse(repo)?,
        path: validate_remote_path(path)?.to_owned(),
        git_ref: git_ref.map(str::to_owned),
    })
}

fn parse_github_url(value: &str) -> Result<RemoteSource> {
    let without_prefix = value
        .strip_prefix("https://github.com/")
        .context("GitHub URL must start with https://github.com/")?;
    let (path, git_ref) = split_ref_query(without_prefix)?;
    let parts = path.split('/').collect::<Vec<_>>();
    if parts.len() < 2 {
        bail!("GitHub URL must include OWNER/REPO");
    }

    let repo_name = parts[1].trim_end_matches(".git");
    let repo = RepoRef::parse(&format!("{}/{}", parts[0], repo_name))?;
    if parts.get(2) == Some(&"tree") {
        if git_ref.is_some() {
            bail!(
                "GitHub tree URLs must not also include ?ref=; use github:OWNER/REPO//path?ref=<branch-or-tag> instead"
            );
        }
        let tree_tail = parts.get(3..).unwrap_or(&[]).join("/");
        let (git_ref, source_path) = resolve_tree_url_ref(&repo, &tree_tail)?;
        return Ok(RemoteSource {
            repo,
            path: validate_remote_path(&source_path)?.to_owned(),
            git_ref: Some(git_ref),
        });
    }

    let source_path = parts.get(2..).unwrap_or(&[]).join("/");
    Ok(RemoteSource {
        repo,
        path: validate_remote_path(&source_path)?.to_owned(),
        git_ref: git_ref.map(str::to_owned),
    })
}

fn resolve_tree_url_ref(repo: &RepoRef, tree_tail: &str) -> Result<(String, String)> {
    if tree_tail.trim().is_empty() {
        bail!("GitHub tree URL must include a ref");
    }

    let refs = list_github_refs(repo)?;
    match_tree_tail_to_ref(tree_tail, refs.iter().map(String::as_str)).with_context(|| {
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

    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1))
        .filter_map(|name| {
            name.strip_prefix("refs/heads/")
                .or_else(|| name.strip_prefix("refs/tags/"))
        })
        .map(str::to_owned)
        .collect())
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
                    .map(|source_path| (git_ref, source_path))
            }
        })
        .collect::<Vec<_>>();

    matches.sort_by_key(|(git_ref, _)| std::cmp::Reverse(git_ref.len()));
    let Some((git_ref, source_path)) = matches.first() else {
        bail!("no matching branch or tag");
    };

    Ok(((*git_ref).to_owned(), (*source_path).to_owned()))
}

fn split_ref_query(value: &str) -> Result<(&str, Option<&str>)> {
    let Some((path, query)) = value.split_once('?') else {
        return Ok((value, None));
    };

    let Some(git_ref) = query.strip_prefix("ref=") else {
        bail!("remote source supports only ?ref=<branch-or-tag>");
    };
    if git_ref.contains('&') || git_ref.trim().is_empty() {
        bail!("remote source supports only ?ref=<branch-or-tag>");
    }

    Ok((path, Some(git_ref)))
}

fn validate_remote_path(value: &str) -> Result<&str> {
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
        bail!("remote source path '{value}' must be relative and stay inside the repository");
    }
    Ok(value)
}

fn looks_like_repo_ref(value: &str) -> bool {
    let parts = value.split('/').collect::<Vec<_>>();
    parts.len() == 2
        && parts
            .iter()
            .all(|part| !part.is_empty() && !part.contains('\\') && !part.contains(':'))
}

fn temp_source_root() -> Result<PathBuf> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before UNIX_EPOCH")?
        .as_nanos();
    Ok(std::env::temp_dir().join(format!("autorepo-pack-from-{}-{now}", std::process::id())))
}

fn collect_candidates(
    source: &Path,
    include: &Option<GlobSet>,
    exclude: &Option<GlobSet>,
    skipped: &mut Vec<Skipped>,
) -> Result<Vec<Candidate>> {
    let mut files = Vec::new();
    collect_dir(source, source, include, exclude, skipped, &mut files)?;
    files.sort_by(|left, right| left.relative.cmp(&right.relative));
    Ok(files)
}

fn collect_dir(
    source: &Path,
    dir: &Path,
    include: &Option<GlobSet>,
    exclude: &Option<GlobSet>,
    skipped: &mut Vec<Skipped>,
    files: &mut Vec<Candidate>,
) -> Result<()> {
    let mut entries = fs::read_dir(dir)
        .with_context(|| format!("failed to read directory '{}'", dir.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("failed to read directory entry in '{}'", dir.display()))?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("failed to inspect '{}'", path.display()))?;
        let relative = repo_relative(source, &path)?;
        if metadata.file_type().is_symlink() {
            skipped.push(skip(relative, "symlink"));
            continue;
        }
        if let Some(reason) = hard_exclusion_reason(&relative, metadata.is_dir()) {
            skipped.push(skip(relative, reason));
            continue;
        }
        if exclude
            .as_ref()
            .is_some_and(|patterns| patterns.is_match(&relative))
        {
            skipped.push(skip(relative, "excluded by --exclude"));
            continue;
        }
        if metadata.is_dir() {
            collect_dir(source, &path, include, exclude, skipped, files)?;
            continue;
        }
        if !metadata.is_file() {
            skipped.push(skip(relative, "not a regular file"));
            continue;
        }
        if include
            .as_ref()
            .is_some_and(|patterns| !patterns.is_match(&relative))
        {
            continue;
        }

        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                skipped.push(skip(relative, format!("unreadable: {error}")));
                continue;
            }
        };
        if bytes.contains(&0) {
            skipped.push(skip(relative, "binary or non-text"));
            continue;
        }
        let content = match String::from_utf8(bytes) {
            Ok(content) => content,
            Err(_) => {
                skipped.push(skip(relative, "binary or non-UTF-8"));
                continue;
            }
        };
        let template = format!("templates/files/{relative}");
        files.push(Candidate {
            absolute: path,
            relative,
            template,
            id: String::new(),
            content,
        });
    }

    Ok(())
}

fn build_manifest(options: &FromRepoOptions, files: &[Candidate]) -> Result<Manifest> {
    let mut write_count = files.len();
    let issues = if options.with_issues {
        write_count += 1;
        vec![IssueEntry {
            id: "review_extracted_pack".to_owned(),
            title: "Review extracted pack".to_owned(),
            template: "templates/issues/review-extracted-pack.md".to_owned(),
        }]
    } else {
        Vec::new()
    };
    let warmup = if options.with_warmup {
        write_count += 2;
        vec![
            WarmupEntry {
                id: "open_repo".to_owned(),
                title: "Open repository in Copilot app".to_owned(),
                kind: "app_link".to_owned(),
                target: Some("repo".to_owned()),
                mode: None,
                prompt_template: None,
            },
            WarmupEntry {
                id: "review_pack".to_owned(),
                title: "Review extracted pack".to_owned(),
                kind: "app_session".to_owned(),
                target: None,
                mode: Some("plan".to_owned()),
                prompt_template: Some("templates/warmup/review-pack.md".to_owned()),
            },
        ]
    } else {
        Vec::new()
    };

    Ok(Manifest {
        schema: 1,
        id: options.id.clone(),
        name: options.name.clone(),
        description: options.description.clone(),
        safety: Safety {
            max_writes: u32::try_from(write_count).context("pack declares too many writes")?,
        },
        files: files
            .iter()
            .map(|file| FileEntry {
                id: file.id.clone(),
                path: file.relative.clone(),
                template: file.template.clone(),
            })
            .collect(),
        issues,
        warmup,
    })
}

fn write_pack(
    out: &Path,
    manifest_text: &str,
    candidates: &[Candidate],
    with_issues: bool,
    with_warmup: bool,
) -> Result<()> {
    fs::create_dir_all(out).with_context(|| format!("failed to create '{}'", out.display()))?;
    for candidate in candidates {
        let target = out.join(&candidate.template);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create '{}'", parent.display()))?;
        }
        fs::write(&target, &candidate.content)
            .with_context(|| format!("failed to write '{}'", target.display()))?;
    }
    if with_issues {
        let target = out.join("templates/issues/review-extracted-pack.md");
        fs::create_dir_all(target.parent().expect("issue template has parent"))
            .with_context(|| format!("failed to create '{}'", target.display()))?;
        fs::write(
            &target,
            "Review the extracted files, safety exclusions, and warmup guidance before using this pack.\n",
        )
        .with_context(|| format!("failed to write '{}'", target.display()))?;
    }
    if with_warmup {
        let target = out.join("templates/warmup/review-pack.md");
        fs::create_dir_all(target.parent().expect("warmup template has parent"))
            .with_context(|| format!("failed to create '{}'", target.display()))?;
        fs::write(
            &target,
            "Inspect the prepared repository and verify the extracted starter files match the intended demo or workshop flow.\n",
        )
        .with_context(|| format!("failed to write '{}'", target.display()))?;
    }
    fs::write(out.join("pack.yml"), manifest_text)
        .with_context(|| format!("failed to write '{}'", out.join("pack.yml").display()))?;
    Ok(())
}

fn assign_ids(candidates: &mut [Candidate]) {
    let mut seen = HashMap::<String, usize>::new();
    for candidate in candidates {
        let base = stable_file_id(&candidate.relative);
        let count = seen.entry(base.clone()).or_default();
        *count += 1;
        candidate.id = if *count == 1 {
            base
        } else {
            format!("{base}_{}", count)
        };
    }
}

fn stable_file_id(path: &str) -> String {
    let without_extension = path
        .rsplit_once('.')
        .filter(|(prefix, _)| !prefix.is_empty())
        .map(|(prefix, _)| prefix)
        .unwrap_or(path);
    let mut id = String::new();
    let mut last_was_separator = false;
    for ch in without_extension.chars() {
        let normalized = ch.to_ascii_lowercase();
        if normalized.is_ascii_lowercase() || normalized.is_ascii_digit() {
            id.push(normalized);
            last_was_separator = false;
        } else if !last_was_separator {
            id.push('_');
            last_was_separator = true;
        }
    }
    let id = id.trim_matches('_');
    if id.is_empty() {
        "file".to_owned()
    } else {
        id.to_owned()
    }
}

fn hard_exclusion_reason(path: &str, is_dir: bool) -> Option<&'static str> {
    let lower = path.to_ascii_lowercase();
    let parts = lower.split('/').collect::<Vec<_>>();
    let excluded_dirs = HashSet::from([
        ".git",
        ".hg",
        ".svn",
        ".idea",
        ".vscode",
        ".cache",
        ".next",
        ".turbo",
        ".venv",
        ".pytest_cache",
        ".mypy_cache",
        ".ruff_cache",
        "__pycache__",
        "build",
        "coverage",
        "dist",
        "env",
        "node_modules",
        "obj",
        "out",
        "target",
        "venv",
    ]);
    if parts.iter().any(|part| excluded_dirs.contains(part)) {
        return Some(if is_dir {
            "hard-excluded directory"
        } else {
            "inside hard-excluded directory"
        });
    }

    let file_name = parts.last().copied().unwrap_or("");
    if file_name == ".env"
        || (file_name.starts_with(".env.") && !is_env_template_name(file_name))
        || matches!(
            file_name,
            ".ds_store"
                | "thumbs.db"
                | "desktop.ini"
                | "credentials"
                | "credentials.json"
                | "secrets.json"
                | "id_rsa"
                | "id_dsa"
                | "id_ecdsa"
                | "id_ed25519"
        )
    {
        return Some("hard-excluded local or secret file");
    }

    fn is_env_template_name(file_name: &str) -> bool {
        matches!(file_name, ".env.example" | ".env.sample" | ".env.template")
    }

    let extension = Path::new(file_name)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("");
    if matches!(
        extension,
        "db" | "sqlite" | "sqlite3" | "log" | "pem" | "key" | "pfx" | "p12"
    ) {
        return Some("hard-excluded local or secret file");
    }

    None
}

fn compile_patterns(patterns: &[String], name: &str) -> Result<Option<GlobSet>> {
    if patterns.is_empty() {
        return Ok(None);
    }

    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        for expanded in expand_pattern(pattern) {
            builder.add(
                Glob::new(&expanded)
                    .with_context(|| format!("invalid {name} pattern '{pattern}'"))?,
            );
        }
    }
    Ok(Some(builder.build().with_context(|| {
        format!("failed to build {name} patterns")
    })?))
}

fn expand_pattern(pattern: &str) -> Vec<String> {
    vec![pattern.replace('\\', "/")]
}

fn repo_relative(root: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(root)
        .with_context(|| format!("'{}' is not under '{}'", path.display(), root.display()))?;
    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

fn is_empty_dir(path: &Path) -> Result<bool> {
    if !path.is_dir() {
        bail!(
            "output path '{}' exists and is not a directory",
            path.display()
        );
    }
    Ok(fs::read_dir(path)
        .with_context(|| format!("failed to read '{}'", path.display()))?
        .next()
        .is_none())
}

fn print_summary(
    out: &Path,
    candidates: &[Candidate],
    skipped: &[Skipped],
    manifest_text: &str,
    dry_run: bool,
) {
    if dry_run {
        println!("Dry run: would write pack to {}", out.display());
    } else {
        println!("Writing pack to {}", out.display());
    }
    println!("Files selected: {}", candidates.len());
    println!("Files skipped: {}", skipped.len());
    for candidate in candidates {
        println!(
            "copy {} -> {}",
            candidate.absolute.display(),
            candidate.template
        );
    }
    for skipped in skipped {
        println!("skip {}: {}", skipped.relative, skipped.reason);
    }
    println!("--- pack.yml ---\n{manifest_text}");
}

fn skip(relative: String, reason: impl Into<String>) -> Skipped {
    Skipped {
        relative,
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::{
        FromRepoOptions, from_repo, hard_exclusion_reason, match_tree_tail_to_ref,
        parse_remote_source, stable_file_id,
    };
    use crate::pack::Pack;

    #[test]
    fn creates_valid_pack_from_selected_text_files() {
        let source = TempDir::new().unwrap();
        write(source.path(), "README.md", "hello");
        write(source.path(), "nested/README.md", "not included");
        write(source.path(), "AGENTS.md", "agents");
        write(source.path(), ".github/copilot-instructions.md", "guide");
        write(source.path(), ".gitignore", "target");
        write(source.path(), "azure.yaml", "name: demo");
        write(source.path(), "data/sample.json", "{}");
        write(source.path(), "docs/overview.md", "overview");
        write(source.path(), "docs/private.md", "private");
        write(
            source.path(),
            "src/contract-policy-expert-agent/main.py",
            "print('hi')",
        );
        write(source.path(), ".env", "SECRET=1");
        write(source.path(), "node_modules/pkg/index.js", "ignored");

        let out = TempDir::new().unwrap().path().join("contract-expert");
        from_repo(FromRepoOptions {
            source: source.path().display().to_string(),
            git_ref: None,
            out: out.clone(),
            id: "contract-expert".to_owned(),
            name: "Contract expert".to_owned(),
            description: Some("Extracted starter pack.".to_owned()),
            include: vec![
                "README.md".to_owned(),
                "AGENTS.md".to_owned(),
                ".github/copilot-instructions.md".to_owned(),
                ".gitignore".to_owned(),
                "azure.yaml".to_owned(),
                "data/**".to_owned(),
                "docs/**".to_owned(),
                "src/contract-policy-expert-agent/**".to_owned(),
                ".env".to_owned(),
                "node_modules/**".to_owned(),
            ],
            exclude: vec!["docs/private.md".to_owned()],
            with_issues: true,
            with_warmup: true,
            dry_run: false,
        })
        .unwrap();

        assert!(out.join("templates/files/README.md").is_file());
        assert!(
            out.join("templates/files/.github/copilot-instructions.md")
                .is_file()
        );
        assert!(
            out.join("templates/files/src/contract-policy-expert-agent/main.py")
                .is_file()
        );
        assert!(out.join("templates/files/azure.yaml").is_file());
        assert!(out.join("templates/files/data/sample.json").is_file());
        assert!(out.join("templates/files/docs/overview.md").is_file());
        assert!(out.join("templates/files/AGENTS.md").is_file());
        assert!(out.join("templates/files/.gitignore").is_file());
        assert!(!out.join("templates/files/nested/README.md").exists());
        assert!(!out.join("templates/files/docs/private.md").exists());
        assert!(!out.join("templates/files/.env").exists());
        assert!(
            !out.join("templates/files/node_modules/pkg/index.js")
                .exists()
        );
        let manifest = fs::read_to_string(out.join("pack.yml")).unwrap();
        assert!(manifest.contains("path: README.md"));
        assert!(manifest.contains("template: templates/files/README.md"));
        assert!(manifest.contains("review_extracted_pack"));
        assert!(manifest.contains("review_pack"));
        Pack::load(out).unwrap().validate().unwrap();
    }

    #[test]
    fn dry_run_does_not_write_output() {
        let source = TempDir::new().unwrap();
        write(source.path(), "README.md", "hello");
        let out = source.path().join("pack");

        from_repo(FromRepoOptions {
            source: source.path().display().to_string(),
            git_ref: None,
            out: out.clone(),
            id: "dry-run-pack".to_owned(),
            name: "Dry run pack".to_owned(),
            description: None,
            include: vec!["README.md".to_owned()],
            exclude: Vec::new(),
            with_issues: false,
            with_warmup: false,
            dry_run: true,
        })
        .unwrap();

        assert!(!out.exists());
    }

    #[test]
    fn refuses_non_empty_output() {
        let source = TempDir::new().unwrap();
        write(source.path(), "README.md", "hello");
        let out = TempDir::new().unwrap();
        write(out.path(), "existing.txt", "existing");

        let error = from_repo(FromRepoOptions {
            source: source.path().display().to_string(),
            git_ref: None,
            out: out.path().to_path_buf(),
            id: "existing-pack".to_owned(),
            name: "Existing pack".to_owned(),
            description: None,
            include: vec!["README.md".to_owned()],
            exclude: Vec::new(),
            with_issues: false,
            with_warmup: false,
            dry_run: false,
        })
        .unwrap_err()
        .to_string();

        assert!(error.contains("already exists and is not empty"));
    }

    #[test]
    fn skips_non_utf8_files() {
        let source = TempDir::new().unwrap();
        fs::write(source.path().join("image.bin"), [0, 159, 146, 150]).unwrap();
        let out = TempDir::new().unwrap().path().join("pack");

        from_repo(FromRepoOptions {
            source: source.path().display().to_string(),
            git_ref: None,
            out: out.clone(),
            id: "binary-pack".to_owned(),
            name: "Binary pack".to_owned(),
            description: None,
            include: vec!["image.bin".to_owned()],
            exclude: Vec::new(),
            with_issues: false,
            with_warmup: false,
            dry_run: false,
        })
        .unwrap();

        assert!(!out.join("templates/files/image.bin").exists());
        Pack::load(out).unwrap().validate().unwrap();
    }

    #[test]
    fn stable_file_ids_are_path_based_and_valid() {
        assert_eq!(
            stable_file_id(".github/copilot-instructions.md"),
            "github_copilot_instructions"
        );
        assert_eq!(
            stable_file_id("src/contract-policy-expert-agent/main.py"),
            "src_contract_policy_expert_agent_main"
        );
        assert_eq!(stable_file_id("..."), "file");
    }

    #[test]
    fn hard_exclusions_cover_secrets_and_local_outputs() {
        assert!(hard_exclusion_reason(".git/config", false).is_some());
        assert!(hard_exclusion_reason(".env.local", false).is_some());
        assert!(hard_exclusion_reason(".env.example", false).is_none());
        assert!(hard_exclusion_reason("node_modules/pkg/index.js", false).is_some());
        assert!(hard_exclusion_reason("target/debug/app", false).is_some());
        assert!(hard_exclusion_reason(".github/copilot-instructions.md", false).is_none());
        assert!(hard_exclusion_reason(".gitignore", false).is_none());
    }

    #[test]
    fn parses_remote_repo_sources() {
        let source = parse_remote_source("sethjuarez/ghcp-starters").unwrap();
        assert_eq!(source.repo.owner, "sethjuarez");
        assert_eq!(source.repo.name, "ghcp-starters");
        assert_eq!(source.path, "");
        assert_eq!(source.git_ref, None);

        let source =
            parse_remote_source("github:caldova/contract-policy-expert//fixtures/demo?ref=main")
                .unwrap();
        assert_eq!(source.repo.owner, "caldova");
        assert_eq!(source.repo.name, "contract-policy-expert");
        assert_eq!(source.path, "fixtures/demo");
        assert_eq!(source.git_ref.as_deref(), Some("main"));

        assert!(parse_remote_source("github:sethjuarez/ghcp-starters//../secret").is_err());
    }

    #[test]
    fn resolves_tree_url_refs_with_slashes() {
        assert_eq!(
            match_tree_tail_to_ref(
                "release/v1/fixtures/source",
                ["main", "release/v1", "release"].into_iter(),
            )
            .unwrap(),
            ("release/v1".to_owned(), "fixtures/source".to_owned())
        );
        assert_eq!(
            match_tree_tail_to_ref("main", ["main"].into_iter()).unwrap(),
            ("main".to_owned(), "".to_owned())
        );
    }

    fn write(root: &std::path::Path, relative: &str, content: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
}

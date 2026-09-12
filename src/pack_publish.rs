use std::{
    fs,
    path::{Component, Path, PathBuf},
    process::Command as ProcessCommand,
};

use anyhow::{Context, Result, bail};
use serde_yaml::{Mapping, Value};

use crate::{
    github::RepoRef,
    pack::Pack,
    pack_scaffold::{self, FromRepoOptions},
};

#[derive(Debug)]
pub struct PackUpdateOptions {
    pub source: String,
    pub git_ref: Option<String>,
    pub out: PathBuf,
    pub id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub with_issues: bool,
    pub with_warmup: bool,
    pub replace: bool,
    pub dry_run: bool,
}

#[derive(Debug)]
pub struct PackPublishOptions {
    pub source: String,
    pub git_ref: Option<String>,
    pub target_repo: RepoRef,
    pub target_path: PathBuf,
    pub branch: String,
    pub base: Option<String>,
    pub target_checkout: Option<PathBuf>,
    pub id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub with_issues: bool,
    pub with_warmup: bool,
    pub replace: bool,
    pub pr: bool,
    pub dry_run: bool,
}

pub fn update(options: PackUpdateOptions) -> Result<()> {
    let existing = options.out.exists() && !is_empty_dir(&options.out)?;
    if existing && !options.replace {
        update_existing_pack(&options)
    } else if existing && options.replace {
        replace_existing_pack(options)
    } else {
        let metadata =
            resolve_metadata(&options.out, options.id, options.name, options.description)?;
        pack_scaffold::from_repo(FromRepoOptions {
            source: options.source,
            git_ref: options.git_ref,
            out: options.out,
            id: metadata.id,
            name: metadata.name,
            description: metadata.description,
            include: options.include,
            exclude: options.exclude,
            with_issues: options.with_issues,
            with_warmup: options.with_warmup,
            dry_run: options.dry_run,
        })
    }
}

pub fn publish(options: PackPublishOptions) -> Result<()> {
    validate_relative_path(&options.target_path, "target path")?;
    validate_branch(&options.branch)?;

    if let Some(target_checkout) = &options.target_checkout {
        verify_origin_matches(target_checkout, &options.target_repo)?;
    }

    let temp_checkout = tempfile::Builder::new()
        .prefix("autorepo-pack-publish-")
        .tempdir()
        .context("failed to create temporary target checkout")?;
    clone_target_repo(&options, temp_checkout.path())?;
    let checkout = temp_checkout.path().join("repo");
    verify_origin_matches(&checkout, &options.target_repo)?;

    let publish_branch_existed =
        checkout_publish_branch(&checkout, &options.branch, options.base.as_deref())?;

    let target_pack = checkout.join(&options.target_path);
    let existing_pack = target_pack.exists() && !is_empty_dir(&target_pack)?;
    update(PackUpdateOptions {
        source: options.source.clone(),
        git_ref: options.git_ref.clone(),
        out: target_pack,
        id: metadata_option(existing_pack, options.replace, &options.id),
        name: metadata_option(existing_pack, options.replace, &options.name),
        description: metadata_option(existing_pack, options.replace, &options.description),
        include: options.include.clone(),
        exclude: options.exclude.clone(),
        with_issues: options.with_issues,
        with_warmup: options.with_warmup,
        replace: options.replace,
        dry_run: false,
    })?;

    let status = git_output(
        &checkout,
        ["status", "--short", "--", &path_arg(&options.target_path)],
    )?;
    if status.trim().is_empty() {
        println!(
            "No changes to publish for {}/{}:{}.",
            options.target_repo.owner,
            options.target_repo.name,
            options.target_path.display()
        );
        if options.pr && publish_branch_existed {
            open_or_reuse_pr(&options)?;
        }
        return Ok(());
    }

    fn metadata_option(
        existing_pack: bool,
        replace: bool,
        value: &Option<String>,
    ) -> Option<String> {
        if existing_pack && !replace {
            None
        } else {
            value.clone()
        }
    }

    println!("Pack publish branch: {}", options.branch);
    println!("Changed files:\n{status}");

    if options.dry_run {
        git(
            &checkout,
            ["add", "-A", "--", &path_arg(&options.target_path)],
        )?;
        let diff = git_output(&checkout, ["diff", "--", &path_arg(&options.target_path)])?;
        let cached_diff = git_output(
            &checkout,
            ["diff", "--cached", "--", &path_arg(&options.target_path)],
        )?;
        if !diff.trim().is_empty() {
            println!("--- diff ---\n{diff}");
        }
        if !cached_diff.trim().is_empty() {
            println!("--- staged diff ---\n{cached_diff}");
        }
        println!("Dry run: not committing, pushing, or opening a PR.");
        return Ok(());
    }

    git(
        &checkout,
        ["add", "-A", "--", &path_arg(&options.target_path)],
    )?;
    let commit_message = format!(
        "Update {} starter pack\n\nGenerated by autorepo pack publish.",
        pack_name_for_message(&options.target_path)
    );
    git(
        &checkout,
        [
            "-c",
            "user.name=Autorepo",
            "-c",
            "user.email=autorepo@users.noreply.github.com",
            "commit",
            "-m",
            &commit_message,
        ],
    )?;
    git(
        &checkout,
        [
            "push",
            "origin",
            &format!("HEAD:refs/heads/{}", options.branch),
        ],
    )?;
    println!("Pushed branch '{}'.", options.branch);

    if options.pr {
        open_or_reuse_pr(&options)?;
    }

    Ok(())
}

#[derive(Debug)]
struct Metadata {
    id: String,
    name: String,
    description: Option<String>,
}

fn update_existing_pack(options: &PackUpdateOptions) -> Result<()> {
    if options.id.is_some() || options.name.is_some() || options.description.is_some() {
        bail!("--id, --name, and --description require --replace when updating an existing pack");
    }
    let metadata = resolve_metadata(
        &options.out,
        options.id.clone(),
        options.name.clone(),
        options.description.clone(),
    )?;
    let temp = tempfile::Builder::new()
        .prefix("autorepo-pack-update-")
        .tempdir()
        .context("failed to create temporary pack update directory")?;
    let generated = temp.path().join("pack");
    pack_scaffold::from_repo(FromRepoOptions {
        source: options.source.clone(),
        git_ref: options.git_ref.clone(),
        out: generated.clone(),
        id: metadata.id,
        name: metadata.name,
        description: metadata.description,
        include: options.include.clone(),
        exclude: options.exclude.clone(),
        with_issues: false,
        with_warmup: false,
        dry_run: false,
    })?;

    if options.dry_run {
        println!(
            "Dry run: would refresh '{}' from generated snapshot '{}'.",
            options.out.display(),
            generated.display()
        );
        return Ok(());
    }

    refresh_files_and_manifest(&options.out, &generated)?;
    let pack = Pack::load(options.out.clone())?;
    pack.validate()
        .with_context(|| format!("updated pack '{}' did not validate", options.out.display()))?;
    println!("Updated pack '{}' is valid.", pack.manifest().id);
    Ok(())
}

fn replace_existing_pack(options: PackUpdateOptions) -> Result<()> {
    let metadata = resolve_metadata(
        &options.out,
        options.id.clone(),
        options.name.clone(),
        options.description.clone(),
    )?;
    let temp = tempfile::Builder::new()
        .prefix("autorepo-pack-replace-")
        .tempdir()
        .context("failed to create temporary pack replace directory")?;
    let generated = temp.path().join("pack");
    pack_scaffold::from_repo(FromRepoOptions {
        source: options.source,
        git_ref: options.git_ref,
        out: generated.clone(),
        id: metadata.id,
        name: metadata.name,
        description: metadata.description,
        include: options.include,
        exclude: options.exclude,
        with_issues: options.with_issues,
        with_warmup: options.with_warmup,
        dry_run: options.dry_run,
    })?;

    if options.dry_run {
        return Ok(());
    }

    fs::remove_dir_all(&options.out)
        .with_context(|| format!("failed to remove '{}'", options.out.display()))?;
    copy_dir(&generated, &options.out)?;
    let pack = Pack::load(options.out.clone())?;
    pack.validate()
        .with_context(|| format!("replaced pack '{}' did not validate", options.out.display()))?;
    println!("Replaced pack '{}' is valid.", pack.manifest().id);
    Ok(())
}

fn resolve_metadata(
    pack_dir: &Path,
    id: Option<String>,
    name: Option<String>,
    description: Option<String>,
) -> Result<Metadata> {
    if pack_dir.exists()
        && let Ok(pack) = Pack::load(pack_dir.to_path_buf())
    {
        let manifest = pack.manifest();
        return Ok(Metadata {
            id: id.unwrap_or_else(|| manifest.id.clone()),
            name: name.unwrap_or_else(|| manifest.name.clone()),
            description: description.or_else(|| manifest.description.clone()),
        });
    }

    Ok(Metadata {
        id: id.context("--id is required when creating a new pack")?,
        name: name.context("--name is required when creating a new pack")?,
        description,
    })
}

fn refresh_files_and_manifest(target: &Path, generated: &Path) -> Result<()> {
    let target_manifest_path = manifest_path(target)?;
    let generated_manifest_path = manifest_path(generated)?;
    let target_text = fs::read_to_string(&target_manifest_path)
        .with_context(|| format!("failed to read '{}'", target_manifest_path.display()))?;
    let generated_text = fs::read_to_string(&generated_manifest_path)
        .with_context(|| format!("failed to read '{}'", generated_manifest_path.display()))?;
    let target_yaml: Value = serde_yaml::from_str(&target_text)
        .with_context(|| format!("failed to parse '{}'", target_manifest_path.display()))?;
    let generated_yaml: Value = serde_yaml::from_str(&generated_text)
        .with_context(|| format!("failed to parse '{}'", generated_manifest_path.display()))?;

    let generated_files = get_mapping(&generated_yaml)?
        .get(Value::String("files".to_owned()))
        .cloned()
        .unwrap_or_else(|| Value::Sequence(Vec::new()));
    let generated_file_count = generated_files.as_sequence().map_or(0, Vec::len);
    let target_mapping = get_mapping(&target_yaml)?;
    let non_file_writes = count_non_file_writes(target_mapping);
    let max_writes = u32::try_from(non_file_writes + generated_file_count)
        .context("updated pack declares too many writes")?;

    let rendered = update_manifest_text(&target_text, generated_files, max_writes)?;
    validate_rendered_manifest(&rendered, generated_file_count, max_writes)?;

    let target_templates = target.join("templates").join("files");
    if target_templates.exists() {
        fs::remove_dir_all(&target_templates)
            .with_context(|| format!("failed to remove '{}'", target_templates.display()))?;
    }
    let generated_templates = generated.join("templates").join("files");
    if generated_templates.exists() {
        copy_dir(&generated_templates, &target_templates)?;
    }

    fs::write(&target_manifest_path, rendered)
        .with_context(|| format!("failed to write '{}'", target_manifest_path.display()))?;
    Ok(())
}

fn update_manifest_text(
    manifest_text: &str,
    generated_files: Value,
    max_writes: u32,
) -> Result<String> {
    let eol = dominant_line_ending(manifest_text);
    let rendered_files =
        render_top_level_value("files", generated_files).context("failed to render files block")?;
    let rendered_files = normalize_line_endings(&rendered_files, eol);
    let with_files = replace_top_level_block(manifest_text, "files", &rendered_files);
    update_safety_max_writes(&with_files, max_writes, eol)
}

fn render_top_level_value(key: &str, value: Value) -> Result<String> {
    let mut mapping = Mapping::new();
    mapping.insert(Value::String(key.to_owned()), value);
    serde_yaml::to_string(&Value::Mapping(mapping)).context("failed to render YAML block")
}

fn replace_top_level_block(text: &str, key: &str, replacement: &str) -> String {
    let mut lines = split_lines(text);
    let Some(start) = lines.iter().position(|line| is_top_level_key(line, key)) else {
        let mut out = ensure_trailing_newline(text);
        out.push_str(replacement);
        return out;
    };
    let end = find_next_top_level_key(&lines, start + 1).unwrap_or(lines.len());
    let end = preserve_leading_comments_before_key(&lines, start + 1, end);
    let replacement_lines = split_lines(replacement);
    lines.splice(start..end, replacement_lines);
    lines.concat()
}

fn update_safety_max_writes(text: &str, max_writes: u32, eol: &str) -> Result<String> {
    let mut lines = split_lines(text);
    let Some(safety_start) = lines
        .iter()
        .position(|line| is_top_level_key(line, "safety"))
    else {
        bail!("pack manifest must contain a safety block");
    };
    if lines[safety_start]
        .strip_prefix("safety:")
        .is_some_and(|rest| !rest.trim().is_empty())
    {
        bail!("pack manifest safety block must use block mapping style for pack update");
    }
    let safety_end = find_next_top_level_key(&lines, safety_start + 1).unwrap_or(lines.len());
    let replacement = format!("  max_writes: {max_writes}{eol}");

    if let Some(index) = (safety_start + 1..safety_end).find(|index| {
        lines[*index]
            .trim_start()
            .strip_prefix("max_writes:")
            .is_some()
    }) {
        let indent = lines[index]
            .chars()
            .take_while(|ch| ch.is_whitespace() && *ch != '\r' && *ch != '\n')
            .collect::<String>();
        lines[index] = format!("{indent}max_writes: {max_writes}{eol}");
    } else {
        lines.insert(safety_start + 1, replacement);
    }

    Ok(lines.concat())
}

fn preserve_leading_comments_before_key(lines: &[String], start: usize, end: usize) -> usize {
    let mut adjusted = end;
    while adjusted > start {
        let previous = &lines[adjusted - 1];
        if previous.trim().is_empty() || previous.trim_start().starts_with('#') {
            adjusted -= 1;
        } else {
            break;
        }
    }
    adjusted
}

fn validate_rendered_manifest(
    rendered: &str,
    expected_file_count: usize,
    expected_max_writes: u32,
) -> Result<()> {
    let rendered_yaml: Value =
        serde_yaml::from_str(rendered).context("updated pack manifest would not parse")?;
    let mapping = get_mapping(&rendered_yaml)?;
    let actual_file_count = mapping
        .get(Value::String("files".to_owned()))
        .and_then(Value::as_sequence)
        .map_or(0, Vec::len);
    if actual_file_count != expected_file_count {
        bail!(
            "updated pack manifest would contain {actual_file_count} files, expected {expected_file_count}"
        );
    }
    let actual_max_writes = mapping
        .get(Value::String("safety".to_owned()))
        .and_then(Value::as_mapping)
        .and_then(|safety| safety.get(Value::String("max_writes".to_owned())))
        .and_then(Value::as_u64)
        .context("updated pack manifest would not contain safety.max_writes")?;
    if actual_max_writes != u64::from(expected_max_writes) {
        bail!(
            "updated pack manifest would set safety.max_writes to {actual_max_writes}, expected {expected_max_writes}"
        );
    }
    Ok(())
}

fn dominant_line_ending(text: &str) -> &str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

fn normalize_line_endings(text: &str, eol: &str) -> String {
    if eol == "\n" {
        text.replace("\r\n", "\n")
    } else {
        text.replace("\r\n", "\n").replace('\n', "\r\n")
    }
}

fn split_lines(text: &str) -> Vec<String> {
    text.split_inclusive('\n').map(str::to_owned).collect()
}

fn ensure_trailing_newline(text: &str) -> String {
    let mut out = text.to_owned();
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn is_top_level_key(line: &str, key: &str) -> bool {
    line.strip_prefix(&format!("{key}:")).is_some()
}

fn find_next_top_level_key(lines: &[String], start: usize) -> Option<usize> {
    lines
        .iter()
        .enumerate()
        .skip(start)
        .find(|(_, line)| {
            !line.trim().is_empty()
                && !line.starts_with(char::is_whitespace)
                && !line.trim_start().starts_with('#')
                && !line.starts_with('-')
                && line.contains(':')
        })
        .map(|(index, _)| index)
}

fn count_non_file_writes(manifest: &Mapping) -> usize {
    [
        "labels",
        "milestones",
        "branches",
        "issues",
        "pull_requests",
        "workflow_dispatches",
    ]
    .iter()
    .map(|key| {
        manifest
            .get(Value::String((*key).to_owned()))
            .and_then(Value::as_sequence)
            .map_or(0, Vec::len)
    })
    .sum()
}

fn get_mapping(value: &Value) -> Result<&Mapping> {
    value
        .as_mapping()
        .context("pack manifest must be a YAML mapping")
}

fn copy_dir(source: &Path, target: &Path) -> Result<()> {
    for entry in fs::read_dir(source)
        .with_context(|| format!("failed to read directory '{}'", source.display()))?
    {
        let entry = entry
            .with_context(|| format!("failed to read directory entry in '{}'", source.display()))?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        let metadata = entry
            .metadata()
            .with_context(|| format!("failed to inspect '{}'", source_path.display()))?;
        if metadata.is_dir() {
            copy_dir(&source_path, &target_path)?;
        } else if metadata.is_file() {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create '{}'", parent.display()))?;
            }
            fs::copy(&source_path, &target_path).with_context(|| {
                format!(
                    "failed to copy '{}' to '{}'",
                    source_path.display(),
                    target_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn manifest_path(pack_dir: &Path) -> Result<PathBuf> {
    let yml = pack_dir.join("pack.yml");
    if yml.is_file() {
        return Ok(yml);
    }
    let yaml = pack_dir.join("pack.yaml");
    if yaml.is_file() {
        return Ok(yaml);
    }
    bail!(
        "pack manifest not found in {}; expected pack.yml or pack.yaml",
        pack_dir.display()
    );
}

fn clone_target_repo(options: &PackPublishOptions, temp: &Path) -> Result<()> {
    let checkout = temp.join("repo");
    let source = target_clone_url(options)?;
    command(
        ProcessCommand::new("git")
            .args(["clone", &source])
            .arg(&checkout),
    )
    .with_context(|| format!("failed to clone target repository from {source}"))?;
    Ok(())
}

fn target_clone_url(options: &PackPublishOptions) -> Result<String> {
    if let Some(target_checkout) = &options.target_checkout {
        return Ok(
            git_output(target_checkout, ["remote", "get-url", "origin"])?
                .trim()
                .to_owned(),
        );
    }

    Ok({
        format!(
            "https://github.com/{}/{}.git",
            options.target_repo.owner, options.target_repo.name
        )
    })
}

fn checkout_publish_branch(checkout: &Path, branch: &str, base: Option<&str>) -> Result<bool> {
    git(checkout, ["fetch", "origin"])?;
    let remote_branch = format!("origin/{branch}");
    if ref_exists(checkout, &remote_branch) {
        git(checkout, ["checkout", "-B", branch, &remote_branch])?;
        return Ok(true);
    } else if let Some(base) = base {
        git(checkout, ["fetch", "origin", base])?;
        git(
            checkout,
            ["checkout", "-B", branch, &format!("origin/{base}")],
        )?;
    } else if let Some(default_branch) = remote_default_branch(checkout) {
        git(
            checkout,
            [
                "checkout",
                "-B",
                branch,
                &format!("origin/{default_branch}"),
            ],
        )?;
    } else {
        git(checkout, ["checkout", "-B", branch])?;
    }
    Ok(false)
}

fn ref_exists(checkout: &Path, name: &str) -> bool {
    git_output(checkout, ["rev-parse", "--verify", "--quiet", name]).is_ok()
}

fn remote_default_branch(checkout: &Path) -> Option<String> {
    let head = git_output(checkout, ["rev-parse", "--abbrev-ref", "origin/HEAD"]).ok()?;
    head.trim()
        .strip_prefix("origin/")
        .filter(|branch| !branch.trim().is_empty() && *branch != "HEAD")
        .map(str::to_owned)
}

fn verify_origin_matches(checkout: &Path, expected: &RepoRef) -> Result<()> {
    let origin = git_output(checkout, ["remote", "get-url", "origin"])?;
    if !origin_matches_repo(origin.trim(), expected) {
        bail!(
            "target checkout origin '{}' does not match --target-repo {}/{}",
            origin.trim(),
            expected.owner,
            expected.name
        );
    }
    Ok(())
}

fn origin_matches_repo(origin: &str, expected: &RepoRef) -> bool {
    let normalized = origin.replace('\\', "/").to_ascii_lowercase();
    let normalized = normalized.trim_end_matches(".git");
    let suffix = format!(
        "{}/{}",
        expected.owner.to_ascii_lowercase(),
        expected.name.to_ascii_lowercase()
    );
    normalized == suffix
        || normalized.ends_with(&format!("/{suffix}"))
        || normalized.ends_with(&format!(":{suffix}"))
}

fn open_or_reuse_pr(options: &PackPublishOptions) -> Result<()> {
    let repo = format!("{}/{}", options.target_repo.owner, options.target_repo.name);
    let existing = command_output(ProcessCommand::new("gh").args([
        "pr",
        "list",
        "--repo",
        &repo,
        "--head",
        &options.branch,
        "--state",
        "open",
        "--json",
        "url",
        "--jq",
        ".[0].url // \"\"",
    ]))
    .context("failed to check for existing pull request with gh")?;

    if !existing.trim().is_empty() {
        println!("Pull request already exists: {}", existing.trim());
        return Ok(());
    }

    let title = format!(
        "Update {} starter pack",
        pack_name_for_message(&options.target_path)
    );
    let body = format!(
        "Refreshes `{}` from `{}` using `autorepo pack publish`.",
        options.target_path.display(),
        options.source
    );
    let mut command = ProcessCommand::new("gh");
    command.args([
        "pr",
        "create",
        "--repo",
        &repo,
        "--head",
        &options.branch,
        "--title",
        &title,
        "--body",
        &body,
    ]);
    if let Some(base) = &options.base {
        command.args(["--base", base]);
    }
    let url = command_output(&mut command).context("failed to create pull request with gh")?;
    println!("Pull request: {}", url.trim());
    Ok(())
}

fn git<const N: usize>(cwd: &Path, args: [&str; N]) -> Result<()> {
    command(ProcessCommand::new("git").arg("-C").arg(cwd).args(args))
}

fn git_output<const N: usize>(cwd: &Path, args: [&str; N]) -> Result<String> {
    command_output(ProcessCommand::new("git").arg("-C").arg(cwd).args(args))
}

fn command(command: &mut ProcessCommand) -> Result<()> {
    let output = command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .env("GIT_ASKPASS", "echo")
        .output()
        .context("failed to start command")?;
    if !output.status.success() {
        bail!(
            "command failed with status {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn command_output(command: &mut ProcessCommand) -> Result<String> {
    let output = command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .env("GIT_ASKPASS", "echo")
        .output()
        .context("failed to start command")?;
    if !output.status.success() {
        bail!(
            "command failed with status {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn validate_relative_path(path: &Path, name: &str) -> Result<()> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        bail!(
            "{name} '{}' must be a non-empty relative path",
            path.display()
        );
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        bail!(
            "{name} '{}' must not contain parent-directory or root components",
            path.display()
        );
    }
    Ok(())
}

fn validate_branch(branch: &str) -> Result<()> {
    if branch.trim().is_empty()
        || branch.starts_with('-')
        || branch.contains("..")
        || branch.contains('\\')
        || branch.contains('@')
        || branch.ends_with('/')
        || branch.ends_with('.')
        || branch.ends_with(".lock")
        || branch.chars().any(char::is_whitespace)
        || branch
            .chars()
            .any(|ch| ch.is_control() || matches!(ch, ':' | '~' | '^' | '?' | '*' | '['))
    {
        bail!("branch '{branch}' is not a safe git branch name");
    }
    Ok(())
}

fn is_empty_dir(path: &Path) -> Result<bool> {
    if !path.is_dir() {
        bail!(
            "pack path '{}' exists and is not a directory",
            path.display()
        );
    }
    Ok(fs::read_dir(path)
        .with_context(|| format!("failed to read '{}'", path.display()))?
        .next()
        .is_none())
}

fn path_arg(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn pack_name_for_message(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "pack".to_owned())
}

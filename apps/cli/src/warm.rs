use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::{
    github::RepoRef,
    pack::{Pack, WarmupAppTarget, WarmupAutomationTrigger, WarmupKind, WarmupMode},
};

pub fn run(
    repo: &str,
    pack: &Pack,
    start_agent_tasks: bool,
    open_app: bool,
    only: &[String],
) -> Result<()> {
    let repo = RepoRef::parse(repo)?;
    let warmup = selected_warmup(pack, only)?;
    ensure_open_app_is_bounded(open_app, only)?;
    ensure_open_app_targets_app_items(open_app, &warmup)?;
    let mut open_errors = Vec::new();

    println!(
        "Warmup guidance for {}/{} using pack '{}':",
        repo.owner,
        repo.name,
        pack.manifest().id
    );

    for item in &warmup {
        match item.kind {
            WarmupKind::Note | WarmupKind::Checklist => {
                println!("- {}: {}", item.id, item.title);
                if let Some(body) = &item.body {
                    println!("  {body}");
                }
            }
            WarmupKind::AppSession => {
                let prompt = warmup_prompt(pack, item)?;
                let mode = item.mode.unwrap_or(WarmupMode::Plan).as_str();
                let app_link = app_session_link(&repo, mode, &prompt);
                let launch_link = launcher_link(&app_link);

                println!("- app session {}: {}", item.id, item.title);
                println!("  App link: {app_link}");
                println!("  Browser launcher: {launch_link}");
                println!("  Prompt: {prompt}");
                maybe_open_link(open_app, &launch_link, &mut open_errors);
            }
            WarmupKind::AppLink => {
                let target = item
                    .target
                    .context("validated app_link warmup is missing target")?;
                let app_link = app_target_link(&repo, target);
                let launch_link = launcher_link(&app_link);

                println!("- app link {}: {}", item.id, item.title);
                println!("  App link: {app_link}");
                println!("  Browser launcher: {launch_link}");
                maybe_open_link(open_app, &launch_link, &mut open_errors);
            }
            WarmupKind::AutomationDraft => {
                let prompt = warmup_prompt(pack, item)?;
                let prompt = format!("Repository: {}/{}\n\n{prompt}", repo.owner, repo.name);
                let trigger = item
                    .trigger
                    .unwrap_or(WarmupAutomationTrigger::Manual)
                    .as_str();
                let app_link = automation_draft_link(
                    &item.title,
                    trigger,
                    item.time.as_deref(),
                    item.day.map(|day| day.as_str()),
                    &prompt,
                );
                let launch_link = launcher_link(&app_link);

                println!("- automation draft {}: {}", item.id, item.title);
                println!("  App link: {app_link}");
                println!("  Browser launcher: {launch_link}");
                println!("  Prompt: {prompt}");
                maybe_open_link(open_app, &launch_link, &mut open_errors);
            }
        }
    }

    if start_agent_tasks {
        let tasks = warmup
            .iter()
            .copied()
            .filter(|item| item.start_agent_task)
            .collect::<Vec<_>>();

        if tasks.is_empty() {
            println!("- no selected warmup items request Copilot cloud-agent task guidance");
        }

        for item in &tasks {
            println!("- copilot task {}: {}", item.id, item.title);
        }

        if !tasks.is_empty() {
            println!(
                "Copilot cloud-agent tasks are async and nondeterministic; review before starting them."
            );
        }
    }

    if !open_errors.is_empty() {
        bail!("failed to open one or more Copilot app launcher links");
    }

    Ok(())
}

fn selected_warmup<'a>(
    pack: &'a Pack,
    only: &[String],
) -> Result<Vec<&'a crate::pack::WarmupItem>> {
    if only.is_empty() {
        return Ok(pack.manifest().warmup.iter().collect());
    }

    let available = pack
        .manifest()
        .warmup
        .iter()
        .map(|item| item.id.as_str())
        .collect::<Vec<_>>();
    let unknown = only
        .iter()
        .filter(|id| !available.contains(&id.as_str()))
        .map(String::as_str)
        .collect::<Vec<_>>();

    if !unknown.is_empty() {
        bail!(
            "unknown --only warmup id(s): {}; available ids: {}",
            unknown.join(", "),
            available.join(", ")
        );
    }

    Ok(pack
        .manifest()
        .warmup
        .iter()
        .filter(|item| only.iter().any(|id| id == &item.id))
        .collect())
}

fn ensure_open_app_is_bounded(open_app: bool, only: &[String]) -> Result<()> {
    if open_app && only.is_empty() {
        bail!(
            "--open-app requires exactly one --only <ID> so autorepo opens one intentional warmup target"
        );
    }

    if open_app && only.len() != 1 {
        bail!("--open-app accepts exactly one --only <ID>");
    }

    Ok(())
}

fn ensure_open_app_targets_app_items(
    open_app: bool,
    warmup: &[&crate::pack::WarmupItem],
) -> Result<()> {
    if !open_app {
        return Ok(());
    }

    if warmup.iter().any(|item| {
        matches!(
            item.kind,
            WarmupKind::AppLink | WarmupKind::AppSession | WarmupKind::AutomationDraft
        )
    }) {
        return Ok(());
    }

    bail!("selected warmup id has no Copilot app target to open")
}

fn warmup_prompt(pack: &Pack, item: &crate::pack::WarmupItem) -> Result<String> {
    let prompt = if let Some(template) = &item.prompt_template {
        pack.template_text(template)?
    } else if let Some(prompt) = &item.prompt {
        prompt.clone()
    } else {
        bail!("warmup '{}' requires prompt or prompt_template", item.id)
    };

    Ok(prompt.replace("\r\n", "\n"))
}

fn app_target_link(repo: &RepoRef, target: WarmupAppTarget) -> String {
    match target {
        WarmupAppTarget::Home => "ghapp://".to_string(),
        WarmupAppTarget::MyWork => "ghapp://mywork".to_string(),
        WarmupAppTarget::Repo => format!(
            "ghapp://github.com/{}/{}",
            urlencoding::encode(&repo.owner),
            urlencoding::encode(&repo.name)
        ),
    }
}

fn app_session_link(repo: &RepoRef, mode: &str, prompt: &str) -> String {
    format!(
        "ghapp://session/new?repo={}&mode={}&prompt={}",
        urlencoding::encode(&format!("{}/{}", repo.owner, repo.name)),
        urlencoding::encode(mode),
        urlencoding::encode(prompt)
    )
}

fn automation_draft_link(
    name: &str,
    trigger: &str,
    time: Option<&str>,
    day: Option<&str>,
    prompt: &str,
) -> String {
    let mut link = format!(
        "ghapp://automations/new?name={}&trigger={}&prompt={}",
        urlencoding::encode(name),
        urlencoding::encode(trigger),
        urlencoding::encode(prompt)
    );

    if let Some(time) = time {
        link.push_str("&time=");
        link.push_str(&urlencoding::encode(time));
    }

    if let Some(day) = day {
        link.push_str("&day=");
        link.push_str(&urlencoding::encode(day));
    }

    link
}

fn launcher_link(app_link: &str) -> String {
    format!(
        "https://github.com/copilot/app/launch?open={}",
        urlencoding::encode(app_link)
    )
}

fn maybe_open_link(open_app: bool, launch_link: &str, open_errors: &mut Vec<anyhow::Error>) {
    if !open_app {
        return;
    }

    match open_url(launch_link) {
        Ok(()) => println!("  Opened in GitHub Copilot app launcher."),
        Err(error) => {
            println!("  Failed to open launcher: {error:#}");
            open_errors.push(error);
        }
    }
}

fn open_url(url: &str) -> Result<()> {
    let (program, args) = open_url_command(url);
    let status = Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("failed to launch URL with {program}"))?;

    if !status.success() {
        bail!("failed to open Copilot app launcher");
    }

    Ok(())
}

fn open_url_command(url: &str) -> (&'static str, Vec<String>) {
    if cfg!(target_os = "windows") {
        (
            "rundll32",
            vec!["url.dll,FileProtocolHandler".to_string(), url.to_string()],
        )
    } else if cfg!(target_os = "macos") {
        ("open", vec![url.to_string()])
    } else {
        ("xdg-open", vec![url.to_string()])
    }
}

#[cfg(test)]
mod tests {
    use crate::{github::RepoRef, pack::WarmupAppTarget};

    use super::{
        app_session_link, app_target_link, automation_draft_link, ensure_open_app_is_bounded,
        launcher_link, open_url_command,
    };

    #[test]
    fn builds_encoded_app_session_link() {
        let repo = RepoRef::parse("sethjuarez/autorepo-test").unwrap();
        let link = app_session_link(&repo, "plan", "Inspect README and issue #1");

        assert_eq!(
            link,
            "ghapp://session/new?repo=sethjuarez%2Fautorepo-test&mode=plan&prompt=Inspect%20README%20and%20issue%20%231"
        );
    }

    #[test]
    fn builds_encoded_launcher_link() {
        let app = "ghapp://session/new?repo=sethjuarez%2Fdemo&mode=plan&prompt=hello";
        let link = launcher_link(app);

        assert_eq!(
            link,
            "https://github.com/copilot/app/launch?open=ghapp%3A%2F%2Fsession%2Fnew%3Frepo%3Dsethjuarez%252Fdemo%26mode%3Dplan%26prompt%3Dhello"
        );
    }

    #[test]
    fn builds_repo_and_my_work_app_links() {
        let repo = RepoRef::parse("sethjuarez/autorepo-test").unwrap();

        assert_eq!(
            app_target_link(&repo, WarmupAppTarget::Repo),
            "ghapp://github.com/sethjuarez/autorepo-test"
        );
        assert_eq!(
            app_target_link(&repo, WarmupAppTarget::MyWork),
            "ghapp://mywork"
        );
    }

    #[test]
    fn builds_automation_draft_link() {
        let link = automation_draft_link(
            "Daily demo triage",
            "daily",
            Some("09:00"),
            None,
            "Repository: sethjuarez/demo\n\nSummarize issues and PRs",
        );

        assert_eq!(
            link,
            "ghapp://automations/new?name=Daily%20demo%20triage&trigger=daily&prompt=Repository%3A%20sethjuarez%2Fdemo%0A%0ASummarize%20issues%20and%20PRs&time=09%3A00"
        );
    }

    #[test]
    fn builds_open_url_command_without_shell_expansion_on_windows() {
        let url = "https://github.com/copilot/app/launch?open=ghapp%3A%2F%2Fsession%2Fnew%3Fa%3D1%26b%3D2";
        let (program, args) = open_url_command(url);

        if cfg!(target_os = "windows") {
            assert_eq!(program, "rundll32");
            assert_eq!(args, vec!["url.dll,FileProtocolHandler", url]);
        } else {
            assert_eq!(args, vec![url]);
        }
    }

    #[test]
    fn open_app_requires_only() {
        assert!(ensure_open_app_is_bounded(true, &[]).is_err());
        assert!(
            ensure_open_app_is_bounded(
                true,
                &["facilitator_session".to_string(), "open_repo".to_string()]
            )
            .is_err()
        );
        assert!(ensure_open_app_is_bounded(false, &[]).is_ok());
        assert!(ensure_open_app_is_bounded(true, &["facilitator_session".to_string()]).is_ok());
    }
}

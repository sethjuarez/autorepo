use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::{
    github::RepoRef,
    ops::Operation,
    pack::{Pack, WarmupKind, WarmupMode},
};

pub fn run(repo: &str, pack: &Pack, start_agent_tasks: bool, open_app: bool) -> Result<()> {
    let repo = RepoRef::parse(repo)?;

    println!(
        "Warmup guidance for {}/{} using pack '{}':",
        repo.owner,
        repo.name,
        pack.manifest().id
    );

    for item in &pack.manifest().warmup {
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

                if open_app {
                    open_url(&launch_link)?;
                    println!("  Opened in GitHub Copilot app launcher.");
                }
            }
        }
    }

    if start_agent_tasks {
        for task in pack.manifest().copilot_tasks() {
            let Operation::CopilotTask { id, title } = task else {
                continue;
            };
            println!("- copilot task {id}: {title}");
        }
        println!(
            "Copilot cloud-agent tasks are async and nondeterministic; review before starting them."
        );
    }

    Ok(())
}

fn warmup_prompt(pack: &Pack, item: &crate::pack::WarmupItem) -> Result<String> {
    let prompt = if let Some(template) = &item.prompt_template {
        pack.template_text(template)?
    } else if let Some(prompt) = &item.prompt {
        prompt.clone()
    } else {
        bail!(
            "warmup app_session '{}' requires prompt or prompt_template",
            item.id
        )
    };

    Ok(prompt.replace("\r\n", "\n"))
}

fn app_session_link(repo: &RepoRef, mode: &str, prompt: &str) -> String {
    format!(
        "ghapp://session/new?repo={}&mode={}&prompt={}",
        urlencoding::encode(&format!("{}/{}", repo.owner, repo.name)),
        urlencoding::encode(mode),
        urlencoding::encode(prompt)
    )
}

fn launcher_link(app_link: &str) -> String {
    format!(
        "https://github.com/copilot/app/launch?open={}",
        urlencoding::encode(app_link)
    )
}

fn open_url(url: &str) -> Result<()> {
    let status = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", "start", "", url])
            .status()
            .context("failed to launch URL with cmd start")?
    } else if cfg!(target_os = "macos") {
        Command::new("open")
            .arg(url)
            .status()
            .context("failed to launch URL with open")?
    } else {
        Command::new("xdg-open")
            .arg(url)
            .status()
            .context("failed to launch URL with xdg-open")?
    };

    if !status.success() {
        bail!("failed to open Copilot app launcher");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::github::RepoRef;

    use super::{app_session_link, launcher_link};

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
}

use anyhow::Result;

use crate::{ops::Operation, pack::Pack};

pub fn run(repo: &str, pack: &Pack, start_agent_tasks: bool) -> Result<()> {
    println!(
        "Warmup guidance for {repo} using pack '{}':",
        pack.manifest().id
    );

    for item in &pack.manifest().warmup {
        println!("- {}: {}", item.id, item.title);
        if let Some(body) = &item.body {
            println!("  {body}");
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

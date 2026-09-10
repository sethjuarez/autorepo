use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};

use crate::{
    doctor,
    executor::Executor,
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
    /// Validate a pack directory.
    Validate {
        /// Pack directory, or "builtin" for the generic starter pack.
        pack_dir: String,
    },
    /// Plan or apply deterministic repository preparation operations.
    Prepare(PrepareArgs),
    /// Render optional warmup notes/checklists and agent-task guidance.
    Warm(WarmArgs),
}

#[derive(Debug, Args)]
pub struct PrepareArgs {
    /// GitHub repository in OWNER/REPO form.
    repo: String,
    /// Pack directory, pack id, or "builtin".
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
    /// Pack directory, pack id, or "builtin".
    #[arg(long)]
    pack: String,
    /// Include explicit Copilot cloud-agent task startup guidance.
    #[arg(long)]
    start_agent_tasks: bool,
}

pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Doctor { repo } => {
            doctor::run(&repo).await?;
        }
        Command::Validate { pack_dir } => {
            let pack = Pack::load(resolve_pack(&pack_dir)?)?;
            pack.validate()?;
            println!("Pack '{}' is valid.", pack.manifest().id);
        }
        Command::Prepare(args) => {
            if !args.dry_run && !args.yes {
                bail!("prepare requires either --dry-run or --yes");
            }

            let pack = Pack::load(resolve_pack(&args.pack)?)?;
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
            let pack = Pack::load(resolve_pack(&args.pack)?)?;
            pack.validate()?;
            warm::run(&args.repo, &pack, args.start_agent_tasks)?;
        }
    }

    Ok(())
}

fn resolve_pack(value: &str) -> Result<PathBuf> {
    if value == "builtin" || value == "generic-starter" {
        return Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("packs")
            .join("generic-starter"));
    }

    PathBuf::from(value)
        .canonicalize()
        .with_context(|| format!("pack path '{}' does not exist", value))
}

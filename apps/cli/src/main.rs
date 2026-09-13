use anyhow::Result;

mod cli;
mod executor;
mod labs;
mod pack_publish;
mod pack_scaffold;
mod render;
mod warm;

pub use autorepo::{doctor, github, markers, ops, pack, planner};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .without_time()
        .init();

    cli::run().await
}

use anyhow::Result;

mod cli;
mod doctor;
mod executor;
mod github;
mod labs;
mod markers;
mod ops;
mod pack;
mod planner;
mod render;
mod warm;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .without_time()
        .init();

    cli::run().await
}

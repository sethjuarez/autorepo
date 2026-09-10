use anyhow::Result;

use crate::planner::Plan;

#[derive(Debug, Default)]
pub struct Executor;

impl Executor {
    pub async fn execute_serial(&self, plan: &Plan) -> Result<()> {
        println!(
            "Serial execution is not implemented yet; validated {} operation(s) for {}.",
            plan.operations.len(),
            plan.repo
        );
        println!("No GitHub writes were made.");
        Ok(())
    }
}

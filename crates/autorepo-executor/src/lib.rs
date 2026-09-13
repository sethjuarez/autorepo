//! Safe execution boundary for autorepo plans.

pub use autorepo::{OperationId, WorkflowRunId};

pub type Result<T> = anyhow::Result<T>;
